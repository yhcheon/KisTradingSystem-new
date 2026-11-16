// ============================================================
// 파일명: market/history.rs
// 설명: KIS 과거 1분봉을 조회해 CandleStore 초기 데이터 구성
// ============================================================

use crate::core::candles::CandleStore;
use crate::kis;
use crate::master::FuturesSymbol;

mod convert;
mod pagination;
mod time;

use convert::to_candle;
use pagination::load_trade_date;
use time::subtract_one_minute;


/// 18시 이전에는 대기하고, 야간 세션 중 서버가 시작되면 현재 시각부터 18시까지 CM 분봉을 로드합니다.
/// 야간 체결 WebSocket 구독은 별도로 서버 시작 시 신청되어 있으므로 이후 봉은 실시간으로 누적됩니다.
pub fn spawn_night_candle_loader(
    http_client: reqwest::Client,
    app_key: String,
    app_secret: String,
    access_token: String,
    symbols: Vec<String>,
    candle_store: std::sync::Arc<tokio::sync::Mutex<CandleStore>>,
) {
    tokio::spawn(async move {
        loop {
            let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
            let hour = chrono::Timelike::hour(&now);
            let session_active = hour >= 18 || hour < 6;

            if !session_active {
                let next_start = now
                    .date_naive()
                    .and_hms_opt(18, 0, 0)
                    .expect("valid night session start");
                let wait = (next_start - now.naive_local())
                    .to_std()
                    .unwrap_or_default();
                tracing::info!("야간 분봉 조회 대기: KST 18:00까지");
                tokio::time::sleep(wait).await;
            }

            let session_now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
            let session_hour = chrono::Timelike::hour(&session_now);
            let (trade_date, cursor_hour) = if session_hour < 6 {
                (
                    (session_now.date_naive() - chrono::Duration::days(1))
                        .format("%Y%m%d")
                        .to_string(),
                    format!("{:02}{}",
                        session_hour + 24,
                        session_now.format("%M%S")
                    ),
                )
            } else {
                (
                    session_now.format("%Y%m%d").to_string(),
                    session_now.format("%H%M%S").to_string(),
                )
            };

            for symbol in &symbols {
                match load_night_session(
                    &http_client,
                    &app_key,
                    &app_secret,
                    &access_token,
                    symbol,
                    &trade_date,
                    &cursor_hour,
                )
                .await
                {
                    Ok(candles) => {
                        tracing::info!(
                            "야간 CM 분봉 조회 완료: code={}, date={}, cursor={}, candles={}",
                            symbol, trade_date, cursor_hour, candles.len()
                        );
                        if !candles.is_empty() {
                            candle_store
                                .lock()
                                .await
                                .load_1m(symbol, &trade_date, candles);
                        }
                    }
                    Err(error) => tracing::warn!(
                        "야간 CM 분봉 조회 실패: code={}, date={}, cursor={}, error={}",
                        symbol, trade_date, cursor_hour, error
                    ),
                }
            }

            // 다음 야간 개장까지 대기합니다. 실시간 H0MFCNT0 체결은 계속 CandleStore에 누적됩니다.
            let after_load = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
            let next_start_date = if chrono::Timelike::hour(&after_load) < 6 {
                after_load.date_naive()
            } else {
                after_load.date_naive() + chrono::Duration::days(1)
            };
            let next_day_start = next_start_date
                .and_hms_opt(18, 0, 0)
                .expect("valid next night session start");
            let wait = (next_day_start - after_load.naive_local())
                .to_std()
                .unwrap_or_default();
            tokio::time::sleep(wait).await;
        }
    });
}

