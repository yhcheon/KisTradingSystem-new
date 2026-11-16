use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::core::candles::CandleStore;

use super::protocol::{
    DAY_TR_ID, NIGHT_TR_ID, RealtimeTick, handle_text, parse_tick,
};

pub async fn run(
    approval_key: String,
    symbols: Vec<String>,
    night_symbol: String,
    tx: broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
) {
    loop {
        match connect_and_listen(
            &approval_key, &symbols, &night_symbol, &tx, candle_store.clone()
        ).await {
            Ok(_) => tracing::warn!("KIS WS 연결 종료, 5초 후 재연결"),
            Err(e) => tracing::error!("KIS WS 오류: {}, 5초 후 재연결", e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn subscribe(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>
    >,
    approval_key: &str,
    tr_id: &str,
    symbol: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let msg = json!({
        "header": {
            "approval_key": approval_key,
            "custtype": "P",
            "tr_type": "1",
            "content-type": "utf-8"
        },
        "body": { "input": { "tr_id": tr_id, "tr_key": symbol } }
    });
    ws.send(Message::Text(msg.to_string().into())).await?;
    tracing::info!("KIS WS 구독 요청: tr_id={}, tr_key={}", tr_id, symbol);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    Ok(())
}

async fn connect_and_listen(
    approval_key: &str,
    symbols: &[String],
    night_symbol: &str,
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
        subscribe(&mut ws, approval_key, DAY_TR_ID, symbol).await?;
    }
    subscribe(&mut ws, approval_key, NIGHT_TR_ID, night_symbol).await?;

    let mut night_logged = 0usize;
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(tick) = parse_tick(&text) {
                            let tr_id = text.split('|').nth(1).unwrap_or_default();
                            if tr_id == NIGHT_TR_ID {
                                // 검증 단계: 야간 체결은 우선 로그만 남기고 기존 주간 차트 저장소에는 합치지 않습니다.
                                if night_logged < 10 {
                                    tracing::info!(
                                        "KRX 야간선물 체결 수신({}/10): code={}, time={}, price={}, change={}, volume={}",
                                        night_logged + 1, tick.code, tick.time, tick.price, tick.change, tick.volume
                                    );
                                } else {
                                    tracing::debug!(
                                        "KRX 야간선물 체결: code={}, time={}, price={}, volume={}",
                                        tick.code, tick.time, tick.price, tick.volume
                                    );
                                }
                                night_logged += 1;
                            } else {
                                {
                                    let mut store = candle_store.lock().await;
                                    let price = tick.price.parse::<f64>().unwrap_or_default();
                                    let qty = tick.last_qty.parse::<u64>().unwrap_or_default();
                                    store.on_tick(&tick.code, &tick.date, &tick.time, price, qty);
                                }
                                let _ = tx.send(tick);
                            }
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
