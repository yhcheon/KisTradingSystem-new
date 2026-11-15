// ============================================================
// 파일명: server/handlers/market/candle_params.rs
// 설명: 분봉 캔들 조회 API 요청 파라미터
// ============================================================

use serde::Deserialize;

/// 캔들 조회 API의 선택적 쿼리 파라미터입니다.
#[derive(Debug, Deserialize)]
pub(super) struct CandleParams {
    pub code: Option<String>,
    pub interval: Option<String>,
    pub date: Option<String>,
}
