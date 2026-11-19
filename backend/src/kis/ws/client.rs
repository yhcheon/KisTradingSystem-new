// ============================================================
// 파일명: kis/ws/client.rs
// 설명: KIS 실시간 WebSocket 클라이언트
//       - approval_key로 KIS WS 접속, 정규/야간 선물 구독
//       - 체결 수신 시 3곳에 전달:
//         ① CandleStore.on_tick() → 분봉 누적
//         ② PriceCache.update_by_tick() → 현재가 갱신 (부팅 1회 REST 이후 체결로만 갱신)
//         ③ broadcast tx.send() → 브라우저 /ws로 실시간 중계
//       - 연결 끊기면 환경변수 정책에 따라 제한적으로 재연결
// ============================================================

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::cache::price_cache::PriceCache;
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
    price_cache: PriceCache,
) {
    // 재연결 정책은 환경변수로 관리합니다. 누락되거나 0 이하/잘못된 값이면
    // 임의의 기본값으로 동작하지 않고 WS 작업을 시작하지 않습니다.
    let reconnect_delay_secs = match std::env::var("KIS_WS_RECONNECT_DELAY_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => {
            tracing::error!(
                "KIS_WS_RECONNECT_DELAY_SECS 환경변수가 없거나 유효하지 않습니다."
            );
            return;
        }
    };
    let max_reconnect_attempts = match std::env::var("KIS_WS_MAX_RECONNECT_ATTEMPTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
    {
        Some(value) => value,
        None => {
            tracing::error!(
                "KIS_WS_MAX_RECONNECT_ATTEMPTS 환경변수가 없거나 유효하지 않습니다."
            );
            return;
        }
    };

    let mut reconnect_attempts = 0usize;
    loop {
        // Box<dyn Error> 값을 별도 블록 안에서 문자열로 바꿔 다음 await 전에 drop합니다.
        let error_message = match connect_and_listen(
            &approval_key,
            &symbols,
            &night_symbols,
            &tx,
            candle_store.clone(),
            session_range.clone(),
            price_cache.clone(),
        )
        .await
        {
            Ok(()) => None,
            Err(error) => Some(error.to_string()),
        };
        session_range.lock().await.mark_feed_disconnected();

        match error_message {
            None => tracing::warn!("KIS WS 연결 종료"),
            Some(error) => tracing::error!("KIS WS 오류: {}", error),
        }

        if reconnect_attempts >= max_reconnect_attempts {
            tracing::error!(
                "KIS WS 최대 재연결 횟수({}) 초과. 자동 재연결을 중단합니다.",
                max_reconnect_attempts
            );
            break;
        }

        reconnect_attempts += 1;
        tracing::warn!(
            "KIS WS 재연결 대기: {}초 후 시도 ({}/{})",
            reconnect_delay_secs,
            reconnect_attempts,
            max_reconnect_attempts
        );
        tokio::time::sleep(std::time::Duration::from_secs(reconnect_delay_secs)).await;
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
    price_cache: PriceCache,
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

                            // 현재가는 체결로만 갱신합니다. REST를 다시 부르지 않습니다.
                            // 반드시 tx.send(tick)보다 먼저 호출해야 합니다. (tick이 이동되기 때문)
                            price_cache.update_by_tick(&tick).await;

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
