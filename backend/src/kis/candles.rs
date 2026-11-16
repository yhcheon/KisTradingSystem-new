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
    // KIS 응답의 보조 출력 영역을 역직렬화해 응답 구조를 보존합니다.
    #[allow(dead_code)]
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
    // 누적 거래대금은 API 응답에 포함되지만 현재 Candle 모델에는 매핑하지 않습니다.
    #[allow(dead_code)]
    pub acml_tr_pbmn: String,
}

fn ensure_success(response: &MinuteChartResponse) -> Result<(), String> {
    if response.rt_cd == "0" {
        Ok(())
    } else {
        Err(format!(
            "KIS 분봉 조회 실패 ({}): {}",
            response.msg_cd, response.msg1
        ))
    }
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
    get_minute_chart_for_market(
        client, app_key, app_secret, access_token, "F", symbol, date, hour,
    )
    .await
}

/// 시장 구분(F: 주간, CM: 야간)을 지정해 선물 1분봉을 조회합니다.
pub async fn get_minute_chart_for_market(
    client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    market_div_code: &str,
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

    let query = [
        ("FID_COND_MRKT_DIV_CODE", market_div_code),
        ("FID_INPUT_ISCD", symbol),
        ("FID_HOUR_CLS_CODE", "60"),
        ("FID_PW_DATA_INCU_YN", "Y"),
        ("FID_FAKE_TICK_INCU_YN", "N"),
        ("FID_INPUT_DATE_1", date),
        ("FID_INPUT_HOUR_1", hour),
    ];
    let request_started = std::time::Instant::now();
    tracing::info!(
        target: "kis::night_candles",
        "[MINUTE-REQUEST] market={} symbol={} date={} cursor={} tr_id=FHKIF03020200 url={} query={:?}",
        market_div_code, symbol, date, hour, url, query
    );

    let response = client
        .get(&url)
        .header("Content-Type", "application/json; charset=utf-8")
        .header("authorization", format!("Bearer {}", access_token))
        .header("appkey", app_key)
        .header("appsecret", app_secret)
        .header("tr_id", "FHKIF03020200")
        .header("custtype", "P")
        .query(&query)
        .send()
        .await?;
    let status = response.status();
    let response_headers = format!("{:?}", response.headers());
    let body = response.bytes().await?;
    let elapsed_ms = request_started.elapsed().as_millis();

    if !status.is_success() {
        let body_text = String::from_utf8_lossy(&body);
        tracing::error!(
            target: "kis::night_candles",
            "[MINUTE-HTTP-ERROR] market={} symbol={} date={} cursor={} status={} elapsed_ms={} headers={} body={}",
            market_div_code, symbol, date, hour, status, elapsed_ms, response_headers, body_text
        );
        return Err(std::io::Error::other(format!(
            "KIS minute chart HTTP {} (market={}, symbol={}, date={}, cursor={}): {}",
            status, market_div_code, symbol, date, hour, body_text
        )).into());
    }

    tracing::info!(
        target: "kis::night_candles",
        "[MINUTE-HTTP-OK] market={} symbol={} date={} cursor={} status={} bytes={} elapsed_ms={} headers={}",
        market_div_code, symbol, date, hour, status, body.len(), elapsed_ms, response_headers
    );

    let resp: MinuteChartResponse = serde_json::from_slice(&body).map_err(|error| {
        let body_text = String::from_utf8_lossy(&body);
        tracing::error!(
            target: "kis::night_candles",
            "[MINUTE-JSON-ERROR] market={} symbol={} date={} cursor={} error={} body={}",
            market_div_code, symbol, date, hour, error, body_text
        );
        error
    })?;

    tracing::info!(
        target: "kis::night_candles",
        "[MINUTE-RESPONSE] market={} symbol={} date={} cursor={} rt_cd={} msg_cd={} msg1={} output2_count={}",
        market_div_code, symbol, date, hour, resp.rt_cd, resp.msg_cd, resp.msg1, resp.output2.len()
    );
    ensure_success(&resp).map_err(std::io::Error::other)?;

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
    let first = candles.first().map(|c| format!("{} {}", c.stck_bsop_date, c.stck_cntg_hour)).unwrap_or_else(|| "none".to_string());
    let last = candles.last().map(|c| format!("{} {}", c.stck_bsop_date, c.stck_cntg_hour)).unwrap_or_else(|| "none".to_string());
    tracing::info!(
        target: "kis::night_candles",
        "[MINUTE-PARSED] market={} symbol={} date={} cursor={} candles={} first={} last={}",
        market_div_code, symbol, date, hour, candles.len(), first, last
    );
    Ok(candles)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_minute_chart_metadata_and_candle_fields() {
        let response: MinuteChartResponse = serde_json::from_value(serde_json::json!({
            "rt_cd": "0",
            "msg_cd": "OK",
            "msg1": "정상 처리",
            "output1": { "request": "minute-chart" },
            "output2": [{
                "stck_bsop_date": "20261116",
                "stck_cntg_hour": "090100",
                "futs_prpr": "350.25",
                "futs_oprc": "350.00",
                "futs_hgpr": "351.00",
                "futs_lwpr": "349.50",
                "cntg_vol": "12",
                "acml_tr_pbmn": "4203000"
            }]
        }))
        .unwrap();

        assert_eq!(response.rt_cd, "0");
        assert_eq!(response.msg_cd, "OK");
        assert_eq!(response.msg1, "정상 처리");
        assert_eq!(response.output1["request"], "minute-chart");
        assert_eq!(response.output2.len(), 1);
        assert_eq!(response.output2[0].acml_tr_pbmn, "4203000");
        assert_eq!(response.output2[0].cntg_vol, "12");
        assert!(ensure_success(&response).is_ok());
    }

    #[test]
    fn rejects_kis_minute_chart_error_response() {
        let response: MinuteChartResponse = serde_json::from_value(serde_json::json!({
            "rt_cd": "1",
            "msg_cd": "EGW00123",
            "msg1": "요청 오류",
            "output1": {},
            "output2": []
        }))
        .unwrap();

        let error = ensure_success(&response).unwrap_err();
        assert!(error.contains("EGW00123"));
        assert!(error.contains("요청 오류"));
    }
}
