// ============================================================
// 파일명: server/startup.rs
// 설명: 초기화 중에도 Health Check를 제공하는 서버 진입 라우터
//       - HTTP Listener를 먼저 열어 ECONNREFUSED 방지
//       - 초기화 완료 전 API 요청은 503 + Retry-After 반환
//       - 준비 완료 후 요청을 실제 애플리케이션 Router로 전달
// ============================================================

use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::{Request, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use tower::ServiceExt;

#[derive(Clone)]
pub struct StartupShared {
    app: Arc<RwLock<Option<Router>>>,
    status: Arc<RwLock<StartupStatus>>,
}

#[derive(Clone)]
enum StartupStatus {
    Starting,
    Ready,
    Failed,
}

impl StartupShared {
    pub fn new() -> Self {
        Self {
            app: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(StartupStatus::Starting)),
        }
    }

    pub async fn mark_ready(&self, app: Router) {
        *self.app.write().await = Some(app);
        *self.status.write().await = StartupStatus::Ready;
    }

    pub async fn mark_failed(&self) {
        *self.status.write().await = StartupStatus::Failed;
    }
}

async fn health(State(shared): State<StartupShared>) -> Response {
    let status = shared.status.read().await.clone();
    match status {
        StartupStatus::Starting => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "2")],
            Json(json!({
                "status": "starting",
                "ready": false,
                "message": "Backend 초기화가 진행 중입니다."
            })),
        )
            .into_response(),
        StartupStatus::Ready => (
            StatusCode::OK,
            Json(json!({
                "status": "ok",
                "ready": true
            })),
        )
            .into_response(),
        StartupStatus::Failed => (
            StatusCode::SERVICE_UNAVAILABLE,
            [(header::RETRY_AFTER, "5")],
            Json(json!({
                "status": "failed",
                "ready": false,
                "message": "Backend 초기화에 실패했습니다. 백엔드 로그를 확인해 주세요."
            })),
        )
            .into_response(),
    }
}

async fn dispatch(
    State(shared): State<StartupShared>,
    request: Request<Body>,
) -> Response {
    let app = shared.app.read().await.clone();

    if let Some(app) = app {
        return match app.oneshot(request).await {
            Ok(response) => response,
            Err(never) => match never {},
        };
    }

    let status = shared.status.read().await.clone();
    let message = match status {
        StartupStatus::Starting => "Backend 초기화가 진행 중입니다.",
        StartupStatus::Ready => "Backend 서비스가 준비 중입니다.",
        StartupStatus::Failed => {
            "Backend 초기화에 실패했습니다. 백엔드 로그를 확인해 주세요."
        }
    };

    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::RETRY_AFTER, "2")],
        Json(json!({
            "status": "unavailable",
            "ready": false,
            "message": message
        })),
    )
        .into_response()
}

pub fn build(shared: StartupShared) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .fallback(dispatch)
        .with_state(shared)
}
