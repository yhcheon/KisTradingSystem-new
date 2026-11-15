// ============================================================
// 파일명: auth/jwt/verify.rs
// 설명: Supabase ES256 및 Legacy HS256 JWT 검증
// ============================================================

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

use super::{claims::Claims, jwks::get_es256_decoding_key};

/// Supabase JWT 검증. 토큰 원문이나 서명값은 로그에 기록하지 않는다.
pub async fn verify_jwt(
    token: &str,
    jwt_secret: &str,
    supabase_url: &str,
    http_client: &reqwest::Client,
) -> Result<Claims, String> {
    let header = decode_header(token).map_err(|error| {
        tracing::warn!(target: "auth", error = %error, "JWT 헤더 파싱 실패");
        format!("JWT 헤더 파싱 실패: {error}")
    })?;

    let token_alg = header.alg;
    let token_kid = header.kid.as_deref().unwrap_or("없음");
    tracing::debug!(
        target: "auth",
        token_alg = ?token_alg,
        token_kid = %token_kid,
        "JWT 검증 시작"
    );

    let (decoding_key, algorithm) = match token_alg {
        Algorithm::ES256 => {
            let kid = header
                .kid
                .as_deref()
                .ok_or_else(|| "ES256 JWT에 kid 헤더가 없습니다".to_string())?;
            (
                get_es256_decoding_key(http_client, supabase_url, kid).await?,
                Algorithm::ES256,
            )
        }
        Algorithm::HS256 => (
            DecodingKey::from_secret(jwt_secret.as_bytes()),
            Algorithm::HS256,
        ),
        other => {
            tracing::warn!(target: "auth", algorithm = ?other, "허용하지 않는 JWT 알고리즘");
            return Err(format!("허용하지 않는 JWT 알고리즘: {other:?}"));
        }
    };

    let mut validation = Validation::new(algorithm);
    validation.validate_aud = false;

    let data = decode::<Claims>(token, &decoding_key, &validation).map_err(|error| {
        tracing::warn!(
            target: "auth",
            token_alg = ?token_alg,
            token_kid = %token_kid,
            error = %error,
            "JWT 서명 또는 클레임 검증 실패"
        );
        format!("JWT 검증 실패 (alg={token_alg:?}, kid={token_kid}): {error}")
    })?;

    let expected_issuer = format!("{}/auth/v1", supabase_url.trim_end_matches('/'));
    if data.claims.iss.as_deref() != Some(expected_issuer.as_str()) {
        tracing::warn!(target: "auth", "JWT issuer 불일치");
        return Err("JWT issuer가 현재 Supabase 프로젝트와 일치하지 않습니다".to_string());
    }

    tracing::debug!(target: "auth", token_alg = ?token_alg, "JWT 검증 성공");
    Ok(data.claims)
}
