// ============================================================
// 파일명: server/handlers/market/price_fetch.rs
// 설명: 캐시 확인 및 KIS 선물 현재가 조회
//       - 캐시에 있으면 바로 반환 (TTL 없음)
//       - 캐시에 없으면 KIS에서 1회만 조회 후 캐시에 저장
//       - 이후 현재가는 WebSocket 체결로만 갱신 (update_by_tick)
// ============================================================

use crate::cache::price_cache::CachedPrice;
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;

pub(super) async fn fetch_one(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> Result<CachedPrice, String> {
    // 1. 메모리 캐시 확인 (TTL 없음 - 부팅 시 1회 저장된 값을 계속 사용)
    if let Some(cached) = state.price_cache.get(&symbol.code).await {
        return Ok(cached);
    }

    // 2. 캐시 없으면 KIS에서 1회 조회
    let cached = fetch_from_kis(state, symbol).await?;

    // 3. 메모리 캐시에 저장 (다음부터는 체결로만 갱신)
    state.price_cache.set(&cached).await;
    Ok(cached)
}

// KIS API를 호출하고 응답을 CachedPrice로 변환하는 것만 담당합니다.
// 캐시 로직은 fetch_one이 담당합니다.
async fn fetch_from_kis(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> Result<CachedPrice, String> {
    let price = kis::get_futures_price(
        &state.http_client,
        &state.kis.app_key,
        &state.kis.app_secret,
        &state.kis.access_token,
        &symbol.code,
    )
    .await
    .map_err(|error| error.to_string())?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    Ok(CachedPrice {
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
    })
}

