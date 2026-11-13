
// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 로깅 초기화 (logs/yyyy-mm-dd.log)
//       - Redis 연결 확인
//       - KIS 토큰 발급(캐시 우선)
//       - 종목 마스터 다운로드 및 파싱
//       - 선물 시세 조회 (Redis 캐시 우선)
//       - Axum 웹 서버 시작
// 작성일: 2026-11-14
// ============================================================

mod cache;
mod kis;
mod logging;
mod master;

use axum::{routing::get, Router};

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
    tracing::info!("총 {}개 선물 종목 파싱됨", symbols.len());

    // ── 시세 조회 (캐시 우선) ──
    if let Some(f) = master::pick_nearest(&symbols, "1") {
        tracing::info!("정규선물 시세 조회 (캐시 우선): {} ({})", f.name, f.code);

        // 1) 캐시 확인
        match cache::redis_cache::get(&mut conn, &f.code).await {
            Ok(Some(cached)) => {
                tracing::info!("[CACHE HIT] {} 현재가: {}", cached.name, cached.price);
            }
            Ok(None) => {
                tracing::info!("[CACHE MISS] KIS API 호출");
                match kis::get_futures_price(&app_key, &app_secret, &token.access_token, &f.code).await {
                    Ok(price) => {
                        tracing::info!("결과코드: {}", price.rt_cd);
                        tracing::info!("현재가: {}", price.output1.futs_prpr);
                        tracing::info!("전일대비: {} ({}%)", price.output1.futs_prdy_vrss, price.output1.futs_prdy_ctrt);
                        tracing::info!("거래량: {}", price.output1.acml_vol);

                        // 2) Redis에 캐시 저장 (TTL 3초)
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_secs();
                        let cached = cache::redis_cache::CachedPrice {
                            code: f.code.clone(),
                            name: price.output1.hts_kor_isnm.clone(),
                            price: price.output1.futs_prpr.clone(),
                            change: price.output1.futs_prdy_vrss.clone(),
                            change_rate: price.output1.futs_prdy_ctrt.clone(),
                            volume: price.output1.acml_vol.clone(),
                            cached_at: now,
                        };
                        if let Err(e) = cache::redis_cache::set(&mut conn, &cached, 3).await {
                            tracing::warn!("캐시 저장 실패: {}", e);
                        } else {
                            tracing::info!("[CACHE SET] {} (TTL 3초)", f.code);
                        }
                    }
                    Err(e) => tracing::error!("시세 조회 실패: {}", e),
                }
            }
            Err(e) => tracing::error!("캐시 조회 실패: {}", e),
        }
    }

    // ── 서버 시작 ──
    let app = Router::new().route("/", get(|| async { "backend ok" }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    tracing::info!("backend listening on http://127.0.0.1:8080");
    axum::serve(listener, app).await.unwrap();
}


