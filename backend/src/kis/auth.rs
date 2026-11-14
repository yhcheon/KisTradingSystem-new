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
