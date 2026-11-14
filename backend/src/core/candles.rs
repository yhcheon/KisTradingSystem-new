// ============================================================
// 파일명: core/candles.rs
// 설명: 캔들(봉) 메모리 저장소
//       - 1분봉 원본을 받아 2m/5m/10m/30m/60m 파생 계산
//       - 계산은 로드 시 1회만, 이후 조회는 메모리 복사만
//       - 그룹 키 = (날짜, 분 / N) 방식으로 시간 경계 정확히 맞춤
// 작성일: 2026-11-14
// ============================================================

use std::collections::{BTreeMap, HashMap};

/// 캔들 1개 (1분봉 또는 N분봉)
#[derive(Debug, Clone)]
pub struct Candle {
    pub time: String,   // "HHMM" (예: "0930")
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

/// 지원하는 interval 목록
pub const INTERVALS: &[&str] = &["1m", "2m", "5m", "10m", "30m", "60m"];

/// interval 문자열 → 분 단위 숫자
fn interval_to_minutes(interval: &str) -> Option<u32> {
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

/// 캔들 저장소
pub struct CandleStore {
    // 종목코드 → interval → 봉 배열
    data: HashMap<String, HashMap<String, Vec<Candle>>>,
}

impl CandleStore {
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// 1분봉 일괄 로드 + 모든 interval 파생 계산
    /// - source: KIS REST에서 받은 1분봉 (날짜순 오름차순 정렬)
    pub fn load_1m(&mut self, code: &str, candles_1m: Vec<Candle>) {
        let mut per_interval: HashMap<String, Vec<Candle>> = HashMap::new();

        // 1m 원본
        per_interval.insert("1m".to_string(), candles_1m.clone());

        // 상위 interval 파생
        for &interval in INTERVALS {
            if interval == "1m" { continue; }
            let n = match interval_to_minutes(interval) {
                Some(v) => v,
                None => continue,
            };
            let agg = aggregate(&candles_1m, n);
            per_interval.insert(interval.to_string(), agg);
        }

        self.data.insert(code.to_string(), per_interval);
        tracing::info!(
            "[CandleStore] {} 로드 완료: 1m={}건, 5m={}건, 60m={}건",
            code,
            self.count(code, "1m"),
            self.count(code, "5m"),
            self.count(code, "60m"),
        );
    }

    /// interval 조회 (메모리에서 복사만)
    pub fn get(&self, code: &str, interval: &str) -> Vec<Candle> {
        self.data
            .get(code)
            .and_then(|m| m.get(interval))
            .cloned()
            .unwrap_or_default()
    }

    /// interval별 봉 개수
    pub fn count(&self, code: &str, interval: &str) -> usize {
        self.data
            .get(code)
            .and_then(|m| m.get(interval))
            .map(|v| v.len())
            .unwrap_or(0)
    }
}

/// 1분봉을 N분봉으로 파생
/// 그룹 키 = (날짜, 분 / N)
pub fn aggregate(candles: &[Candle], n: u32) -> Vec<Candle> {
    if n <= 1 || candles.is_empty() {
        return candles.to_vec();
    }

    // 그룹 키 → 병합 중인 캔들 (BTreeMap으로 시간순 유지)
    let mut groups: BTreeMap<u32, Candle> = BTreeMap::new();

    for c in candles {
        let total_min = parse_hhmm_to_minutes(&c.time);
        let group_key = total_min / n;

        groups
            .entry(group_key)
            .and_modify(|agg| {
                if c.high > agg.high { agg.high = c.high; }
                if c.low < agg.low { agg.low = c.low; }
                agg.close = c.close;
                agg.volume += c.volume;
            })
            .or_insert_with(|| c.clone());
    }

    groups.into_values().collect()
}

/// "HHMM" → 분 단위 숫자 (예: "0930" → 570)
fn parse_hhmm_to_minutes(hhmm: &str) -> u32 {
    if hhmm.len() < 4 { return 0; }
    let h: u32 = hhmm[0..2].parse().unwrap_or(0);
    let m: u32 = hhmm[2..4].parse().unwrap_or(0);
    h * 60 + m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(time: &str, open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
        Candle {
            time: time.to_string(), open, high, low, close, volume,
        }
    }

    #[test]
    fn test_aggregate_5m() {
        // 09:00~09:04 5개 봉 → 5분봉 1개
        let input = vec![
            c("0900", 100.0, 105.0, 99.0, 104.0, 10),
            c("0901", 104.0, 108.0, 103.0, 107.0, 15),
            c("0902", 107.0, 109.0, 106.0, 108.0, 12),
            c("0903", 108.0, 110.0, 107.0, 109.0, 8),
            c("0904", 109.0, 111.0, 108.0, 110.0, 20),
        ];
        let result = aggregate(&input, 5);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].open, 100.0);
        assert_eq!(result[0].high, 111.0);
        assert_eq!(result[0].low, 99.0);
        assert_eq!(result[0].close, 110.0);
        assert_eq!(result[0].volume, 65);
        assert_eq!(result[0].time, "0900");   // 그룹 첫 봉 시각
    }

    #[test]
    fn test_aggregate_boundary() {
        // 08:45~08:49 → 5분봉 1, 08:50~08:54 → 5분봉 2
        let input: Vec<Candle> = (45..55)
            .map(|i| c(&format!("08{:02}", i), 100.0, 101.0, 99.0, 100.5, 1))
            .collect();
        let result = aggregate(&input, 5);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].time, "0845");
        assert_eq!(result[1].time, "0850");
    }

    #[test]
    fn test_aggregate_1m_identity() {
        let input = vec![c("0900", 100.0, 105.0, 99.0, 104.0, 10)];
        let result = aggregate(&input, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].close, 104.0);
    }
}


