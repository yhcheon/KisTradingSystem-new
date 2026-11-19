// ============================================================
// 파일명: main.rs  (PATCHED - 호출부 1줄만 변경)
// ============================================================
mod auth;
mod cache;
pub mod chat;
mod core;
mod kis;
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
    // PATCH: initialize가 shutdown을 인자로 받도록 변경
    server::runtime::serve_with_startup(|shutdown| core::bootstrap::initialize(shutdown)).await?;
    Ok(())
}
