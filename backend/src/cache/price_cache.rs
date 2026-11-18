// 파일명: cache/price_cache.rs
// 설명: Redis 없는 메모리 시세 캐시 (TTL 없음 - 부팅 1회 + 체결로 갱신)

use crate::kis_ws::RealtimeTick;
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
    inner: Arc<Mutex<HashMap<String, (CachedPrice, Instant)>>>,
}

impl PriceCache {
    pub fn new() -> Self {
        Self::default()
    }

    // TTL 체크 없음. 있으면 무조건 반환, 없으면 None
    pub async fn get(&self, code: &str) -> Option<CachedPrice> {
        let map = self.inner.lock().await;
        let (price, _at) = map.get(code)?;
        Some(price.clone())
    }

    pub async fn set(&self, price: &CachedPrice) {
        let mut map = self.inner.lock().await;
        map.insert(price.code.clone(), (price.clone(), Instant::now()));
    }

    // 체결이 들어올 때마다 현재가/거래량/고가/저가만 갱신
    pub async fn update_by_tick(&self, tick: &RealtimeTick) {
        let mut map = self.inner.lock().await;
        // map 값은 (CachedPrice, Instant) 튜플이라서 .0으로 접근해야 해요
        if let Some((cached, _at)) = map.get_mut(&tick.code) {
            cached.price = tick.price.clone();
            cached.volume = tick.volume.clone();
            cached.change = tick.change.clone();
            cached.change_rate = tick.change_rate.clone();

            // 고가/저가는 체결 가격으로 갱신
            let price: f64 = tick.price.parse().unwrap_or(0.0);
            let high: f64 = cached.high.parse().unwrap_or(0.0);
            let low: f64 = cached.low.parse().unwrap_or(0.0);
            if price > high {
                cached.high = tick.price.clone();
            }
            if low == 0.0 || price < low {
                cached.low = tick.price.clone();
            }
        }
    }
}

