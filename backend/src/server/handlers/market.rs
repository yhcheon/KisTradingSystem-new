// ============================================================
// 파일명: server/handlers/market.rs
// 설명: 시세, 캔들 및 WebSocket API
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;

use axum::{
    Json,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::collections::HashMap;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

async fn fetch_one(state: &AppState, symbol: &FuturesSymbol) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;
    if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
        return Ok(cached);
    }
    let price = kis::get_futures_price(
        &state.http_client,
        &state.app_key,
        &state.app_secret,
        &state.access_token,
        &symbol.code,
    )
    .await
    .map_err(|e| format!("KIS 호출 실패({}): {}", symbol.code, e))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
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

pub async fn candles(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => return Json(json!({ "ok": false, "error": "code 파라미터 필요" })),
    };
    let interval = params.get("interval").map(|s| s.as_str()).unwrap_or("1m");
    let date = params.get("date").map(|s| s.as_str()).unwrap_or("both");

    let store = state.candles.lock().await;
    let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
        .format("%Y%m%d")
        .to_string(); // ← 수정
    let today = chrono::Local::now().format("%Y%m%d").to_string(); // ← 수정

    let to_json = |candles: &[crate::core::candles::Candle], date_str: &str| -> Vec<Value> {
        candles
            .iter()
            .map(|c| {
                json!({
                    "date": date_str,
                    "time": format!("{}00", c.time),
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume as f64,
                })
            })
            .collect()
    };

    let result = match date {
        "yesterday" => to_json(&store.get(&code, &yesterday, interval), &yesterday),
        "today" => to_json(&store.get(&code, &today, interval), &today),
        _ => {
            let mut all = to_json(&store.get(&code, &yesterday, interval), &yesterday);
            all.extend(to_json(&store.get(&code, &today, interval), &today));
            all
        }
    };

    Json(json!({ "ok": true, "interval": interval, "data": result }))
}

fn subtract_one_minute(hhmmss: &str) -> String {
    if hhmmss.len() < 6 {
        return "154500".to_string();
    }
    let h: u32 = hhmmss[0..2].parse().unwrap_or(15);
    let m: u32 = hhmmss[2..4].parse().unwrap_or(45);
    let s: u32 = hhmmss[4..6].parse().unwrap_or(0);
    let total = h * 3600 + m * 60 + s;
    let new_total = if total >= 60 { total - 60 } else { 0 };
    format!(
        "{:02}{:02}{:02}",
        new_total / 3600,
        (new_total % 3600) / 60,
        new_total % 60
    )
}

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
