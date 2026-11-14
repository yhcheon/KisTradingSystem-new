// ============================================================
// 파일명: auth/jwt.rs
// 설명: Supabase JWT 검증
//       - Legacy JWT Secret으로 서명 검증
//       - Claims에서 사용자 ID, 이메일 추출
// 작성일: 2026-11-14
// ============================================================

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String, // 사용자 UUID
    pub email: Option<String>,
    pub role: String, // "authenticated" | "anon" | "service_role"
    pub exp: usize,   // 만료 시각 (Unix)
    pub iat: usize,   // 발급 시각
}

/// Supabase JWT 검증
pub fn verify_jwt(token: &str, jwt_secret: &str) -> Result<Claims, String> {
    let mut validation = Validation::new(Algorithm::HS256);

    // Supabase JWT의 aud는 보통 "authenticated" 이므로, 검증을 비활성화하거나 허용
    validation.validate_aud = false; // 또는 validation.set_audience(&["authenticated"]);


    let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());

    decode::<Claims>(token, &decoding_key, &validation)
        .map(|data| data.claims)
        .map_err(|e| format!("JWT 검증 실패: {}", e))


}
