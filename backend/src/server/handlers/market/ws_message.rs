// ============================================================
// 파일명: server/handlers/market/ws_message.rs
// 설명: 브라우저 WebSocket 시세 메시지 직렬화
// ============================================================

use crate::kis_ws::RealtimeTick;
use serde_json::json;

/// 실시간 체결 데이터를 기존 브라우저 WebSocket 메시지 형식으로 변환합니다.
pub(super) fn tick_message(tick: &RealtimeTick) -> String {
    serde_json::to_string(&json!({
        "type": "tick",
        "code": tick.code,
        "price": tick.price,
        "change": tick.change,
        "change_rate": tick.change_rate,
        "volume": tick.volume,
        "last_qty": tick.last_qty,
        "time": tick.time,
    }))
    .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick() -> RealtimeTick {
        RealtimeTick {
            code: "101S".to_string(),
            date: "20261116".to_string(),
            price: "350.25".to_string(),
            change: "+1.25".to_string(),
            change_rate: "0.36".to_string(),
            volume: "1200".to_string(),
            last_qty: "3".to_string(),
            time: "091530".to_string(),
        }
    }

    #[test]
    fn serializes_tick_with_expected_browser_message_fields() {
        let result = tick_message(&tick());
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "tick",
                "code": "101S",
                "price": "350.25",
                "change": "+1.25",
                "change_rate": "0.36",
                "volume": "1200",
                "last_qty": "3",
                "time": "091530"
            })
        );
    }

    #[test]
    fn does_not_expose_internal_tick_date_in_browser_message() {
        let result = tick_message(&tick());
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert!(value.get("date").is_none());
    }
}
