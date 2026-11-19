// ============================================================
// 파일명: core/bootstrap.rs
// 설명: 외부 서비스 연결 및 애플리케이션 공유 상태 초기화
//       - Redis 없음, 주간/야간 분리 메모리 PriceCache 사용
//       - HTTP 클라이언트, KIS 시장 데이터
//       - Supabase, KIS WebSocket, AppState 구성
// ============================================================

use crate::{core, kis, market};
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::core::state::{AppState, JwtConfig, KisCredentials};

/// 외부 의존성을 초기화하고 HTTP 서버에서 공유할 AppState를 구성합니다.
pub async fn initialize() -> Result<AppState, Box<dyn std::error::Error>> {
    // Chat configuration is validated before connecting to external services.
    let chat = crate::chat::ChatService::load_from_env()?;
    chat.spawn_cleanup_task();

    // 주간/야간 세션별로 독립된 메모리 시세 캐시를 사용합니다.
    let price_cache = crate::cache::price_cache::PriceCache::new();

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

    // 주간/야간 가격은 각각 최초 시세 요청 시 REST로 초기화하고,
    // 이후 해당 세션의 WebSocket 체결로만 갱신합니다.
    let night_symbols = vec![regular.code.clone(), mini.code.clone()];

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

    // KIS WebSocket (CandleStore + PriceCache 공유)
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
    // 야간 분봉 로더는 별도로 동작하며, WebSocket 구독은 KST 시간표에 따라 관리합니다.
    market::history::spawn_night_candle_loader(
        http_client.clone(),
        app_key.clone(),
        app_secret.clone(),
        access_token.clone(),
        night_symbols.clone(),
        candle_store.clone(),
    );

    let (ws_tx, _) = tokio::sync::broadcast::channel::<kis::ws::RealtimeTick>(256);
    let ws_symbols = vec![regular.code.clone(), mini.code.clone()];
    {
        let tx = ws_tx.clone();
        let key = approval_key;
        let store = candle_store.clone();
        let ws_session_range = session_range.clone();
        let ws_price_cache = price_cache.clone();
        let ws_http_client = http_client.clone();
        let ws_app_key = app_key.clone();
        let ws_app_secret = app_secret.clone();
        let ws_access_token = access_token.clone();
        tokio::spawn(async move {
            kis::ws::run(
                key,
                ws_symbols,
                night_symbols,
                tx,
                store,
                ws_session_range,
                ws_price_cache,
                ws_http_client,
                ws_app_key,
                ws_app_secret,
                ws_access_token,
            )
            .await;
        });
    }
    tracing::info!("KIS WebSocket 백그라운드 시작 (KST 시간표 기반 주간/야간 구독)");

    Ok(AppState {
        kis: KisCredentials {
            app_key,
            app_secret,
            access_token,
        },
        jwt: JwtConfig {
            legacy_hs256_secret: supabase_jwt_secret,
        },
        price_cache,
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



