// ============================================================
// 파일명: auth/jwt.rs
// 설명: Supabase JWT 검증
//       - Legacy JWT Secret으로 서명 검증
//       - Claims에서 사용자 ID, 이메일 추출
// 작성일: 2026-11-14
// ============================================================

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
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
    // 토큰 원문은 절대 기록하지 않고, 디버깅에 필요한 헤더 메타데이터만 기록한다.
    let (token_alg, token_kid) = match decode_header(token) {
        Ok(header) => (format!("{:?}", header.alg), header.kid.unwrap_or_else(|| "없음".to_string())),
        Err(error) => {
            tracing::warn!(target: "auth", error = %error, "JWT 헤더 파싱 실패");
            return Err(format!("JWT 헤더 파싱 실패: {}", error));
        }
    };

    tracing::debug!(
        target: "auth",
        token_alg = %token_alg,
        token_kid = %token_kid,
        expected_alg = "HS256",
        "JWT 검증 시작"
    );

    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_aud = false;

    let decoding_key = DecodingKey::from_secret(jwt_secret.as_bytes());

    match decode::<Claims>(token, &decoding_key, &validation) {
        Ok(data) => {
            tracing::debug!(target: "auth", "JWT 검증 성공");
            Ok(data.claims)
        }
        Err(error) => {
            tracing::warn!(
                target: "auth",
                token_alg = %token_alg,
                token_kid = %token_kid,
                expected_alg = "HS256",
                error = %error,
                "JWT 검증 실패"
            );
            Err(format!(
                "JWT 검증 실패 (token_alg={}, expected_alg=HS256): {}",
                token_alg, error
            ))
        }
    }


}
