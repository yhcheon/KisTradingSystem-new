// ============================================================
// 파일명: server/handlers/market/ws_message.rs
// 설명: 브라우저 WebSocket 시세 메시지 직렬화
// ============================================================

use crate::core::session_range::RangeMonitorSnapshot;
use crate::kis_ws::RealtimeTick;
use serde_json::json;

/// 실시간 체결 데이터를 기존 브라우저 WebSocket 메시지 형식으로 변환합니다.
pub(super) fn tick_message(tick: &RealtimeTick, range: &RangeMonitorSnapshot) -> String {
    serde_json::to_string(&json!({
        "type": "tick",
        "market_session": tick.market_session,
        "code": tick.code,
        "date": tick.date,
        "price": tick.price,
        "change": tick.change,
        "change_rate": tick.change_rate,
        "volume": tick.volume,
        "last_qty": tick.last_qty,
        "time": tick.time,
        "range": range,
        "range_alert": tick.range_alert,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(market_session: &str) -> RealtimeTick {
        RealtimeTick {
            code: "101S".to_string(),
            date: "20261116".to_string(),
            price: "350.25".to_string(),
            change: "+1.25".to_string(),
            change_rate: "0.36".to_string(),
            volume: "1200".to_string(),
            last_qty: "3".to_string(),
            time: "091530".to_string(),
            market_session: market_session.to_string(),
            range_alert: None,
        }
    }

    #[test]
    fn serializes_tick_with_expected_browser_message_fields() {
        let result = tick_message(&tick("night"), &RangeMonitorSnapshot {\n            day: crate::core::session_range::SessionRangeSnapshot { session: "day".to_string(), date: None, status: "waiting".to_string(), high: None, low: None, high_alerted: false, low_alerted: false },\n            night: crate::core::session_range::SessionRangeSnapshot { session: "night".to_string(), date: None, status: "waiting".to_string(), high: None, low: None, high_alerted: false, low_alerted: false },\n        });
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "tick",
                "market_session": "night",
                "code": "101S",
                "date": "20261116",
                "price": "350.25",
                "change": "+1.25",
                "change_rate": "0.36",
                "volume": "1200",
                "last_qty": "3",
                "time": "091530"
            })
        );
    }
}
