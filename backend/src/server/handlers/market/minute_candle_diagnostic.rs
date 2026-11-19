//! KIS 1분봉 과거 데이터 검증을 백그라운드에서 실행하고 로그 파일에 출력합니다.
use crate::{core::state::AppState, kis};
use axum::{Json, extract::State, http::StatusCode};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::Ordering,
    },
};
use chrono::{NaiveTime, Timelike};

const DAY_START: &str = "084500";
const MAX_PAGES: usize = 16;
const PAGE_DELAY_MS: u64 = 250;

struct RunningGuard(Arc<std::sync::atomic::AtomicBool>);

impl Drop for RunningGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

/// POST /api/admin/market/minute-candle-check
/// 응답은 즉시 반환하고, 실제 REST 조회와 로그 출력은 독립 Tokio task에서 수행합니다.
pub async fn start_minute_candle_check(
    State(state): State<AppState>,
) -> (StatusCode, Json<Value>) {
    if state
        .minute_candle_check_running
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return (
            StatusCode::CONFLICT,
            Json(json!({
                "ok": false,
                "running": true,
                "message": "분봉 검증 작업이 이미 실행 중입니다."
            })),
        );
    }

    let running = state.minute_candle_check_running.clone();
    tokio::spawn(async move {
        let _guard = RunningGuard(running);
        if let Err(error) = run_minute_candle_check(state).await {
            tracing::error!(target: "minute_candle_validation", "[MINUTE-CHECK] 검증 작업 실패: {}", error);
        }
    });

    (
        StatusCode::ACCEPTED,
        Json(json!({
            "ok": true,
            "running": true,
            "message": "분봉 검증을 백그라운드에서 시작했습니다. backend 로그 파일을 확인하세요."
        })),
    )
}

async fn run_minute_candle_check(state: AppState) -> Result<(), String> {
    let now = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
    let start = NaiveTime::parse_from_str(DAY_START, "%H%M%S")
        .map_err(|error| error.to_string())?;
    if now.time() < start {
        return Err("현재 시각이 08:45 이전이므로 오늘 주간 분봉을 조회하지 않습니다.".to_string());
    }

    let date = now.format("%Y%m%d").to_string();
    let end_time = now.format("%H%M%S").to_string();

    tracing::info!(
        target: "minute_candle_validation",
        "========== KIS 1분봉 검증 시작: date={}, range={}~{} KST ==========",
        date, DAY_START, end_time
    );

    let symbols = [&state.regular, &state.mini];
    for symbol in symbols {
        tracing::info!(
            target: "minute_candle_validation",
            "----- 종목 시작: {} ({}) -----",
            symbol.name, symbol.code
        );

        match fetch_day_candles(
            &state,
            &symbol.code,
            &date,
            &end_time,
        )
        .await
        {
            Ok(candles) if candles.is_empty() => {
                tracing::info!(
                    target: "minute_candle_validation",
                    "[MINUTE-CHECK] {} ({}) : 08:45~현재 조회 데이터 없음. 장 시작 지연 또는 미수신 가능성이 있으며 오류로 판정하지 않습니다.",
                    symbol.name, symbol.code
                );
            }
            Ok(candles) => {
                let first_time = candles.keys().next().cloned().unwrap_or_default();
                let last_time = candles.keys().next_back().cloned().unwrap_or_default();
                tracing::info!(
                    target: "minute_candle_validation",
                    "[MINUTE-CHECK] {} ({}) : 최초 데이터={}, 마지막 데이터={}, 총 {}개. 최초 데이터 시각을 실제 데이터 시작 시각으로 기록합니다.",
                    symbol.name, symbol.code, first_time, last_time, candles.len()
                );
                tracing::info!(
                    target: "minute_candle_validation",
                    "시간(KST) | 시가(Open) | 고가(High) | 저가(Low) | 종가(Close)"
                );

                for candle in candles.values() {
                    tracing::info!(
                        target: "minute_candle_validation",
                        "{} | {} | {} | {} | {}",
                        candle.stck_cntg_hour,
                        candle.futs_oprc,
                        candle.futs_hgpr,
                        candle.futs_lwpr,
                        candle.futs_prpr
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    target: "minute_candle_validation",
                    "[MINUTE-CHECK] {} ({}) 조회 실패: {}",
                    symbol.name, symbol.code, error
                );
            }
        }
    }

    tracing::info!(
        target: "minute_candle_validation",
        "========== KIS 1분봉 검증 종료: date={} ==========",
        date
    );
    Ok(())
}

async fn fetch_day_candles(
    state: &AppState,
    symbol: &str,
    date: &str,
    end_time: &str,
) -> Result<BTreeMap<String, kis::MinuteCandle>, String> {
    let mut result = BTreeMap::new();
    let mut cursor = end_time.to_string();

    for page in 0..MAX_PAGES {
        let candles = kis::get_minute_chart_for_market(
            &state.http_client,
            &state.kis.app_key,
            &state.kis.app_secret,
            &state.kis.access_token,
            "F",
            symbol,
            date,
            &cursor,
        )
        .await
        .map_err(|error| error.to_string())?;

        if candles.is_empty() {
            break;
        }

        let earliest = candles
            .iter()
            .filter(|candle| candle.stck_bsop_date == date)
            .map(|candle| candle.stck_cntg_hour.as_str())
            .min()
            .map(str::to_string);

        for candle in candles {
            if candle.stck_bsop_date == date
                && candle.stck_cntg_hour.as_str() >= DAY_START
                && candle.stck_cntg_hour.as_str() <= end_time
            {
                result.insert(candle.stck_cntg_hour.clone(), candle);
            }
        }

        let Some(earliest) = earliest else {
            break;
        };
        if earliest <= DAY_START {
            break;
        }

        let next_cursor = previous_second(&earliest)
            .ok_or_else(|| format!("잘못된 분봉 시간 형식: {}", earliest))?;
        if next_cursor >= cursor {
            tracing::warn!(
                target: "minute_candle_validation",
                "[MINUTE-CHECK] 페이지 커서 진행 중단: symbol={}, cursor={}, next={}",
                symbol, cursor, next_cursor
            );
            break;
        }

        cursor = next_cursor;
        if page + 1 < MAX_PAGES {
            tokio::time::sleep(std::time::Duration::from_millis(PAGE_DELAY_MS)).await;
        }
    }

    if result.len() > 0 {
        tracing::info!(
            target: "minute_candle_validation",
            "[MINUTE-CHECK] symbol={} 누적 조회 {}개 분봉",
            symbol, result.len()
        );
    }
    Ok(result)
}

fn previous_second(value: &str) -> Option<String> {
    let time = NaiveTime::parse_from_str(value, "%H%M%S").ok()?;
    let previous = time.checked_sub_signed(chrono::Duration::seconds(1))?;
    Some(format!(
        "{:02}{:02}{:02}",
        previous.hour(),
        previous.minute(),
        previous.second()
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn previous_second_moves_cursor_back_without_repeating_boundary() {
        assert_eq!(previous_second("140201").as_deref(), Some("140200"));
        assert_eq!(previous_second("084501").as_deref(), Some("084500"));
        assert_eq!(previous_second("084500").as_deref(), Some("084459"));
        assert!(previous_second("bad").is_none());
    }
}
