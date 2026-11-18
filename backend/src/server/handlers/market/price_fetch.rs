// ============================================================
// 파일명: server/handlers/market/price_fetch.rs
// 설명: Redis 캐시 확인 및 KIS 선물 현재가 조회
//       - 캐시 미스 또는 KIS 일시 오류 시 제한된 재시도
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;
use std::time::Duration;

/// Redis 캐시를 우선 확인하고, 없거나 KIS가 일시 실패하면 최대 3회 조회합니다.
pub(super) async fn fetch_one(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> Result<CachedPrice, String> {
    {
        let mut conn = state.redis.lock().await;
        if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
            return Ok(cached);
        }
    }

    let mut last_error = String::new();
    for attempt in 1..=3 {
        match kis::get_futures_price(
            &state.http_client,
            &state.kis.app_key,
            &state.kis.app_secret,
            &state.kis.access_token,
            &symbol.code,
        )
        .await
        {
            Ok(price) => {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let cached = CachedPrice {
                    code: symbol.code.clone(),
                    name: price.output1.hts_kor_isnm.clone(),
                    price: price.output1.futs_prpr.clone(),
                    prev_close: price.output1.futs_prdy_clpr.clone(),
                    open: price.output1.futs_oprc.clone(),
                    high: price.output1.futs_hgpr.clone(),
                    low: price.output1.futs_lwpr.clone(),
                    change: price.output1.futs_prdy_vrss.clone(),
                    change_rate: price.output1.futs_prdy_ctrt.clone(),
                    volume: price.output1.acml_vol.clone(),
                    cached_at: now,
                };

                let mut conn = state.redis.lock().await;
                if let Err(error) = redis_cache::set(&mut conn, &cached, 3).await {
                    tracing::warn!("Redis 시세 캐시 저장 실패({}): {}", symbol.code, error);
                }
                return Ok(cached);
            }
            Err(error) => {
                let error_message = error.to_string();
                drop(error);
                last_error = format!("KIS 호출 실패({}): {}", symbol.code, error_message);
                tracing::warn!(
                    "현재가 조회 실패: code={}, attempt={}/3, error={}",
                    symbol.code,
                    attempt,
                    error_message
                );
                if attempt < 3 {
                    tokio::time::sleep(Duration::from_millis(400 * attempt as u64)).await;
                }
            }
        }
    }

    Err(last_error)
}
