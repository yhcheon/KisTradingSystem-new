// ============================================================
// 파일명: core/state.rs
// 설명: Axum 핸들러가 공유하는 AppState
//       - KIS 토큰, Redis 연결, 최근월물 종목
//       - WebSocket 브로드캐스트 채널
//       - HTTP 클라이언트 (reqwest 연결 풀 재사용)
// 작성일: 2026-11-14
// ============================================================

use crate::kis_ws::RealtimeTick;
use crate::master::FuturesSymbol;
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub redis: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    pub regular: FuturesSymbol,
    pub mini: FuturesSymbol,
    pub ws_tx: broadcast::Sender<RealtimeTick>,
    pub http_client: reqwest::Client,   // ← 추가: HTTP 연결 풀 공유
}

