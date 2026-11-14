// ============================================================
// 파일명: server/handlers.rs
// 설명: HTTP + WebSocket 핸들러
//       - GET /api/health, GET /api/prices
//       - GET /api/candles (1분봉)
//       - GET /ws (실시간 체결 푸시)
// 작성일: 2026-11-14
// ============================================================

use crate::cache::redis_cache::{self, CachedPrice};
use crate::core::state::AppState;
use crate::kis;
use crate::master::FuturesSymbol;
use chrono::Timelike;

use axum::{
    Json,
    extract::{
        Query, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::IntoResponse,
};
use serde_json::{Value, json};
use std::collections::HashMap;

pub async fn health(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "status": "ok", "regular": state.regular.name, "mini": state.mini.name }))
}

async fn fetch_one(state: &AppState, symbol: &FuturesSymbol) -> Result<CachedPrice, String> {
    let mut conn = state.redis.lock().await;
    if let Ok(Some(cached)) = redis_cache::get(&mut conn, &symbol.code).await {
        return Ok(cached);
    }
    let price = kis::get_futures_price(
        &state.http_client,
        &state.app_key,
        &state.app_secret,
        &state.access_token,
        &symbol.code,
    )
    .await
    .map_err(|e| format!("KIS 호출 실패({}): {}", symbol.code, e))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let cached = CachedPrice {
        code: symbol.code.clone(),
        name: price.output1.hts_kor_isnm.clone(),
        price: price.output1.futs_prpr.clone(),
        prev_close: price.output1.futs_prdy_clpr.clone(),
        open: price.output1.futs_oprc.clone(),
        high: price.output1.futs_hgpr.clone(),
        low: price.output1.futs_lwpr.clone(),
        change: price.output1.futs_prdy_vrss.clone(),
        change_rate: price.output1.futs_prdy_ctrt.clone(),
        volume: price.output1.acml_vol.clone(),
        cached_at: now,
    };
    let _ = redis_cache::set(&mut conn, &cached, 3).await;
    Ok(cached)
}

