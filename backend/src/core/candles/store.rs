// ============================================================
// 파일명: core/candles/store.rs
// 설명: 종목별·날짜별 캔들 메모리 저장 및 실시간 갱신
// ============================================================

use super::{
    aggregation::{aggregate, parse_hhmm_to_minutes},
    types::{Candle, INTERVALS, interval_to_minutes},
};
use std::collections::{BTreeMap, HashMap};

fn is_valid_tick_time(value: &str) -> bool {
    if value.len() != 6 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }

    let hour = value[0..2].parse::<u8>().unwrap_or(u8::MAX);
    let minute = value[2..4].parse::<u8>().unwrap_or(u8::MAX);
    let second = value[4..6].parse::<u8>().unwrap_or(u8::MAX);

    hour <= 29 && minute <= 59 && second <= 59
}

fn is_valid_tick_date(value: &str) -> bool {
    value.len() == 8 && value.bytes().all(|byte| byte.is_ascii_digit())
}

pub struct CandleStore {
    // 종목코드 → 날짜("YYYY-MM-DD") → interval → 봉 배열
    data: HashMap<String, BTreeMap<String, HashMap<String, Vec<Candle>>>>,
}

impl CandleStore {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    pub fn load_1m(&mut self, code: &str, date: &str, candles_1m: Vec<Candle>) {
        // 주간(F)과 야간(CM) 조회 결과가 같은 영업일에 들어오므로
        // 같은 날짜 데이터는 시각 기준으로 병합하고 중복 분은 최신 입력으로 교체합니다.
        let existing = self
            .data
            .get(code)
            .and_then(|dates| dates.get(date))
            .and_then(|intervals| intervals.get("1m"))
            .cloned()
            .unwrap_or_default();
        let mut merged: BTreeMap<String, Candle> = existing
            .into_iter()
            .map(|candle| (candle.time.clone(), candle))
            .collect();
        for candle in candles_1m {
            merged.insert(candle.time.clone(), candle);
        }
        let merged_1m: Vec<Candle> = merged.into_values().collect();

        let mut per_interval: HashMap<String, Vec<Candle>> = HashMap::new();
        per_interval.insert("1m".to_string(), merged_1m.clone());

        for &interval in INTERVALS {
            if interval == "1m" {
                continue;
            }
            let n = match interval_to_minutes(interval) {
                Some(v) => v,
                None => continue,
            };
            per_interval.insert(interval.to_string(), aggregate(&merged_1m, n));
        }

        self.data
            .entry(code.to_string())
            .or_default()
            .insert(date.to_string(), per_interval);

        if let Some(dates) = self.data.get_mut(code) {
            while dates.len() > 2 {
                if let Some(oldest) = dates.keys().next().cloned() {
                    dates.remove(&oldest);
                }
            }
        }

        tracing::info!(
            "[CandleStore] {} {} 병합 로드: 1m={}건, 5m={}건, 60m={}건",
            code,
            date,
            self.count(code, date, "1m"),
            self.count(code, date, "5m"),
            self.count(code, date, "60m"),
        );
    }

