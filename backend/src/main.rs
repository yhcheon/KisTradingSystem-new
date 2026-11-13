
// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 로깅 초기화 (logs/yyyy-mm-dd.log)
//       - Redis 연결 확인
//       - KIS 토큰 발급(캐시 우선)
//       - 종목 마스터 다운로드 및 파싱
//       - AppState 구성 후 Axum 서버 시작
// 작성일: 2026-11-14
// ============================================================

mod cache;
mod core;
mod kis;
mod logging;
mod master;
mod server;

use std::sync::Arc;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() {
    let _log_guard = logging::init();
    dotenvy::dotenv().ok();
    tracing::info!("=== 백엔드 시작 ===");

    // ── Redis 연결 ──
    let client = redis::Client::open("redis://127.0.0.1:6379").unwrap();
    let mut conn = client.get_multiplexed_async_connection().await.unwrap();
    let pong: String = redis::cmd("PING").query_async(&mut conn).await.unwrap();
    tracing::info!("Redis PING → {}", pong);

    // ── KIS 토큰 ──
    let app_key = std::env::var("KIS_APP_KEY").expect("KIS_APP_KEY missing");
    let app_secret = std::env::var("KIS_APP_SECRET").expect("KIS_APP_SECRET missing");
    let cache_path = std::env::var("TOKEN_CACHE_FILE").unwrap_or("./.token".to_string());
    let token = match kis::get_or_issue_token(&app_key, &app_secret, &cache_path).await {
        Ok(t) => {
            tracing::info!("KIS token: {}...", &t.access_token[..20.min(t.access_token.len())]);
            t
        }
        Err(e) => {
            tracing::error!("KIS token failed: {}", e);
            return;
        }
    };

    // ── 마스터 다운로드 + 파싱 ──
    tracing::info!("마스터 파일 다운로드 중");
    let symbols = match master::download_master_text().await {
        Ok(text) => master::parse_master(&text),
        Err(e) => {
            tracing::error!("마스터 다운로드 실패: {}", e);
            return;
        }
    };
    let regular = match master::pick_nearest(&symbols, "1") {
        Some(f) => { tracing::info!("정규선물 최근월물: {} ({})", f.name, f.code); f }
        None => { tracing::error!("정규선물 최근월물 없음"); return; }
    };
    let mini = match master::pick_nearest(&symbols, "B") {
        Some(f) => { tracing::info!("미니선물 최근월물: {} ({})", f.name, f.code); f }
        None => { tracing::error!("미니선물 최근월물 없음"); return; }
    };

    // ── AppState 구성 ──
    let state = core::state::AppState {
        app_key,
        app_secret,
        access_token: token.access_token,
        redis: Arc::new(Mutex::new(conn)),
        regular,
        mini,
    };

    // ── 서버 시작 ──
    let app = server::router::build(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    tracing::info!("backend listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}