pub async fn prices(State(state): State<AppState>) -> Json<Value> {
    let regular = fetch_one(&state, &state.regular).await;
    let mini = fetch_one(&state, &state.mini).await;
    Json(json!({
        "regular": match regular { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
        "mini": match mini { Ok(p) => json!({"ok":true,"data":p}), Err(e) => json!({"ok":false,"error":e}) },
    }))
}

pub async fn candles(
    State(state): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Json<Value> {
    let code = match params.get("code") {
        Some(c) => c.clone(),
        None => return Json(json!({ "ok": false, "error": "code 파라미터 필요" })),
    };
    let interval = params.get("interval").map(|s| s.as_str()).unwrap_or("1m");
    let date = params.get("date").map(|s| s.as_str()).unwrap_or("both");

    let store = state.candles.lock().await;
    let yesterday = (chrono::Local::now() - chrono::Duration::days(1))
        .format("%Y%m%d")
        .to_string(); // ← 수정
    let today = chrono::Local::now().format("%Y%m%d").to_string(); // ← 수정

    let to_json = |candles: &[crate::core::candles::Candle], date_str: &str| -> Vec<Value> {
        candles
            .iter()
            .map(|c| {
                json!({
                    "date": date_str,
                    "time": format!("{}00", c.time),
                    "open": c.open,
                    "high": c.high,
                    "low": c.low,
                    "close": c.close,
                    "volume": c.volume as f64,
                })
            })
            .collect()
    };

    let result = match date {
        "yesterday" => to_json(&store.get(&code, &yesterday, interval), &yesterday),
        "today" => to_json(&store.get(&code, &today, interval), &today),
        _ => {
            let mut all = to_json(&store.get(&code, &yesterday, interval), &yesterday);
            all.extend(to_json(&store.get(&code, &today, interval), &today));
            all
        }
    };

    Json(json!({ "ok": true, "interval": interval, "data": result }))
}

fn subtract_one_minute(hhmmss: &str) -> String {
    if hhmmss.len() < 6 {
        return "154500".to_string();
    }
    let h: u32 = hhmmss[0..2].parse().unwrap_or(15);
    let m: u32 = hhmmss[2..4].parse().unwrap_or(45);
    let s: u32 = hhmmss[4..6].parse().unwrap_or(0);
    let total = h * 3600 + m * 60 + s;
    let new_total = if total >= 60 { total - 60 } else { 0 };
    format!(
        "{:02}{:02}{:02}",
        new_total / 3600,
        (new_total % 3600) / 60,
        new_total % 60
    )
}

pub async fn ws_handler(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    tracing::info!("브라우저 WebSocket 연결됨");
    let mut rx = state.ws_tx.subscribe();

    loop {
        tokio::select! {
            result = rx.recv() => {
                match result {
                    Ok(tick) => {
                        let json = serde_json::to_string(&json!({
                            "type": "tick",
                            "code": tick.code,
                            "price": tick.price,
                            "change": tick.change,
                            "change_rate": tick.change_rate,
                            "volume": tick.volume,
                            "last_qty": tick.last_qty,
                            "time": tick.time,
                        })).unwrap_or_default();
                        if socket.send(Message::Text(json.into())).await.is_err() { break; }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Close(_))) | None => break,
                    _ => {}
                }
            }
        }
    }
    tracing::info!("브라우저 WebSocket 연결 종료");
}

// ── 인증 미들웨어 ──

use crate::auth::jwt;
use axum::http::HeaderMap;

/// Authorization 헤더에서 JWT 추출 + 검증
async fn require_auth(
    headers: &HeaderMap,
    state: &AppState,
) -> Result<jwt::Claims, (axum::http::StatusCode, String)> {
    let auth = match headers.get("authorization").and_then(|v| v.to_str().ok()) {
        Some(value) => value,
        None => {
            tracing::warn!(target: "auth", "인증 실패: Authorization 헤더 누락 또는 유효하지 않은 헤더 값");
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "Authorization 헤더 필요".to_string(),
            ));
        }
    };

    let token = match auth.strip_prefix("Bearer ") {
        Some(value) if !value.is_empty() => value,
        _ => {
            tracing::warn!(target: "auth", "인증 실패: Bearer 토큰 형식 오류 또는 토큰 누락");
            return Err((
                axum::http::StatusCode::UNAUTHORIZED,
                "Bearer 토큰 필요".to_string(),
            ));
        }
    };

    jwt::verify_jwt(token, &state.jwt_secret, &state.supabase_url, &state.http_client).await.map_err(|error| {
        tracing::warn!(target: "auth", status = 401, error = %error, "요청 인증 거부");
        (axum::http::StatusCode::UNAUTHORIZED, error)
    })
}

