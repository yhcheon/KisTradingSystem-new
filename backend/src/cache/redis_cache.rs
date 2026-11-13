
// ============================================================
// 파일명: cache/redis_cache.rs
// 설명: Redis 시세 캐시
//       - KIS API 호출 결과를 종목코드별로 TTL 3초간 캐시
//       - 동일 종목 반복 조회 시 Redis에서 즉시 반환
//       - KIS REST 호출 제한(초당 20건) 보호
// 작성일: 2026-11-14
// ============================================================

use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

/// 캐시에 저장할 시세 데이터 (프론트엔드 전달용 요약 구조)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CachedPrice {
    pub code: String,          // 종목코드 (예: A01612)
    pub name: String,          // 종목명
    pub price: String,         // 현재가
    pub change: String,        // 전일 대비
    pub change_rate: String,   // 전일 대비율
    pub volume: String,        // 거래량
    pub cached_at: u64,        // 캐시 시각 (Unix timestamp)
}

/// Redis에서 캐시 조회. 없으면 None.
pub async fn get(
    conn: &mut redis::aio::MultiplexedConnection,
    code: &str,
) -> Result<Option<CachedPrice>, Box<dyn std::error::Error>> {
    let key = format!("price:{}", code);
    let raw: Option<String> = conn.get(&key).await?;
    match raw {
        Some(json) => {
            let price: CachedPrice = serde_json::from_str(&json)?;
            Ok(Some(price))
        }
        None => Ok(None),
    }
}

/// Redis에 캐시 저장 (TTL 초 단위)
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


