// Redis 없이 프로세스 메모리에 주간/야간 가격을 독립 보관합니다.
use crate::kis::ws::RealtimeTick;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedPrice {
    pub code: String,
    pub name: String,
    pub price: String,
    pub prev_close: String,
    pub open: String,
    pub high: String,
    pub low: String,
    pub change: String,
    pub change_rate: String,
    pub volume: String,
    pub cached_at: u64,
}

#[derive(Clone, Default)]
pub struct PriceCache {
    // 종목코드만 키로 사용하던 구조를 (종목코드, 세션)으로 분리합니다.
    inner: Arc<Mutex<HashMap<(String, String), (CachedPrice, Instant)>>>,
}

impl PriceCache {
    pub fn new() -> Self { Self::default() }

    pub async fn get(&self, code: &str, session: &str) -> Option<CachedPrice> {
        let map = self.inner.lock().await;
        let (price, _) = map.get(&(code.to_string(), session.to_string()))?;
        Some(price.clone())
    }

    pub async fn set(&self, price: &CachedPrice, session: &str) {
        let mut map = self.inner.lock().await;
        map.insert((price.code.clone(), session.to_string()), (price.clone(), Instant::now()));
    }

    pub async fn clear_session(&self, session: &str) {
        let mut map = self.inner.lock().await;
        map.retain(|(_, cached_session), _| cached_session != session);
    }

    pub async fn update_by_tick(&self, tick: &RealtimeTick) {
        let mut map = self.inner.lock().await;
        let key = (tick.code.clone(), tick.market_session.clone());
        if let Some((cached, _)) = map.get_mut(&key) {
            cached.price = tick.price.clone();
            cached.volume = tick.volume.clone();
            cached.change = tick.change.clone();
            cached.change_rate = tick.change_rate.clone();

            let price = tick.price.parse::<f64>().unwrap_or(0.0);
            let high = cached.high.parse::<f64>().unwrap_or(0.0);
            let low = cached.low.parse::<f64>().unwrap_or(0.0);
            if price > high { cached.high = tick.price.clone(); }
            if low == 0.0 || price < low { cached.low = tick.price.clone(); }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kis::ws::RealtimeTick;

    fn tick(session: &str, price: &str) -> RealtimeTick {
        RealtimeTick {
            code: "101W9000".into(), date: "20261119".into(),
            price: price.into(), change: "1".into(), change_rate: "0.1".into(),
            volume: "10".into(), last_qty: "1".into(), time: "100000".into(),
            market_session: session.into(), range_alert: None,
        }
    }

    fn quote(price: &str) -> CachedPrice {
        CachedPrice {
            code: "101W9000".into(), name: "선물".into(), price: price.into(),
            prev_close: "100".into(), open: "100".into(), high: "100".into(),
            low: "100".into(), change: "0".into(), change_rate: "0".into(),
            volume: "0".into(), cached_at: 0,
        }
    }

    #[tokio::test]
    async fn day_and_night_prices_are_isolated_for_same_symbol() {
        let cache = PriceCache::new();
        cache.set(&quote("101"), "day").await;
        cache.set(&quote("102"), "night").await;
        cache.update_by_tick(&tick("night", "103")).await;
        assert_eq!(cache.get("101W9000", "day").await.unwrap().price, "101");
        assert_eq!(cache.get("101W9000", "night").await.unwrap().price, "103");
    }
}
