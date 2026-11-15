// ============================================================
// 파일명: market/history/convert.rs
// 설명: KIS 1분봉 응답을 내부 Candle 타입으로 변환
// ============================================================

use crate::core::candles::Candle;
use crate::kis::MinuteCandle;

pub(super) fn to_candle(c: &MinuteCandle) -> Candle {
    Candle {
        time: c.stck_cntg_hour[0..4].to_string(),
        open: c.futs_oprc.parse().unwrap_or(0.0),
        high: c.futs_hgpr.parse().unwrap_or(0.0),
        low: c.futs_lwpr.parse().unwrap_or(0.0),
        close: c.futs_prpr.parse().unwrap_or(0.0),
        volume: c.cntg_vol.parse().unwrap_or(0),
    }
}
