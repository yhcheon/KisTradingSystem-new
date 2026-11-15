// ============================================================
// 파일명: server/handlers/market/candles.rs
// 설명: 분봉 캔들 조회 API
// ============================================================

use crate::core::state::AppState;
use axum::{Json, extract::{Query, State}};
use serde_json::{Value, json};
use std::collections::HashMap;

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

#[allow(dead_code)]
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
