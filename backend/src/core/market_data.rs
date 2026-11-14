// ============================================================
// 파일명: core/market_data.rs
// 설명: KIS 시장 데이터 초기화
//       - OAuth 토큰 준비
//       - 선물 마스터 다운로드 및 최근월물 선택
//       - 초기 1분봉 데이터 로드
// ============================================================

use crate::kis;
use crate::market;
use crate::master::{self, FuturesSymbol};
use crate::core::candles::CandleStore;
use std::sync::Arc;
use tokio::sync::Mutex;

/// AppState 및 WebSocket 시작에 필요한 시장 데이터 초기화 결과
pub struct MarketData {
    pub access_token: String,
    pub regular: FuturesSymbol,
    pub mini: FuturesSymbol,
    pub candles: Arc<Mutex<CandleStore>>,
}

/// KIS 토큰, 최근월물 종목, 초기 캔들 저장소를 준비합니다.
pub async fn initialize(
    http_client: &reqwest::Client,
    app_key: &str,
    app_secret: &str,
    cache_path: &str,
) -> Result<MarketData, Box<dyn std::error::Error>> {
    // KIS 토큰
    let token = match kis::get_or_issue_token(http_client, app_key, app_secret, cache_path).await {
        Ok(t) => {
            // 인증 토큰은 로그에 남기지 않습니다.
            tracing::info!("KIS access token 발급/로드 성공");
            t
        }
        Err(e) => {
            tracing::error!("KIS 토큰 초기화 실패: {}", e);
            return Err(
                std::io::Error::new(std::io::ErrorKind::Other, "KIS 토큰 초기화 실패").into(),
            );
        }
    };

    // 마스터
    tracing::info!("마스터 파일 다운로드 중");
    let symbols = match master::download_master_text(http_client).await {
        Ok(text) => master::parse_master(&text),
        Err(e) => {
            tracing::error!("마스터 다운로드 실패: {}", e);
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "마스터 파일 다운로드 실패",
            )
            .into());
        }
    };
    let regular = match master::pick_nearest(&symbols, "1") {
        Some(f) => {
            tracing::info!("정규선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("정규선물 최근월물 없음");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "정규선물 최근월물 없음",
            )
            .into());
        }
    };
    let mini = match master::pick_nearest(&symbols, "B") {
        Some(f) => {
            tracing::info!("미니선물 최근월물: {} ({})", f.name, f.code);
            f
        }
        None => {
            tracing::error!("미니선물 최근월물 없음");
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "미니선물 최근월물 없음",
            )
            .into());
        }
    };

    // 과거 및 당일 1분봉 초기 데이터 로드
    let candle_store = market::history::load_initial_candles(
        http_client,
        app_key,
        app_secret,
        &token.access_token,
        &[regular.clone(), mini.clone()],
    )
    .await;

    // CandleStore를 Arc<Mutex<>>로 감싸 WS와 공유
    let candles = Arc::new(Mutex::new(candle_store));

    Ok(MarketData {
        access_token: token.access_token,
        regular,
        mini,
        candles,
    })
}
