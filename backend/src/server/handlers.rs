
// ============================================================
// 파일명: server/handlers.rs
// 설명: HTTP + WebSocket 핸들러
//       - GET /api/health, GET /api/prices
//       - GET /ws (실시간 체결 푸시)
// 작성일: 2026-11-14
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

async fn fetch_one(state: &AppState, symbol: &FuturesSymbol) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;
    if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
        return Ok(cached);
    }
    let price = kis::get_futures_price(&state.app_key, &state.app_secret, &state.access_token, &symbol.code)
        .await
        .map_err(|e| format!("KIS 호출 실패({}): {}", symbol.code, e))?;
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    let cached = CachedPrice {
        code: symbol.code.clone(),
        name: price.output1.hts_kor_isnm.clone(),
        price: price.output1.futs_prpr.clone(),
        prev_close: price.output1.futs_prdy_clpr.clone(),
        open: price.output1.futs_oprc.clone(),
        high: price.output1.futs_hgpr.clone(),
        low: price.output1.futs_lwpr.clone(),
        change: price.output1.futs_prdy_vrss.clone(),
        change_rate: price.output1.futs_prdy_ctrt.clone(),
        volume: price.output1.acml_vol.clone(),
        cached_at: now,
    };
    let _ = redis_cache::set(&mut conn, &cached, 3).await;
    Ok(cached)
}

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;
    Json(json!({
        "regular": match regular { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
        "mini": match mini { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
    }))
}

/// GET /ws — 브라우저 WebSocket 연결. KIS 체결을 실시간 중계.
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("브라우저 WebSocket 연결됨");
    let mut rx = state.ws_tx.subscribe();

    loop {
        tokio::select! {
            // KIS 체결 → 브라우저 전송
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
                        if socket.send(Message::Text(json.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            // 브라우저에서 오는 메시지 (ping 등)
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


