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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::candles::Candle;

    fn candle(time: &str, price: f64) -> Candle {
        Candle {
            time: time.to_string(),
            open: price,
            high: price,
            low: price,
            close: price,
            volume: 1,
        }
    }

    fn store_with_two_dates() -> CandleStore {
        let mut store = CandleStore::new();
        store.load_1m("101S", "20261113", vec![candle("0900", 100.0)]);
        store.load_1m("101S", "20261114", vec![candle("0900", 101.0)]);
        store
    }

    #[test]
    fn yesterday_returns_only_the_requested_previous_calendar_date() {
        let store = store_with_two_dates();

        let result = query_candles(&store, "101S", "1m", "yesterday", "20261113", "20261114");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["date"], "20261113");
        assert_eq!(result[0]["close"], 100.0);
    }

    #[test]
    fn today_returns_only_the_requested_current_date() {
        let store = store_with_two_dates();

        let result = query_candles(&store, "101S", "1m", "today", "20261113", "20261114");

        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["date"], "20261114");
        assert_eq!(result[0]["close"], 101.0);
    }

    #[test]
    fn both_returns_stored_trading_dates_in_order() {
        let store = store_with_two_dates();

        let result = query_candles(&store, "101S", "1m", "both", "20261114", "20261115");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["date"], "20261113");
        assert_eq!(result[1]["date"], "20261114");
    }

    #[test]
    fn unknown_date_filter_falls_back_to_both_stored_dates() {
        let store = store_with_two_dates();

        let result = query_candles(&store, "101S", "1m", "invalid", "20261113", "20261114");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["date"], "20261113");
        assert_eq!(result[1]["date"], "20261114");
    }
}
