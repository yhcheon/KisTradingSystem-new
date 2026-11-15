// ============================================================
// 파일명: core/candles/types.rs
// 설명: 캔들 자료형과 지원 interval 정의
// ============================================================

/// 캔들 1개 (1분봉 또는 N분봉)
#[derive(Debug, Clone)]
pub struct Candle {
    pub time: String, // "HHMM" (예: "0930")
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// 지원하는 interval 목록
pub const INTERVALS: &[&str] = &["1m", "2m", "5m", "10m", "30m", "60m"];

/// interval 문자열 → 분 단위 숫자
pub(super) fn interval_to_minutes(interval: &str) -> Option<u32> {
    match interval {
        "1m" => Some(1),
        "2m" => Some(2),
        "5m" => Some(5),
        "10m" => Some(10),
        "30m" => Some(30),
        "60m" => Some(60),
        _ => None,
    }
}
