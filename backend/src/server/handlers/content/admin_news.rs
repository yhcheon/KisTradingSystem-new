// ============================================================
// 파일명: server/handlers/content/admin_news.rs
// 설명: 관리자 뉴스 링크 조회, 등록, 삭제 API
// ============================================================

use super::{
    access::is_uuid,
    auth::{require_admin, require_auth},
};
use crate::core::state::AppState;
use axum::{Json, extract::State, http::{HeaderMap, StatusCode}};
use serde_json::{Value, json};

/// GET /api/admin/news
pub async fn admin_news(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let data = state
        .supabase
        .select("news_links?select=id,title,url,source,published_at,created_at&order=created_at.desc&limit=200")
        .await
        .map_err(|error| {
            tracing::error!(target: "admin", error = %error, "관리자 뉴스 목록 조회 실패");
            (StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;

    Ok(Json(json!({ "ok": true, "data": data })))
}

/// POST /api/admin/news
pub async fn create_news_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let title = body.get("title").and_then(Value::as_str).unwrap_or("").trim();
    let url = body.get("url").and_then(Value::as_str).unwrap_or("").trim();
    let source = body.get("source").and_then(Value::as_str).unwrap_or("").trim();
    let published_at = body.get("published_at").and_then(Value::as_str).filter(|value| !value.trim().is_empty());

    let parsed_url = reqwest::Url::parse(url).map_err(|_| {
        (StatusCode::BAD_REQUEST, "올바른 뉴스 URL이 필요합니다".to_string())
    })?;
    if title.is_empty() || !matches!(parsed_url.scheme(), "http" | "https") || parsed_url.host_str().is_none() {
        return Err((StatusCode::BAD_REQUEST, "제목과 http(s) 뉴스 URL이 필요합니다".to_string()));
    }

    let link = json!({
        "title": title,
        "url": parsed_url.as_str(),
        "source": if source.is_empty() { Value::Null } else { json!(source) },
        "published_at": published_at,
    });

    state.supabase.insert("news_links", &link).await.map_err(|error| {
        tracing::error!(target: "admin", error = %error, "뉴스 링크 등록 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;

    Ok(Json(json!({ "ok": true })))
}

/// DELETE /api/admin/news
pub async fn delete_news_link(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let id = body.get("id").and_then(Value::as_str).unwrap_or("");
    if !is_uuid(id) {
        return Err((StatusCode::BAD_REQUEST, "올바른 뉴스 ID가 필요합니다".to_string()));
    }

    state.supabase.delete_by_id("news_links", id).await.map_err(|error| {
        tracing::error!(target: "admin", news_id = %id, error = %error, "뉴스 링크 삭제 실패");
        (StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;

    Ok(Json(json!({ "ok": true })))
}
