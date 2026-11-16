// ============================================================
// 파일명: kis_ws.rs
// 설명: KIS 실시간 WebSocket 클라이언트 모듈 진입점
//       기존 kis_ws::run 및 kis_ws::RealtimeTick 경로를 유지합니다.
// ============================================================

mod client;
mod protocol;

pub use client::run;
pub use protocol::RealtimeTick;
