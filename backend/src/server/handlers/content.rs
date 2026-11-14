// ============================================================
// 파일명: server/handlers/content.rs
// 설명: JWT 인증, 콘텐츠 조회 및 관리자 콘텐츠 API
// ============================================================

use crate::auth::jwt;
use crate::core::state::AppState;
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use chrono::Timelike;
use serde_json::{Value, json};
use std::collections::HashMap;

// ── 인증 미들웨어 ──

/// Authorization 헤더에서 JWT 추출 + 검증
async fn require_auth(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<jwt::Claims, (axum::http::StatusCode, String)> {
    let auth = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(value) => value,
        None => {
            tracing::warn!(target: "auth", "인증 실패: Authorization 헤더 누락 또는 유효하지 않은 헤더 값");
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "Authorization 헤더 필요".to_string(),
            ));
        }
    };

    let token = match auth.strip_prefix("Bearer ") {
        Some(value) if !value.is_empty() => value,
        _ => {
            tracing::warn!(target: "auth", "인증 실패: Bearer 토큰 형식 오류 또는 토큰 누락");
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "Bearer 토큰 필요".to_string(),
            ));
        }
    };

    jwt::verify_jwt(
        token,
        &state.jwt_secret,
        &state.supabase_url,
        &state.http_client,
    )
    .await
    .map_err(|error| {
        tracing::warn!(target: "auth", status = 401, error = %error, "요청 인증 거부");
        (axum::http::StatusCode::UNAUTHORIZED, error)
    })
}

/// 관리자 여부 확인 (user_profiles.tier = 'admin')
async fn require_admin(
    claims: &jwt::Claims,
    state: &AppState,
) -> Result<(), (axum::http::StatusCode, String)> {
    let query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let resp = state.supabase.select(&query).await.map_err(|error| {
        tracing::error!(target: "admin", error = %error, "관리자 권한 조회 중 Supabase 오류");
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;

    let tier = resp
        .get(0)
        .and_then(|v| v.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("free");

    if tier != "admin" {
        tracing::warn!(target: "admin", tier = %tier, "관리자 API 접근 거부: user_profiles tier가 admin이 아님");
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            "관리자 권한 필요".to_string(),
        ));
    }
    Ok(())
}

// ── 콘텐츠 API ──

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
