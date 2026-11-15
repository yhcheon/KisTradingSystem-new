// ============================================================
// 파일명: server/handlers/content/detail.rs
// 설명: 분석 글 상세 조회 API
// ============================================================

use super::access::{is_uuid, require_content_access};
use crate::core::state::AppState;
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
};
use serde_json::{Value, json};

/// GET /api/notes/:id — 상세 본문은 반드시 백엔드 인증/등급 검사를 거칩니다.
pub async fn note_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Value>, (StatusCode, String)> {
    require_content_access(&headers, &state).await?;

    if !is_uuid(&id) {
        return Err((StatusCode::BAD_REQUEST, "잘못된 글 ID입니다".to_string()));
    }

    let query = format!("notes?select=*&id=eq.{}", id);
    let mut rows = state
        .supabase
        .select(&query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let note = rows
        .as_array_mut()
        .and_then(|items| {
            if items.is_empty() {
                None
            } else {
                Some(items.remove(0))
            }
        })
        .ok_or((StatusCode::NOT_FOUND, "글을 찾을 수 없습니다".to_string()))?;

    Ok(Json(json!({ "ok": true, "data": note })))
}
