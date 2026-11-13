
// ============================================================
// 파일명: server/handlers.rs
// 설명: HTTP 핸들러
//       - GET /api/health  : 서버 상태
//       - GET /api/prices  : 정규선물 + 미니선물 시세 (캐시 우선)
// 작성일: 2026-11-14
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;
use axum::{extract::State, Json};
use serde_json::{json, Value};

/// GET /api/health
pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({
        "status": "ok",
        "regular": state.regular.name,
        "mini": state.mini.name,
    }))
}

/// 시세 1건 조회 (캐시 우선). 실패 시 캐시만 반환.
async fn fetch_one(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;

    // 1) 캐시 확인
    match redis_cache::get(&mut conn, &symbol.code).await {
        Ok(Some(cached)) => return Ok(cached),
        Ok(None) => {}
        Err(e) => tracing::warn!("캐시 조회 실패({}): {}", symbol.code, e),
    }

    // 2) KIS API 호출
    let price = kis::get_futures_price(
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
        change: price.output1.futs_prdy_vrss.clone(),
        change_rate: price.output1.futs_prdy_ctrt.clone(),
        volume: price.output1.acml_vol.clone(),
        cached_at: now,
    };

    // 3) 캐시 저장 (TTL 3초)
    if let Err(e) = redis_cache::set(&mut conn, &cached, 3).await {
        tracing::warn!("캐시 저장 실패({}): {}", symbol.code, e);
    }

    Ok(cached)
}

/// GET /api/prices
pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;

    Json(json!({
        "regular": match regular {
            Ok(p) => json!({ "ok": true, "data": p }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
        "mini": match mini {
            Ok(p) => json!({ "ok": true, "data": p }),
            Err(e) => json!({ "ok": false, "error": e }),
        },
    }))
}


