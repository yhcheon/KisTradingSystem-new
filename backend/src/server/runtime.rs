// ============================================================
// 파일명: server/runtime.rs
// 설명: Axum 서버 실행 및 CORS 설정
//       - TCP Listener를 초기화보다 먼저 열어 연결 거부 방지
//       - 초기화 중 /api/health 응답 제공
// ============================================================

use axum::http::{HeaderValue, Method, header};
use std::future::Future;
use std::net::SocketAddr;
use tower_http::cors::CorsLayer;

pub async fn serve_with_startup<F>(
    initialize: F,
) -> Result<(), Box<dyn std::error::Error>>
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
                    format!("CORS_ALLOWED_ORIGINS에 잘못된 Origin이 있습니다: {}", origin),
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

    let server = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    );
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
            return Ok(());
        }
        result = &mut initialize => {
            match result {
                Ok(state) => {
                    let app = super::router::build(state);
                    startup.mark_ready(app).await;
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
    }

    server.await.map_err(|e| {
        tracing::error!("Axum 서버 실행 중 오류: {}", e);
        e
    })?;

    Ok(())
}
