// ============================================================
// 파일명: core/state.rs
// 설명: Axum 핸들러가 공유하는 AppState 및 외부 인증 설정
// 수정일: 2026-11-19 - 시세 WS 카운팅 제거, 접속자수는 채팅(ChatService)으로 일원화
// ============================================================

use crate::chat::ChatService;
use crate::core::candles::CandleStore;
use crate::core::db::SupabaseClient;
use crate::core::session_range::SessionRangeMonitor;
use crate::kis::ws::RealtimeTick;
use crate::master::FuturesSymbol;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::{Mutex, broadcast};

/// KIS REST API 호출에 필요한 인증 정보
#[derive(Clone)]
pub struct KisCredentials {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
}

/// 레거시 HS256 JWT 검증에 필요한 설정
#[derive(Clone)]
pub struct JwtConfig {
    pub legacy_hs256_secret: String,
}

#[derive(Clone)]
pub struct AppState {
    pub kis: KisCredentials,
    pub jwt: JwtConfig,

    // redis 사용하지 않기로
    //pub redis: Arc<Mutex<redis::aio::MultiplexedConnection>>,
    pub price_cache: crate::cache::price_cache::PriceCache,

    pub regular: FuturesSymbol,
    pub mini: FuturesSymbol,
    pub ws_tx: broadcast::Sender<RealtimeTick>,
    pub http_client: reqwest::Client,
    pub candles: Arc<Mutex<CandleStore>>,
    pub session_range: Arc<Mutex<SessionRangeMonitor>>,
    pub minute_candle_check_running: Arc<AtomicBool>,
    pub supabase: SupabaseClient,
    pub chat: ChatService,
}
