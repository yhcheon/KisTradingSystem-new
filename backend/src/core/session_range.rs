use chrono::{DateTime, Duration, NaiveDate, TimeZone, Timelike, Utc};
use serde::Serialize;

const DAY_START: (u32, u32) = (8, 45);
const DAY_END: (u32, u32) = (9, 30);
const NIGHT_START: (u32, u32) = (18, 0);
const NIGHT_END: (u32, u32) = (18, 45);

#[derive(Debug, Clone, Serialize)]
pub struct RangeAlert {
    pub session: String,
    pub direction: String,
    pub price: f64,
    pub reference_price: f64,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SessionRangeSnapshot {
    pub session: String,
    pub date: Option<String>,
    pub status: String,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub high_alerted: bool,
    pub low_alerted: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct RangeMonitorSnapshot {
    pub day: SessionRangeSnapshot,
    pub night: SessionRangeSnapshot,
}

#[derive(Debug, Default)]
struct SessionState {
    date: Option<String>,
    eligible: bool,
    high: Option<f64>,
    low: Option<f64>,
    high_alerted: bool,
    low_alerted: bool,
}

impl SessionState {
    fn new(date: &str, eligible: bool) -> Self {
        Self {
            date: Some(date.to_string()),
            eligible,
            ..Self::default()
        }
    }
}

/// 서버에서 정규선물 틱만 받아 주간/야간 시초 레인지를 독립적으로 관리합니다.
pub struct SessionRangeMonitor {
    regular_code: String,
    /// 최초 KIS WebSocket 구독 완료 시각. 구독 준비 전에는 경보 자격이 없습니다.
    started_at: Option<DateTime<Utc>>,
    day: SessionState,
    night: SessionState,
}

impl SessionRangeMonitor {
    pub fn new(regular_code: String) -> Self {
        Self {
            regular_code,
            started_at: None,
            day: SessionState::default(),
            night: SessionState::default(),
        }
    }

    fn new_at(regular_code: String, started_at: DateTime<Utc>) -> Self {
        Self {
            regular_code,
            started_at: Some(started_at),
            day: SessionState::default(),
            night: SessionState::default(),
        }
    }

    /// KIS WebSocket 구독 요청이 완료된 첫 시각을 기록합니다. 재연결 때는 최초 시각을 유지합니다.
    pub fn mark_feed_ready(&mut self) {
        let now = Utc::now();
        if self.started_at.is_none() {
            self.started_at = Some(now);
        } else {
            self.invalidate_if_collecting(now);
        }
    }

    /// 기준 구간 중 WebSocket이 끊기면 불완전한 레인지를 폐기합니다.
    pub fn mark_feed_disconnected(&mut self) {
        self.invalidate_if_collecting(Utc::now());
    }

    fn invalidate_if_collecting(&mut self, now_utc: DateTime<Utc>) {
        let now = now_utc.with_timezone(&chrono_tz::Asia::Seoul);
        let date = now.date_naive().format("%Y%m%d").to_string();
        let hhmm = format!("{:02}{:02}", now.hour(), now.minute());

        if hhmm >= "0845" && hhmm < "0930" {
            if self.day.date.as_deref() != Some(date.as_str()) {
                self.day = SessionState::new(&date, true);
            }
            self.day.eligible = false;
            self.day.high = None;
            self.day.low = None;
        }

        if hhmm >= "1800" && hhmm < "1845" {
            if self.night.date.as_deref() != Some(date.as_str()) {
                self.night = SessionState::new(&date, true);
            }
            self.night.eligible = false;
            self.night.high = None;
            self.night.low = None;
        }
    }

    pub fn on_tick(
        &mut self,
        code: &str,
        date: &str,
        time: &str,
        market_session: &str,
        price: f64,
    ) -> Option<RangeAlert> {
        if code != self.regular_code || !price.is_finite() || price <= 0.0 {
            return None;
        }

        let started_at = self.started_at.clone();
        let (session, start, end, state) = match market_session {
            "day" => ("day", DAY_START, DAY_END, &mut self.day),
            "night" => ("night", NIGHT_START, NIGHT_END, &mut self.night),
            _ => return None,
        };

        if !valid_date(date) || !valid_time(time) {
            return None;
        }

        if state.date.as_deref() != Some(date) {
            let eligible = session_start_utc(date, start)
                .is_some_and(|session_start| started_at.as_ref().is_some_and(|started| *started <= session_start));
            *state = SessionState::new(date, eligible);
        }

        if !state.eligible {
            return None;
        }

        let hhmm = &time[..4];
        let start_hhmm = format!("{:02}{:02}", start.0, start.1);
        let end_hhmm = format!("{:02}{:02}", end.0, end.1);

        if hhmm < start_hhmm.as_str() {
            return None;
        }

        if hhmm < end_hhmm.as_str() {
            state.high = Some(state.high.map_or(price, |high| high.max(price)));
            state.low = Some(state.low.map_or(price, |low| low.min(price)));
            return None;
        }

        let (Some(high), Some(low)) = (state.high, state.low) else {
            return None;
        };

        if price > high && !state.high_alerted {
            state.high_alerted = true;
            return Some(RangeAlert {
                session: session.to_string(),
                direction: "high".to_string(),
                price,
                reference_price: high,
                message: "시세가 고가 돌파 했습니다. 숏 포지션은 위험 관리 하세요.".to_string(),
            });
        }

        if price < low && !state.low_alerted {
            state.low_alerted = true;
            return Some(RangeAlert {
                session: session.to_string(),
                direction: "low".to_string(),
                price,
                reference_price: low,
                message: "시세가 저가를 깼습니다. 롱 포지션은 위험 관리 하세요.".to_string(),
            });
        }

        None
    }

    pub fn snapshot(&self) -> RangeMonitorSnapshot {
        let now = Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
        let today = now.date_naive();
        let day_date = today.format("%Y%m%d").to_string();
        let day_hhmm = format!("{:02}{:02}", now.hour(), now.minute());

        // KIS 야간 체결은 자정 이후 영업일 전날의 24시 이후 시각으로 정규화됩니다.
        let night_date = if now.hour() < 6 {
            (today - Duration::days(1)).format("%Y%m%d").to_string()
        } else {
            day_date.clone()
        };
        let night_hour = if now.hour() < 6 { now.hour() + 24 } else { now.hour() };
        let night_hhmm = format!("{:02}{:02}", night_hour, now.minute());

        RangeMonitorSnapshot {
            day: self.snapshot_session(&self.day, "day", &day_date, &day_hhmm, DAY_START, DAY_END),
            night: self.snapshot_session(
                &self.night,
                "night",
                &night_date,
                &night_hhmm,
                NIGHT_START,
                NIGHT_END,
            ),
        }
    }

    fn snapshot_session(
        &self,
        state: &SessionState,
        session: &str,
        current_date: &str,
        current_hhmm: &str,
        start: (u32, u32),
        end: (u32, u32),
    ) -> SessionRangeSnapshot {
        let date = state.date.as_deref().unwrap_or(current_date);
        let eligible = state.eligible
            || (state.date.is_none()
                && session_start_utc(date, start)
                    .is_some_and(|session_start| self.started_at.as_ref().is_some_and(|started| *started <= session_start)));
        let start_hhmm = format!("{:02}{:02}", start.0, start.1);
        let end_hhmm = format!("{:02}{:02}", end.0, end.1);
        let status = if !eligible {
            "unavailable"
        } else if current_hhmm < start_hhmm.as_str() {
            "waiting"
        } else if current_hhmm < end_hhmm.as_str() {
            "collecting"
        } else if state.high.is_some() && state.low.is_some() {
            "monitoring"
        } else {
            "unavailable"
        };

        SessionRangeSnapshot {
            session: session.to_string(),
            date: Some(date.to_string()),
            status: status.to_string(),
            high: state.high,
            low: state.low,
            high_alerted: state.high_alerted,
            low_alerted: state.low_alerted,
        }
    }
}

fn valid_date(value: &str) -> bool {
    value.len() == 8
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && NaiveDate::parse_from_str(value, "%Y%m%d").is_ok()
}

fn valid_time(value: &str) -> bool {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    let hour = value[0..2].parse::<u32>().unwrap_or(u32::MAX);
    let minute = value[2..4].parse::<u32>().unwrap_or(u32::MAX);
    let second = value[4..6].parse::<u32>().unwrap_or(u32::MAX);
    hour <= 29 && minute <= 59 && second <= 59
}

fn session_start_utc(date: &str, start: (u32, u32)) -> Option<DateTime<Utc>> {
    let date = NaiveDate::parse_from_str(date, "%Y%m%d").ok()?;
    let local = date.and_hms_opt(start.0, start.1, 0)?;
    chrono_tz::Asia::Seoul
        .from_local_datetime(&local)
        .single()
        .map(|value| value.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(started_at: &str) -> SessionRangeMonitor {
        let started_at = chrono::DateTime::parse_from_rfc3339(started_at)
            .unwrap()
            .with_timezone(&Utc);
        SessionRangeMonitor::new_at("101W9000".to_string(), started_at)
    }

    #[test]
    fn day_session_collects_then_alerts_once_per_direction() {
        let mut monitor = monitor("2026-11-18T23:30:00Z"); // KST 08:30
        for (time, price) in [
            ("084500", 350.0),
            ("090000", 351.0),
            ("092959", 349.0),
        ] {
            assert!(monitor.on_tick("101W9000", "20261119", time, "day", price).is_none());
        }
        assert_eq!(monitor.snapshot().day.high, Some(351.0));
        assert_eq!(monitor.snapshot().day.low, Some(349.0));

        let high = monitor.on_tick("101W9000", "20261119", "093000", "day", 351.05).unwrap();
        assert_eq!(high.direction, "high");
        assert!(monitor.on_tick("101W9000", "20261119", "093001", "day", 351.10).is_none());
        assert!(monitor.on_tick("101W9000", "20261119", "093002", "day", 350.0).is_none());
        assert!(monitor.on_tick("101W9000", "20261119", "093003", "day", 348.95).is_some());
        assert!(monitor.on_tick("101W9000", "20261119", "093004", "day", 348.90).is_none());
    }

    #[test]
    fn late_start_disables_only_the_session_that_has_already_started() {
        let mut monitor = monitor("2026-11-19T00:00:00Z"); // KST 09:00
        assert!(monitor.on_tick("101W9000", "20261119", "090000", "day", 350.0).is_none());
        assert_eq!(monitor.snapshot().day.status, "unavailable");

        assert!(monitor.on_tick("101W9000", "20261119", "180000", "night", 350.0).is_none());
        assert_eq!(monitor.snapshot().night.high, Some(350.0));
    }

    #[test]
    fn ignores_mini_ticks_and_keeps_day_and_night_independent() {
        let mut monitor = monitor("2026-11-18T23:00:00Z"); // KST 08:00
        assert!(monitor.on_tick("201W9000", "20261119", "084500", "day", 500.0).is_none());
        assert_eq!(monitor.snapshot().day.high, None);

        monitor.on_tick("101W9000", "20261119", "084500", "day", 350.0);
        monitor.on_tick("101W9000", "20261119", "092959", "day", 349.0);
        assert_eq!(monitor.snapshot().night.high, None);
        monitor.on_tick("101W9000", "20261119", "093000", "day", 351.0);
        assert_eq!(monitor.snapshot().day.high, Some(350.0));
    }

    #[test]
    fn next_business_date_resets_alert_flags() {
        let mut monitor = monitor("2026-11-18T23:00:00Z");
        monitor.on_tick("101W9000", "20261119", "084500", "day", 350.0);
        monitor.on_tick("101W9000", "20261119", "092959", "day", 349.0);
        assert!(monitor.on_tick("101W9000", "20261119", "093000", "day", 351.0).is_some());
        monitor.on_tick("101W9000", "20261120", "084500", "day", 352.0);
        assert_eq!(monitor.snapshot().day.high_alerted, false);
        assert_eq!(monitor.snapshot().day.date.as_deref(), Some("20261120"));
    }

    #[test]
    fn night_range_survives_kis_extended_time_after_midnight() {
        let mut monitor = monitor("2026-11-19T08:00:00Z"); // KST 17:00
        monitor.on_tick("101W9000", "20261119", "180000", "night", 350.0);
        monitor.on_tick("101W9000", "20261119", "184459", "night", 349.0);
        assert!(monitor.on_tick("101W9000", "20261119", "184500", "night", 351.0).is_some());
        assert!(monitor.on_tick("101W9000", "20261119", "250001", "night", 348.9).is_some());
        assert_eq!(monitor.snapshot().night.low_alerted, true);
    }
}
