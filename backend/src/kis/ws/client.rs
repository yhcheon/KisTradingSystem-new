// ============================================================
// 파일명: kis/ws/client.rs  (PATCHED)
// 변경점:
//  - run(..., shutdown: Arc<Notify>) 추가
//  - connect_and_listen(..., shutdown) 추가, select!에 종료 분기 추가
//  - 바깥 재연결 루프의 sleep(5s)도 select!로 교체
//  - 종료 시 구독 해제(tr_type=2) 시도 후 리턴. 구독 해제 실패해도 종료는 강행.
//  - 종료 사유를 구분하기 위해 ShutdownReason enum 추가
//  - FIXED2: 모든 Box<dyn Error> -> Box<dyn Error + Send + Sync> (tokio::spawn의 Send 요구 충족)
//    (이 enum은 connect_and_listen의 반환 타입에만 쓰입니다)
// ============================================================
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify, broadcast, mpsc};
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

use crate::cache::price_cache::{CachedPrice, PriceCache};
use crate::core::candles::CandleStore;
use crate::core::market_session::{ActiveSessions, current_active_sessions};
use crate::core::session_range::SessionRangeMonitor;
use crate::kis;

use super::protocol::{DAY_TR_ID, NIGHT_TR_ID, RealtimeTick, handle_text, parse_tick};

/// connect_and_listen이 왜 끝났는지. Shutdown이면 바깥 루프가 재연결하지 않습니다.
enum ShutdownReason { Disconnected, Shutdown }

pub async fn run(
    approval_key: String,
    symbols: Vec<String>,
    night_symbols: Vec<String>,
    tx: broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
    session_range: Arc<Mutex<SessionRangeMonitor>>,
    price_cache: PriceCache,
    http_client: reqwest::Client,
    app_key: String,
    app_secret: String,
    access_token: String,
    shutdown: Arc<Notify>, // PATCH
) {
    let startup_kst = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
    let day_refresh_time = chrono::NaiveTime::from_hms_opt(8, 45, 0).unwrap();
    let mut last_day_refresh_date = if startup_kst.time() >= day_refresh_time {
        Some(startup_kst.date_naive())
    } else { None };
    let night_start = chrono::NaiveTime::from_hms_opt(17, 55, 0).unwrap();
    let night_end = chrono::NaiveTime::from_hms_opt(6, 5, 0).unwrap();
    let mut last_night_refresh_date = if startup_kst.time() >= night_start {
        Some(startup_kst.date_naive())
    } else if startup_kst.time() < night_end {
        Some(startup_kst.date_naive() - chrono::Duration::days(1))
    } else { None };

    loop {
        let outcome = connect_and_listen(
            &approval_key, &symbols, &night_symbols, &tx,
            candle_store.clone(), session_range.clone(), price_cache.clone(),
            &http_client, &app_key, &app_secret, &access_token,
            &mut last_day_refresh_date, &mut last_night_refresh_date,
            shutdown.clone(), // PATCH
        ).await;

        // FIXED (Send): Box<dyn Error + Send + Sync>는 Send가 아니라서 await 너머로 들고 가면
        // tokio::spawn이 요구하는 Future + Send를 만족하지 못합니다.
        // 그래서 여기서 즉시 String으로 바꾸고 Box를 버립니다.
        // 원래 코드도 같은 이유로 Err를 바로 to_string() 하고 있었습니다.
        // FIXED2: outcome을 블록 안에서 소비하고 drop해서, 다음 await 전에
        // Box가 살아남지 않게 합니다. (is_shutdown / error_message는 Send인 String/bool만 남음)
        let (is_shutdown, error_message): (bool, Option<String>) = {
            let is_shutdown = matches!(outcome, Ok(ShutdownReason::Shutdown));
            let error_message: Option<String> = match outcome {
                Ok(_) => None,
                Err(error) => Some(error.to_string()),
            };
            (is_shutdown, error_message)
        };
        if is_shutdown {
            tracing::info!("KIS WS 종료 신호 수신 - 재연결 없이 종료");
            break;
        }
        match &error_message {
            None => tracing::warn!("KIS WS 연결 종료, 5초 후 재연결"),
            Some(error) => tracing::error!("KIS WS 오류: {}, 5초 후 재연결", error),
        }
        session_range.lock().await.mark_feed_disconnected();
        // PATCH: 재연결 대기 5초도 종료 신호로 깰 수 있게 select!
        tokio::select! {
            _ = shutdown.notified() => {
                tracing::info!("KIS WS 재연결 대기 중 종료 신호 수신 - 종료");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_secs(5)) => {}
        }
    }
    tracing::info!("KIS WS 태스크 종료 완료");
}

