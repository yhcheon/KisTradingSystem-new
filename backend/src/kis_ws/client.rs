use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::core::candles::CandleStore;

use super::protocol::{RealtimeTick, TR_ID, handle_text, parse_tick};

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
    let ws_url = std::env::var("KIS_WS_DOMAIN")
        .unwrap_or_else(|_| "ws://ops.koreainvestment.com:21000".to_string());
    tracing::info!("KIS WS 연결 시도: {}", ws_url);
    let (mut ws, _) = connect_async(&ws_url).await?;
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
                        // 체결 프레임을 한 번만 파싱해 캔들 저장과 브라우저 방송에 동일하게 사용합니다.
                        if let Some(tick) = parse_tick(&text) {
                            {
                                let mut store = candle_store.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                let qty = tick.last_qty.parse::<u64>().unwrap_or_default();
                                store.on_tick(&tick.code, &tick.date, &tick.time, price, qty);
                            }
                            let _ = tx.send(tick);
                        } else {
                            handle_text(&text, &pong_tx);
                        }
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
