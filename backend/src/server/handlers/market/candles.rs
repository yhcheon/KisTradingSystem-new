// ============================================================
// 파일명: server/handlers/market/candles.rs
// 설명: 분봉 캔들 조회 API
// ============================================================

use super::{candle_params::CandleParams, candle_query::query_candles};
use crate::core::{state::AppState, time::{previous_calendar_day_yyyymmdd, today_yyyymmdd}};
use axum::{Json, extract::{Query, State}};
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
