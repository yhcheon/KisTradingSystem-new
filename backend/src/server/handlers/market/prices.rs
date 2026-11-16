// ============================================================
// 파일명: server/handlers/market/prices.rs
// 설명: 상태 확인 및 주간/야간 선물 현재가 API
// ============================================================

use super::price_fetch::fetch_one;
use crate::core::state::AppState;
use crate::kis;

use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;

    // KRX 야간선물은 정규 KOSPI200 최근월물 코드를 사용합니다.
    // 야간 기준가는 당일 주간 정규장의 마지막 현재가(종가)를 사용합니다.
    let day_close = regular.as_ref().ok().map(|price| price.price.clone());
    let night = match kis::get_futures_price_for_market(
        &state.http_client,
        &state.kis.app_key,
        &state.kis.app_secret,
        &state.kis.access_token,
        "CM",
        &state.regular.code,
    )
    .await
    {
        Ok(price) => json!({
            "ok": true,
            "data": {
                "code": state.regular.code,
                "name": price.output1.hts_kor_isnm,
                "price": price.output1.futs_prpr,
                "reference_price": day_close,
                "open": price.output1.futs_oprc,
                "high": price.output1.futs_hgpr,
                "low": price.output1.futs_lwpr,
                "change": price.output1.futs_prdy_vrss,
                "change_rate": price.output1.futs_prdy_ctrt,
                "volume": price.output1.acml_vol
            }
        }),
        Err(error) => json!({ "ok": false, "error": error.to_string() }),
    };

    Json(json!({
        "regular": match regular {
            Ok(price) => json!({ "ok": true, "data": price, "night": night }),
            Err(error) => json!({ "ok": false, "error": error }),
        },
        "mini": match mini {
            Ok(price) => json!({ "ok": true, "data": price }),
            Err(error) => json!({ "ok": false, "error": error }),
        },
    }))
}
