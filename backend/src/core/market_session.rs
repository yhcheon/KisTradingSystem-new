//! KST 기준 KIS 선물 세션 구독 시간 판정.
use chrono::NaiveTime;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActiveSessions {
    pub day: bool,
    pub night: bool,
}

pub fn active_sessions_at(time: NaiveTime) -> ActiveSessions {
    let day_start = NaiveTime::from_hms_opt(8, 40, 0).unwrap();
    let day_end = NaiveTime::from_hms_opt(15, 50, 0).unwrap();
    let night_start = NaiveTime::from_hms_opt(17, 55, 0).unwrap();
    let night_end = NaiveTime::from_hms_opt(6, 5, 0).unwrap();

    ActiveSessions {
        day: time >= day_start && time < day_end,
        // 자정을 넘는 구간: 17:55 이상 또는 06:05 이전.
        night: time >= night_start || time < night_end,
    }
}

pub fn current_active_sessions() -> ActiveSessions {
    let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
    active_sessions_at(now.time())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(h: u32, m: u32) -> NaiveTime { NaiveTime::from_hms_opt(h, m, 0).unwrap() }

    #[test]
    fn day_and_night_session_boundaries_are_kst() {
        assert_eq!(active_sessions_at(at(3, 0)), ActiveSessions { day: false, night: true });
        assert_eq!(active_sessions_at(at(6, 4)), ActiveSessions { day: false, night: true });
        assert_eq!(active_sessions_at(at(6, 5)), ActiveSessions { day: false, night: false });
        assert_eq!(active_sessions_at(at(8, 39)), ActiveSessions { day: false, night: false });
        assert_eq!(active_sessions_at(at(8, 40)), ActiveSessions { day: true, night: false });
        assert_eq!(active_sessions_at(at(15, 49)), ActiveSessions { day: true, night: false });
        assert_eq!(active_sessions_at(at(15, 50)), ActiveSessions { day: false, night: false });
        assert_eq!(active_sessions_at(at(17, 54)), ActiveSessions { day: false, night: false });
        assert_eq!(active_sessions_at(at(17, 55)), ActiveSessions { day: false, night: true });
        assert_eq!(active_sessions_at(at(23, 0)), ActiveSessions { day: false, night: true });
    }
}
