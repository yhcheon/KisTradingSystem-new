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
