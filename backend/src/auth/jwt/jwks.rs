// ============================================================
// 파일명: auth/jwt/jwks.rs
// 설명: Supabase ES256 JWKS 공개키 조회 및 캐시
// ============================================================

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use jsonwebtoken::{DecodingKey, jwk::JwkSet};
use tokio::sync::{Mutex, OnceCell};

const JWKS_CACHE_TTL: Duration = Duration::from_secs(60 * 60);

struct CachedJwks {
    jwks: JwkSet,
    fetched_at: Instant,
}

static JWKS_CACHE: OnceCell<Mutex<HashMap<String, CachedJwks>>> = OnceCell::const_new();

pub(super) async fn get_es256_decoding_key(
    http_client: &reqwest::Client,
    supabase_url: &str,
    kid: &str,
) -> Result<DecodingKey, String> {
    let project_url = supabase_url.trim_end_matches('/').to_string();

    let cache = JWKS_CACHE
        .get_or_init(|| async { Mutex::new(HashMap::new()) })
        .await;

    let mut cache = cache.lock().await;

    let needs_refresh = cache
        .get(&project_url)
        .map(|cached| {
            cached.fetched_at.elapsed() >= JWKS_CACHE_TTL
                || !cached.jwks.keys.iter().any(|key| key.common.key_id.as_deref() == Some(kid))
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
