// ============================================================
// 파일명: core/bootstrap.rs
// 설명: 외부 서비스 연결 및 애플리케이션 공유 상태 초기화
//       - Redis, HTTP 클라이언트, KIS 시장 데이터
//       - Supabase, KIS WebSocket, AppState 구성
// ============================================================

use crate::{core, kis, kis_ws, market};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::state::{AppState, JwtConfig, KisCredentials};

/// 외부 의존성을 초기화하고 HTTP 서버에서 공유할 AppState를 구성합니다.
pub async fn initialize() -> Result<AppState, Box<dyn std::error::Error>> {
    // Chat configuration is validated before connecting to external services.
    let chat = crate::chat::ChatService::load_from_env()?;
    chat.spawn_cleanup_task();

    // Redis
    let redis_url =
        std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string());
    let client = redis::Client::open(redis_url).map_err(|e| {
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
    // KIS 실시간 구독을 시작하는 시각을 기준으로 세션별 기준 수집 자격을 고정합니다.
    let session_range = Arc::new(Mutex::new(
        core::session_range::SessionRangeMonitor::new(regular.code.clone()),
    ));

    // 야간선물은 별도 고정 코드를 쓰지 않고, 선물 마스터에서 선택한 정규/미니 최근월물 코드를 사용합니다.
    // REST는 CM, WebSocket은 H0MFCNT0로 야간 시장을 구분합니다.
    let night_symbols = vec![regular.code.clone(), mini.code.clone()];
    for night_symbol in &night_symbols {
        tracing::info!(
            "야간선물 REST 검증 시작: market=CM, code={} (선물 마스터 선택 결과)",
            night_symbol
        );
        match kis::get_futures_price_for_market(
            &http_client,
            &app_key,
            &app_secret,
            &access_token,
            "CM",
            night_symbol,
        )
        .await
        {
            Ok(price) => tracing::info!(
                "야간선물 REST 조회 성공: code={}, name={}, price={}, change={}, change_rate={}, volume={}",
                night_symbol,
                price.output1.hts_kor_isnm,
                price.output1.futs_prpr,
                price.output1.futs_prdy_vrss,
                price.output1.futs_prdy_ctrt,
                price.output1.acml_vol
            ),
            Err(error) => tracing::warn!(
                "야간선물 REST 조회 실패: market=CM, code={}, error={}",
                night_symbol,
                error
            ),
        }
    }

    // Supabase 초기화
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

    // KIS WebSocket (CandleStore 공유)
    let approval_key = match kis::get_approval_key(&http_client, &app_key, &app_secret).await {
        Ok(key) => {
            // WebSocket approval key는 인증정보이므로 로그에 남기지 않습니다.
            tracing::info!("KIS WebSocket approval key 발급 성공");
            key
        }
        Err(error) => {
            tracing::error!("approval_key 발급 실패: {}", error);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "KIS WebSocket approval key 발급 실패",
            )
            .into());
        }
    };
    // 야간 분봉은 18시 전에는 조회하지 않고, 야간 세션 중 재시작하면 현재 시각부터 조회합니다.
    // H0MFCNT0 구독은 아래 WebSocket 연결에서 장 시작 전부터 미리 신청합니다.
    market::history::spawn_night_candle_loader(
        http_client.clone(),
        app_key.clone(),
        app_secret.clone(),
        access_token.clone(),
        night_symbols.clone(),
        candle_store.clone(),
    );

    let (ws_tx, _) = tokio::sync::broadcast::channel::<kis_ws::RealtimeTick>(256);
    let ws_symbols = vec![regular.code.clone(), mini.code.clone()];
    {
        let tx = ws_tx.clone();
        let key = approval_key;
        let store = candle_store.clone();
        let ws_session_range = session_range.clone();
        tokio::spawn(async move {
            kis_ws::run(key, ws_symbols, night_symbols, tx, store, ws_session_range).await;
        });
    }
    tracing::info!("KIS WebSocket 백그라운드 시작 (주간/야간 정규선물 + 미니선물)");

    Ok(AppState {
        kis: KisCredentials {
            app_key,
            app_secret,
            access_token,
        },
        jwt: JwtConfig {
            legacy_hs256_secret: supabase_jwt_secret,
        },
        redis: Arc::new(Mutex::new(conn)),
        regular,
        mini,
        ws_tx,
        http_client,
        candles: candle_store,
        session_range,
        supabase,
        chat,
    })
}
