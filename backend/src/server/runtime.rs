// ============================================================
// 파일명: server/runtime.rs
// 설명: Axum 서버 실행 및 CORS 설정
//       - TCP Listener를 초기화보다 먼저 열어 연결 거부 방지
//       - 초기화 중 /api/health 응답 제공
//       - Graceful shutdown: Ctrl+C / SIGTERM 수신 시
//         새 연결 거부 + 진행 중 요청/WS 드레인 후 종료 (2026-11-19)
// ============================================================

use axum::http::{HeaderValue, Method, header};
use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tower_http::cors::CorsLayer;

pub async fn serve_with_startup<F>(initialize: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: Future<Output = Result<crate::core::state::AppState, Box<dyn std::error::Error>>>,
{
    let cors_origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string());
    let allowed_origins = cors_origins
        .split(',')
        .map(str::trim)
        .filter(|origin| !origin.is_empty())
        .map(|origin| {
            origin.parse::<HeaderValue>().map_err(|e| {
                tracing::error!("잘못된 CORS Origin 설정({}): {}", origin, e);
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!(
                        "CORS_ALLOWED_ORIGINS에 잘못된 Origin이 있습니다: {}",
                        origin
                    ),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if allowed_origins.is_empty() {
        let message = "CORS_ALLOWED_ORIGINS에 허용 Origin을 하나 이상 설정해야 합니다.";
        tracing::error!("{}", message);
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into());
    }

    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);

    let bind_addr =
        std::env::var("BACKEND_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| {
            tracing::error!("백엔드 서버 주소({}) 바인딩 실패: {}", bind_addr, e);
            e
        })?;

    let startup = super::startup::StartupShared::new();
    let app = super::startup::build(startup.clone()).layer(cors);

    tracing::info!(
        "backend listening on {} (startup health endpoint active)",
        bind_addr
    );

    // --- Graceful shutdown 준비 ---
    // 종료 신호는 한 번만 오지만, 두 곳에서 기다려야 합니다:
    //  1) axum::serve의 with_graceful_shutdown (HTTP/WS 드레인)
    //  2) 아래 select!의 초기화 단계 (초기화 중에 Ctrl+C가 와도 바로 끝내기)
    // 그래서 Notify를 공유합니다. Future를 두 번 쓸 수 없어서 Notify로 푼 겁니다.
    let shutdown = Arc::new(Notify::new());
    super::shutdown::spawn_shutdown_watcher(shutdown.clone());

    // Axum 0.8의 Serve는 Future가 아니라 IntoFuture를 구현하므로
    // select!에서 사용할 Future로 변환합니다.
    let shutdown_for_server = shutdown.clone();
    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async move {
        shutdown_for_server.notified().await;
    })
    .into_future();
    tokio::pin!(server);
    tokio::pin!(initialize);

    // HTTP 서버와 외부 서비스 초기화를 같은 Tokio task에서 동시에 진행합니다.
    // initialize Future를 별도 tokio::spawn에 넘기지 않아도 되므로
    // 기존 Box<dyn Error> 반환 타입을 유지할 수 있습니다.
    tokio::select! {
        result = &mut server => {
            result.map_err(|e| {
                tracing::error!("Axum 서버 실행 중 오류: {}", e);
                e
            })?;
            tracing::info!("백엔드 종료 완료 (서버 소켓 닫힘)");
            return Ok(());
        }
        result = &mut initialize => {
            match result {
                Ok(state) => {
                    let regular = state.regular.name.clone();
                    let mini = state.mini.name.clone();
                    let app = super::router::build(state);
                    startup.mark_ready(app, regular, mini).await;
                    tracing::info!("Backend 초기화 완료 - API 요청 처리 준비 완료");
                }
                Err(error) => {
                    tracing::error!("Backend 초기화 실패: {}", error);
                    startup.mark_failed().await;
                    tracing::error!(
                        "Health Check는 유지됩니다. 원인 해결 후 Backend를 재시작해 주세요."
                    );
                }
            }
        }
        // 초기화 중에 Ctrl+C / SIGTERM이 온 경우:
        // 초기화가 끝날 때까지 기다리지 않고 바로 종료합니다.
        // (아직 실제 API가 안 열렸고, health만 떠 있던 단계라 드레인할 게 없습니다)
        _ = shutdown.notified() => {
            tracing::info!("초기화 중 종료 신호 수신 - 초기화 중단 후 종료합니다");
            return Ok(());
        }
    }

    // 초기화 완료 후: 종료 신호가 올 때까지 서버를 계속 띄웁니다.
    // with_graceful_shutdown 덕분에 신호가 오면 Axum이
    // 새 연결을 거부하고, 진행 중 요청/WS가 끝날 때까지 기다렸다가 닫습니다.
    server.await.map_err(|e| {
        tracing::error!("Axum 서버 실행 중 오류: {}", e);
        e
    })?;

    tracing::info!("백엔드 종료 완료 (Graceful shutdown)");
    Ok(())
}


