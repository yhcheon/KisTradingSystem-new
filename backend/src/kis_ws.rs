// ============================================================
// 파일명: kis_ws.rs
// 설명: KIS 지수선물 실시간체결가(H0IFCNT0) WebSocket 클라이언트
//       - approval_key로 접속, 2종목 구독
//       - 수신한 체결을 broadcast로 중계 + CandleStore::on_tick 호출
//       - PINGPONG 응답 (연결 유지)
// 작성일: 2026-11-14
// ============================================================

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::core::candles::CandleStore;

const WS_URL: &str = "ws://ops.koreainvestment.com:21000";
const TR_ID: &str = "H0IFCNT0";

#[derive(Debug, Clone)]
pub struct RealtimeTick {
    pub code: String,
    pub date: String,
    pub price: String,
    pub change: String,
    pub change_rate: String,
    pub volume: String,
    pub last_qty: String,
    pub time: String,
}

pub async fn run(
    approval_key: String,
    symbols: Vec<String>,
    tx: broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
) {
    loop {
        match connect_and_listen(&approval_key, &symbols, &tx, candle_store.clone()).await {
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
    candle_store: Arc<Mutex<CandleStore>>,
) -> Result<(), Box<dyn std::error::Error>> {
    tracing::info!("KIS WS 연결 시도: {}", WS_URL);
    let (mut ws, _) = connect_async(WS_URL).await?;
    tracing::info!("KIS WS 연결됨");

    let (pong_tx, mut pong_rx) = mpsc::channel::<String>(32);

    for symbol in symbols {
        let msg = json!({
            "header": {
                "approval_key": approval_key,
                "custtype": "P",
                "tr_type": "1",
                "content-type": "utf-8"
            },
            "body": { "input": { "tr_id": TR_ID, "tr_key": symbol } }
        });
        ws.send(Message::Text(msg.to_string().into())).await?;
        tracing::info!("구독 등록: {}", symbol);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        // 체결 데이터 → on_tick 호출
                        let parts: Vec<&str> = text.split('|').collect();
                        if parts.len() >= 4 && parts[1] == TR_ID {
                            let fields: Vec<&str> = parts[3].split('^').collect();
                            if fields.len() > 10 {
                                let code = fields[0].to_string();
                                let price = fields[5].parse::<f64>().unwrap_or(0.0);
                                let qty = fields[9].parse::<u64>().unwrap_or(0);
                                let today = chrono::Local::now().format("%Y%m%d").to_string();
                                {
                                    let mut store = candle_store.lock().await;
                                    store.on_tick(&code, &today, fields[1], price, qty);
                                }
                            }
                        }
                        handle_text(&text, tx, &pong_tx);
                    }
                    Some(Ok(Message::Ping(p))) => { ws.send(Message::Pong(p)).await?; }
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

fn handle_text(text: &str, tx: &broadcast::Sender<RealtimeTick>, pong_tx: &mpsc::Sender<String>) {
    if text.contains("\"tr_id\":\"PINGPONG\"") {
        tracing::debug!("PINGPONG 수신, 응답 전송 예약");
        let _ = pong_tx.try_send(text.to_string());
        return;
    }

    let parts: Vec<&str> = text.split('|').collect();
    if parts.len() >= 4 && parts[1] == TR_ID {
        let fields: Vec<&str> = parts[3].split('^').collect();
        if fields.len() > 10 {
            let today = chrono::Local::now().format("%Y%m%d").to_string();
            let tick = RealtimeTick {
                code: fields[0].to_string(),
                date: today,
                time: fields[1].to_string(),
                change: format!("{}{}", fields[3], fields[2]),
                change_rate: fields[4].to_string(),
                price: fields[5].to_string(),
                last_qty: fields[9].to_string(),
                volume: fields[10].to_string(),
            };
            let _ = tx.send(tick);
        }
        return;
    }
    tracing::debug!("WS 응답: {}", text);
}
