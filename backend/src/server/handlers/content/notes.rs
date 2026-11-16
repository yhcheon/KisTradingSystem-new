// ============================================================
// 파일명: server/handlers/content/notes.rs
// 설명: 일반 회원용 콘텐츠 목록 API
// ============================================================

use super::access::{is_note_category, require_content_access};
use crate::core::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use serde_json::{Value, json};
use std::collections::HashMap;

/// GET /api/notes
pub async fn notes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    require_content_access(&headers, &state).await?;

    let category = params.get("category").map(|s| s.as_str()).unwrap_or("");
    if !category.is_empty() && !is_note_category(category) {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "올바른 카테고리가 필요합니다".to_string(),
        ));
    }

    let query = if category.is_empty() {
        "notes?select=id,title,category,created_at&order=created_at.desc&limit=20".to_string()
    } else {
        format!(
            "notes?select=id,title,category,created_at&category=eq.{}&order=created_at.desc&limit=20",
            category
        )
    };

    let data = state
        .supabase
        .select(&query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(json!({ "ok": true, "data": data })))
}
