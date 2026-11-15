// ============================================================
// 파일명: server/handlers/content/news.rs
// 설명: 일반 회원용 뉴스 링크 목록 API
// ============================================================

use super::access::require_content_access;
use crate::core::state::AppState;
use axum::{Json, extract::State, http::HeaderMap};
use serde_json::{Value, json};

/// GET /api/news
pub async fn news(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    require_content_access(&headers, &state).await?;

    let data = state
        .supabase
        .select("news_links?select=id,title,url,source,published_at,created_at&order=published_at.desc.nullslast,created_at.desc&limit=100")
        .await
        .map_err(|error| {
            tracing::error!(target: "news", error = %error, "뉴스 링크 조회 실패");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;

    Ok(Json(json!({ "ok": true, "data": data })))
}
