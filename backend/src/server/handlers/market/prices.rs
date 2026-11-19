// 주간/야간 가격은 각각 독립 캐시에서 읽어 프론트에 함께 전달합니다.
use super::price_fetch::fetch_one;
use crate::cache::price_cache::CachedPrice;
use crate::core::state::AppState;
use crate::master::FuturesSymbol;
use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    Json(build_prices(&state).await)
}

fn night_result(
    result: Result<CachedPrice, String>,
    reference_price: Option<String>,
) -> Value {
    match result {
        Ok(price) => json!({
            "ok": true,
            "data": {
                "code": price.code,
                "name": price.name,
                "price": price.price,
                "reference_price": reference_price.unwrap_or(price.prev_close),
                "open": price.open,
                "high": price.high,
                "low": price.low,
                "change": price.change,
                "change_rate": price.change_rate,
                "volume": price.volume
            }
        }),
        Err(error) => json!({ "ok": false, "error": error }),
    }
}

async fn fetch_pair(
    state: &AppState,
    symbol: &FuturesSymbol,
) -> (Result<CachedPrice, String>, Result<CachedPrice, String>) {
    tokio::join!(
        fetch_one(state, symbol, "day"),
        fetch_one(state, symbol, "night"),
    )
}

pub(super) async fn build_prices(state: &AppState) -> Value {
    let (regular_pair, mini_pair) = tokio::join!(
        fetch_pair(state, &state.regular),
        fetch_pair(state, &state.mini),
    );
    let (regular_day, regular_night) = regular_pair;
    let (mini_day, mini_night) = mini_pair;

    let regular_reference = regular_day.as_ref().ok().map(|p| p.price.clone());
    let mini_reference = mini_day.as_ref().ok().map(|p| p.price.clone());

    let regular_night_json = night_result(regular_night, regular_reference);
    let mini_night_json = night_result(mini_night, mini_reference);

    json!({
        "regular": match regular_day {
            Ok(price) => json!({ "ok": true, "data": price, "night": regular_night_json }),
            Err(error) => json!({ "ok": false, "error": error, "night": regular_night_json }),
        },
        "mini": match mini_day {
            Ok(price) => json!({ "ok": true, "data": price, "night": mini_night_json }),
            Err(error) => json!({ "ok": false, "error": error, "night": mini_night_json }),
        },
    })
}
