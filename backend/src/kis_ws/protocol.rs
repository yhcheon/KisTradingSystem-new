use crate::core::time::{previous_calendar_day_yyyymmdd, today_yyyymmdd};
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
    hour <= 29 && minute <= 59 && second <= 59
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
    /// 주간 체결은 "day", KRX 야간 체결은 "night".
    pub market_session: String,
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

    let utc_now = chrono::Utc::now();
    let is_night = tr_id == NIGHT_TR_ID;
    let hour = fields[FIELD_TIME][0..2].parse::<u32>().ok()?;
    let (date, time) = if is_night && hour < 6 {
        // CM 분봉 API는 자정 이후를 전일 영업일의 24시 이후(예: 25:30)로 표현합니다.
        (
            previous_calendar_day_yyyymmdd(utc_now),
            format!("{:02}{}", hour + 24, &fields[FIELD_TIME][2..]),
        )
    } else {
        (today_yyyymmdd(utc_now), fields[FIELD_TIME].to_string())
    };

    Some(RealtimeTick {
        code: fields[FIELD_CODE].to_string(),
        date,
        time,
        change: format!("{}{}", fields[FIELD_CHANGE_SIGN], fields[FIELD_CHANGE]),
        change_rate: fields[FIELD_CHANGE_RATE].to_string(),
        price: fields[FIELD_PRICE].to_string(),
        last_qty: last_qty.to_string(),
        volume: cumulative_volume.to_string(),
        market_session: if is_night { "night" } else { "day" }.to_string(),
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
    fn parses_day_and_night_trade_frames_with_session() {
        let day = parse_tick(&frame(DAY_TR_ID, "350.25", "091530", "3")).unwrap();
        assert_eq!(day.market_session, "day");
        let night = parse_tick(&frame(NIGHT_TR_ID, "350.25", "201530", "3")).unwrap();
        assert_eq!(night.market_session, "night");
        assert_eq!(night.code, "101W9000");
        assert_eq!(night.time, "201530");
        assert_eq!(night.price, "350.25");
        assert_eq!(night.last_qty, "3");
        assert_eq!(night.volume, "1200");
        assert_eq!(night.change, "+1.25");
    }

    #[test]
    fn normalizes_after_midnight_night_tick_to_extended_business_time() {
        let tick = parse_tick(&frame(NIGHT_TR_ID, "350.25", "010203", "3")).unwrap();
        assert_eq!(tick.time, "250203");
        assert_eq!(
            tick.date,
            previous_calendar_day_yyyymmdd(chrono::Utc::now())
        );
    }

    #[test]
    fn rejects_invalid_price_and_time() {
        assert!(parse_tick(&frame(DAY_TR_ID, "not-a-price", "091530", "3")).is_none());
        assert!(parse_tick(&frame(NIGHT_TR_ID, "0", "201530", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "91530", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "09AB30", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "246000", "3")).is_none());
        assert!(parse_tick(&frame(DAY_TR_ID, "350.25", "096060", "3")).is_none());
    }

    #[test]
    fn rejects_invalid_quantities() {
        assert!(parse_tick(&frame(NIGHT_TR_ID, "350.25", "201530", "x")).is_none());
        let invalid_volume =
            format!("0|{NIGHT_TR_ID}|001|101W9000^201530^1.25^+^0.36^350.25^0^0^0^3^x");
        assert!(parse_tick(&invalid_volume).is_none());
    }

    #[test]
    fn ignores_non_trade_frames() {
        assert!(parse_tick("0|PINGPONG|001|{}").is_none());
    }
}
