// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 환경변수 및 로깅 초기화
//       - 애플리케이션 상태 초기화
//       - Axum HTTP 서버 실행
// ============================================================

mod auth;
mod cache;
pub mod chat;
mod core;
mod kis;
mod kis_ws;
mod logging;
mod market;
mod master;
mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();
    let _log_guard = logging::init();

    tracing::info!("============================================================");
    tracing::info!("============================================================");
    tracing::info!(
        "🚀 백엔드 시작  ({} 실행)",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    tracing::info!("============================================================");
    tracing::info!("============================================================");

    let state = core::bootstrap::initialize().await?;
    server::runtime::serve(state).await?;

    Ok(())
}
