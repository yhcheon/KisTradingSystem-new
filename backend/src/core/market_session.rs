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

    fn at(h: u32, m: u32) -> NaiveTime {
        NaiveTime::from_hms_opt(h, m, 0).unwrap()
    }

    #[test]
    fn day_and_night_session_boundaries_are_kst() {
        assert_eq!(
            active_sessions_at(at(3, 0)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        assert_eq!(
            active_sessions_at(at(6, 4)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        assert_eq!(
            active_sessions_at(at(6, 5)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(8, 39)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(8, 40)),
            ActiveSessions {
                day: true,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(15, 49)),
            ActiveSessions {
                day: true,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(15, 50)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(17, 54)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(17, 55)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        assert_eq!(
            active_sessions_at(at(23, 0)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
    }

    #[test]
    fn gap_periods_have_no_active_session() {
        // 빈틈 1: 야간 종료(06:05) ~ 주간 시작(08:40) — 아무것도 안 켜져야 함
        assert_eq!(
            active_sessions_at(at(6, 5)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(7, 0)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(8, 39)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        // 빈틈 2: 주간 종료(15:50) ~ 야간 시작(17:55)
        assert_eq!(
            active_sessions_at(at(15, 50)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(16, 30)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
        assert_eq!(
            active_sessions_at(at(17, 54)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
    }

    #[test]
    fn night_session_wraps_past_midnight() {
        // 자정 넘김: 23:59와 00:00이 같은 야간 세션이어야 함
        assert_eq!(
            active_sessions_at(at(23, 59)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        assert_eq!(
            active_sessions_at(at(0, 0)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        assert_eq!(
            active_sessions_at(at(6, 4)),
            ActiveSessions {
                day: false,
                night: true
            }
        );
        // 그런데 06:05가 되면 끝나야 함
        assert_eq!(
            active_sessions_at(at(6, 5)),
            ActiveSessions {
                day: false,
                night: false
            }
        );
    }

    #[test]
    fn day_session_never_overlaps_night() {
        // 주간 시간에는 야간이, 야간 시간에는 주간이 절대 같이 켜지면 안 됨
        for (h, m) in [(9, 0), (12, 0), (15, 0)] {
            let s = active_sessions_at(at(h, m));
            assert!(s.day && !s.night, "{h}:{m}에서 겹침: {s:?}");
        }
        for (h, m) in [(18, 0), (22, 0), (2, 0)] {
            let s = active_sessions_at(at(h, m));
            assert!(!s.day && s.night, "{h}:{m}에서 겹침: {s:?}");
        }
    }
}
