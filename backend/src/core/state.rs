
// ============================================================
// 파일명: core/state.rs
// 설명: Axum 핸들러가 공유하는 AppState
//       - KIS 토큰
//       - Redis 연결
//       - 최근월물 종목 정보 (정규선물, 미니선물)
// 작성일: 2026-11-14
// ============================================================

use crate::master::FuturesSymbol;
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AppState {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
    pub redis: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    pub regular: FuturesSymbol,   // 정규선물 최근월물
    pub mini: FuturesSymbol,      // 미니선물 최근월물
}