async fn set_subscription<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    approval_key: &str, tr_id: &str, symbol: &str, enabled: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    let msg = json!({
        "header": {
            "approval_key": approval_key, "custtype": "P",
            "tr_type": if enabled { "1" } else { "2" }, "content-type": "utf-8"
        },
        "body": { "input": { "tr_id": tr_id, "tr_key": symbol } }
    });
    ws.send(Message::Text(msg.to_string().into())).await?;
    tracing::info!("KIS WS {} 요청: tr_id={}, tr_key={}",
        if enabled { "구독 등록" } else { "구독 해제" }, tr_id, symbol);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    Ok(())
}

async fn set_session<S>(
    ws: &mut tokio_tungstenite::WebSocketStream<S>,
    approval_key: &str, tr_id: &str, symbols: &[String], enabled: bool,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin,
{
    for symbol in symbols { set_subscription(ws, approval_key, tr_id, symbol, enabled).await?; }
    Ok(())
}

async fn refresh_session_quote(
    http_client: &reqwest::Client, app_key: &str, app_secret: &str, access_token: &str,
    code: &str, market_div: &str, session: &str, price_cache: &PriceCache,
) -> Result<(), String> {
    let response = kis::get_futures_price_for_market(
        http_client, app_key, app_secret, access_token, market_div, code,
    ).await.map_err(|error| error.to_string())?;
    let cached_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs();
    let output = response.output1;
    let cached = CachedPrice {
        code: code.to_string(), name: output.hts_kor_isnm, price: output.futs_prpr,
        prev_close: output.futs_prdy_clpr, open: output.futs_oprc, high: output.futs_hgpr,
        low: output.futs_lwpr, change: output.futs_prdy_vrss, change_rate: output.futs_prdy_ctrt,
        volume: output.acml_vol, cached_at,
    };
    price_cache.set(&cached, session).await;
    tracing::info!("KIS {} REST 기준 시세 갱신: code={}, price={}", session, code, cached.price);
    Ok(())
}

async fn connect_and_listen(
    approval_key: &str, symbols: &[String], night_symbols: &[String],
    tx: &broadcast::Sender<RealtimeTick>,
    candle_store: Arc<Mutex<CandleStore>>,
    session_range: Arc<Mutex<SessionRangeMonitor>>,
    price_cache: PriceCache,
    http_client: &reqwest::Client, app_key: &str, app_secret: &str, access_token: &str,
    last_day_refresh_date: &mut Option<chrono::NaiveDate>,
    last_night_refresh_date: &mut Option<chrono::NaiveDate>,
    shutdown: Arc<Notify>, // PATCH
) -> Result<ShutdownReason, Box<dyn std::error::Error + Send + Sync>> {
    let ws_url = std::env::var("KIS_WS_DOMAIN").unwrap_or_else(|_| "ws://ops.koreainvestment.com:21000".to_string());
    tracing::info!("KIS WS 연결 시도: {}", ws_url);
    let (mut ws, _) = connect_async(&ws_url).await?;
    tracing::info!("KIS WS 연결됨");

    let (pong_tx, mut pong_rx) = mpsc::channel::<String>(32);
    let mut active = current_active_sessions();
    let day_quote_refresh_time = chrono::NaiveTime::from_hms_opt(8, 45, 0).unwrap();
    if active.day { set_session(&mut ws, approval_key, DAY_TR_ID, symbols, true).await?; }
    if active.night { set_session(&mut ws, approval_key, NIGHT_TR_ID, night_symbols, true).await?; }
    if !active.day && !active.night {
        tracing::info!("현재 KST 시간은 주간/야간 구독 구간이 아니므로 대기합니다.");
    }
    session_range.lock().await.mark_feed_ready();

    let mut schedule = tokio::time::interval(std::time::Duration::from_secs(1));
    let mut night_logged = 0usize;
    loop {
        tokio::select! {
            // PATCH: 종료 분기 - 구독 해제 시도 후 Shutdown으로 리턴
            _ = shutdown.notified() => {
                tracing::info!("KIS WS 종료 신호 수신 - 구독 해제 시도");
                // 구독 해제에 실패해도 종료는 강행합니다. close()는 best-effort입니다.
                if active.day {
                    let _ = set_session(&mut ws, approval_key, DAY_TR_ID, symbols, false).await;
                }
                if active.night {
                    let _ = set_session(&mut ws, approval_key, NIGHT_TR_ID, night_symbols, false).await;
                }
                let _ = ws.close(None).await;
                return Ok(ShutdownReason::Shutdown);
            }
            msg = ws.next() => {
                match msg {
                    Some(Ok(Message::Text(text))) => {
                        if let Some(mut tick) = parse_tick(&text) {
                            if tick.market_session == "night" {
                                if night_logged < 10 {
                                    tracing::info!("KRX 야간선물 체결 수신({}/10): code={}, date={}, time={}, price={}, volume={}",
                                        night_logged + 1, tick.code, tick.date, tick.time, tick.price, tick.volume);
                                } else {
                                    tracing::debug!("KRX 야간선물 체결: code={}, date={}, time={}, price={}, volume={}",
                                        tick.code, tick.date, tick.time, tick.price, tick.volume);
                                }
                                night_logged += 1;
                            }
                            {
                                let mut monitor = session_range.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                tick.range_alert = monitor.on_tick(&tick.code, &tick.date, &tick.time, &tick.market_session, price);
                            }
                            {
                                let mut store = candle_store.lock().await;
                                let price = tick.price.parse::<f64>().unwrap_or_default();
                                let qty = tick.last_qty.parse::<u64>().unwrap_or_default();
                                store.on_tick(&tick.code, &tick.date, &tick.time, price, qty);
                            }
                            price_cache.update_by_tick(&tick).await;
                            let _ = tx.send(tick);
                        } else {
                            handle_text(&text, &pong_tx);
                        }
                    }
                    Some(Ok(Message::Ping(p))) => { ws.send(Message::Pong(p)).await?; }
                    Some(Ok(Message::Close(_))) | None => {
                        tracing::warn!("KIS WS 서버에서 연결 종료");
                        return Ok(ShutdownReason::Disconnected);
                    }
                    Some(Err(e)) => {
                        tracing::error!("KIS WS 수신 오류: {}", e);
                        return Ok(ShutdownReason::Disconnected);
                    }
                    _ => {}
                }
            }
            _ = schedule.tick() => {
                // 기존 1초 스케줄 로직 그대로 (생략 없이 유지)
                let now_kst = chrono::Utc::now().with_timezone(&chrono_tz::Asia::Seoul);
                let today = now_kst.date_naive();
                if now_kst.time() >= day_quote_refresh_time && *last_day_refresh_date != Some(today) {
                    price_cache.clear_session("day").await;
                    for symbol in symbols {
                        if let Err(error) = refresh_session_quote(http_client, app_key, app_secret, access_token, symbol, "F", "day", &price_cache).await {
                            tracing::warn!("08:45 주간 REST 기준 시세 조회 실패: code={}, error={}", symbol, error);
                        }
                    }
                    *last_day_refresh_date = Some(today);
                }
                let desired: ActiveSessions = current_active_sessions();
                if desired.day != active.day {
                    set_session(&mut ws, approval_key, DAY_TR_ID, symbols, desired.day).await?;
                    active.day = desired.day;
                }
                if desired.night {
                    let night_session_date = if now_kst.time() >= chrono::NaiveTime::from_hms_opt(17, 55, 0).unwrap() {
                        today
                    } else { today - chrono::Duration::days(1) };
                    if *last_night_refresh_date != Some(night_session_date) {
                        price_cache.clear_session("night").await;
                        for symbol in night_symbols {
                            if let Err(error) = refresh_session_quote(http_client, app_key, app_secret, access_token, symbol, "CM", "night", &price_cache).await {
                                tracing::warn!("야간 REST 기준 시세 조회 실패: code={}, error={}", symbol, error);
                            }
                        }
                        *last_night_refresh_date = Some(night_session_date);
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
                        return Ok(ShutdownReason::Disconnected);
                    }
                    tracing::debug!("PINGPONG 응답 전송 완료");
                }
            }
        }
    }
}

