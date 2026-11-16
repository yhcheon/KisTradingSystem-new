use crate::core::time::today_yyyymmdd;
use tokio::sync::{broadcast, mpsc};

pub(super) const TR_ID: &str = "H0IFCNT0";

#[derive(Debug, Clone)]
pub struct RealtimeTick {
    pub code: String,
    // 거래일은 내부 시세 메타데이터로 보존하며 브라우저 메시지에는 노출하지 않습니다.
    #[allow(dead_code)]
    pub date: String,
    pub price: String,
    pub change: String,
    pub change_rate: String,
    pub volume: String,
    pub last_qty: String,
    pub time: String,
}

/// KIS 체결 프레임을 한 번만 해석해 내부 공통 Tick 모델로 변환합니다.
pub(super) fn parse_tick(text: &str) -> Option<RealtimeTick> {
    let mut parts = text.splitn(4, '|');
    let _encrypted = parts.next()?;
    if parts.next()? != TR_ID {
        return None;
    }
    let _count = parts.next()?;
    let fields: Vec<&str> = parts.next()?.split('^').collect();
    if fields.len() <= 10
        || fields[0].is_empty()
        || fields[1].len() != 6
        || !fields[1].bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }

    let price = fields[5].parse::<f64>().ok()?;
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    let last_qty = fields[9].parse::<u64>().ok()?;

    Some(RealtimeTick {
        code: fields[0].to_string(),
        date: today_yyyymmdd(chrono::Utc::now()),
        time: fields[1].to_string(),
        change: format!("{}{}", fields[3], fields[2]),
        change_rate: fields[4].to_string(),
        price: fields[5].to_string(),
        last_qty: last_qty.to_string(),
        volume: fields[10].to_string(),
    })
}

pub(super) fn handle_text(
    text: &str,
    pong_tx: &mpsc::Sender<String>,
) {
    if text.contains("\\"tr_id\\":\\"PINGPONG\\"") {
        tracing::debug!("PINGPONG 수신, 응답 전송 예약");
        let _ = pong_tx.try_send(text.to_string());
        return;
    }

    if !text.split('|').nth(1).is_some_and(|tr_id| tr_id == TR_ID) {
        tracing::debug!("WS 응답: {}", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(price: &str, time: &str, qty: &str) -> String {
        format!("0|{TR_ID}|001|101S^{time}^1.25^+^0.36^{price}^0^0^0^{qty}^1200")
    }

    #[test]
    fn parses_a_valid_trade_frame() {
        let tick = parse_tick(&frame("350.25", "091530", "3")).unwrap();
        assert_eq!(tick.code, "101S");
        assert_eq!(tick.time, "091530");
        assert_eq!(tick.price, "350.25");
        assert_eq!(tick.last_qty, "3");
        assert_eq!(tick.volume, "1200");
        assert_eq!(tick.change, "+1.25");
    }

    #[test]
    fn rejects_invalid_price_and_time() {
        assert!(parse_tick(&frame("not-a-price", "091530", "3")).is_none());
        assert!(parse_tick(&frame("0", "091530", "3")).is_none());
        assert!(parse_tick(&frame("350.25", "91530", "3")).is_none());
        assert!(parse_tick(&frame("350.25", "09AB30", "3")).is_none());
    }

    #[test]
    fn rejects_invalid_last_quantity() {
        assert!(parse_tick(&frame("350.25", "091530", "x")).is_none());
    }

    #[test]
    fn ignores_non_trade_frames() {
        assert!(parse_tick("0|PINGPONG|001|{}").is_none());
    }
}
