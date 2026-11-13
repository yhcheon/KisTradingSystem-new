
// ============================================================
// 파일명: core/candles.rs
// 설명: 1분봉 메모리 저장소 + WebSocket 체결 조립
//       - BTreeMap<날짜, BTreeMap<HHMM, Candle>> 구조
//       - 체결 수신 시 현재 분봉에 누적, 분 바뀌면 확정
//       - 2일치 초과 시 가장 오래된 날짜 제거
// 작성일: 2026-11-14
// ============================================================

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct Candle {
    pub time: String,   // "HHMM"
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

pub struct CandleStore {
    // 종목코드 → 날짜 → HHMM → Candle
    data: BTreeMap<String, BTreeMap<String, BTreeMap<String, Candle>>>,
    // 종목코드 → 현재 분봉 (마지막 체결로 조립 중)
    current: BTreeMap<String, Candle>,
    // 종목코드 → 현재 분봉의 날짜
    current_date: BTreeMap<String, String>,
}

impl CandleStore {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
            current: BTreeMap::new(),
            current_date: BTreeMap::new(),
        }
    }

    /// 과거 1분봉 일괄 로드 (REST 응답). 날짜별로 저장.
    pub fn load_bulk(&mut self, code: &str, candles: Vec<(String, Candle)>) {
        let entry = self.data.entry(code.to_string()).or_default();
        for (date, c) in candles {
            entry.entry(date).or_default().insert(c.time.clone(), c);
        }
        self.prune(code);
    }

    /// WebSocket 체결 수신 → 1분봉 조립
    /// - date: YYYYMMDD
    /// - time_hhmmss: HHMMSS (예: "093015")
    pub fn on_tick(&mut self, code: &str, date: &str, time_hhmmss: &str, price: f64, qty: u64) {
        if time_hhmmss.len() < 4 {
            return;
        }
        let hhmm = time_hhmmss[0..4].to_string(); // "HHMM"

        let current_date = self.current_date.get(code).cloned().unwrap_or_default();
        let current = self.current.get(code).cloned();

        match current {
            Some(c) if current_date == date && c.time == hhmm => {
                // 같은 분봉: high/low/close/volume 갱신
                let mut updated = c;
                if price > updated.high { updated.high = price; }
                if price < updated.low { updated.low = price; }
                updated.close = price;
                updated.volume += qty;
                self.current.insert(code.to_string(), updated);
            }
            _ => {
                // 새 분봉 시작 (이전 분봉은 확정)
                if let Some(c) = current {
                    if current_date == date || !current_date.is_empty() {
                        // 이전 분봉을 data에 확정 저장
                        let prev_date = self.current_date.get(code).cloned().unwrap_or_default();
                        if !prev_date.is_empty() {
                            self.data
                                .entry(code.to_string())
                                .or_default()
                                .entry(prev_date)
                                .or_default()
                                .insert(c.time.clone(), c);
                        }
                    }
                }

                let new_candle = Candle {
                    time: hhmm.clone(),
                    open: price,
                    high: price,
                    low: price,
                    close: price,
                    volume: qty,
                };
                self.current.insert(code.to_string(), new_candle);
                self.current_date.insert(code.to_string(), date.to_string());
            }
        }
    }

    /// 1분봉 조회. include_current=true면 현재 조립 중인 분봉도 포함.
    pub fn get_1m(&self, code: &str, include_current: bool) -> Vec<(String, Candle)> {
        let mut out = Vec::new();
        if let Some(dates) = self.data.get(code) {
            for (date, candles) in dates {
                for (hhmm, c) in candles {
                    out.push((date.clone(), c.clone()));
                    let _ = hhmm;
                }
            }
        }
        if include_current {
            if let (Some(c), Some(d)) = (self.current.get(code), self.current_date.get(code)) {
                out.push((d.clone(), c.clone()));
            }
        }
        out
    }

    /// 2일치 초과 시 가장 오래된 날짜 제거
    fn prune(&mut self, code: &str) {
        if let Some(dates) = self.data.get_mut(code) {
            while dates.len() > 2 {
                if let Some(oldest) = dates.keys().next().cloned() {
                    dates.remove(&oldest);
                }
            }
        }
    }
}

/// 1분봉을 N분봉으로 조합 (2분, 5분)
pub fn aggregate(candles: &[(String, Candle)], n: usize) -> Vec<(String, Candle)> {
    let mut out: Vec<(String, Candle)> = Vec::new();
    for (i, (date, c)) in candles.iter().enumerate() {
        if i % n == 0 {
            out.push((date.clone(), Candle {
                time: c.time.clone(),
                open: c.open,
                high: c.high,
                low: c.low,
                close: c.close,
                volume: c.volume,
            }));
        } else if let Some(last) = out.last_mut() {
            let agg = &mut last.1;
            if c.high > agg.high { agg.high = c.high; }
            if c.low < agg.low { agg.low = c.low; }
            agg.close = c.close;
            agg.volume += c.volume;
        }
    }
    out
}

