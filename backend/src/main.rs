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
mod market;
mod master;
mod server;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
    let client = redis::Client::open("redis://127.0.0.1:6379").map_err(|e| {
        tracing::error!("Redis 클라이언트 생성 실패: {}", e);
        e
    })?;
    let mut conn = client
        .get_multiplexed_async_connection()
        .await
        .map_err(|e| {
            tracing::error!("Redis 연결 실패: {}", e);
            e
        })?;
    let pong: String = redis::cmd("PING")
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            tracing::error!("Redis PING 실패: {}", e);
            e
        })?;
    tracing::info!("Redis PING → {}", pong);

    // HTTP 클라이언트
    let http_client = reqwest::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| {
            tracing::error!("HTTP 클라이언트 생성 실패: {}", e);
            e
        })?;

    // KIS 토큰
    let app_key = std::env::var("KIS_APP_KEY").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_KEY를 읽을 수 없습니다: {}", e);
        e
    })?;
    let app_secret = std::env::var("KIS_APP_SECRET").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_SECRET를 읽을 수 없습니다: {}", e);
        e
    })?;
    let cache_path = std::env::var("TOKEN_CACHE_FILE").unwrap_or("./.token".to_string());
    let token =
        match kis::get_or_issue_token(&http_client, &app_key, &app_secret, &cache_path).await {
            Ok(t) => {
                // 인증 토큰은 로그에 남기지 않습니다.
                tracing::info!("KIS access token 발급/로드 성공");
                t
            }
            Err(e) => {
                tracing::error!("KIS 토큰 초기화 실패: {}", e);
                return Err(
                    std::io::Error::new(std::io::ErrorKind::Other, "KIS 토큰 초기화 실패").into(),
                );
            }
        };

    // 마스터
    tracing::info!("마스터 파일 다운로드 중");
    let symbols = match master::download_master_text(&http_client).await {
        Ok(text) => master::parse_master(&text),
        Err(e) => {
            tracing::error!("마스터 다운로드 실패: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "마스터 파일 다운로드 실패",
            )
            .into());
        }
    };
    let regular = match master::pick_nearest(&symbols, "1") {
        Some(f) => {
            tracing::info!("정규선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("정규선물 최근월물 없음");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "정규선물 최근월물 없음",
            )
            .into());
        }
    };
    let mini = match master::pick_nearest(&symbols, "B") {
        Some(f) => {
            tracing::info!("미니선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("미니선물 최근월물 없음");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "미니선물 최근월물 없음",
            )
            .into());
        }
    };

    // 과거 및 당일 1분봉 초기 데이터 로드
    let candle_store = market::history::load_initial_candles(
        &http_client,
        &app_key,
        &app_secret,
        &token.access_token,
        &[regular.clone(), mini.clone()],
    )
    .await;

    // ★ CandleStore를 Arc<Mutex<>>로 감싸기 (WS와 공유)
    let candle_store = Arc::new(Mutex::new(candle_store));

    // ── Supabase 초기화 ──
    let supabase_url = std::env::var("SUPABASE_URL").map_err(|e| {
        tracing::error!("필수 환경변수 SUPABASE_URL을 읽을 수 없습니다: {}", e);
        e
    })?;
    let supabase_service_key = std::env::var("SUPABASE_SERVICE_ROLE_KEY").map_err(|e| {
        tracing::error!(
            "필수 환경변수 SUPABASE_SERVICE_ROLE_KEY를 읽을 수 없습니다: {}",
            e
        );
        e
    })?;
    let supabase_jwt_secret = std::env::var("SUPABASE_JWT_SECRET").map_err(|e| {
        tracing::error!(
            "필수 환경변수 SUPABASE_JWT_SECRET를 읽을 수 없습니다: {}",
            e
        );
        e
    })?;
    let supabase = core::db::SupabaseClient::new(
        supabase_url.clone(),
        supabase_service_key,
        http_client.clone(),
    );

    // 매일 KST 07:00에 30일 지난 데이터 삭제 (백그라운드)
    {
        let supabase_cleanup = supabase.clone();
        tokio::spawn(async move {
            use chrono::TimeZone;
            let kst = chrono_tz::Asia::Seoul;
            loop {
                let now = chrono::Utc::now().with_timezone(&kst);
                let Some(today_7am) = now.date_naive().and_hms_opt(7, 0, 0) else {
                    tracing::error!("KST 데이터 정리 기준 시각 계산 실패");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                };
                let Some(seven_am) = chrono::NaiveTime::from_hms_opt(7, 0, 0) else {
                    tracing::error!("KST 데이터 정리 시각 생성 실패");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                };
                let target_local = if now.time() < seven_am {
                    Some(today_7am)
                } else {
                    today_7am.checked_add_signed(chrono::Duration::days(1))
                };
                let Some(target_local) = target_local else {
                    tracing::error!("다음 데이터 정리 날짜 계산 실패");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
                };
                let Some(next_7am) = kst.from_local_datetime(&target_local).single() else {
                    tracing::error!("다음 KST 데이터 정리 시각 변환 실패");
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                    continue;
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
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "KIS WebSocket approval key 발급 실패",
            )
            .into());
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

        // HTTP 서버 실행 (라우터, CORS, listener)
    server::runtime::serve(state).await?;

    Ok(())
}
