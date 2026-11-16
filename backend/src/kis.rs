// ============================================================
// 파일명: kis.rs
// 설명: KIS API 클라이언트 공개 인터페이스
//       세부 구현은 인증, 시세, 분봉 모듈로 분리합니다.
// ============================================================

pub mod auth;
pub mod candles;
pub mod quotes;

#[allow(unused_imports)]
pub use auth::{ApprovalResponse, TokenResponse, get_approval_key, get_or_issue_token};
#[allow(unused_imports)]
pub use candles::{MinuteCandle, MinuteChartOutput2, MinuteChartResponse, get_minute_chart, get_minute_chart_for_market};
#[allow(unused_imports)]
pub use quotes::{
    FuturesPriceOutput1, FuturesPriceOutput2, FuturesPriceResponse, get_futures_price,
    get_futures_price_for_market,
};
