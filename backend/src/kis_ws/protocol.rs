use crate::core::time::today_yyyymmdd;
use tokio::sync::mpsc;

pub(super) const DAY_TR_ID: &str = "H0IFCNT0";
pub(super) const NIGHT_TR_ID: &str = "H0MFCNT0";

// 주간/야간 선물 체결 프레임의 공통 필드 인덱스.
const FIELD_CODE: usize = 0;
const FIELD_TIME: usize = 1;
const FIELD_CHANGE: usize = 2;
const FIELD_CHANGE_SIGN: usize = 3;
const FIELD_CHANGE_RATE: usize = 4;
const FIELD_PRICE: usize = 5;
const FIELD_LAST_QTY: usize = 9;
const FIELD_CUMULATIVE_VOLUME: usize = 10;

fn is_valid_hhmmss(value: &str) -> bool {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let hour = value[0..2].parse::<u8>().unwrap_or(u8::MAX);
    let minute = value[2..4].parse::<u8>().unwrap_or(u8::MAX);
    let second = value[4..6].parse::<u8>().unwrap_or(u8::MAX);
    hour <= 23 && minute <= 59 && second <= 59
}

#[derive(Debug, Clone)]
pub struct RealtimeTick {
    pub code: String,
    #[allow(dead_code)]
    pub date: String,
    pub price: String,
    pub change: String,
    pub change_rate: String,
    pub volume: String,
    pub last_qty: String,
    pub time: String,
}

/// 주간(H0IFCNT0) 및 KRX 야간선물(H0MFCNT0) 체결을 공통 Tick으로 변환합니다.
pub(super) fn parse_tick(text: &str) -> Option<RealtimeTick> {
    let mut parts = text.splitn(4, '|');
    let _encrypted = parts.next()?;
    let tr_id = parts.next()?;
    if tr_id != DAY_TR_ID && tr_id != NIGHT_TR_ID {
        return None;
    }
    let _count = parts.next()?;
    let fields: Vec<&str> = parts.next()?.split('^').collect();
    if fields.len() <= FIELD_CUMULATIVE_VOLUME
        || fields[FIELD_CODE].is_empty()
        || !is_valid_hhmmss(fields[FIELD_TIME])
    {
        return None;
    }

    let price = fields[FIELD_PRICE].parse::<f64>().ok()?;
    if !price.is_finite() || price <= 0.0 {
        return None;
    }
    let last_qty = fields[FIELD_LAST_QTY].parse::<u64>().ok()?;
    let cumulative_volume = fields[FIELD_CUMULATIVE_VOLUME].parse::<u64>().ok()?;

    Some(RealtimeTick {
        code: fields[FIELD_CODE].to_string(),
        date: today_yyyymmdd(chrono::Utc::now()),
        time: fields[FIELD_TIME].to_string(),
        change: format!("{}{}", fields[FIELD_CHANGE_SIGN], fields[FIELD_CHANGE]),
        change_rate: fields[FIELD_CHANGE_RATE].to_string(),
        price: fields[FIELD_PRICE].to_string(),
        last_qty: last_qty.to_string(),
        volume: cumulative_volume.to_string(),
    })
}

pub(super) fn handle_text(text: &str, pong_tx: &mpsc::Sender<String>) {
    if text.contains(r#""tr_id":"PINGPONG""#) {
        tracing::debug!("PINGPONG 수신, 응답 전송 예약");
        let _ = pong_tx.try_send(text.to_string());
        return;
    }

    let tr_id = text.split('|').nth(1).unwrap_or_default();
    if tr_id != DAY_TR_ID && tr_id != NIGHT_TR_ID {
        tracing::debug!("WS 응답: {}", text);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(tr_id: &str, price: &str, time: &str, qty: &str) -> String {
        format!("0|{tr_id}|001|101W9000^{time}^1.25^+^0.36^{price}^0^0^0^{qty}^1200")
    }

    #[test]
    fn parses_day_and_night_trade_frames() {
        for tr_id in [DAY_TR_ID, NIGHT_TR_ID] {
            let tick = parse_tick(&frame(tr_id, "350.25", "091530", "3")).unwrap();
            assert_eq!(tick.code, "101W9000");
            assert_eq!(tick.time, "091530");
            assert_eq!(tick.price, "350.25");
            assert_eq!(tick.last_qty, "3");
            assert_eq!(tick.volume, "1200");
            assert_eq!(tick.change, "+1.25");
        }
    }

    #[test]
    fn rejects_invalid_price_and_time() {
        assert!(parse_tick(&frame(DAY_TR_ID, "not-a-price", "091530", "3")).is_none());
        assert!(parse_tick(&frame(NIGHT_TR_ID, "0", "091530", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "91530", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "09AB30", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "246000", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "096060", "3")).is_none());
    }

    #[test]
    fn rejects_invalid_quantities() {
        assert!(parse_tick(&frame(NIGHT_TR_ID, "350.25", "091530", "x")).is_none());
        let invalid_volume = format!("0|{NIGHT_TR_ID}|001|101W9000^091530^1.25^+^0.36^350.25^0^0^0^3^x");
        assert!(parse_tick(&invalid_volume).is_none());
    }

    #[test]
    fn ignores_non_trade_frames() {
        assert!(parse_tick("0|PINGPONG|001|{}").is_none());
    }
}
