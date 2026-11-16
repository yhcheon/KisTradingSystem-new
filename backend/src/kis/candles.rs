use serde::Deserialize;

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
    let rest_domain = std::env::var("KIS_REST_DOMAIN")
        .unwrap_or_else(|_| "https://openapi.koreainvestment.com:9443".to_string());
    let url = format!(
        "{}/uapi/domestic-futureoption/v1/quotations/inquire-time-fuopchartprice",
        rest_domain.trim_end_matches('/')
    );

    let resp = client
        .get(&url)
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
