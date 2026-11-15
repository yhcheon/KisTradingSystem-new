// ============================================================
// 파일명: server/router.rs
// 설명: Axum 라우터 (HTTP + WebSocket)
// ============================================================

use crate::core::state::AppState;
use crate::server::handlers::{content, market};
use axum::{Router, routing::get};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "backend ok" }))
        .route("/api/health", get(market::health))
        .route("/api/prices", get(market::prices))
        .route("/api/candles", get(market::candles))
        .route("/ws", get(market::ws_handler))
        // ── 콘텐츠 API ──
        .route("/api/notes", get(content::notes))
        .route("/api/notes/{id}", get(content::note_detail))
        .route(
            "/api/admin/notes",
            get(content::admin_notes)
                .post(content::create_note)
                .put(content::update_note)
                .delete(content::delete_note),
        )
        .with_state(state)
}
