// ============================================================
// 파일명: market/history/pagination.rs
// 설명: 과거 거래일 1분봉 페이지 순회 및 정렬
// ============================================================

use super::{convert::to_candle, time::subtract_one_minute};
use crate::core::candles::Candle;
use crate::kis;
use reqwest::Client;

pub(super) async fn load_trade_date(
    http_client: &Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbol: &str,
    trade_date: &str,
    first_chunk: Vec<kis::MinuteCandle>,
) -> Vec<Candle> {
    let mut result = Vec::new();
    let mut first_chunk = Some(first_chunk);
    let mut hour = "154500".to_string();

    for page in 0..5 {
        let chunk = if page == 0 {
            first_chunk.take().unwrap_or_default()
        } else {
            match kis::get_minute_chart(
                http_client,
                app_key,
                app_secret,
                access_token,
                symbol,
                trade_date,
                &hour,
            )
            .await
            {
                Ok(chunk) => chunk,
                Err(e) => {
                    tracing::warn!("과거 1분봉 추가 조회 실패({} / {}): {}", symbol, hour, e);
                    break;
                }
            }
        };

        let mut earliest = String::new();
        for candle in chunk.iter().filter(|c| c.stck_bsop_date == trade_date) {
            if candle.stck_cntg_hour.len() < 6 {
                continue;
            }
            if earliest.is_empty() || candle.stck_cntg_hour < earliest {
                earliest = candle.stck_cntg_hour.clone();
            }
            result.push(to_candle(candle));
        }

        if earliest.is_empty() || earliest.as_str() <= "084500" {
            break;
        }
        hour = subtract_one_minute(&earliest);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    result.sort_by(|a, b| a.time.cmp(&b.time));
    result.dedup_by(|a, b| a.time == b.time);
    result
}
