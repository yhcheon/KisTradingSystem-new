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
    require_content_access(&headers, &state).await?;

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


/// GET /api/notes/:id — 상세 본문은 반드시 백엔드 인증/등급 검사를 거칩니다.
pub async fn note_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    require_content_access(&headers, &state).await?;

    if !is_uuid(&id) {
        return Err((axum::http::StatusCode::BAD_REQUEST, "잘못된 글 ID입니다".to_string()));
    }

    let query = format!("notes?select=*&id=eq.{}", id);
    let mut rows = state.supabase.select(&query).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let note = rows.as_array_mut()
        .and_then(|items| if items.is_empty() { None } else { Some(items.remove(0)) })
        .ok_or((axum::http::StatusCode::NOT_FOUND, "글을 찾을 수 없습니다".to_string()))?;

    Ok(Json(json!({ "ok": true, "data": note })))
}

/// 목록/상세 API가 동일한 로그인 및 무료 회원 시간 제한을 적용합니다.
async fn require_content_access(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), (axum::http::StatusCode, String)> {
    let claims = require_auth(headers, state).await?;
    let tier_query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let tier_resp = state.supabase.select(&tier_query).await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tier = tier_resp.get(0).and_then(|v| v.get("tier"))
        .and_then(|v| v.as_str()).unwrap_or("free");

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
    Ok(())
}

fn is_uuid(value: &str) -> bool {
    value.len() == 36 && value.bytes().enumerate().all(|(index, byte)| {
        match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        }
    })
}

#[cfg(test)]
mod detail_tests {
    use super::is_uuid;

    #[test]
    fn accepts_uuid_format() {
        assert!(is_uuid("550e8400-e29b-41d4-a716-446655440000"));
    }

    #[test]
    fn rejects_invalid_note_ids() {
        assert!(!is_uuid("not-a-uuid"));
        assert!(!is_uuid("550e8400e29b41d4a716446655440000"));
        assert!(!is_uuid("550e8400-e29b-41d4-a716-44665544000Z"));
    }
}
