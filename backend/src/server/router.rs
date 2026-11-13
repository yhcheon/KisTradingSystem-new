// ============================================================
// 파일명: server/router.rs
// 설명: Axum 라우터 구성
// 작성일: 2026-11-14
// ============================================================

use crate::core::state::AppState;
use crate::server::handlers;
use axum::{routing::get, Router};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "backend ok" }))
        .route("/api/health", get(handlers::health))
        .route("/api/prices", get(handlers::prices))
        .with_state(state)
}


