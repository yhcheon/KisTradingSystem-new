// ============================================================
// 파일명: server/handlers/market/candle_response.rs
// 설명: 분봉 캔들 API 응답 데이터 변환
// ============================================================

use crate::core::candles::Candle;
use serde_json::{Value, json};

/// 내부 Candle을 API 응답용 JSON 배열로 변환합니다.
pub(super) fn to_json(candles: &[Candle], date_str: &str) -> Vec<Value> {
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
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(time: &str, open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
        Candle {
            time: time.to_string(),
            open,
            high,
            low,
            close,
            volume,
        }
    }

    #[test]
    fn empty_candles_return_empty_json_array() {
        let result = to_json(&[], "20261116");

        assert!(result.is_empty());
    }

    #[test]
    fn serializes_all_candle_fields_with_expected_api_format() {
        let candles = vec![candle("0901", 100.0, 105.0, 99.0, 103.0, 250)];

        let result = to_json(&candles, "20261116");

        assert_eq!(result.len(), 1);
        assert_eq!(
            result[0],
            json!({
                "date": "20261116",
                "time": "090100",
                "open": 100.0,
                "high": 105.0,
                "low": 99.0,
                "close": 103.0,
                "volume": 250.0
            })
        );
    }

    #[test]
    fn preserves_candle_order_when_serializing_multiple_items() {
        let candles = vec![
            candle("0901", 100.0, 101.0, 99.0, 100.5, 10),
            candle("0902", 100.5, 102.0, 100.0, 101.5, 20),
        ];

        let result = to_json(&candles, "20261116");

        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["time"], "090100");
        assert_eq!(result[1]["time"], "090200");
        assert_eq!(result[0]["volume"], 10.0);
        assert_eq!(result[1]["volume"], 20.0);
    }
}
