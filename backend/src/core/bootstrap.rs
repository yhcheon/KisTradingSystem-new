// ============================================================
// 파일명: core/bootstrap.rs  (PATCHED)
// 변경점: initialize(shutdown: Arc<Notify>)
//  - chat.spawn_cleanup_task(shutdown.clone())
//  - cleanup::spawn_daily_cleanup(supabase.clone(), shutdown.clone())
//  - history::spawn_night_candle_loader(..., shutdown.clone())
//  - kis::ws::run(..., shutdown.clone())
//  - AppState { ..., shutdown }
// ============================================================



// ============================================================
// 파일명: core/bootstrap.rs — JoinHandle 수집 버전
// 변경점:
//  - initialize()가 (AppState, Vec<JoinHandle<()>>) 를 반환합니다.
//    AppState는 그대로, 백그라운드 태스크 핸들은 따로 모아서 runtime에 넘깁니다.
//    AppState에 JoinHandle을 넣지 않는 이유: AppState는 Clone인데 JoinHandle은 Clone이 안 됩니다.
//  - chat / cleanup / night_loader / KIS WS 4개의 spawn 반환값을 모두 수집합니다.
// ============================================================
use crate::{core, kis, market};
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::{Mutex, Notify};
use tokio::task::JoinHandle;
use crate::core::state::{AppState, JwtConfig, KisCredentials};

pub async fn initialize(shutdown: Arc<Notify>) -> Result<(AppState, Vec<JoinHandle<()>>), Box<dyn std::error::Error>> {
    let mut background: Vec<JoinHandle<()>> = Vec::new();

    let chat = crate::chat::ChatService::load_from_env()?;
    background.push(chat.spawn_cleanup_task(shutdown.clone())); // JoinHandle 수집

    let price_cache = crate::cache::price_cache::PriceCache::new();
    let minute_candle_check_running = Arc::new(AtomicBool::new(false));
    let http_client = reqwest::Client::builder()
        .pool_idle_timeout(std::time::Duration::from_secs(90))
        .build()
        .map_err(|e| { tracing::error!("HTTP 클라이언트 생성 실패: {}", e); e })?;
    let app_key = std::env::var("KIS_APP_KEY").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_KEY를 읽을 수 없습니다: {}", e); e
    })?;
    let app_secret = std::env::var("KIS_APP_SECRET").map_err(|e| {
        tracing::error!("필수 환경변수 KIS_APP_SECRET를 읽을 수 없습니다: {}", e); e
    })?;
    let cache_path = std::env::var("TOKEN_CACHE_FILE").unwrap_or("./.token".to_string());
    let market_data = core::market_data::initialize(&http_client, &app_key, &app_secret, &cache_path).await?;
    let access_token = market_data.access_token;
    let regular = market_data.regular;
    let mini = market_data.mini;
    let candle_store = market_data.candles;
    let session_range = Arc::new(Mutex::new(core::session_range::SessionRangeMonitor::new(regular.code.clone())));
    let night_symbols = vec![regular.code.clone(), mini.code.clone()];
    let supabase_url = std::env::var("SUPABASE_URL").map_err(|e| {
        tracing::error!("필수 환경변수 SUPABASE_URL를 읽을 수 없습니다: {}", e); e
    })?;
    let supabase_service_key = std::env::var("SUPABASE_SERVICE_ROLE_KEY").map_err(|e| {
        tracing::error!("필수 환경변수 SUPABASE_SERVICE_ROLE_KEY를 읽을 수 없습니다: {}", e); e
    })?;
    let supabase_jwt_secret = std::env::var("SUPABASE_JWT_SECRET").map_err(|e| {
        tracing::error!("필수 환경변수 SUPABASE_JWT_SECRET를 읽을 수 없습니다: {}", e); e
    })?;
    let supabase = core::db::SupabaseClient::new(supabase_url.clone(), supabase_service_key, http_client.clone());
    background.push(core::cleanup::spawn_daily_cleanup(supabase.clone(), shutdown.clone()));

    let approval_key = match kis::get_approval_key(&http_client, &app_key, &app_secret).await {
        Ok(key) => { tracing::info!("KIS WebSocket approval key 발급 성공"); key }
        Err(error) => {
            tracing::error!("approval_key 발급 실패: {}", error);
            return Err(std::io::Error::new(std::io::ErrorKind::Other, "KIS WebSocket approval key 발급 실패").into());
        }
    };
    background.push(market::history::spawn_night_candle_loader(
        http_client.clone(), app_key.clone(), app_secret.clone(), access_token.clone(),
        night_symbols.clone(), candle_store.clone(), shutdown.clone(),
    ));

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
        let ws_shutdown = shutdown.clone();
        // KIS WS도 JoinHandle 수집 — 이제 spawn하고 버리지 않습니다.
        let ws_handle = tokio::spawn(async move {
            kis::ws::run(key, ws_symbols, night_symbols, tx, store, ws_session_range, ws_price_cache,
                ws_http_client, ws_app_key, ws_app_secret, ws_access_token, ws_shutdown).await;
        });
        background.push(ws_handle);
    }
    tracing::info!("KIS WebSocket 백그라운드 시작 (KST 시간표 기반 주간/야간 구독)");

    let state = AppState {
        kis: KisCredentials { app_key, app_secret, access_token },
        jwt: JwtConfig { legacy_hs256_secret: supabase_jwt_secret },
        price_cache, regular, mini, ws_tx, http_client, candles: candle_store,
        session_range, minute_candle_check_running, supabase, chat,
        shutdown,
    };
    Ok((state, background))
}





