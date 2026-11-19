// ============================================================
// 파일명: server/handlers/market/websocket.rs
// 설명: 브라우저 실시간 시세 WebSocket 핸들러
//       - 연결 직후 현재 시세 스냅샷 전송
//       - 스냅샷 생성 중 발생한 체결은 broadcast receiver에서 이어서 전달
//       - 접속자수 카운팅은 하지 않음. 접속자수는 채팅 WS(ChatService)로만 셈
//         (같은 브라우저가 시세 WS + 채팅 WS 2개를 열므로, 둘 다 세면 2배가 됨)
// 수정일: 2026-11-19 - 시세 WS 카운팅 제거, 접속자수는 채팅으로 일원화
// ============================================================

use super::{prices::build_prices, ws_message::tick_message};
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
    // 먼저 구독해 스냅샷 조회 중 발생한 체결도 버퍼에서 이어받습니다.
    let mut rx = state.ws_tx.subscribe();

    let snapshot = build_prices(&state).await;
    let range_snapshot = state.session_range.lock().await.snapshot();
    let snapshot_message = serde_json::to_string(&json!({
        "type": "snapshot",
        "data": snapshot,
        "range": range_snapshot,
    }))
    .unwrap_or_default();

    if socket
        .send(Message::Text(snapshot_message.into()))
        .await
        .is_err()
    {
        tracing::warn!("브라우저 WebSocket 초기 시세 스냅샷 전송 실패");
        return;
    }

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(tick) => {
                        let range_snapshot = state.session_range.lock().await.snapshot();
                        let message = tick_message(&tick, &range_snapshot);
                        if socket.send(Message::Text(message.into())).await.is_err() { break; }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                        // 느린 브라우저 연결은 누락된 틱만 건너뛰고 다음 최신 틱 수신을 계속합니다.
                        tracing::warn!("브라우저 WebSocket 수신 지연: {}개 틱 누락", skipped);
                        continue;
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
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

