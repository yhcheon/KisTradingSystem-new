// ============================================================
// 파일명: master.rs
// 설명: KIS 종목 마스터 모듈 진입점 및 기존 공개 경로 호환 facade
// ============================================================

mod download;
mod parser;
mod selection;
mod types;

pub use download::download_master_text;
pub use parser::parse_master;
pub use selection::pick_nearest;
pub use types::FuturesSymbol;
