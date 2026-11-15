// ============================================================
// 파일명: server/handlers/market/candles.rs
// 설명: 분봉 캔들 조회 API
// ============================================================

use super::{candle_params::CandleParams, candle_query::query_candles};
use crate::core::{
    state::AppState,
    time::{previous_calendar_day_yyyymmdd, today_yyyymmdd},
};
use axum::{
    Json,
    extract::{Query, State},
};
use serde_json::{Value, json};

pub async fn candles(
    State(state): State<AppState>,
    Query(params): Query<CandleParams>,
) -> Json<Value> {
    let code = match params.code {
        Some(code) => code,
        None => return Json(json!({ "ok": false, "error": "code 파라미터 필요" })),
    };
    let interval = params.interval.as_deref().unwrap_or("1m");
    let date = params.date.as_deref().unwrap_or("both");

    let store = state.candles.lock().await;
    let now = chrono::Utc::now();
    let yesterday = previous_calendar_day_yyyymmdd(now);
    let today = today_yyyymmdd(now);
    let result = query_candles(&store, &code, interval, date, &yesterday, &today);

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
