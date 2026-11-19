// ============================================================
// 파일명: server/shutdown.rs
// 설명: Graceful shutdown 신호 대기
//       - Windows: Ctrl+C (ctrl_c) + Ctrl+Break (ctrl_break)
//       - Unix: Ctrl+C (SIGINT) + SIGTERM
//       - 신호가 오면 Notify로 runtime에 알림
// 작성일: 2026-11-19
// ============================================================

use std::sync::Arc;
use tokio::sync::Notify;

/// 종료 신호를 기다렸다가 `notify`를 깨우는 백그라운드 태스크를 띄웁니다.
/// 호출자는 이 Notify를 `with_graceful_shutdown`과 `select!`에서 공유해서 씁니다.
pub fn spawn_shutdown_watcher(notify: Arc<Notify>) {
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;
        tracing::info!("종료 신호 수신 (Ctrl+C / SIGTERM) - Graceful shutdown 시작");
        notify.notify_waiters();
    });
}

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};

        let mut sigint = match signal(SignalKind::interrupt()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("SIGINT 핸들러 등록 실패: {} - Ctrl+C 대기로 폴백", e);
                tokio::signal::ctrl_c().await.ok();
                return;
            }
        };
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("SIGTERM 핸들러 등록 실패: {} - SIGINT만 대기", e);
                sigint.recv().await;
                return;
            }
        };

        tokio::select! {
            _ = sigint.recv() => {
                tracing::info!("SIGINT 수신 (Ctrl+C)");
            }
            _ = sigterm.recv() => {
                tracing::info!("SIGTERM 수신");
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Windows: Ctrl+C와 Ctrl+Break 둘 다 받습니다.
        // `cargo run`에서 Ctrl+C는 ctrl_c, 터미널 닫기/작업 종료는 ctrl_break로 오는 경우가 있습니다.
        let ctrl_c = tokio::signal::ctrl_c();
        // Windows 전용 Ctrl+Break: 별도 태스크 없이 ctrl_c만으로도 충분하지만,
        // 터미널 종료 시그널까지 잡기 위해 함께 대기합니다.
        let ctrl_break = async {
            #[cfg(windows)]
            {
                match tokio::signal::windows::ctrl_break() {
                    Ok(mut sig) => {
                        let _ = sig.recv().await;
                    }
                    Err(_) => {
                        std::future::pending::<()>().await;
                    }
                }
            }
            #[cfg(not(windows))]
            {
                std::future::pending::<()>().await;
            }
        };

        tokio::select! {
            _ = ctrl_c => {
                tracing::info!("Ctrl+C 수신");
            }
            _ = ctrl_break => {
                tracing::info!("Ctrl+Break 수신");
            }
        }
    }
}
