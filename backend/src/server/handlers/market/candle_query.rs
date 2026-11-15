// ============================================================
// 파일명: server/handlers/market/candle_query.rs
// 설명: 요청 날짜 조건에 따른 CandleStore 조회
// ============================================================

use super::candle_response::to_json;
use crate::core::candles::CandleStore;
use serde_json::Value;

/// 요청한 날짜 조건에 맞는 캔들을 조회해 API 응답 데이터로 변환합니다.
pub(super) fn query_candles(
    store: &CandleStore,
    code: &str,
    interval: &str,
    date: &str,
    yesterday: &str,
    today: &str,
) -> Vec<Value> {
    match date {
        "yesterday" => to_json(&store.get(code, yesterday, interval), yesterday),
        "today" => to_json(&store.get(code, today, interval), today),
        _ => {
            // Return the latest dates actually present in CandleStore, independent of weekdays.
            let mut all = Vec::new();
            for stored_date in store.dates(code) {
                all.extend(to_json(
                    &store.get(code, &stored_date, interval),
                    &stored_date,
                ));
            }
            all
        }
    }
}
