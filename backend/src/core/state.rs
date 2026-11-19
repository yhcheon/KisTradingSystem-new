// ============================================================
// 파일명: core/state.rs  (PATCHED)
// 변경점: AppState에 shutdown: Arc<Notify> 필드 추가
// ============================================================

use crate::chat::ChatService;
use crate::core::candles::CandleStore;
use crate::core::db::SupabaseClient;
use crate::core::session_range::SessionRangeMonitor;
use crate::kis::ws::RealtimeTick;
use crate::master::FuturesSymbol;
use std::sync::{Arc, atomic::AtomicBool};
use tokio::sync::{Mutex, Notify, broadcast};

#[derive(Clone)]
pub struct KisCredentials {
    pub app_key: String,
    pub app_secret: String,
    pub access_token: String,
}

#[derive(Clone)]
pub struct JwtConfig {
    pub legacy_hs256_secret: String,
}

#[derive(Clone)]
pub struct AppState {
    pub kis: KisCredentials,
    pub jwt: JwtConfig,
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
    /// Graceful shutdown 신호. runtime.rs의 Notify와 같은 인스턴스입니다.
    /// Notify는 notify_waiters()가 호출된 뒤에 등록된 waiter는 깨우지 않으므로,
    /// 이 필드를 직접 await하는 곳에서는 반드시 아래 주의사항을 따르세요:
    /// - 이미 종료 신호가 온 뒤에 시작하는 태스크는 is_notified()를 먼저 확인할 것.
    ///   (이 패치에서는 runtime이 initialize()에 같은 Notify를 미리 넘기므로,
    ///    초기화 완료 시점에는 아직 신호가 안 온 경우에만 태스크가 뜹니다.)
    pub shutdown: Arc<Notify>,
}
