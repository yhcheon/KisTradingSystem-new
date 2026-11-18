use crate::{
    chat::{ChatActionDefinition, ChatCommandDefinition, ChatError, ChatService, ChatTicketResponse},
    core::state::AppState,
};
use axum::{
    Json,
    extract::{ConnectInfo, Extension, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use std::net::SocketAddr;

mod websocket;
pub(crate) use websocket::ws_handler;

#[derive(Serialize)]
pub(crate) struct TicketError {
    code: &'static str,
}

pub(crate) async fn list_commands(
    Extension(chat): Extension<ChatService>,
) -> Json<Vec<ChatCommandDefinition>> {
    Json(chat.list_commands())
}

pub(crate) async fn list_actions(
    Extension(chat): Extension<ChatService>,
) -> Json<Vec<ChatActionDefinition>> {
    Json(chat.list_actions())
}

pub(crate) async fn issue_ticket(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<ChatTicketResponse>, (StatusCode, Json<TicketError>)> {
    let forwarded_for = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok());

    state
        .chat
        .issue_ticket(peer.ip(), forwarded_for)
        .map(Json)
        .map_err(ticket_error_response)
}

fn ticket_error_response(error: ChatError) -> (StatusCode, Json<TicketError>) {
    let (status, code) = match error {
        ChatError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
        ChatError::ConnectionLimitReached | ChatError::NicknameSequenceExhausted => {
            (StatusCode::TOO_MANY_REQUESTS, "chat_capacity_reached")
        }
        ChatError::InvalidClientAddress => (StatusCode::BAD_REQUEST, "invalid_client_address"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "chat_unavailable"),
    };
    (status, Json(TicketError { code }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn lists_commands_from_the_loaded_chat_config() {
        let chat = ChatService::new_for_test(r#"{"words":["guest"]}"#);

        let Json(commands) = list_commands(Extension(chat)).await;
        let response = serde_json::to_value(commands).unwrap();

        assert_eq!(
            response,
            serde_json::json!([{
                "command": "/test",
                "response": "{nickname} test",
            }])
        );
    }
}
