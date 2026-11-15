// ============================================================
// 파일명: server/handlers/market/prices.rs
// 설명: 상태 확인 및 선물 현재가 API
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;

use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

async fn fetch_one(state: &AppState, symbol: &FuturesSymbol) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;
    if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
        return Ok(cached);
    }
    let price = kis::get_futures_price(
        &state.http_client,
        &state.app_key,
        &state.app_secret,
        &state.access_token,
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

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;
    Json(json!({
        "regular": match regular { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
        "mini": match mini { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
    }))
}
