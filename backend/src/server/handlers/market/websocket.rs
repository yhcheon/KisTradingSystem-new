// ============================================================
// 파일명: server/handlers/market/websocket.rs
// 설명: 브라우저 실시간 시세 WebSocket 핸들러
// ============================================================

use crate::core::state::AppState;
use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use serde_json::json;

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("브라우저 WebSocket 연결됨");
    let mut rx = state.ws_tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(tick) => {
                        let json = serde_json::to_string(&json!({
                            "type": "tick",
                            "code": tick.code,
                            "price": tick.price,
                            "change": tick.change,
                            "change_rate": tick.change_rate,
                            "volume": tick.volume,
                            "last_qty": tick.last_qty,
                            "time": tick.time,
                        })).unwrap_or_default();
                        if socket.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
    tracing::info!("브라우저 WebSocket 연결 종료");
}
