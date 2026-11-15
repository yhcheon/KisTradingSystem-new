// ============================================================
// 파일명: server/handlers/content/access.rs
// 설명: 콘텐츠 목록/상세 조회의 공통 접근 권한 검사
// ============================================================

use super::auth::require_auth;
use crate::core::state::AppState;
use axum::http::{HeaderMap, StatusCode};
use chrono::Timelike;

/// 콘텐츠 조회 공통 정책: 로그인 필수, 무료 회원은 KST 14시부터 열람 가능.
pub async fn require_content_access(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<(), (StatusCode, String)> {
    let claims = require_auth(headers, state).await?;
    let tier_query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let tier_resp = state
        .supabase
        .select(&tier_query)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
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
                StatusCode::FORBIDDEN,
                "무료 회원은 오후 2시부터 열람 가능합니다".to_string(),
            ));
        }
    }
    Ok(())
}

/// UUID v4 형식의 글 ID인지 확인합니다.
pub fn is_uuid(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| match index {
            8 | 13 | 18 | 23 => byte == b'-',
            _ => byte.is_ascii_hexdigit(),
        })
}

#[cfg(test)]
mod tests {
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
