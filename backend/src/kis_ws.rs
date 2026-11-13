// ============================================================
// 파일명: kis_ws.rs
// 설명: KIS 지수선물 실시간체결가(H0IFCNT0) WebSocket 클라이언트
//       - approval_key로 접속
//       - 정규선물/미니선물 2종목 구독
//       - 수신한 체결가를 tokio::sync::broadcast로 중계
//       - KIS PINGPONG에 원본 그대로 응답 (연결 유지)
// 작성일: 2026-11-14
// ============================================================

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

const WS_URL: &str = "ws://ops.koreainvestment.com:21000";
const TR_ID: &str = "H0IFCNT0";

/// 실시간 체결 1건
#[derive(Debug, Clone)]
pub struct RealtimeTick {
    pub code: String,          // 선물 단축 종목코드
    pub price: String,         // FUTS_PRPR (현재가)
    pub change: String,        // FUTS_PRDY_VRSS
    pub change_rate: String,   // FUTS_PRDY_CTRT
    pub volume: String,        // ACML_VOL (누적)
    pub last_qty: String,      // LAST_CNQN (체결량)
    pub time: String,          // BSOP_HOUR
}

/// KIS WebSocket에 연결하고 2종목을 구독.
/// 수신한 체결을 broadcast 채널로 전송.
pub async fn run(
    approval_key: String,
    symbols: Vec<String>,      // 예: ["A01612", "A05612"]
    tx: broadcast::Sender<RealtimeTick>,
) {
    loop {
        match connect_and_listen(&approval_key, &symbols, &tx).await {
            Ok(_) => tracing::warn!("KIS WS 연결 종료, 5초 후 재연결"),
            Err(e) => tracing::error!("KIS WS 오류: {}, 5초 후 재연결", e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn connect_and_listen(
    approval_key: &str,
    symbols: &[String],
    tx: &broadcast::Sender<RealtimeTick>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("KIS WS 연결 시도: {}", WS_URL);
    let (mut ws, _) = connect_async(WS_URL).await?;
    tracing::info!("KIS WS 연결됨");

    // PINGPONG 응답을 위한 mpsc 채널
    let (pong_tx, mut pong_rx) = mpsc::channel::<String>(32);

    // 1) 구독 등록 메시지 전송
    for symbol in symbols {
        let msg = json!({
            "header": {
                "approval_key": approval_key,
                "custtype": "P",
                "tr_type": "1",
                "content-type": "utf-8"
            },
            "body": {
                "input": {
                    "tr_id": TR_ID,
                    "tr_key": symbol
                }
            }
        });
        ws.send(Message::Text(msg.to_string().into())).await?;
        tracing::info!("구독 등록: {}", symbol);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    // 2) 수신 루프 + PINGPONG 응답 전송
    loop {
        tokio::select! {
            // KIS에서 오는 메시지
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        handle_text(&text, tx, &pong_tx);
                    }
                    Some(Ok(Message::Ping(p))) => {
                        ws.send(Message::Pong(p)).await?;
                    }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::warn!("KIS WS 서버에서 연결 종료");
                        break;
                    }
                    Some(Err(e)) => {
                        tracing::error!("KIS WS 수신 오류: {}", e);
                        break;
                    }
                    _ => {}
                }
            }
            // PINGPONG 응답 전송
            pong_msg = pong_rx.recv() => {
                if let Some(text) = pong_msg {
                    if let Err(e) = ws.send(Message::Text(text.into())).await {
                        tracing::error!("PINGPONG 응답 전송 실패: {}", e);
                        break;
                    }
                    tracing::debug!("PINGPONG 응답 전송 완료");
                }
            }
        }
    }
    Ok(())
}

/// KIS WebSocket 텍스트 메시지 처리
fn handle_text(
    text: &str,
    tx: &broadcast::Sender<RealtimeTick>,
    pong_tx: &mpsc::Sender<String>,
) {
    // PINGPONG 감지 → 받은 그대로 되돌려보내기 위해 pong_tx로 전달
    if text.contains("\"tr_id\":\"PINGPONG\"") {
        tracing::debug!("PINGPONG 수신, 응답 전송 예약");
        let _ = pong_tx.try_send(text.to_string());
        return;
    }

    // 실시간 데이터: `|` 구분자로 시작
    // 예: `0|H0IFCNT0|001|A01612^093000^...`
    let parts: Vec<&str> = text.split('|').collect();
    if parts.len() >= 4 && parts[1] == TR_ID {
        let fields: Vec<&str> = parts[3].split('^').collect();
        // H0IFCNT0 필드 순서:
        // 0: FUTS_SHRN_ISCD, 1: BSOP_HOUR, 2: FUTS_PRDY_VRSS, 3: PRDY_VRSS_SIGN,
        // 4: FUTS_PRDY_CTRT, 5: FUTS_PRPR, ..., 9: LAST_CNQN, 10: ACML_VOL, ...
        if fields.len() > 10 {
            let tick = RealtimeTick {
                code: fields[0].to_string(),
                time: fields[1].to_string(),
                change: format!("{}{}", fields[3], fields[2]), // 부호+대비
                change_rate: fields[4].to_string(),
                price: fields[5].to_string(),
                last_qty: fields[9].to_string(),
                volume: fields[10].to_string(),
            };
            tracing::debug!(
                "[WS] {} 현재가={} 대비={} 거래량={}",
                tick.code, tick.price, tick.change, tick.volume
            );
            let _ = tx.send(tick);
        } else {
            tracing::warn!("WS 필드 부족: {}개", fields.len());
        }
        return;
    }

    // 구독 응답 (JSON)
    tracing::debug!("WS 응답: {}", text);
}

