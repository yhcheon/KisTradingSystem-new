use crate::chat::{ChatConnection, ChatError, ChatMessage, ChatService, validate_message_frame};
use axum::{
    Extension,
    extract::{
        Query,
        ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade},
    },
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::broadcast;

const SEND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize)]
pub(crate) struct TicketQuery {
    ticket: String,
}

pub(crate) async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(query): Query<TicketQuery>,
    Extension(chat): Extension<ChatService>,
) -> Result<Response, StatusCode> {
    let connection = chat.consume_ticket(&query.ticket).map_err(ticket_status)?;

    Ok(ws
        .max_message_size(crate::chat::MAX_MESSAGE_FRAME_BYTES)
        .max_frame_size(crate::chat::MAX_MESSAGE_FRAME_BYTES)
        .on_upgrade(move |socket| handle_socket(socket, chat, connection))
        .into_response())
}

fn ticket_status(error: ChatError) -> StatusCode {
    match error {
        ChatError::InvalidTicket | ChatError::ExpiredTicket => StatusCode::UNAUTHORIZED,
        ChatError::RateLimited
        | ChatError::ConnectionLimitReached
        | ChatError::NicknameSequenceExhausted => StatusCode::TOO_MANY_REQUESTS,
        ChatError::InvalidClientAddress => StatusCode::BAD_REQUEST,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    }
}

async fn handle_socket(mut socket: WebSocket, chat: ChatService, connection: ChatConnection) {
    let mut messages: broadcast::Receiver<ChatMessage> = chat.subscribe_messages();
    loop {
        tokio::select! {
            incoming = socket.recv() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        let text = match validate_message_frame(text.as_bytes()) {
                            Ok(text) => text,
                            Err(_) => {
                                close_with_policy_violation(&mut socket).await;
                                break;
                            }
                        };

                        if chat.publish_message(&connection, text).is_err() {
                            close_with_policy_violation(&mut socket).await;
                            break;
                        }
                    }
                    Some(Ok(Message::Close(_))) | None => break,
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Binary(_))) | Some(Err(_)) => {
                        close_with_policy_violation(&mut socket).await;
                        break;
                    }
                }
            }
            delivery = messages.recv() => {
                match delivery {
                    Ok(message) => {
                        if send_message(&mut socket, &message).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        close_with_unavailable(&mut socket).await;
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            },
            count = presence.recv() => {
                match count {
                    Ok(online_count) => {
                        if send_presence(&mut socket, online_count).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if send_presence(&mut socket, chat.online_count()).await.is_err() {
                            break;
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }
}

async fn send_presence(socket: &mut WebSocket, online_count: usize) -> Result<(), ()> {
    let payload = serde_json::json!({
        "type": "presence",
        "online_count": online_count,
    });
    let payload = serde_json::to_string(&payload).map_err(|_| ())?;
    tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Text(payload.into())))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}

async fn send_message(socket: &mut WebSocket, message: &ChatMessage) -> Result<(), ()> {
    let payload = serde_json::to_string(message).map_err(|_| ())?;
    tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Text(payload.into())))
        .await
        .map_err(|_| ())?
        .map_err(|_| ())
}

async fn close_with_policy_violation(socket: &mut WebSocket) {
    close_with(socket, 1008, "invalid chat frame").await;
}

async fn close_with_unavailable(socket: &mut WebSocket) {
    close_with(socket, 1013, "chat delivery unavailable").await;
}

