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
