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

    // KIS 인증 환경변수
    let app_key = std::env::var("KIS_APP_KEY").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_KEY를 읽을 수 없습니다: {}", e);
        e
    })?;
    let app_secret = std::env::var("KIS_APP_SECRET").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_SECRET를 읽을 수 없습니다: {}", e);
        e
    })?;
    let cache_path = std::env::var("TOKEN_CACHE_FILE").unwrap_or("./.token".to_string());

    // KIS 시장 데이터 초기화 (토큰, 선물 마스터, 초기 캔들)
    let market_data =
        core::market_data::initialize(&http_client, &app_key, &app_secret, &cache_path).await?;
    let access_token = market_data.access_token;
    let regular = market_data.regular;
    let mini = market_data.mini;
    let candle_store = market_data.candles;

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
    core::cleanup::spawn_daily_cleanup(supabase.clone());

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
        access_token,
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
