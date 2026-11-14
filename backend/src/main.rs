// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 로깅, Redis, KIS 토큰, 마스터, KIS WebSocket
//       - 전일 1분봉 로드 → CandleStore 파생 계산
//       - AppState 구성 후 Axum 서버 시작
// 작성일: 2026-11-14
// ============================================================

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
    tracing::info!("🚀 백엔드 시작  ({} 실행)", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"));
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
    let token = match kis::get_or_issue_token(&http_client, &app_key, &app_secret, &cache_path).await {
        Ok(t) => { tracing::info!("KIS token: {}...", &t.access_token[..20.min(t.access_token.len())]); t }
        Err(e) => { tracing::error!("KIS token failed: {}", e); return; }
    };

    // 마스터
    tracing::info!("마스터 파일 다운로드 중");
    let symbols = match master::download_master_text(&http_client).await {
        Ok(text) => master::parse_master(&text),
        Err(e) => { tracing::error!("마스터 다운로드 실패: {}", e); return; }
    };
    let regular = match master::pick_nearest(&symbols, "1") {
        Some(f) => { tracing::info!("정규선물 최근월물: {} ({})", f.name, f.code); f }
        None => { tracing::error!("정규선물 최근월물 없음"); return; }
    };
    let mini = match master::pick_nearest(&symbols, "B") {
        Some(f) => { tracing::info!("미니선물 최근월물: {} ({})", f.name, f.code); f }
        None => { tracing::error!("미니선물 최근월물 없음"); return; }
    };

    // ── 전일 1분봉 로드 → CandleStore 파생 계산 ──
    let mut candle_store = core::candles::CandleStore::new();
    let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
        .format("%Y%m%d").to_string();
    tracing::info!("전일({}) 1분봉 로드 시작", yesterday);

    for symbol in [&regular, &mini] {
        let mut all_1m = Vec::new();
        let mut hour = "154500".to_string();
        for _ in 0..5 {
            match kis::get_minute_chart(
                &http_client, &app_key, &app_secret, &token.access_token,
                &symbol.code, &yesterday, &hour,
            ).await {
                Ok(chunk) => {
                    if chunk.is_empty() { break; }
                    let earliest = chunk.first().map(|c| c.stck_cntg_hour.clone()).unwrap_or_default();
                    if earliest.is_empty() { break; }
                    // MinuteCandle → Candle 변환
                    for c in &chunk {
                        let hhmm = if c.stck_cntg_hour.len() >= 4 {
                            c.stck_cntg_hour[0..4].to_string()
                        } else { continue; };
                        all_1m.push(core::candles::Candle {
                            time: hhmm,
                            open: c.futs_oprc.parse().unwrap_or(0.0),
                            high: c.futs_hgpr.parse().unwrap_or(0.0),
                            low: c.futs_lwpr.parse().unwrap_or(0.0),
                            close: c.futs_prpr.parse().unwrap_or(0.0),
                            volume: c.cntg_vol.parse().unwrap_or(0),
                        });
                    }
                    hour = subtract_one_minute(&earliest);
                }
                Err(e) => { tracing::warn!("1분봉 조회 실패({}): {}", symbol.code, e); break; }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        // 중복 제거 + 오름차순 정렬
        all_1m.sort_by(|a, b| a.time.cmp(&b.time));
        all_1m.dedup_by(|a, b| a.time == b.time);

        candle_store.load_1m(&symbol.code, all_1m);
    }

    // KIS WebSocket
    let approval_key = match kis::get_approval_key(&http_client, &app_key, &app_secret).await {
        Ok(k) => { tracing::info!("approval_key: {}...", &k[..20.min(k.len())]); k }
        Err(e) => { tracing::error!("approval_key 발급 실패: {}", e); return; }
    };
    let (ws_tx, _) = tokio::sync::broadcast::channel::<kis_ws::RealtimeTick>(256);
    let ws_symbols = vec![regular.code.clone(), mini.code.clone()];
    {
        let tx = ws_tx.clone();
        let key = approval_key.clone();
        tokio::spawn(async move { kis_ws::run(key, ws_symbols, tx).await; });
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
        candles: Arc::new(Mutex::new(candle_store)),
    };

    // 서버
    let app = server::router::build(state);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    tracing::info!("backend listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}

/// "HHMMSS"에서 1분 전 계산
fn subtract_one_minute(hhmmss: &str) -> String {
    if hhmmss.len() < 6 { return "154500".to_string(); }
    let h: u32 = hhmmss[0..2].parse().unwrap_or(15);
    let m: u32 = hhmmss[2..4].parse().unwrap_or(45);
    let s: u32 = hhmmss[4..6].parse().unwrap_or(0);
    let total = h * 3600 + m * 60 + s;
    let new_total = if total >= 60 { total - 60 } else { 0 };
    format!("{:02}{:02}{:02}", new_total / 3600, (new_total % 3600) / 60, new_total % 60)
}


