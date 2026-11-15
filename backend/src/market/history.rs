// ============================================================
// 파일명: market/history.rs
// 설명: KIS 과거 1분봉을 조회해 CandleStore 초기 데이터 구성
// ============================================================

use crate::core::candles::{Candle, CandleStore};
use crate::kis;
use crate::master::FuturesSymbol;

/// 직전 거래일과 장중 당일의 1분봉을 로드합니다.
pub async fn load_initial_candles(
    http_client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    access_token: &str,
    symbols: &[FuturesSymbol],
) -> CandleStore {
// ── 전일 + 당일 1분봉 로드 ──
let mut candle_store = CandleStore::new();
// 서버 OS 시간대와 무관하게 한국 거래소 기준(KST) 날짜/시간 사용
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
let is_market_hours = is_weekday && now_hhmm >= 845 && now_hhmm <= 1545;

for symbol in symbols {
    // 달력상 어제부터 과거로 최대 14일 조회해 실제 분봉이 존재하는
    // 가장 최근 영업일을 찾습니다. 주말/공휴일 목록을 직접 관리하지 않습니다.
    let mut previous_trade_date: Option<String> = None;
    let mut first_chunk = Vec::new();

    for days_back in 1..=14 {
        let candidate_date = (today_dt.date_naive()
            - chrono::Duration::days(days_back))
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
                // 조회 기준일이 휴장일이어도 응답의 실제 영업일자를 확인합니다.
                let latest_date = chunk
                    .iter()
                    .map(|c| c.stck_bsop_date.as_str())
                    .filter(|date| *date < today.as_str())
                    .max()
                    .map(str::to_string);

                if let Some(date) = latest_date {
                    previous_trade_date = Some(date);
                    first_chunk = chunk;
                    break;
                }
            }
            Err(e) => {
                tracing::warn!(
                    "직전 거래일 탐색 중 1분봉 조회 실패({} / {}): {}",
                    symbol.code,
                    candidate_date,
                    e
                );
                // 통신/API 오류를 휴장일로 오인해 과거 날짜를 계속 조회하지 않습니다.
                break;
            }
        }

        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    let Some(previous_trade_date) = previous_trade_date else {
        tracing::warn!(
            "직전 14일 내 거래일 1분봉을 찾지 못했습니다: {}",
            symbol.code
        );
        continue;
    };

    tracing::info!(
        "직전 거래일({}) 1분봉 로드 시작: {}",
        previous_trade_date,
        symbol.code
    );

    let mut y_1m = Vec::new();
    let mut hour = "154500".to_string();

    // 첫 조회 결과를 사용하고, 이후에는 실제 영업일자/가장 이른 시각을
    // 다음 조회 커서로 사용해 동일 거래일의 과거 분봉을 이어서 가져옵니다.
    for page in 0..5 {
        let chunk = if page == 0 {
            std::mem::take(&mut first_chunk)
        } else {
            match kis::get_minute_chart(
                http_client,
                app_key,
                app_secret,
                access_token,
                &symbol.code,
                &previous_trade_date,
                &hour,
            )
            .await
            {
                Ok(chunk) => chunk,
                Err(e) => {
                    tracing::warn!(
                        "직전 거래일 추가 1분봉 조회 실패({} / {}): {}",
                        symbol.code,
                        hour,
                        e
                    );
                    break;
                }
            }
        };

        let mut earliest = String::new();
        for c in chunk
            .iter()
            .filter(|c| c.stck_bsop_date == previous_trade_date)
        {
            if c.stck_cntg_hour.len() < 6 {
                continue;
            }
            if earliest.is_empty() || c.stck_cntg_hour.as_str() < earliest.as_str() {
                earliest = c.stck_cntg_hour.clone();
            }

            let hhmm = c.stck_cntg_hour[0..4].to_string();
            y_1m.push(Candle {
                time: hhmm,
                open: c.futs_oprc.parse().unwrap_or(0.0),
                high: c.futs_hgpr.parse().unwrap_or(0.0),
                low: c.futs_lwpr.parse().unwrap_or(0.0),
                close: c.futs_prpr.parse().unwrap_or(0.0),
                volume: c.cntg_vol.parse().unwrap_or(0),
            });
        }

        if earliest.is_empty() || earliest.as_str() <= "084500" {
            break;
        }
        hour = subtract_one_minute(&earliest);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    y_1m.sort_by(|a, b| a.time.cmp(&b.time));
    y_1m.dedup_by(|a, b| a.time == b.time);
    tracing::info!(
        "직전 거래일 1분봉 로드 완료: {} {}건",
        previous_trade_date,
        y_1m.len()
    );
    candle_store.load_1m(&symbol.code, &previous_trade_date, y_1m);

    // 당일 (장중일 때만)
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
                        let hhmm = if c.stck_cntg_hour.len() >= 4 {
                            c.stck_cntg_hour[0..4].to_string()
                        } else {
                            continue;
                        };
                        t_1m.push(Candle {
                            time: hhmm,
                            open: c.futs_oprc.parse().unwrap_or(0.0),
                            high: c.futs_hgpr.parse().unwrap_or(0.0),
                            low: c.futs_lwpr.parse().unwrap_or(0.0),
                            close: c.futs_prpr.parse().unwrap_or(0.0),
                            volume: c.cntg_vol.parse().unwrap_or(0),
                        });
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

/// "HHMMSS"에서 1분 전 계산
fn subtract_one_minute(hhmmss: &str) -> String {
    if hhmmss.len() < 6 {
        return "154500".to_string();
    }
    let h: u32 = hhmmss[0..2].parse().unwrap_or(15);
    let m: u32 = hhmmss[2..4].parse().unwrap_or(45);
    let s: u32 = hhmmss[4..6].parse().unwrap_or(0);
    let total = h * 3600 + m * 60 + s;
    let new_total = if total >= 60 { total - 60 } else { 0 };
    format!(
        "{:02}{:02}{:02}",
        new_total / 3600,
        (new_total % 3600) / 60,
        new_total % 60
    )
}
