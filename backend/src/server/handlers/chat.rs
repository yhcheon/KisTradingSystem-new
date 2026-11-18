use crate::{
    chat::{ChatError, ChatTicketResponse},
    core::state::AppState,
};
use axum::{
    Json,
    extract::{ConnectInfo, State},
    http::{HeaderMap, StatusCode},
};
use serde::Serialize;
use std::net::SocketAddr;

#[derive(Serialize)]
pub(crate) struct TicketError {
    code: &'static str,
}

pub(crate) async fn issue_ticket(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
) -> Result<Json<ChatTicketResponse>, (StatusCode, Json<TicketError>)> {
    let forwarded_for = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok());

    state
        .chat
        .issue_ticket(peer.ip(), forwarded_for)
        .map(Json)
        .map_err(ticket_error_response)
}

fn ticket_error_response(error: ChatError) -> (StatusCode, Json<TicketError>) {
    let (status, code) = match error {
        ChatError::RateLimited => (StatusCode::TOO_MANY_REQUESTS, "rate_limited"),
        ChatError::ConnectionLimitReached | ChatError::AliasPoolExhausted => {
            (StatusCode::TOO_MANY_REQUESTS, "chat_capacity_reached")
        }
        ChatError::InvalidClientAddress => (StatusCode::BAD_REQUEST, "invalid_client_address"),
        _ => (StatusCode::SERVICE_UNAVAILABLE, "chat_unavailable"),
    };
    (status, Json(TicketError { code }))
}
