// ============================================================
// 파일명: logging.rs
// 설명: 일별 로그 파일 시스템 (파일 전용, 콘솔 출력 없음)
//       - logs/ 디렉토리에 yyyy-mm-dd.log 형식으로 저장
//       - RUST_LOG 환경변수로 레벨 제어 (기본: debug)
// 작성일: 2026-11-14
// ============================================================

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

/// 로깅 초기화. 반환된 guard는 프로그램 종료 시까지 유지해야 함.
pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
    // logs 디렉토리 생성
    let log_dir = PathBuf::from("./logs");
    std::fs::create_dir_all(&log_dir).expect("logs 디렉토리 생성 실패");

    // 일별 로테이션 파일 appender: logs/2026-11-14.log
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_suffix("log")
        .build(&log_dir)
        .expect("로그 파일 appender 생성 실패");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 파일 레이어만 사용 (콘솔 출력 없음)
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // 파일에 색상 코드 없음
        .with_target(true)
        .with_line_number(true)
        .with_file(true);

    // 필터: RUST_LOG 없으면 debug
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .init();

    guard
}
