// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 로깅, Redis, KIS 토큰, 마스터, KIS WebSocket
//       - 전일 1분봉 로드 → CandleStore 파생 계산
//       - CandleStore를 Arc<Mutex<>>로 감싸 WS on_tick과 공유
//       - AppState 구성 후 Axum 서버 시작
// 작성일: 2026-11-14
// ============================================================

mod auth;
mod cache;
mod core;
mod kis;
mod kis_ws;
mod logging;
mod master;
mod server;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let _log_guard = logging::init();
    dotenvy::dotenv().ok();

    tracing::info!("============================================================");
    tracing::info!("============================================================");
    tracing::info!(
        "🚀 백엔드 시작  ({} 실행)",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    tracing::info!("============================================================");
    tracing::info!("============================================================");

    // Redis
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let pong: String = redis::cmd("PING").query_async(&mut conn).await.unwrap();
    tracing::info!("Redis PING → {}", pong);

    // HTTP 클라이언트
    let http_client = reqwest::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .unwrap();

    // KIS 토큰
    let app_key = std::env::var("KIS_APP_KEY").expect("KIS_APP_KEY missing");
    let app_secret = std::env::var("KIS_APP_SECRET").expect("KIS_APP_SECRET missing");
    let cache_path = std::env::var("TOKEN_CACHE_FILE").unwrap_or("./.token".to_string());
    let token =
        match kis::get_or_issue_token(&http_client, &app_key, &app_secret, &cache_path).await {
            Ok(t) => {
                // 인증 토큰은 로그에 남기지 않습니다.
                tracing::info!("KIS access token 발급/로드 성공");
                t
            }
            Err(e) => {
                tracing::error!("KIS token failed: {}", e);
                return;
            }
        };

    // 마스터
    tracing::info!("마스터 파일 다운로드 중");
    let symbols = match master::download_master_text(&http_client).await {
        Ok(text) => master::parse_master(&text),
        Err(e) => {
            tracing::error!("마스터 다운로드 실패: {}", e);
            return;
        }
    };
    let regular = match master::pick_nearest(&symbols, "1") {
        Some(f) => {
            tracing::info!("정규선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("정규선물 최근월물 없음");
            return;
        }
    };
    let mini = match master::pick_nearest(&symbols, "B") {
        Some(f) => {
            tracing::info!("미니선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("미니선물 최근월물 없음");
            return;
        }
    };

    // ── 전일 + 당일 1분봉 로드 ──
    let mut candle_store = core::candles::CandleStore::new();
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

    for symbol in [&regular, &mini] {
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
                &http_client,
                &app_key,
                &app_secret,
                &token.access_token,
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
                    &http_client,
                    &app_key,
                    &app_secret,
                    &token.access_token,
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
                y_1m.push(core::candles::Candle {
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
                    &http_client,
                    &app_key,
                    &app_secret,
                    &token.access_token,
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
                            t_1m.push(core::candles::Candle {
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

    // ★ CandleStore를 Arc<Mutex<>>로 감싸기 (WS와 공유)
    let candle_store = Arc::new(Mutex::new(candle_store));

    // ── Supabase 초기화 ──
    let supabase_url = std::env::var("SUPABASE_URL").expect("SUPABASE_URL missing");
    let supabase_service_key =
        std::env::var("SUPABASE_SERVICE_ROLE_KEY").expect("SUPABASE_SERVICE_ROLE_KEY missing");
    let supabase_jwt_secret =
        std::env::var("SUPABASE_JWT_SECRET").expect("SUPABASE_JWT_SECRET missing");
    let supabase =
        core::db::SupabaseClient::new(supabase_url.clone(), supabase_service_key, http_client.clone());

    // 매일 KST 07:00에 30일 지난 데이터 삭제 (백그라운드)
    {
        let supabase_cleanup = supabase.clone();
        tokio::spawn(async move {
            use chrono::TimeZone;
            let kst = chrono_tz::Asia::Seoul;
            loop {
                let now = chrono::Utc::now().with_timezone(&kst);
                let today_7am = now.date_naive().and_hms_opt(7, 0, 0).unwrap();
                let next_7am = if now.time() < chrono::NaiveTime::from_hms_opt(7, 0, 0).unwrap() {
                    kst.from_local_datetime(&today_7am).unwrap()
                } else {
                    kst.from_local_datetime(&(today_7am + chrono::Duration::days(1)))
                        .unwrap()
                };
                let wait = (next_7am - now)
                    .to_std()
                    .unwrap_or(std::time::Duration::from_secs(60));

                tracing::info!(
                    "다음 데이터 삭제 예정: {} ({}초 후)",
                    next_7am,
                    wait.as_secs()
                );
                tokio::time::sleep(wait).await;

                if let Err(e) = supabase_cleanup.cleanup_old_data().await {
                    tracing::warn!("오래된 데이터 삭제 실패: {}", e);
                } else {
                    tracing::info!("30일 지난 데이터 삭제 완료");
                }
            }
        });
    }

    // KIS WebSocket (candle_store 전달)
    let approval_key = match kis::get_approval_key(&http_client, &app_key, &app_secret).await {
        Ok(k) => {
            // WebSocket approval key는 인증정보이므로 로그에 남기지 않습니다.
            tracing::info!("KIS WebSocket approval key 발급 성공");
            k
        }
        Err(e) => {
            tracing::error!("approval_key 발급 실패: {}", e);
            return;
        }
    };
    let (ws_tx, _) = tokio::sync::broadcast::channel::<kis_ws::RealtimeTick>(256);
    let ws_symbols = vec![regular.code.clone(), mini.code.clone()];
    {
        let tx = ws_tx.clone();
        let key = approval_key.clone();
        let store = candle_store.clone(); // ★ 공유
        tokio::spawn(async move {
            kis_ws::run(key, ws_symbols, tx, store).await; // ★ store 전달
        });
    }
    tracing::info!("KIS WebSocket 백그라운드 시작");

    // AppState
    let state = core::state::AppState {
        app_key,
        app_secret,
        access_token: token.access_token,
        redis: Arc::new(Mutex::new(conn)),
        regular,
        mini,
        ws_tx,
        http_client,
        candles: candle_store, // ★ Arc<Mutex<>> 그대로
        supabase,
        jwt_secret: supabase_jwt_secret,
        supabase_url: supabase_url.clone(),
    };


        // ── CORS 설정 ──
    use tower_http::cors::{Any, CorsLayer};
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);



    // 서버    // 서버 (CORS 적용)
    let app = server::router::build(state).layer(cors);



    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    tracing::info!("backend listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
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
