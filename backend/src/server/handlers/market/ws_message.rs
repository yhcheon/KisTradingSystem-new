// ============================================================
// 파일명: server/handlers/market/ws_message.rs
// 설명: 브라우저 WebSocket 시세 메시지 직렬화
// ============================================================

use crate::core::session_range::RangeMonitorSnapshot;
use crate::kis_ws::RealtimeTick;
use serde_json::json;

/// 실시간 체결 데이터와 최신 세션 레인지 상태를 브라우저 메시지로 변환합니다.
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
    use crate::core::session_range::SessionRangeSnapshot;

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
            market_session: "night".to_string(),
            range_alert: None,
        }
    }

    #[test]
    fn serializes_tick_and_range_state_for_browser() {
        let range = RangeMonitorSnapshot {
            day: SessionRangeSnapshot {
                session: "day".to_string(),
                date: Some("20261116".to_string()),
                status: "monitoring".to_string(),
                high: Some(351.0),
                low: Some(349.0),
                high_alerted: false,
                low_alerted: false,
            },
            night: SessionRangeSnapshot {
                session: "night".to_string(),
                date: None,
                status: "waiting".to_string(),
                high: None,
                low: None,
                high_alerted: false,
                low_alerted: false,
            },
        };

        let result = tick_message(&tick(), &range);
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();

        assert_eq!(value["type"], "tick");
        assert_eq!(value["code"], "101S");
        assert_eq!(value["range"]["day"]["high"], 351.0);
        assert_eq!(value["range"]["day"]["low"], 349.0);
        assert_eq!(value["range"]["night"]["status"], "waiting");
        assert!(value["range_alert"].is_null());
    }
}
