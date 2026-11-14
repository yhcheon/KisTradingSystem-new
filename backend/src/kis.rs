// ============================================================
// 파일명: kis.rs
// 설명: 한국투자증권(KIS) API 클라이언트
//       - OAuth 토큰 발급 (공유 HTTP 클라이언트 사용)
//       - 국내 선물옵션 시세 조회
//       - 1분봉 조회
//       - WebSocket approval_key 발급
// 작성일: 2026-11-14
// ============================================================

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── 토큰 관련 ──

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
    client: &reqwest::Client,
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
    let cached = CachedToken {
        token: token.clone(),
        issued_at: now,
    };
    std::fs::write(cache_path, serde_json::to_string(&cached)?)?;
    tracing::info!("KIS token issued and cached");

    Ok(token)
}

// ── WebSocket 접속키 발급 ──

#[derive(Debug, Serialize)]
struct ApprovalRequest {
    grant_type: String,
    appkey: String,
    secretkey: String,
}

#[derive(Debug, Deserialize)]
pub struct ApprovalResponse {
    pub approval_key: String,
}

pub async fn get_approval_key(
    client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    let body = ApprovalRequest {
        grant_type: "client_credentials".to_string(),
        appkey: app_key.to_string(),
        secretkey: app_secret.to_string(),
    };

    let resp = client
        .post("https://openapi.koreainvestment.com:9443/oauth2/Approval")
        .header("Content-Type", "application/json; charset=UTF-8")
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json::<ApprovalResponse>()
        .await?;

    Ok(resp.approval_key)
}

// ── 선물옵션 시세 ──

#[derive(Debug, Deserialize)]
pub struct FuturesPriceOutput1 {
    pub hts_kor_isnm: String,
    pub futs_prpr: String,
    pub futs_prdy_vrss: String,
    pub futs_prdy_ctrt: String,
    pub futs_oprc: String,
    pub futs_hgpr: String,
    pub futs_lwpr: String,
    pub futs_prdy_clpr: String,
    pub acml_vol: String,
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
    client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbol: &str,
) -> Result<FuturesPriceResponse, Box<dyn std::error::Error>> {
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

// ── 1분봉 조회 ──

#[derive(Debug, Deserialize)]
pub struct MinuteCandle {
    pub stck_bsop_date: String,
    pub stck_cntg_hour: String,
    pub futs_prpr: String,
    pub futs_oprc: String,
    pub futs_hgpr: String,
    pub futs_lwpr: String,
    pub cntg_vol: String,
}

#[derive(Debug, Deserialize)]
pub struct MinuteChartResponse {
    pub rt_cd: String,
    pub msg_cd: String,
    pub msg1: String,
    pub output1: serde_json::Value,
    pub output2: Vec<MinuteChartOutput2>,
}

#[derive(Debug, Deserialize)]
pub struct MinuteChartOutput2 {
    pub stck_bsop_date: String,
    pub stck_cntg_hour: String,
    pub futs_prpr: String,
    pub futs_oprc: String,
    pub futs_hgpr: String,
    pub futs_lwpr: String,
    pub cntg_vol: String,
    pub acml_tr_pbmn: String,
}

pub async fn get_minute_chart(
    client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbol: &str,
    date: &str,
    hour: &str,
) -> Result<Vec<MinuteCandle>, Box<dyn std::error::Error>> {
    let resp = client
        .get("https://openapi.koreainvestment.com:9443/uapi/domestic-futureoption/v1/quotations/inquire-time-fuopchartprice")
        .header("Content-Type", "application/json; charset=utf-8")
        .header("authorization", format!("Bearer {}", access_token))
        .header("appkey", app_key)
        .header("appsecret", app_secret)
        .header("tr_id", "FHKIF03020200")
        .header("custtype", "P")
        .query(&[
            ("FID_COND_MRKT_DIV_CODE", "F"),
            ("FID_INPUT_ISCD", symbol),
            ("FID_HOUR_CLS_CODE", "60"),
            ("FID_PW_DATA_INCU_YN", "Y"),
            ("FID_FAKE_TICK_INCU_YN", "N"),
            ("FID_INPUT_DATE_1", date),
            ("FID_INPUT_HOUR_1", hour),
        ])
        .send()
        .await?
        .error_for_status()?
        .json::<MinuteChartResponse>()
        .await?;

    let mut candles: Vec<MinuteCandle> = resp
        .output2
        .into_iter()
        .map(|c| MinuteCandle {
            stck_bsop_date: c.stck_bsop_date,
            stck_cntg_hour: c.stck_cntg_hour,
            futs_prpr: c.futs_prpr,
            futs_oprc: c.futs_oprc,
            futs_hgpr: c.futs_hgpr,
            futs_lwpr: c.futs_lwpr,
            cntg_vol: c.cntg_vol,
        })
        .collect();

    candles.sort_by(|a, b| {
        (a.stck_bsop_date.clone(), a.stck_cntg_hour.clone())
            .cmp(&(b.stck_bsop_date.clone(), b.stck_cntg_hour.clone()))
    });
    Ok(candles)
}
