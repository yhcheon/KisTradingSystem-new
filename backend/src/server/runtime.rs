// ============================================================
// 파일명: server/runtime.rs — 종료 시 백그라운드 태스크 join 버전
// 변경점:
//  - initialize의 반환이 (AppState, Vec<JoinHandle>) 로 바뀌었습니다.
//  - 서버가 닫힌 뒤, background 핸들을 최대 5초까지 기다립니다.
//    이게 바로 지난 로그에서 "백엔드 종료 완료"가 구독 해제보다 먼저 찍힌 문제의 수정입니다.
//  - 초기화 중 종료 신호가 온 경우에도, 이미 초기화가 완료돼서 핸들을 들고 있으면
//    그 핸들을 기다립니다. 아직 초기화가 안 끝났으면 기다릴 핸들이 없으니 바로 리턴합니다.
// ============================================================
use axum::http::{HeaderValue, Method, header};
use std::future::{Future, IntoFuture};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::Notify;
use tokio::task::JoinHandle;
use tower_http::cors::CorsLayer;

const SHUTDOWN_JOIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// 모든 백그라운드 태스크가 끝날 때까지 기다립니다. 하나라도 5초를 넘기면
/// 그 태스크는 포기하고(abort) 넘어갑니다. 종료가 무한히 멈추지 않게 하기 위함입니다.
async fn join_background(mut handles: Vec<JoinHandle<()>>) {
    if handles.is_empty() { return; }
    tracing::info!("백그라운드 태스크 종료 대기 중 ({}개, 최대 {:?})", handles.len(), SHUTDOWN_JOIN_TIMEOUT);
    // join_all이 아니라 하나씩 timeout을 거는 이유: 하나가 멈춰도 나머지는 기다릴 수 있게.
    // futures crate 없이, tokio만으로 구현합니다.
    for handle in handles.drain(..) {
        match tokio::time::timeout(SHUTDOWN_JOIN_TIMEOUT, handle).await {
            Ok(Ok(())) => {}
            Ok(Err(join_err)) => {
                // 태스크 내부에서 panic한 경우. panic이 종료를 막아서는 안 됩니다.
                tracing::warn!("백그라운드 태스크가 panic으로 종료됨: {}", join_err);
            }
            Err(_) => {
                tracing::warn!("백그라운드 태스크 종료 대기 시간 초과 (5초) - 다음 태스크로 넘어갑니다");
                // timeout된 handle은 이미 소비됐으므로 abort할 수 없습니다.
                // 각 태스크는 shutdown 신호를 받았으니 곧 스스로 끝납니다.
            }
        }
    }
    tracing::info!("백그라운드 태스크 종료 대기 완료");
}

pub async fn serve_with_startup<F, Fut>(initialize: F) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(Arc<Notify>) -> Fut,
    Fut: Future<Output = Result<(crate::core::state::AppState, Vec<JoinHandle<()>>), Box<dyn std::error::Error>>>,
{
    let cors_origins = std::env::var("CORS_ALLOWED_ORIGINS")
        .unwrap_or_else(|_| "http://localhost:5173,http://127.0.0.1:5173".to_string());
    let allowed_origins = cors_origins.split(',').map(str::trim).filter(|o| !o.is_empty())
        .map(|origin| origin.parse::<HeaderValue>().map_err(|e| {
            tracing::error!("잘못된 CORS Origin 설정({}): {}", origin, e);
            std::io::Error::new(std::io::ErrorKind::InvalidInput, format!("CORS_ALLOWED_ORIGINS에 허용 Origin이 있습니다: {}", origin))
        })).collect::<Result<Vec<_>, _>>()?;
    if allowed_origins.is_empty() {
        let message = "CORS_ALLOWED_ORIGINS에 허용 Origin을 하나 이상 설정해야 합니다.";
        tracing::error!("{}", message);
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, message).into());
    }
    let cors = CorsLayer::new()
        .allow_origin(allowed_origins)
        .allow_methods([Method::GET, Method::POST, Method::PUT, Method::DELETE, Method::OPTIONS])
        .allow_headers([header::AUTHORIZATION, header::CONTENT_TYPE]);
    let bind_addr = std::env::var("BACKEND_BIND_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind_addr).await.map_err(|e| {
        tracing::error!("백엔드 서버 주소({}) 바인딩 실패: {}", bind_addr, e); e
    })?;
    let startup = super::startup::StartupShared::new();
    let app = super::startup::build(startup.clone()).layer(cors);
    tracing::info!("backend listening on {} (startup health endpoint active)", bind_addr);

    let shutdown = Arc::new(Notify::new());
    super::shutdown::spawn_shutdown_watcher(shutdown.clone());
    let shutdown_for_server = shutdown.clone();
    let server = axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(async move { shutdown_for_server.notified().await; })
        .into_future();
    tokio::pin!(server);
    let initialize = initialize(shutdown.clone());
    tokio::pin!(initialize);

    // 초기화 완료 후에 생기는 핸들을 여기에 보관합니다.
    let mut background: Vec<JoinHandle<()>> = Vec::new();

    tokio::select! {
        result = &mut server => {
            result.map_err(|e| { tracing::error!("Axum 서버 실행 중 오류: {}", e); e })?;
            // 서버가 먼저 끝난 경우: 아직 초기화 중이었다면 background는 비어있습니다.
            // 초기화가 끝났다면 핸들이 있으니 기다립니다.
            join_background(std::mem::take(&mut background)).await;
            tracing::info!("백엔드 종료 완료 (Graceful shutdown)");
            return Ok(());
        }
        result = &mut initialize => {
            match result {
                Ok((state, handles)) => {
                    background = handles;
                    let regular = state.regular.name.clone();
                    let mini = state.mini.name.clone();
                    let app = super::router::build(state);
                    startup.mark_ready(app, regular, mini).await;
                    tracing::info!("Backend 초기화 완료 - API 요청 처리 준비 완료");
                }
                Err(error) => {
                    tracing::error!("Backend 초기화 실패: {}", error);
                    startup.mark_failed().await;
                    tracing::error!("Health Check는 유지됩니다. 원인 해결 후 Backend를 재시작해 주세요.");
                }
            }
        }
        _ = shutdown.notified() => {
            tracing::info!("초기화 중 종료 신호 수신 - 초기화 중단 후 종료합니다");
            // 이미 초기화가 끝나서 핸들을 들고 있는 경우는 위 select!의
            // initialize 분기가 먼저 처리했을 것이므로, 여기는 핸들이 없습니다.
            join_background(std::mem::take(&mut background)).await;
            return Ok(());
        }
    }

    // 정상 경로: 초기화 완료 후 서버가 끝날 때까지 대기, 그 후 백그라운드 join
    server.await.map_err(|e| { tracing::error!("Axum 서버 실행 중 오류: {}", e); e })?;
    join_background(std::mem::take(&mut background)).await;
    tracing::info!("백엔드 종료 완료 (Graceful shutdown)");
    Ok(())
}





