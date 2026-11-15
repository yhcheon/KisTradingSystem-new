// ============================================================
// 파일명: market/history.rs
// 설명: KIS 과거 1분봉을 조회해 CandleStore 초기 데이터 구성
// ============================================================

use crate::core::candles::{Candle, CandleStore};
use crate::kis;
use crate::master::FuturesSymbol;

mod time;
use time::subtract_one_minute;

/// 최근 실제 거래일 최대 2일과 장중 당일의 1분봉을 로드합니다.
pub async fn load_initial_candles(
    http_client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbols: &[FuturesSymbol],
) -> CandleStore {
    let mut candle_store = CandleStore::new();
    let today_dt = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
    let today = today_dt.format("%Y%m%d").to_string();
    let now_hhmm = today_dt.format("%H%M").to_string().parse::<u32>().unwrap_or(0);
    let is_weekday = today_dt.format("%u").to_string().parse::<u32>().unwrap_or(7) <= 5;
    let is_market_hours = is_weekday && (845..=1545).contains(&now_hhmm);

    for symbol in symbols {
        // 달력상 과거 14일에서 실제 분봉이 존재하는 서로 다른 거래일 2개를 찾습니다.
        let mut found: Vec<(String, Vec<kis::MinuteCandle>)> = Vec::new();
        for days_back in 1..=14 {
            let candidate_date = (today_dt.date_naive() - chrono::Duration::days(days_back))
                .format("%Y%m%d").to_string();
            match kis::get_minute_chart(http_client, app_key, app_secret, access_token,
                &symbol.code, &candidate_date, "154500").await {
                Ok(chunk) => {
                    let latest_date = chunk.iter()
                        .map(|c| c.stck_bsop_date.as_str())
                        .filter(|date| *date < today.as_str())
                        .max().map(str::to_string);
                    if let Some(date) = latest_date {
                        if !found.iter().any(|(existing, _)| existing == &date) {
                            let day_chunk: Vec<kis::MinuteCandle> = chunk.into_iter()
                                .filter(|c| c.stck_bsop_date == date).collect();
                            if !day_chunk.is_empty() {
                                tracing::info!("거래일 탐색 확인: {} {}", symbol.code, date);
                                found.push((date, day_chunk));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("거래일 탐색 중 1분봉 조회 실패({} / {}): {}", symbol.code, candidate_date, e);
                    break;
                }
            }
            if found.len() >= 2 { break; }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        // 최신 거래일부터 순서대로 처리합니다. CandleStore가 최대 2개 날짜를 유지합니다.
        found.sort_by(|a, b| a.0.cmp(&b.0));
        if found.is_empty() {
            tracing::warn!("직전 14일 내 거래일 1분봉을 찾지 못했습니다: {}", symbol.code);
        }
        for (trade_date, first_chunk) in found {
            tracing::info!("거래일({}) 1분봉 로드 시작: {}", trade_date, symbol.code);
            let candles = load_trade_date(http_client, app_key, app_secret, access_token,
                &symbol.code, &trade_date, first_chunk).await;
            tracing::info!("거래일 1분봉 로드 완료: {} {}건", trade_date, candles.len());
            candle_store.load_1m(&symbol.code, &trade_date, candles);
        }

        // 당일 장중 데이터는 최신 거래일로 저장되어 CandleStore에 함께 유지됩니다.
        if is_market_hours {
            tracing::info!("장중이므로 당일({}) 1분봉도 로드", today);
            let mut t_1m = Vec::new();
            let mut hour = format!("{:02}{:02}00", now_hhmm / 100, now_hhmm % 100);
            for _ in 0..5 {
                match kis::get_minute_chart(http_client, app_key, app_secret, access_token,
                    &symbol.code, &today, &hour).await {
                    Ok(chunk) => {
                        if chunk.is_empty() { break; }
                        let earliest = chunk.iter().filter(|c| c.stck_bsop_date == today)
                            .map(|c| c.stck_cntg_hour.as_str()).min().unwrap_or("").to_string();
                        for c in chunk.iter().filter(|c| c.stck_bsop_date == today) {
                            if c.stck_cntg_hour.len() < 4 { continue; }
                            t_1m.push(to_candle(c));
                        }
                        if earliest.is_empty() || earliest.as_str() <= "084500" { break; }
                        hour = subtract_one_minute(&earliest);
                    }
                    Err(e) => { tracing::warn!("당일 1분봉 조회 실패({}): {}", symbol.code, e); break; }
                }
                tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            }
            t_1m.sort_by(|a, b| a.time.cmp(&b.time));
            t_1m.dedup_by(|a, b| a.time == b.time);
            candle_store.load_1m(&symbol.code, &today, t_1m);
        }
    }
    candle_store
}

async fn load_trade_date(
    http_client: &reqwest::Client,
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
        let chunk = if page == 0 { first_chunk.take().unwrap_or_default() } else {
            match kis::get_minute_chart(http_client, app_key, app_secret, access_token,
                symbol, trade_date, &hour).await {
                Ok(chunk) => chunk,
                Err(e) => { tracing::warn!("과거 1분봉 추가 조회 실패({} / {}): {}", symbol, hour, e); break; }
            }
        };
        let mut earliest = String::new();
        for candle in chunk.iter().filter(|c| c.stck_bsop_date == trade_date) {
            if candle.stck_cntg_hour.len() < 6 { continue; }
            if earliest.is_empty() || candle.stck_cntg_hour < earliest { earliest = candle.stck_cntg_hour.clone(); }
            result.push(to_candle(candle));
        }
        if earliest.is_empty() || earliest.as_str() <= "084500" { break; }
        hour = subtract_one_minute(&earliest);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    result.sort_by(|a, b| a.time.cmp(&b.time));
    result.dedup_by(|a, b| a.time == b.time);
    result
}

fn to_candle(c: &kis::MinuteCandle) -> Candle {
    Candle {
        time: c.stck_cntg_hour[0..4].to_string(),
        open: c.futs_oprc.parse().unwrap_or(0.0),
        high: c.futs_hgpr.parse().unwrap_or(0.0),
        low: c.futs_lwpr.parse().unwrap_or(0.0),
        close: c.futs_prpr.parse().unwrap_or(0.0),
        volume: c.cntg_vol.parse().unwrap_or(0),
    }
}