    /// 해당 종목에 저장된 거래일을 오래된 순서로 반환합니다.
    pub fn dates(&self, code: &str) -> Vec<String> {
        self.data
            .get(code)
            .map(|dates| dates.keys().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get(&self, code: &str, date: &str, interval: &str) -> Vec<Candle> {
        self.data
            .get(code)
            .and_then(|dates| dates.get(date))
            .and_then(|intervals| intervals.get(interval))
            .cloned()
            .unwrap_or_default()
    }

    pub fn count(&self, code: &str, date: &str, interval: &str) -> usize {
        self.data
            .get(code)
            .and_then(|dates| dates.get(date))
            .and_then(|intervals| intervals.get(interval))
            .map(|v| v.len())
            .unwrap_or(0)
    }

    /// WebSocket 체결 수신 → 모든 interval 봉 실시간 갱신
    /// - date: "YYYYMMDD" (예: "20261114")
    /// - hhmmss: "HHMMSS" (예: "093015")
    pub fn on_tick(&mut self, code: &str, date: &str, hhmmss: &str, price: f64, qty: u64) {
        if code.trim().is_empty()
            || !is_valid_tick_date(date)
            || !is_valid_tick_time(hhmmss)
            || !price.is_finite()
            || price <= 0.0
        {
            return;
        }
        let hhmm = &hhmmss[0..4]; // "HHMM"
        let hh: u32 = hhmm[0..2].parse().unwrap_or(0);
        let mm: u32 = hhmm[2..4].parse().unwrap_or(0);
        let total_min = hh * 60 + mm;

        for &interval in INTERVALS {
            let n = match interval_to_minutes(interval) {
                Some(v) => v,
                None => continue,
            };
            let group_key = total_min / n;

            let per_date = self
                .data
                .entry(code.to_string())
                .or_default()
                .entry(date.to_string())
                .or_default();

            let candles = per_date.entry(interval.to_string()).or_default();

            // 마지막 봉이 같은 그룹이면 갱신, 아니면 새 봉 시작
            let need_new = candles
                .last()
                .map(|c| parse_hhmm_to_minutes(&c.time) / n != group_key)
                .unwrap_or(true);

            if need_new {
                candles.push(Candle {
                    time: hhmm.to_string(),
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume: qty,
                });
            } else if let Some(last) = candles.last_mut() {
                if price > last.high {
                    last.high = price;
                }
                if price < last.low {
                    last.low = price;
                }
                last.close = price;
                last.volume += qty;
            }
        }

        // 2일치 초과 시 가장 오래된 날짜 제거
        if let Some(dates) = self.data.get_mut(code) {
            while dates.len() > 2 {
                if let Some(oldest) = dates.keys().next().cloned() {
                    dates.remove(&oldest);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candle(time: &str, price: f64, volume: u64) -> Candle {
        Candle {
            time: time.to_string(),
            open: price,
            high: price,
            low: price,
            close: price,
            volume,
        }
    }

    #[test]
    fn dates_returns_only_the_latest_two_dates_in_order() {
        let mut store = CandleStore::new();
        store.load_1m("101S", "20261112", vec![candle("0900", 100.0, 1)]);
        store.load_1m("101S", "20261113", vec![candle("0900", 101.0, 1)]);
        store.load_1m("101S", "20261114", vec![candle("0900", 102.0, 1)]);

        assert_eq!(
            store.dates("101S"),
            vec!["20261113".to_string(), "20261114".to_string()]
        );
        assert!(store.get("101S", "20261112", "1m").is_empty());
        assert_eq!(store.count("101S", "20261113", "1m"), 1);
        assert_eq!(store.count("101S", "20261114", "1m"), 1);
    }

    #[test]
    fn load_1m_builds_supported_intervals() {
        let mut store = CandleStore::new();
        store.load_1m(
            "101S",
            "20261114",
            vec![
                candle("0900", 100.0, 2),
                candle("0901", 101.0, 3),
                candle("0902", 102.0, 4),
            ],
        );

        assert_eq!(store.count("101S", "20261114", "1m"), 3);
        assert_eq!(store.count("101S", "20261114", "2m"), 2);
        assert_eq!(store.count("101S", "20261114", "5m"), 1);
        assert!(store.get("101S", "20261114", "15m").is_empty());
    }

    #[test]
    fn dates_are_independent_per_symbol() {
        let mut store = CandleStore::new();
        store.load_1m("101S", "20261113", vec![candle("0900", 100.0, 1)]);
        store.load_1m("201S", "20261114", vec![candle("0900", 200.0, 1)]);

        assert_eq!(store.dates("101S"), vec!["20261113".to_string()]);
        assert_eq!(store.dates("201S"), vec!["20261114".to_string()]);
        assert!(store.dates("missing").is_empty());
    }

    #[test]
    fn on_tick_updates_ohlcv_within_the_same_interval_bucket() {
        let mut store = CandleStore::new();

        store.on_tick("101S", "20261116", "090001", 100.0, 2);
        store.on_tick("101S", "20261116", "090259", 103.0, 3);
        store.on_tick("101S", "20261116", "090401", 98.0, 4);

        let five_minute = store.get("101S", "20261116", "5m");
        assert_eq!(five_minute.len(), 1);
        assert_eq!(five_minute[0].time, "0900");
        assert_eq!(five_minute[0].open, 100.0);
        assert_eq!(five_minute[0].high, 103.0);
        assert_eq!(five_minute[0].low, 98.0);
        assert_eq!(five_minute[0].close, 98.0);
        assert_eq!(five_minute[0].volume, 9);

        let one_minute = store.get("101S", "20261116", "1m");
        assert_eq!(one_minute.len(), 3);
    }

    #[test]
    fn on_tick_starts_a_new_candle_at_the_next_interval_boundary() {
        let mut store = CandleStore::new();

        store.on_tick("101S", "20261116", "090459", 100.0, 1);
        store.on_tick("101S", "20261116", "090500", 105.0, 2);

        let five_minute = store.get("101S", "20261116", "5m");
        assert_eq!(five_minute.len(), 2);
        assert_eq!(five_minute[0].time, "0904");
        assert_eq!(five_minute[0].close, 100.0);
        assert_eq!(five_minute[1].time, "0905");
        assert_eq!(five_minute[1].open, 105.0);
        assert_eq!(five_minute[1].volume, 2);
    }

    #[test]
    fn on_tick_ignores_invalid_price_time_date_or_code() {
        let mut store = CandleStore::new();

        store.on_tick("101S", "20261116", "090", 100.0, 1);
        store.on_tick("101S", "20261116", "090001", 0.0, 1);
        store.on_tick("101S", "20261116", "090001", -1.0, 1);
        store.on_tick("101S", "20261116", "090001", f64::NAN, 1);
        store.on_tick("101S", "20261116", "090001", f64::INFINITY, 1);
        store.on_tick("101S", "20261116", "246000", 100.0, 1);
        store.on_tick("101S", "20261116", "096060", 100.0, 1);
        store.on_tick("101S", "20261116", "09AB01", 100.0, 1);
        store.on_tick("101S", "20261116", "0900010", 100.0, 1);
        store.on_tick("1011", "2026111", "090001", 100.0, 1);
        store.on_tick("", "20261116", "090001", 100.0, 1);

        assert!(store.dates("101S").is_empty());
        assert!(store.dates("1011").is_empty());
        assert!(store.dates("").is_empty());
    }
}
