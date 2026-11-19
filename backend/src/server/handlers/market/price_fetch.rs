// 주간/야간 세션별 메모리 캐시와 KIS REST 초기값 조회.
use crate::cache::price_cache::CachedPrice;
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;

pub(super) async fn fetch_one(
    state: &AppState,
    symbol: &FuturesSymbol,
    session: &str,
) -> Result<CachedPrice, String> {
    if let Some(cached) = state.price_cache.get(&symbol.code, session).await {
        return Ok(cached);
    }

    let market_div = match session {
        "day" => "F",
        "night" => "CM",
        _ => return Err(format!("알 수 없는 시장 세션: {session}")),
    };
    let price = kis::get_futures_price_for_market(
        &state.http_client,
        &state.kis.app_key,
        &state.kis.app_secret,
        &state.kis.access_token,
        market_div,
        &symbol.code,
    )
    .await
    .map_err(|error| error.to_string())?;

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
    state.price_cache.set(&cached, session).await;
    Ok(cached)
}
