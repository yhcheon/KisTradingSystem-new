// ============================================================
// 파일명: auth/jwt.rs
// 설명: Supabase JWT 검증 (현재 ES256 + Legacy HS256)
//       - ES256: Supabase JWKS 공개키로 서명 검증
//       - HS256: Legacy JWT Secret으로 이전 토큰 검증
//       - JWKS는 프로젝트별로 캐시하고 주기적으로 갱신
// ============================================================

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use jsonwebtoken::{
    Algorithm, DecodingKey, Validation, decode, decode_header,
    jwk::JwkSet,
};
use serde::{Deserialize, Serialize};
use tokio::sync::{Mutex, OnceCell};

const JWKS_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

struct CachedJwks {
    jwks: JwkSet,
    fetched_at: Instant,
}

static JWKS_CACHE: OnceCell<Mutex<HashMap<String, CachedJwks>>> = OnceCell::const_new();

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: Option<String>,
}

async fn get_es256_decoding_key(
    http_client: &reqwest::Client,
    supabase_url: &str,
    kid: &str,
) -> Result<DecodingKey, String> {
    let project_url = supabase_url.trim_end_matches('/').to_string();
    let cache = JWKS_CACHE.get_or_init(|| async { Mutex::new(HashMap::new()) });
    let mut cache = cache.lock().await;

    let needs_refresh = cache
        .get(&project_url)
        .map(|cached| {
            cached.fetched_at.elapsed() >= JWKS_CACHE_TTL
                || !cached
                    .jwks
                    .keys
                    .iter()
                    .any(|key| key.common.key_id.as_deref() == Some(kid))
        })
        .unwrap_or(true);

    if needs_refresh {
        let url = format!("{project_url}/auth/v1/.well-known/jwks.json");
        tracing::info!(target: "auth", "Supabase JWKS 공개키 조회 시작");
        let response = http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Supabase JWKS 요청 실패: {e}"))?
            .error_for_status()
            .map_err(|e| format!("Supabase JWKS HTTP 오류: {e}"))?;

        let jwks = response
            .json::<JwkSet>()
            .await
            .map_err(|e| format!("Supabase JWKS 응답 파싱 실패: {e}"))?;

        if jwks.keys.is_empty() {
            return Err("Supabase JWKS에 공개키가 없습니다".to_string());
        }

        tracing::info!(
            target: "auth",
            key_count = jwks.keys.len(),
            "Supabase JWKS 공개키 캐시 갱신 완료"
        );
        cache.insert(
            project_url.clone(),
            CachedJwks {
                jwks,
                fetched_at: Instant::now(),
            },
        );
    }

    let cached = cache
        .get(&project_url)
        .ok_or_else(|| "Supabase JWKS 캐시를 찾을 수 없습니다".to_string())?;
    let jwk = cached
        .jwks
        .keys
        .iter()
        .find(|key| key.common.key_id.as_deref() == Some(kid))
        .ok_or_else(|| {
            tracing::warn!(
                target: "auth",
                token_kid = %kid,
                "갱신된 JWKS에서도 JWT kid와 일치하는 공개키를 찾지 못함"
            );
            format!("JWKS에서 kid={kid} 공개키를 찾을 수 없습니다")
        })?;

    DecodingKey::from_jwk(jwk).map_err(|e| format!("JWKS 공개키 변환 실패: {e}"))
}

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
