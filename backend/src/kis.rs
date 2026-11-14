// ============================================================
// 파일명: kis.rs
// 설명: KIS API 클라이언트 공개 인터페이스
//       세부 구현은 인증, 시세, 분봉 모듈로 분리합니다.
// ============================================================

pub mod auth;
pub mod candles;
pub mod quotes;

// 기존 호출부와 외부 모듈의 타입 경로 호환성을 위해 재노출합니다.
pub use auth::{
    get_approval_key, get_or_issue_token, ApprovalResponse, TokenResponse,
};
pub use candles::{
    get_minute_chart, MinuteCandle, MinuteChartOutput2, MinuteChartResponse,
};
pub use quotes::{
    get_futures_price, FuturesPriceOutput1, FuturesPriceOutput2, FuturesPriceResponse,
};
