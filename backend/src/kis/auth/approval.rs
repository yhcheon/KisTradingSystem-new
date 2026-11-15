use serde::{Deserialize, Serialize};

// ── WebSocket 접속 승인키 ──

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
