// ============================================================
// 파일명: server/router.rs
// 설명: Axum 라우터 (HTTP + WebSocket)
// 작성일: 2026-11-14
// ============================================================

use crate::core::state::AppState;
use crate::server::handlers;
use axum::{
    routing::{get, post, put},
    Router,
};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "backend ok" }))
        .route("/api/health", get(handlers::health))
        .route("/api/prices", get(handlers::prices))
        .route("/api/candles", get(handlers::candles))
        .route("/ws", get(handlers::ws_handler))
        // ── 콘텐츠 API ──
        .route("/api/notes", get(handlers::notes))
        .route("/api/admin/notes", get(handlers::admin_notes).post(handlers::create_note).put(handlers::update_note).delete(handlers::delete_note))
        .with_state(state)
}

