//! 한국 거래소 기준 날짜 계산 유틸리티.

use chrono::{DateTime, Duration, Utc};

/// UTC 시각을 한국 표준시(KST) 기준 YYYYMMDD 문자열로 변환합니다.
pub fn today_yyyymmdd(utc_now: DateTime<Utc>) -> String {
    utc_now
        .with_timezone(&chrono_tz::Asia::Seoul)
        .format("%Y%m%d")
        .to_string()
}

/// UTC 시각을 KST로 변환한 뒤 달력상 전날의 YYYYMMDD를 반환합니다.
pub fn previous_calendar_day_yyyymmdd(utc_now: DateTime<Utc>) -> String {
    (utc_now.with_timezone(&chrono_tz::Asia::Seoul).date_naive() - Duration::days(1))
        .format("%Y%m%d")
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{previous_calendar_day_yyyymmdd, today_yyyymmdd};
    use chrono::{DateTime, Utc};

    #[test]
    fn kst_date_rolls_over_before_utc_date_does() {
        let utc_now = DateTime::parse_from_rfc3339("2026-11-15T15:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(today_yyyymmdd(utc_now), "20261116");
        assert_eq!(previous_calendar_day_yyyymmdd(utc_now), "20261115");
    }

    #[test]
    fn kst_date_before_midnight_stays_on_same_day() {
        let utc_now = DateTime::parse_from_rfc3339("2026-11-15T14:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        assert_eq!(today_yyyymmdd(utc_now), "20261115");
        assert_eq!(previous_calendar_day_yyyymmdd(utc_now), "20261114");
    }
}
