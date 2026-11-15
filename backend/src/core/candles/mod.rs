// ============================================================
// 파일명: core/candles/mod.rs
// 설명: 캔들 저장소 모듈의 공개 인터페이스
// ============================================================

mod aggregation;
mod store;
mod types;

pub use aggregation::aggregate;
pub use store::CandleStore;
pub use types::{Candle, INTERVALS};
