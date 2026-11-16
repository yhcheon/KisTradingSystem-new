// ============================================================
// 파일명: server/handlers/market/candles.rs
// 설명: 분봉 캔들 조회 API
// ============================================================

use super::{candle_params::CandleParams, candle_response::to_json};
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

    let result = match date {
        "yesterday" => to_json(&store.get(&code, &yesterday, interval), &yesterday),
        "today" => to_json(&store.get(&code, &today, interval), &today),
        _ => {
            // "both" returns the two latest dates actually present in CandleStore.
            // This naturally handles weekends and exchange holidays without weekday arithmetic.
            let mut all = Vec::new();
            for stored_date in store.dates(&code) {
                all.extend(to_json(&store.get(&code, &stored_date, interval), &stored_date));
            }
            all
        }
    };

    Json(json!({ "ok": true, "interval": interval, "data": result }))
}
