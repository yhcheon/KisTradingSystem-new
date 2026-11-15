// ============================================================
// 파일명: kis.rs
// 설명: KIS API 클라이언트 공개 인터페이스
//       세부 구현은 인증, 시세, 분봉 모듈로 분리합니다.
// ============================================================

pub mod auth;
pub mod candles;
pub mod quotes;

// 기존 호출부와 외부 모듈의 타입 경로 호환성을 위해 재노출합니다.
#[allow(unused_imports)]
pub use auth::{ApprovalResponse, TokenResponse, get_approval_key, get_or_issue_token};
#[allow(unused_imports)]
pub use candles::{MinuteCandle, MinuteChartOutput2, MinuteChartResponse, get_minute_chart};
#[allow(unused_imports)]
pub use quotes::{
    FuturesPriceOutput1, FuturesPriceOutput2, FuturesPriceResponse, get_futures_price,
};
