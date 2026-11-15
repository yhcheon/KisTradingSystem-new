// ============================================================
// 파일명: server/handlers/market/prices.rs
// 설명: 상태 확인 및 선물 현재가 API
// ============================================================

use super::price_fetch::fetch_one;
use crate::core::state::AppState;

use axum::{Json, extract::State};
use serde_json::{Value, json};

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;
    Json(json!({
        "regular": match regular { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
        "mini": match mini { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
    }))
}
