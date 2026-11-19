// KIS 실시간 WebSocket 클라이언트. KST 시간표에 따라 주간/야간 구독을 전환합니다.
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::cache::price_cache::PriceCache;
use crate::core::candles::CandleStore;
use crate::core::session_range::SessionRangeMonitor;
use crate::core::market_session::{ActiveSessions, current_active_sessions};

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
    let startup_kst = chrono::Utc::now().with_timezone(&chrono_tz::Asia_Seoul);
    let day_refresh_time = chrono::NaiveTime::from_hms_opt(8, 45, 0).unwrap();
    let mut last_day_refresh_date = if startup_kst.time() >= day_refresh_time {
        Some(startup_kst.date_naive())
    } else {
        None
    };
    let night_start = chrono::NaiveTime::from_hms_opt(17, 55, 0).unwrap();
    let night_end = chrono::NaiveTime::from_hms_opt(6, 5, 0).unwrap();
    let mut last_night_refresh_date = if startup_kst.time() >= night_start {
        Some(startup_kst.date_naive())
    } else if startup_kst.time() < night_end {
        Some(startup_kst.date_naive() - chrono::Duration::days(1))
    } else {
        None
    };

    loop {
        let error_message = match connect_and_listen(
            &approval_key, &symbols, &night_symbols, &tx,
            candle_store.clone(), session_range.clone(), price_cache.clone(),
            &mut last_day_refresh_date,
            &mut last_night_refresh_date,
        ).await {
            Ok(()) => None,
            Err(error) => Some(error.to_string()),
        };
        session_range.lock().await.mark_feed_disconnected();
        match error_message {
            None => tracing::warn!("KIS WS 연결 종료, 5초 후 재연결"),
            Some(error) => tracing::error!("KIS WS 오류: {}, 5초 후 재연결", error),
        }
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    }
}

async fn set_subscription<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    approval_key: &str,
    tr_id: &str,
    symbol: &str,
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let msg = json!({
        "header": {
            "approval_key": approval_key,
            "custtype": "P",
            "tr_type": if enabled { "1" } else { "2" },
            "content-type": "utf-8"
        },
        "body": { "input": { "tr_id": tr_id, "tr_key": symbol } }
    });
    ws.send(Message::Text(msg.to_string().into())).await?;
    tracing::info!(
        "KIS WS {} 요청: tr_id={}, tr_key={}",
        if enabled { "구독 등록" } else { "구독 해제" }, tr_id, symbol
    );
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    Ok(())
}

async fn set_session<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    approval_key: &str,
    tr_id: &str,
    symbols: &[String],
    enabled: bool,
) -> Result<(), Box<dyn std::error::Error>>
where
    S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for symbol in symbols {
        set_subscription(ws, approval_key, tr_id, symbol, enabled).await?;
    }
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
    last_day_refresh_date: &mut Option<chrono::NaiveDate>,
    last_night_refresh_date: &mut Option<chrono::NaiveDate>,
) -> Result<(), Box<dyn std::error::Error>> {
    let ws_url = std::env::var("KIS_WS_DOMAIN")
        .unwrap_or_else(|_| "ws://ops.koreainvestment.com:21000".to_string());
    tracing::info!("KIS WS 연결 시도: {}", ws_url);
    let (mut ws, _) = connect_async(&ws_url).await?;
    tracing::info!("KIS WS 연결됨");

    let (pong_tx, mut pong_rx) = mpsc::channel::<String>(32);

    // 서버 시작/재연결 시 현재 KST 시각에 진행 중인 세션만 즉시 복구합니다.
    let mut active = current_active_sessions();
    let initial_kst = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
    let day_quote_refresh_time = chrono::NaiveTime::from_hms_opt(8, 45, 0).unwrap();
    // 연결이 08:45 이후 시작됐다면 새 프로세스의 캐시는 비어 있으므로 오늘 초기화는 완료된 것으로 봅니다.
    let mut last_day_refresh_date = if initial_kst.time() >= day_quote_refresh_time {
        Some(initial_kst.date_naive())
    } else {
        None
    };
    if active.day {
        set_session(&mut ws, approval_key, DAY_TR_ID, symbols, true).await?;
    }
    if active.night {
        set_session(&mut ws, approval_key, NIGHT_TR_ID, night_symbols, true).await?;
    }
    if !active.day && !active.night {
        tracing::info!("현재 KST 시간은 주간/야간 구독 구간이 아니므로 대기합니다.");
    }
    session_range.lock().await.mark_feed_ready();

    // 연결을 유지하면서 매초 KST 구독 상태를 확인합니다. 재시작/재연결에도 동일한 판정을 사용합니다.
    let mut schedule = tokio::time::interval(std::time::Duration::from_secs(1));
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
                            {
                                let mut monitor = session_range.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                tick.range_alert = monitor.on_tick(
                                    &tick.code, &tick.date, &tick.time, &tick.market_session, price,
                                );
                            }
                            {
                                let mut store = candle_store.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                let qty = tick.last_qty.parse::<u64>().unwrap_or_default();
                                store.on_tick(&tick.code, &tick.date, &tick.time, price, qty);
                            }
                            // 세션 구분을 포함한 캐시 키로 업데이트하므로 주간/야간 가격이 서로 덮이지 않습니다.
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
            _ = schedule.tick() => {
                let now_kst = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
                let today = now_kst.date_naive();

                // 주간 REST 기준가는 매일 08:45 이후 첫 가격 요청에서 새로 조회되도록 캐시를 비웁니다.
                if now_kst.time() >= day_quote_refresh_time && last_day_refresh_date != Some(today) {
                    price_cache.clear_session("day").await;
                    last_day_refresh_date = Some(today);
                    tracing::info!("08:45 주간 초기 시세 갱신 준비: 주간 가격 캐시 초기화");
                }

                let desired: ActiveSessions = current_active_sessions();
                if desired.day != active.day {
                    set_session(&mut ws, approval_key, DAY_TR_ID, symbols, desired.day).await?;
                    active.day = desired.day;
                }
                if desired.night {
                    let night_session_date = if now_kst.time() >= chrono::NaiveTime::from_hms_opt(17, 55, 0).unwrap() {
                        today
                    } else {
                        today - chrono::Duration::days(1)
                    };
                    if *last_night_refresh_date != Some(night_session_date) {
                        // 새 야간 거래일의 REST 기준값을 다음 가격 요청에서 새로 가져옵니다.
                        price_cache.clear_session("night").await;
                        *last_night_refresh_date = Some(night_session_date);
                        tracing::info!("야간 세션 기준일 변경: 야간 가격 캐시 초기화");
                    }
                }
                if desired.night != active.night {
                    set_session(&mut ws, approval_key, NIGHT_TR_ID, night_symbols, desired.night).await?;
                    active.night = desired.night;
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
