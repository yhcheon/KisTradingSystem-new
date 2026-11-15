// ============================================================
// 파일명: core/candles/store.rs
// 설명: 종목별·날짜별 캔들 메모리 저장 및 실시간 갱신
// ============================================================

use super::{
    aggregation::{aggregate, parse_hhmm_to_minutes},
    types::{Candle, INTERVALS, interval_to_minutes},
};
use std::collections::{BTreeMap, HashMap};

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
        let mut per_interval: HashMap<String, Vec<Candle>> = HashMap::new();
        per_interval.insert("1m".to_string(), candles_1m.clone());

        for &interval in INTERVALS {
            if interval == "1m" {
                continue;
            }
            let n = match interval_to_minutes(interval) {
                Some(v) => v,
                None => continue,
            };
            per_interval.insert(interval.to_string(), aggregate(&candles_1m, n));
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
            "[CandleStore] {} {} 로드: 1m={}건, 5m={}건, 60m={}건",
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
        if hhmmss.len() < 4 || price <= 0.0 {
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