async fn close_with(socket: &mut WebSocket, code: u16, reason: &'static str) {
    let frame = CloseFrame {
        code,
        reason: reason.into(),
    };
    let _ = tokio::time::timeout(SEND_TIMEOUT, socket.send(Message::Close(Some(frame)))).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get};
    use futures_util::{SinkExt, StreamExt};
    use std::net::SocketAddr;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{connect_async, tungstenite::Message as ClientMessage};

    const TEST_NICKNAMES: &str = r#"{"words":["visitor"]}"#;

    async fn start_server(chat: ChatService) -> (SocketAddr, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let app = Router::new()
            .route("/ws/chat", get(ws_handler))
            .layer(Extension(chat));
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        (address, task)
    }

    async fn connect(
        address: SocketAddr,
        ticket: &str,
    ) -> Result<
        (
            tokio_tungstenite::WebSocketStream<
                tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
            >,
            tokio_tungstenite::tungstenite::handshake::client::Response,
        ),
        tokio_tungstenite::tungstenite::Error,
    > {
        connect_async(format!("ws://{address}/ws/chat?ticket={ticket}")).await
    }

    async fn receive_message(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> ChatMessage {
        let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let ClientMessage::Text(text) = frame else {
            panic!("expected chat text frame");
        };
        serde_json::from_str(text.as_ref()).unwrap()
    }

    async fn assert_socket_closed(
        socket: &mut tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) {
        let frame = tokio::time::timeout(Duration::from_secs(2), socket.next())
            .await
            .unwrap();
        assert!(
            matches!(
                frame,
                None | Some(Err(_)) | Some(Ok(ClientMessage::Close(_)))
            ),
            "expected the server to close the invalid connection"
        );
    }

    #[tokio::test]
    async fn broadcasts_once_to_two_clients_including_sender_without_replay() {
        let chat = ChatService::new_for_test(TEST_NICKNAMES);
        let (address, server) = start_server(chat.clone()).await;
        let first_ticket = chat
            .issue_ticket("192.0.2.1".parse().unwrap(), None)
            .unwrap();
        let second_ticket = chat
            .issue_ticket("192.0.2.2".parse().unwrap(), None)
            .unwrap();
        let (mut first, _) = connect(address, &first_ticket.ticket).await.unwrap();
        let (mut second, _) = connect(address, &second_ticket.ticket).await.unwrap();

        first
            .send(ClientMessage::Text(
                r#"{"type":"send","text":"hello"}"#.into(),
            ))
            .await
            .unwrap();
        let echoed = receive_message(&mut first).await;
        let delivered = receive_message(&mut second).await;
        assert_eq!(echoed, delivered);
        assert_eq!(echoed.kind, "message");
        assert_eq!(echoed.nickname, first_ticket.nickname);
        assert_eq!(echoed.text, "hello");
        assert!(!echoed.id.is_empty());
        assert!(!echoed.sent_at.is_empty());
        assert!(
            tokio::time::timeout(Duration::from_millis(100), first.next())
                .await
                .is_err()
        );

        let third_ticket = chat
            .issue_ticket("192.0.2.3".parse().unwrap(), None)
            .unwrap();
        let (mut third, _) = connect(address, &third_ticket.ticket).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), third.next())
                .await
                .is_err()
        );

        third.close(None).await.unwrap();
        second
            .send(ClientMessage::Text(
                r#"{"type":"send","text":"while away"}"#.into(),
            ))
            .await
            .unwrap();
        let _ = receive_message(&mut first).await;
        let _ = receive_message(&mut second).await;

        let reconnect_ticket = chat
            .issue_ticket("192.0.2.4".parse().unwrap(), None)
            .unwrap();
        let (mut reconnected, _) = connect(address, &reconnect_ticket.ticket).await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), reconnected.next())
                .await
                .is_err()
        );

        second
            .send(ClientMessage::Text(
                r#"{"type":"send","text":"live"}"#.into(),
            ))
            .await
            .unwrap();
        let first_live = receive_message(&mut first).await;
        let second_live = receive_message(&mut second).await;
        let third_live = receive_message(&mut reconnected).await;
        assert_eq!(first_live, second_live);
        assert_eq!(second_live, third_live);
        assert_eq!(third_live.text, "live");
        assert!(
            tokio::time::timeout(Duration::from_millis(100), reconnected.next())
                .await
                .is_err()
        );

        drop((first, second, third, reconnected));
        server.abort();
    }

    #[tokio::test]
    async fn rejects_invalid_and_reused_tickets_during_handshake() {
        let chat = ChatService::new_for_test(TEST_NICKNAMES);
        let (address, server) = start_server(chat.clone()).await;
        let invalid = connect(address, "not-a-ticket").await.unwrap_err();
        let tokio_tungstenite::tungstenite::Error::Http(response) = invalid else {
            panic!("invalid ticket should fail the WebSocket handshake");
        };
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);

        let ticket = chat
            .issue_ticket("192.0.2.1".parse().unwrap(), None)
            .unwrap();
        let (_socket, _) = connect(address, &ticket.ticket).await.unwrap();
        let reused = connect(address, &ticket.ticket).await.unwrap_err();
        let tokio_tungstenite::tungstenite::Error::Http(response) = reused else {
            panic!("reused ticket should fail the WebSocket handshake");
        };
        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
        server.abort();
    }

    #[tokio::test]
    async fn closes_connections_for_malformed_oversized_and_rate_limited_frames() {
        let chat = ChatService::new_for_test(TEST_NICKNAMES);
        let (address, server) = start_server(chat.clone()).await;

        let malformed_ticket = chat
            .issue_ticket("192.0.2.1".parse().unwrap(), None)
            .unwrap();
        let (mut malformed, _) = connect(address, &malformed_ticket.ticket).await.unwrap();
        malformed
            .send(ClientMessage::Text("not json".into()))
            .await
            .unwrap();
        assert_socket_closed(&mut malformed).await;

        let oversized_ticket = chat
            .issue_ticket("192.0.2.2".parse().unwrap(), None)
            .unwrap();
        let (mut oversized, _) = connect(address, &oversized_ticket.ticket).await.unwrap();
        let oversized_frame = format!(
            r#"{{"type":"send","text":"{}"}}"#,
            "x".repeat(crate::chat::MAX_MESSAGE_FRAME_BYTES)
        );
        oversized
            .send(ClientMessage::Text(oversized_frame.into()))
            .await
            .unwrap();
        assert_socket_closed(&mut oversized).await;

        let limited_ticket = chat
            .issue_ticket("192.0.2.3".parse().unwrap(), None)
            .unwrap();
        let (mut limited, _) = connect(address, &limited_ticket.ticket).await.unwrap();
        for _ in 0..5 {
            limited
                .send(ClientMessage::Text(
                    r#"{"type":"send","text":"hello"}"#.into(),
                ))
                .await
                .unwrap();
            receive_message(&mut limited).await;
        }
        limited
            .send(ClientMessage::Text(
                r#"{"type":"send","text":"too many"}"#.into(),
            ))
            .await
            .unwrap();
        assert_socket_closed(&mut limited).await;

        server.abort();
    }

    #[tokio::test]
    async fn nickname_sequence_advances_after_websocket_disconnect() {
        let chat = ChatService::new_for_test(r#"{"words":["visitor"]}"#);
        let (address, server) = start_server(chat.clone()).await;
        let ticket = chat
            .issue_ticket("192.0.2.1".parse().unwrap(), None)
            .unwrap();
        assert_eq!(ticket.nickname, "visitor-1");
        let (mut socket, _) = connect(address, &ticket.ticket).await.unwrap();

        socket.close(None).await.unwrap();
        assert_socket_closed(&mut socket).await;
        let next_ticket = chat
            .issue_ticket("192.0.2.2".parse().unwrap(), None)
            .unwrap();
        assert_eq!(next_ticket.nickname, "visitor-2");
        server.abort();
    }
}
