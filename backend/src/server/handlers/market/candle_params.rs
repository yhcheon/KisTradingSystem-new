// ============================================================
// 파일명: server/handlers/market/candle_params.rs
// 설명: 분봉 캔들 조회 API 요청 파라미터
// ============================================================

use serde::Deserialize;

const SUPPORTED_INTERVALS: &[&str] = &["1m", "2m", "5m", "10m", "30m", "60m"];
const SUPPORTED_DATES: &[&str] = &["yesterday", "today", "both"];

impl CandleParams {
    pub(crate) fn validate(&self) -> Result<(&str, &str, &str), &'static str> {
        let code = self.code.as_deref().map(str::trim).filter(|value| !value.is_empty())
            .ok_or("code 파라미터 필요")?;
        if code.len() > 32 || !code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
            return Err("올바른 code 파라미터가 필요합니다");
        }

        let interval = self.interval.as_deref().unwrap_or("1m");
        if !SUPPORTED_INTERVALS.contains(&interval) {
            return Err("지원하지 않는 interval입니다");
        }

        let date = self.date.as_deref().unwrap_or("both");
        if !SUPPORTED_DATES.contains(&date) {
            return Err("지원하지 않는 date 파라미터입니다");
        }

        Ok((code, interval, date))
    }
}

/// 캔들 조회 API의 선택적 쿼리 파라미터입니다.
#[derive(Debug, Deserialize)]
pub(crate) struct CandleParams {
    pub code: Option<String>,
    pub interval: Option<String>,
    pub date: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::CandleParams;

    #[test]
    fn applies_defaults_and_accepts_supported_values() {
        let params = CandleParams { code: Some("101S".into()), interval: None, date: None };
        assert_eq!(params.validate().unwrap(), ("101S", "1m", "both"));
        let params = CandleParams { code: Some("101S".into()), interval: Some("5m".into()), date: Some("today".into()) };
        assert_eq!(params.validate().unwrap(), ("101S", "5m", "today"));
    }

    #[test]
    fn rejects_missing_or_invalid_code() {
        for code in [None, Some("".into()), Some("bad code".into()), Some("A&B".into())] {
            let params = CandleParams { code, interval: None, date: None };
            assert!(params.validate().is_err());
        }
    }

    #[test]
    fn rejects_unsupported_interval_and_date() {
        let params = CandleParams { code: Some("101S".into()), interval: Some("3m".into()), date: None };
        assert!(params.validate().is_err());
        let params = CandleParams { code: Some("101S".into()), interval: None, date: Some("invalid".into()) };
        assert!(params.validate().is_err());
    }
}
