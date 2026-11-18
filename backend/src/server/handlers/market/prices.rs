// ============================================================
// 파일명: server/handlers/market/prices.rs
// 설명: 상태 확인 및 주간/야간 선물 현재가 API
//       - 정규/미니 시세를 병렬 조회해 초기 스냅샷 대기시간 단축
// ============================================================

use super::price_fetch::fetch_one;
use crate::core::state::AppState;
use crate::kis;

use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    Json(build_prices(&state).await)
}

/// HTTP API와 WebSocket 초기 스냅샷이 동일한 시세 구조를 사용하도록 공유합니다.
pub(super) async fn build_prices(state: &AppState) -> Value {
    let (regular, mini, night_result, mini_night_result) = tokio::join!(
        fetch_one(state, &state.regular),
        fetch_one(state, &state.mini),
        kis::get_futures_price_for_market(
            &state.http_client,
            &state.kis.app_key,
            &state.kis.app_secret,
            &state.kis.access_token,
            "CM",
            &state.regular.code,
        ),
        kis::get_futures_price_for_market(
            &state.http_client,
            &state.kis.app_key,
            &state.kis.app_secret,
            &state.kis.access_token,
            "CM",
            &state.mini.code,
        )
    );

    // 각 야간 기준가는 해당 상품의 당일 주간 마지막 가격을 사용합니다.
    let day_close = regular.as_ref().ok().map(|price| price.price.clone());
    let mini_day_close = mini.as_ref().ok().map(|price| price.price.clone());

    let night = match night_result {
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

    let mini_night = match mini_night_result {
        Ok(price) => json!({
            "ok": true,
            "data": {
                "code": state.mini.code,
                "name": price.output1.hts_kor_isnm,
                "price": price.output1.futs_prpr,
                "reference_price": mini_day_close,
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

    json!({
        "regular": match regular {
            Ok(price) => json!({ "ok": true, "data": price, "night": night }),
            Err(error) => json!({ "ok": false, "error": error, "night": night }),
        },
        "mini": match mini {
            Ok(price) => json!({ "ok": true, "data": price, "night": mini_night }),
            Err(error) => json!({ "ok": false, "error": error, "night": mini_night }),
        },
    })
}
