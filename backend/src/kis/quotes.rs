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
    // 미리 알 수 없는 KIS 확장 필드를 보존해 API 변경에 대응합니다.
    #[serde(flatten)]
    #[allow(dead_code)]
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

fn ensure_success(response: &FuturesPriceResponse) -> Result<(), String> {
    if response.rt_cd == "0" {
        Ok(())
    } else {
        Err(format!(
            "KIS 선물옵션 시세 조회 실패 ({}): {}",
            response.msg_cd, response.msg1
        ))
    }
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

    ensure_success(&resp).map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::Other, error)
    })?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_quote_metadata_and_preserves_extra_output_fields() {
        let response: FuturesPriceResponse = serde_json::from_value(serde_json::json!({
            "rt_cd": "0",
            "msg_cd": "OK",
            "msg1": "정상 처리",
            "output1": {
                "hts_kor_isnm": "KOSPI200 선물",
                "futs_prpr": "350.25",
                "futs_prdy_vrss": "1.25",
                "futs_prdy_ctrt": "0.36",
                "futs_oprc": "349.50",
                "futs_hgpr": "351.00",
                "futs_lwpr": "348.75",
                "futs_prdy_clpr": "349.00",
                "acml_vol": "1200"
            },
            "output2": { "some_extra_field": "retained" }
        })).unwrap();

        assert_eq!(response.rt_cd, "0");
        assert_eq!(response.msg_cd, "OK");
        assert_eq!(response.msg1, "정상 처리");
        assert_eq!(response.output1.futs_prpr, "350.25");
        assert_eq!(response.output2.extra["some_extra_field"], "retained");
        assert!(ensure_success(&response).is_ok());
    }

    #[test]
    fn rejects_kis_quote_error_response() {
        let response: FuturesPriceResponse = serde_json::from_value(serde_json::json!({
            "rt_cd": "1",
            "msg_cd": "EGW00123",
            "msg1": "시세 조회 오류",
            "output1": {
                "hts_kor_isnm": "",
                "futs_prpr": "",
                "futs_prdy_vrss": "",
                "futs_prdy_ctrt": "",
                "futs_oprc": "",
                "futs_hgpr": "",
                "futs_lwpr": "",
                "futs_prdy_clpr": "",
                "acml_vol": ""
            },
            "output2": {}
        })).unwrap();

        let error = ensure_success(&response).unwrap_err();
        assert!(error.contains("EGW00123"));
        assert!(error.contains("시세 조회 오류"));
    }
}
