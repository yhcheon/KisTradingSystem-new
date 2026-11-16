// ============================================================
// 파일명: core/candles/aggregation.rs
// 설명: 1분봉에서 N분봉 파생 및 시간 그룹 계산
// ============================================================

use super::types::Candle;
use std::collections::BTreeMap;

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
                if c.high > agg.high {
                    agg.high = c.high;
                }
                if c.low < agg.low {
                    agg.low = c.low;
                }
                agg.close = c.close;
                agg.volume += c.volume;
            })
            .or_insert_with(|| c.clone());
    }

    groups.into_values().collect()
}

/// "HHMM" → 분 단위 숫자 (예: "0930" → 570)
pub(super) fn parse_hhmm_to_minutes(hhmm: &str) -> u32 {
    if hhmm.len() < 4 {
        return 0;
    }
    let h: u32 = hhmm[0..2].parse().unwrap_or(0);
    let m: u32 = hhmm[2..4].parse().unwrap_or(0);
    h * 60 + m
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(time: &str, open: f64, high: f64, low: f64, close: f64, volume: u64) -> Candle {
        Candle {
            time: time.to_string(),
            open,
            high,
            low,
            close,
            volume,
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
        assert_eq!(result[0].time, "0900"); // 그룹 첫 봉 시각
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

    #[test]
    fn test_aggregate_empty_input_returns_empty() {
        let input: Vec<Candle> = vec![];

        let result = aggregate(&input, 5);

        assert!(result.is_empty());
    }

    #[test]
    fn test_aggregate_zero_interval_returns_input_unchanged() {
        let input = vec![
            c("0900", 100.0, 105.0, 99.0, 104.0, 10),
            c("0901", 104.0, 108.0, 103.0, 107.0, 15),
        ];

        let result = aggregate(&input, 0);

        assert_eq!(result.len(), input.len());
        assert_eq!(result[0].time, "0900");
        assert_eq!(result[0].open, 100.0);
        assert_eq!(result[0].high, 105.0);
        assert_eq!(result[0].low, 99.0);
        assert_eq!(result[0].close, 104.0);
        assert_eq!(result[0].volume, 10);
        assert_eq!(result[1].time, "0901");
        assert_eq!(result[1].close, 107.0);
    }

    #[test]
    fn test_aggregate_15m_boundary() {
        let input = vec![
            c("0914", 100.0, 105.0, 99.0, 104.0, 10),
            c("0915", 104.0, 108.0, 103.0, 107.0, 15),
            c("0916", 107.0, 110.0, 106.0, 109.0, 20),
        ];

        let result = aggregate(&input, 15);

        assert_eq!(result.len(), 2);

        assert_eq!(result[0].time, "0914");
        assert_eq!(result[0].open, 100.0);
        assert_eq!(result[0].high, 105.0);
        assert_eq!(result[0].low, 99.0);
        assert_eq!(result[0].close, 104.0);
        assert_eq!(result[0].volume, 10);

        assert_eq!(result[1].time, "0915");
        assert_eq!(result[1].open, 104.0);
        assert_eq!(result[1].high, 110.0);
        assert_eq!(result[1].low, 103.0);
        assert_eq!(result[1].close, 109.0);
        assert_eq!(result[1].volume, 35);
    }
}
