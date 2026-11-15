// ============================================================
// 파일명: server/handlers/content/notes.rs
// 설명: 일반 회원용 콘텐츠 목록 API
// ============================================================

use super::auth::require_auth;
use crate::core::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::Timelike;
use serde_json::{Value, json};
use std::collections::HashMap;

/// GET /api/notes
pub async fn notes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;

    // 등급 확인 + 무료 회원 14시 제한
    let tier_query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let tier_resp = state
        .supabase
        .select(&tier_query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tier = tier_resp
        .get(0)
        .and_then(|v| v.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("free");

    if tier == "free" {
        let kst_hour = chrono::Utc::now()
            .with_timezone(&chrono_tz::Asia::Seoul)
            .hour();
        if kst_hour < 14 {
            return Err((
                axum::http::StatusCode::FORBIDDEN,
                "무료 회원은 오후 2시부터 열람 가능합니다".to_string(),
            ));
        }
    }

    // 글 조회
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("");
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
