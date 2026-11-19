// ============================================================
// 파일명: logging.rs
// 설명: 일별 로그 파일 + 정제된 stdout 로그
//       - 파일 (logs/yyyy-mm-dd.log): 전부 기록 (LOG_LEVEL, 기본: info)
//         체결 틱, PINGPONG, WS 응답 같은 빠른 로그도 파일에는 남김
//       - 터미널 (stdout): 중요 정보만 (STDOUT_LOG_LEVEL, 기본: 아래 정제 필터)
//         * WARN / ERROR는 전부 (어느 모듈이든)
//         * INFO는 시작/종료/초기화/연결 상태 + 야간 체결 첫 10개만
//           (client.rs에서 11개째부터는 debug라 터미널에 안 뜸)
//         * PINGPONG, WS 응답 debug는 터미널에서 제외
// 작성일: 2026-11-14
// 수정일: 2026-11-19 - stdout 레이어 추가 (틱 로그 제외)
// ============================================================

use std::path::PathBuf;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    EnvFilter, fmt,
    layer::{Layer, SubscriberExt},
    util::SubscriberInitExt,
};

/// 터미널용 기본 필터.
///
/// EnvFilter 문법으로 모듈(target) 단위로 끊습니다.
/// 메시지를 보고 걸러내는 방식이 아니라서, 확실하게 걸러집니다.
///
/// 실제 로그를 확인해보니, 체결 틱 INFO는 무한 홍수가 아니었습니다.
/// `kis/ws/client.rs`에서 야간 체결은 처음 10개만 `info!`로 찍고,
/// 11개째부터는 `debug!`로 찍습니다. 그래서 터미널에
/// `KIS WS 연결됨` + `체결 수신 (1/10) ~ (10/10)`이 한 번 뜨고 나면
/// 그 뒤로는 조용해지는 게 정상이고, 원하는 동작입니다.
/// 그래서 KIS WS는 `warn`으로 막지 않고 `info`로 열어둡니다.
///
/// 읽는 법 (뒤에 나온 더 구체적인 규칙이 앞의 규칙을 이깁니다):
/// - `warn`                      : 기본은 WARN 이상만
/// - `backend=info`              : 우리 코드의 INFO는 일단 전부 허용
/// - `backend::server::handlers::market=warn`
///   -> 브라우저 WS의 틱 누락 warn은 보이되, info는 안 보임
/// - `backend::server::handlers::market::websocket=info`
///   -> 단, 브라우저 WS 연결됨/종료 INFO는 다시 허용
/// - `backend::market::history::pagination=warn`
///   -> 과거 분봉 추가 조회 실패 warn은 보이고, info는 안 보임
/// - `backend::kis::ws::protocol=warn`
///   -> WS 응답 debug는 어차피 안 보이지만, 명시적으로 막아둠
///
fn default_stdout_filter() -> EnvFilter {
    // 중요: 한 줄 문자열로만 씁니다. Rust의 `\` + 줄바꿈 이어쓰기를 쓰면
    // EnvFilter가 뒤쪽 지시문을 공백 포함으로 읽고 버립니다.
    // (이번에 터미널에 뜬 "ignoring `         backend::...`" 경고가 그거였습니다.)
    EnvFilter::new(
        "warn,backend=info,backend::server::handlers::market=warn,backend::server::handlers::market::websocket=info,backend::market::history::pagination=warn,backend::kis::ws::protocol=warn",
    )
}
/// STDOUT_LOG_LEVEL이 있으면 그 값을 그대로 쓰고,
/// 없으면 위의 정제 필터를 씁니다.
/// 예: STDOUT_LOG_LEVEL=info  -> 터미널에도 전부 자세히
///     STDOUT_LOG_LEVEL=debug -> 터미널에도 틱/debug까지 전부 (홍수 주의)
fn stdout_filter() -> EnvFilter {
    match std::env::var("STDOUT_LOG_LEVEL") {
        Ok(level) if !level.trim().is_empty() => match EnvFilter::try_new(level.trim()) {
            Ok(f) => f,
            Err(e) => {
                eprintln!(
                    "[logging] STDOUT_LOG_LEVEL 파싱 실패({}), 기본 정제 필터 사용",
                    e
                );
                default_stdout_filter()
            }
        },
        _ => default_stdout_filter(),
    }
}

/// 로깅 초기화. 반환된 guard는 프로그램 종료 시까지 유지해야 함.
pub fn init() -> tracing_appender::non_blocking::WorkerGuard {
    // logs 디렉토리 생성
    let log_dir = PathBuf::from(std::env::var("LOG_DIR").unwrap_or_else(|_| "./logs".to_string()));
    std::fs::create_dir_all(&log_dir).expect("logs 디렉토리 생성 실패");

    // 일별 로테이션 파일 appender: logs/2026-11-14.log
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::DAILY)
        .filename_suffix("log")
        .build(&log_dir)
        .expect("로그 파일 appender 생성 실패");

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // --- 파일 레이어: 기존 그대로, 전부 기록 ---
    let file_filter =
        EnvFilter::try_from_env("LOG_LEVEL").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_ansi(false) // 파일에 색상 코드 없음
        .with_target(true)
        .with_line_number(true)
        .with_file(true)
        .with_filter(file_filter);

    // --- stdout 레이어: 정제된 중요 정보만 ---
    // PowerShell 5.1 콘솔은 기본 인코딩이 CP949라 한글이 깨져 보일 수 있습니다.
    // 그건 서버 로그 문제가 아니라 콘솔 인코딩 문제입니다.
    // 터미널에서 `chcp 65001` 후 실행하면 깨지지 않습니다.
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_ansi(true)
        .with_target(false) // 터미널은 짧게: target 없이 메시지만
        .with_line_number(false)
        .with_file(false)
        .with_filter(stdout_filter());

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .init();

    guard
}
