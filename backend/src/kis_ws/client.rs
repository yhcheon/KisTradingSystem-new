use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::core::candles::CandleStore;
use crate::core::session_range::SessionRangeMonitor;

use super::protocol::{DAY_TR_ID, NIGHT_TR_ID, RealtimeTick, handle_text, parse_tick};

pub async fn run(
    approval_key: String,
    symbols: Vec<String>,
    night_symbols: Vec<String>,
    tx: broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
    session_range: Arc<Mutex<SessionRangeMonitor>>,
) {
    loop {
        let result = connect_and_listen(
            &approval_key,
            &symbols,
            &night_symbols,
            &tx,
            candle_store.clone(),
            session_range.clone(),
        )
        .await;
        session_range.lock().await.mark_feed_disconnected();

        match result {
            Ok(_) => tracing::warn!("KIS WS 연결 종료, 5초 후 재연결"),
            Err(e) => tracing::error!("KIS WS 오류: {}, 5초 후 재연결", e),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn subscribe<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    approval_key: &str,
    tr_id: &str,
    symbol: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
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
    night_symbols: &[String],
    tx: &broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
    session_range: Arc<Mutex<SessionRangeMonitor>>,
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
    for symbol in night_symbols {
        subscribe(&mut ws, approval_key, NIGHT_TR_ID, symbol).await?;
    }
    session_range.lock().await.mark_feed_ready();

    let mut night_logged = 0usize;
    loop {
        tokio::select! {
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(mut tick) = parse_tick(&text) {
                            if tick.market_session == "night" {
                                if night_logged < 10 {
                                    tracing::info!(
                                        "KRX 야간선물 체결 수신({}/10): code={}, date={}, time={}, price={}, volume={}",
                                        night_logged + 1, tick.code, tick.date, tick.time, tick.price, tick.volume
                                    );
                                } else {
                                    tracing::debug!(
                                        "KRX 야간선물 체결: code={}, date={}, time={}, price={}, volume={}",
                                        tick.code, tick.date, tick.time, tick.price, tick.volume
                                    );
                                }
                                night_logged += 1;
                            }

                            // 정규선물 틱만 주간/야간 시초 레인지 감시기에 전달합니다.
                            {
                                let mut monitor = session_range.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                tick.range_alert = monitor.on_tick(
                                    &tick.code, &tick.date, &tick.time, &tick.market_session, price,
                                );
                            }

                            // 주간/야간 모두 동일한 CandleStore에 영업일 기준으로 누적합니다.
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
