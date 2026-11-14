// ============================================================
// 파일명: cache/redis_cache.rs
// 설명: Redis 시세 캐시 (전일종가/시가/고가/저가 포함)
// 작성일: 2026-11-14
// ============================================================

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedPrice {
    pub code: String,
    pub name: String,
    pub price: String,       // 현재가
    pub prev_close: String,  // 전일 종가
    pub open: String,        // 시가
    pub high: String,        // 고가
    pub low: String,         // 저가
    pub change: String,      // 전일 대비
    pub change_rate: String, // 전일 대비율
    pub volume: String,      // 누적 거래량
    pub cached_at: u64,
}

pub async fn get(
    conn: &mut redis::aio::MultiplexedConnection,
    code: &str,
) -> Result<Option<CachedPrice>, Box<dyn std::error::Error>> {
    let key = format!("price:{}", code);
    let raw: Option<String> = conn.get(&key).await?;
    match raw {
        Some(json) => Ok(Some(serde_json::from_str(&json)?)),
        None => Ok(None),
    }
}

pub async fn set(
    conn: &mut redis::aio::MultiplexedConnection,
    price: &CachedPrice,
    ttl_secs: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let key = format!("price:{}", price.code);
    let json = serde_json::to_string(price)?;
    let _: () = conn.set_ex(&key, json, ttl_secs).await?;
    Ok(())
}
