// ============================================================
// 파일명: server/handlers/content/auth.rs
// 설명: 콘텐츠 API JWT 인증 및 관리자 권한 확인
// ============================================================

use crate::auth::jwt;
use crate::core::state::AppState;
use axum::http::HeaderMap;

/// Authorization 헤더에서 JWT 추출 + 검증
pub(super) async fn require_auth(
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
pub(super) async fn require_admin(
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
