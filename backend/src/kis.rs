
// ============================================================
// 파일명: kis.rs
// 설명: 한국투자증권(KIS) API 클라이언트
//       - OAuth 토큰 발급 및 파일 캐시
//       - 국내 선물옵션 시세 조회 (전일종가/시가/고가/저가 포함)
// 작성일: 2026-11-14
// ============================================================

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── 토큰 관련 (기존과 동일) ──

#[derive(Debug, Serialize)]
struct TokenRequest {
    grant_type: String,
    appkey: String,
    appsecret: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    pub access_token_token_expired: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CachedToken {
    token: TokenResponse,
    issued_at: u64,
}

pub async fn get_or_issue_token(
    app_key: &str,
    app_secret: &str,
    cache_path: &str,
) -> Result<TokenResponse, Box<dyn std::error::Error>> {
    if Path::new(cache_path).exists() {
        if let Ok(content) = std::fs::read_to_string(cache_path) {
            if let Ok(cached) = serde_json::from_str::<CachedToken>(&content) {
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)?
                    .as_secs();
                if now.saturating_sub(cached.issued_at) < 23 * 3600 {
                    tracing::info!("KIS token loaded from cache");
                    return Ok(cached.token);
                }
            }
        }
    }

    let client = reqwest::Client::new();
    let body = TokenRequest {
        grant_type: "client_credentials".to_string(),
        appkey: app_key.to_string(),
        appsecret: app_secret.to_string(),
    };

    let token = client
        .post("https://openapi.koreainvestment.com:9443/oauth2/tokenP")
        .header("Content-Type", "application/json; charset=UTF-8")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<TokenResponse>()
        .await?;

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_secs();
    let cached = CachedToken { token: token.clone(), issued_at: now };
    std::fs::write(cache_path, serde_json::to_string(&cached)?)?;
    tracing::info!("KIS token issued and cached");

    Ok(token)
}

// ── 선물옵션 시세 (전일종가/시가/고가/저가 포함) ──

#[derive(Debug, Deserialize)]
pub struct FuturesPriceOutput1 {
    pub hts_kor_isnm: String,     // 종목명
    pub futs_prpr: String,        // 현재가
    pub futs_prdy_vrss: String,   // 전일 대비
    pub futs_prdy_ctrt: String,   // 전일 대비율
    pub futs_oprc: String,        // 시가
    pub futs_hgpr: String,        // 고가
    pub futs_lwpr: String,        // 저가
    pub futs_prdy_clpr: String,   // 전일 종가
    pub acml_vol: String,         // 누적 거래량
}

#[derive(Debug, Deserialize)]
pub struct FuturesPriceOutput2 {
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct FuturesPriceResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output1: FuturesPriceOutput1,
    pub output2: FuturesPriceOutput2,
}

pub async fn get_futures_price(
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbol: &str,
) -> Result<FuturesPriceResponse, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let resp = client
        .get("https://openapi.koreainvestment.com:9443/uapi/domestic-futureoption/v1/quotations/inquire-price")
        .header("Content-Type", "application/json; charset=UTF-8")
        .header("authorization", format!("Bearer {}", access_token))
        .header("appkey", app_key)
        .header("appsecret", app_secret)
        .header("tr_id", "FHMIF10000000")
        .query(&[
            ("FID_COND_MRKT_DIV_CODE", "F"),
            ("FID_INPUT_ISCD", symbol),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<FuturesPriceResponse>()
        .await?;
    Ok(resp)
}


