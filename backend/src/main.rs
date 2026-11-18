// ============================================================
// 파일명: main.rs
// 설명: 백엔드 진입점
//       - 환경변수 및 로깅 초기화
//       - HTTP 서버를 먼저 열고 외부 서비스 초기화는 백그라운드에서 수행
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
    tracing::info!(
        "🚀 백엔드 시작  ({})",
        chrono::Local::now().format("%Y-%m-%d %H:%M:%S")
    );
    tracing::info!("============================================================");

    // Listener를 먼저 열어 초기화 중에도 /api/health가 응답하도록 합니다.
    server::runtime::serve_with_startup(core::bootstrap::initialize()).await?;

    Ok(())
}