async fn load_night_session(
    http_client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbol: &str,
    trade_date: &str,
    initial_hour: &str,
) -> Result<Vec<crate::core::candles::Candle>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();
    let mut cursor = initial_hour.to_string();

    // 실전 API는 호출당 최대 102건이므로, 18시까지 최대 5페이지를 역순 조회합니다.
    for page in 0..5 {
        let chunk = kis::get_minute_chart_for_market(
            http_client,
            app_key,
            app_secret,
            access_token,
            "CM",
            symbol,
            trade_date,
            &cursor,
        )
        .await?;

        if chunk.is_empty() {
            break;
        }

        let mut earliest = String::new();
        for candle in chunk.iter().filter(|c| {
            c.stck_bsop_date == trade_date && c.stck_cntg_hour.as_str() >= "180000"
        }) {
            if earliest.is_empty() || candle.stck_cntg_hour < earliest {
                earliest = candle.stck_cntg_hour.clone();
            }
            result.push(to_candle(candle));
        }

        if earliest.is_empty() || earliest.as_str() <= "180000" {
            break;
        }
        if page == 4 {
            break;
        }
        cursor = subtract_one_minute(&earliest);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    result.sort_by(|a, b| a.time.cmp(&b.time));
    result.dedup_by(|a, b| a.time == b.time);
    Ok(result)
}

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
    let now_hhmm = today_dt
        .format("%H%M")
        .to_string()
        .parse::<u32>()
        .unwrap_or(0);
    let is_weekday = today_dt
        .format("%u")
        .to_string()
        .parse::<u32>()
        .unwrap_or(7)
        <= 5;
    let is_market_hours = is_weekday && (845..=1545).contains(&now_hhmm);

    for symbol in symbols {
        // 달력상 과거 14일에서 실제 분봉이 존재하는 서로 다른 거래일 2개를 찾습니다.
        let mut found: Vec<(String, Vec<kis::MinuteCandle>)> = Vec::new();
        for days_back in 1..=14 {
            let candidate_date = (today_dt.date_naive() - chrono::Duration::days(days_back))
                .format("%Y%m%d")
                .to_string();
            match kis::get_minute_chart(
                http_client,
                app_key,
                app_secret,
                access_token,
                &symbol.code,
                &candidate_date,
                "154500",
            )
            .await
            {
                Ok(chunk) => {
                    let latest_date = chunk
                        .iter()
                        .map(|c| c.stck_bsop_date.as_str())
                        .filter(|date| *date < today.as_str())
                        .max()
                        .map(str::to_string);
                    if let Some(date) = latest_date {
                        if !found.iter().any(|(existing, _)| existing == &date) {
                            let day_chunk: Vec<kis::MinuteCandle> = chunk
                                .into_iter()
                                .filter(|c| c.stck_bsop_date == date)
                                .collect();
                            if !day_chunk.is_empty() {
                                tracing::info!("거래일 탐색 확인: {} {}", symbol.code, date);
                                found.push((date, day_chunk));
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "거래일 탐색 중 1분봉 조회 실패({} / {}): {}",
                        symbol.code,
                        candidate_date,
                        e
                    );
                    // 휴장일 또는 특정 종목의 일시적인 API 오류일 수 있으므로
                    // 탐색을 중단하지 않고 다음 과거 날짜를 확인합니다.
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    continue;
                }
            }
            if found.len() >= 2 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        found.sort_by(|a, b| a.0.cmp(&b.0));
        if found.is_empty() {
            tracing::warn!(
                "직전 14일 내 거래일 1분봉을 찾지 못했습니다: {}",
                symbol.code
            );
        }
        for (trade_date, first_chunk) in found {
            tracing::info!("거래일({}) 1분봉 로드 시작: {}", trade_date, symbol.code);
            let candles = load_trade_date(
                http_client,
                app_key,
                app_secret,
                access_token,
                &symbol.code,
                &trade_date,
                first_chunk,
            )
            .await;
            tracing::info!("거래일 1분봉 로드 완료: {} {}건", trade_date, candles.len());
            candle_store.load_1m(&symbol.code, &trade_date, candles);
        }

        // 당일 장중 데이터는 최신 거래일로 저장되어 CandleStore에 함께 유지됩니다.
        if is_market_hours {
            tracing::info!("장중이므로 당일({}) 1분봉도 로드", today);
            let mut t_1m = Vec::new();
            let mut hour = format!("{:02}{:02}00", now_hhmm / 100, now_hhmm % 100);
            for _ in 0..5 {
                match kis::get_minute_chart(
                    http_client,
                    app_key,
                    app_secret,
                    access_token,
                    &symbol.code,
                    &today,
                    &hour,
                )
                .await
                {
                    Ok(chunk) => {
                        if chunk.is_empty() {
                            break;
                        }
                        let earliest = chunk
                            .iter()
                            .filter(|c| c.stck_bsop_date == today)
                            .map(|c| c.stck_cntg_hour.as_str())
                            .min()
                            .unwrap_or("")
                            .to_string();
                        for c in chunk.iter().filter(|c| c.stck_bsop_date == today) {
                            if c.stck_cntg_hour.len() < 4 {
                                continue;
                            }
                            t_1m.push(to_candle(c));
                        }
                        if earliest.is_empty() || earliest.as_str() <= "084500" {
                            break;
                        }
                        hour = subtract_one_minute(&earliest);
                    }
                    Err(e) => {
                        tracing::warn!("당일 1분봉 조회 실패({}): {}", symbol.code, e);
                        break;
                    }
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
