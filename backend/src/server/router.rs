// ============================================================
// 파일명: server/router.rs
// 설명: 준비 완료된 Axum 애플리케이션 라우터 (HTTP + WebSocket)
// ============================================================

use crate::core::state::AppState;
use crate::server::handlers::{content, market};
use axum::{
    Router,
    routing::{get, post},
};

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/", get(|| async { "backend ok" }))
        .route("/api/prices", get(market::prices))
        .route("/api/candles", get(market::candles))
        .route("/api/admin/market/minute-candle-check", post(market::start_minute_candle_check))
        .route(
            "/api/chat/ticket",
            post(crate::server::handlers::chat::issue_ticket),
        )
        .route(
            "/api/chat/commands",
            get(crate::server::handlers::chat::list_commands),
        )
        .route(
            "/api/chat/actions",
            get(crate::server::handlers::chat::list_actions),
        )
        .route("/ws", get(market::ws_handler))
        .route("/ws/chat", get(crate::server::handlers::chat::ws_handler))
        .layer(axum::Extension(state.chat.clone()))
        .route("/api/notes", get(content::notes))
        .route("/api/news", get(content::news))
        .route("/api/notes/{id}", get(content::note_detail))
        .route(
            "/api/admin/news",
            get(content::admin_news)
                .post(content::create_news_link)
                .delete(content::delete_news_link),
        )
        .route(
            "/api/admin/notes",
            get(content::admin_notes)
                .post(content::create_note)
                .put(content::update_note)
                .delete(content::delete_note),
        )
        .with_state(state)
}
