// ============================================================
// 파일명: server/handlers/content/admin.rs
// 설명: 관리자 콘텐츠 CRUD API
// ============================================================

use super::auth::{require_admin, require_auth};
use crate::core::state::AppState;
use axum::{Json, extract::State, http::HeaderMap};
use serde_json::{Value, json};

/// POST /api/admin/notes (관리자만)
pub async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    tracing::info!(target: "admin", "관리자 글 저장 요청 수신");
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let category = body
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("us_market");

    if title.is_empty() || content.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "제목과 본문 필요".to_string(),
        ));
    }

    let note = json!({
        "title": title,
        "content": content,
        "category": category,
        "author_email": claims.email.unwrap_or_default(),
    });

    state
        .supabase
        .insert("notes", &note)
        .await
        .map_err(|error| {
            tracing::error!(
                target: "admin",
                category = %category,
                error = %error,
                "관리자 글 저장 실패: Supabase INSERT 오류"
            );
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;

    tracing::info!(target: "admin", category = %category, "관리자 글 저장 성공");
    Ok(Json(json!({ "ok": true })))
}

/// GET /api/admin/notes (관리자 글 목록: 본문 포함)
pub async fn admin_notes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;
    let data = state
        .supabase
        .select("notes?select=id,title,content,category,created_at&order=created_at.desc&limit=200")
        .await
        .map_err(|error| {
            tracing::error!(target: "admin", error = %error, "관리자 글 목록 조회 실패");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;
    tracing::info!(target: "admin", count = data.as_array().map_or(0, |v| v.len()), "관리자 글 목록 조회 성공");
    Ok(Json(json!({ "ok": true, "data": data })))
}

/// PUT /api/admin/notes (관리자 글 수정)
pub async fn update_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let category = body
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("us_market");
    if id.is_empty() || title.trim().is_empty() || content.trim().is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "글 ID, 제목과 본문이 필요합니다".to_string(),
        ));
    }

    let update = json!({ "title": title, "content": content, "category": category });
    state
        .supabase
        .update_by_id("notes", id, &update)
        .await
        .map_err(|error| {
            tracing::error!(target: "admin", note_id = %id, error = %error, "관리자 글 수정 실패");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;
    tracing::info!(target: "admin", note_id = %id, "관리자 글 수정 성공");
    Ok(Json(json!({ "ok": true })))
}

/// DELETE /api/admin/notes (관리자 글 삭제)
pub async fn delete_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if id.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "글 ID가 필요합니다".to_string(),
        ));
    }

    state
        .supabase
        .delete_by_id("notes", id)
        .await
        .map_err(|error| {
            tracing::error!(target: "admin", note_id = %id, error = %error, "관리자 글 삭제 실패");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;
    tracing::info!(target: "admin", note_id = %id, "관리자 글 삭제 성공");
    Ok(Json(json!({ "ok": true })))
}
