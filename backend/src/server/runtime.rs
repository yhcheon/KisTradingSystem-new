// ============================================================
// 파일명: server/runtime.rs
// 설명: Axum 서버 실행 및 CORS 설정
// ============================================================

use crate::core::state::AppState;
use axum::http::{HeaderValue, Method, header};
use tower_http::cors::CorsLayer;

pub async fn serve(state: AppState) -> Result<(), Box<dyn std::error::Error>> {
    // 운영 환경에서는 CORS_ALLOWED_ORIGINS에 프론트엔드 Origin을
    // 쉼표로 구분해 지정합니다. 미설정 시 로컬 개발 주소만 허용합니다.
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

    let app = super::router::build(state).layer(cors);

    let bind_addr = std::env::var("BACKEND_BIND_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| {
            tracing::error!("백엔드 서버 주소({}) 바인딩 실패: {}", bind_addr, e);
            e
        })?;
    tracing::info!("backend listening on {}", bind_addr);

    axum::serve(listener, app).await.map_err(|e| {
        tracing::error!("Axum 서버 실행 중 오류: {}", e);
        e
    })?;

    Ok(())
}
