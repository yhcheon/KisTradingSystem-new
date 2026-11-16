// ============================================================
// 파일명: server/handlers/market/price_fetch.rs
// 설명: Redis 캐시 확인 및 KIS 선물 현재가 조회
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;

/// Redis 캐시를 우선 확인하고, 없으면 KIS에서 현재가를 조회합니다.
pub(super) async fn fetch_one(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;
    if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
        return Ok(cached);
    }
    let price = kis::get_futures_price(
        &state.http_client,
        &state.kis.app_key,
        &state.kis.app_secret,
        &state.kis.access_token,
        &symbol.code,
    )
    .await
    .map_err(|e| format!("KIS 호출 실패({}): {}", symbol.code, e))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
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
    let _ = redis_cache::set(&mut conn, &cached, 3).await;
    Ok(cached)
}
