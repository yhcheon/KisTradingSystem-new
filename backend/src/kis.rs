// ============================================================
// 파일명: kis.rs
// 설명: KIS API 클라이언트 공개 인터페이스
//       세부 구현은 인증, 시세, 분봉 모듈로 분리합니다.
// ============================================================

pub mod auth;
pub mod candles;
pub mod quotes;

// 기존 호출부에서 사용하는 API 함수만 재노출합니다.
pub use auth::{get_approval_key, get_or_issue_token};
pub use candles::get_minute_chart;
pub use quotes::get_futures_price;
