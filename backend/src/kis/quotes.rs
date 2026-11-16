use serde::Deserialize;

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
    let rest_domain = std::env::var("KIS_REST_DOMAIN")
        .unwrap_or_else(|_| "https://openapi.koreainvestment.com:9443".to_string());
    let url = format!(
        "{}/uapi/domestic-futureoption/v1/quotations/inquire-price",
        rest_domain.trim_end_matches('/')
    );

    let resp = client
        .get(&url)
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