/// 관리자 여부 확인 (user_profiles.tier = 'admin')
async fn require_admin(
    claims: &jwt::Claims,
    state: &AppState,
) -> Result<(), (axum::http::StatusCode, String)> {
    let query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let resp = state.supabase.select(&query).await.map_err(|error| {
        tracing::error!(target: "admin", error = %error, "관리자 권한 조회 중 Supabase 오류");
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;

    let tier = resp
        .get(0)
        .and_then(|v| v.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("free");

    if tier != "admin" {
        tracing::warn!(target: "admin", tier = %tier, "관리자 API 접근 거부: user_profiles tier가 admin이 아님");
        return Err((
            axum::http::StatusCode::FORBIDDEN,
            "관리자 권한 필요".to_string(),
        ));
    }
    Ok(())
}

// ── 콘텐츠 API ──

/// GET /api/notes
pub async fn notes(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;

    // 등급 확인 + 무료 회원 14시 제한
    let tier_query = format!("user_profiles?user_id=eq.{}&select=tier", claims.sub);
    let tier_resp = state
        .supabase
        .select(&tier_query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let tier = tier_resp
        .get(0)
        .and_then(|v| v.get("tier"))
        .and_then(|v| v.as_str())
        .unwrap_or("free");

    if tier == "free" {
        let kst_hour = chrono::Utc::now()
            .with_timezone(&chrono_tz::Asia::Seoul)
            .hour();
        if kst_hour < 14 {
            return Err((
                axum::http::StatusCode::FORBIDDEN,
                "무료 회원은 오후 2시부터 열람 가능합니다".to_string(),
            ));
        }
    }

    // 글 조회
    let category = params.get("category").map(|s| s.as_str()).unwrap_or("");
    let query = if category.is_empty() {
        "notes?select=id,title,category,created_at&order=created_at.desc&limit=20".to_string()
    } else {
        format!(
            "notes?select=id,title,category,created_at&category=eq.{}&order=created_at.desc&limit=20",
            category
        )
    };

    let data = state
        .supabase
        .select(&query)
        .await
        .map_err(|e| (axum::http::StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(json!({ "ok": true, "data": data })))
}

/// POST /api/admin/notes (관리자만)
pub async fn create_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    tracing::info!(target: "admin", "관리자 글 저장 요청 수신");
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let category = body
        .get("category")
        .and_then(|v| v.as_str())
        .unwrap_or("us_market");

    if title.is_empty() || content.is_empty() {
        return Err((
            axum::http::StatusCode::BAD_REQUEST,
            "제목과 본문 필요".to_string(),
        ));
    }

    let note = json!({
        "title": title,
        "content": content,
        "category": category,
        "author_email": claims.email.unwrap_or_default(),
    });

    state.supabase.insert("notes", &note).await.map_err(|error| {
        tracing::error!(
            target: "admin",
            category = %category,
            error = %error,
            "관리자 글 저장 실패: Supabase INSERT 오류"
        );
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;

    tracing::info!(target: "admin", category = %category, "관리자 글 저장 성공");
    Ok(Json(json!({ "ok": true })))
}


/// GET /api/admin/notes (관리자 글 목록: 본문 포함)
pub async fn admin_notes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;
    let data = state.supabase
        .select("notes?select=id,title,content,category,created_at&order=created_at.desc&limit=200")
        .await
        .map_err(|error| {
            tracing::error!(target: "admin", error = %error, "관리자 글 목록 조회 실패");
            (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
        })?;
    tracing::info!(target: "admin", count = data.as_array().map_or(0, |v| v.len()), "관리자 글 목록 조회 성공");
    Ok(Json(json!({ "ok": true, "data": data })))
}

/// PUT /api/admin/notes (관리자 글 수정)
pub async fn update_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("");
    let title = body.get("title").and_then(|v| v.as_str()).unwrap_or("");
    let content = body.get("content").and_then(|v| v.as_str()).unwrap_or("");
    let category = body.get("category").and_then(|v| v.as_str()).unwrap_or("us_market");
    if id.is_empty() || title.trim().is_empty() || content.trim().is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "글 ID, 제목과 본문이 필요합니다".to_string()));
    }

    let update = json!({ "title": title, "content": content, "category": category });
    state.supabase.update_by_id("notes", id, &update).await.map_err(|error| {
        tracing::error!(target: "admin", note_id = %id, error = %error, "관리자 글 수정 실패");
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;
    tracing::info!(target: "admin", note_id = %id, "관리자 글 수정 성공");
    Ok(Json(json!({ "ok": true })))
}

/// DELETE /api/admin/notes (관리자 글 삭제)
pub async fn delete_note(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (axum::http::StatusCode, String)> {
    let claims = require_auth(&headers, &state).await?;
    require_admin(&claims, &state).await?;

    let id = body.get("id").and_then(|v| v.as_str()).unwrap_or("");
    if id.is_empty() {
        return Err((axum::http::StatusCode::BAD_REQUEST, "글 ID가 필요합니다".to_string()));
    }

    state.supabase.delete_by_id("notes", id).await.map_err(|error| {
        tracing::error!(target: "admin", note_id = %id, error = %error, "관리자 글 삭제 실패");
        (axum::http::StatusCode::INTERNAL_SERVER_ERROR, error)
    })?;
    tracing::info!(target: "admin", note_id = %id, "관리자 글 삭제 성공");
    Ok(Json(json!({ "ok": true })))
}
