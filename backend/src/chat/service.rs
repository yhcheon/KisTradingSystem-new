use super::{
    ChatError,
    config::{
        ChatCommandConfig, NicknameConfig, chat_commands_config_path, nickname_config_path,
        trusted_proxy_ips_from_env,
    },
    message::{ChatMessage, create_message, create_system_message},
    social::{parse_social_action, render_social_action, render_target_social_action},
};

use chrono::{NaiveDate, Utc};
use rand::Rng;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    net::IpAddr,
    sync::{Arc, Mutex, Weak},
    time::{Duration, Instant},
};
use tokio::sync::broadcast;

use tokio::sync::Notify;
use tokio::task::JoinHandle;

const TICKET_TTL: Duration = Duration::from_secs(30);
const TICKET_REQUEST_WINDOW: Duration = Duration::from_secs(60);
const MAX_TICKETS_PER_IP_PER_WINDOW: usize = 100;
const MAX_PENDING_AND_ACTIVE_PER_IP: usize = 50;
const MAX_TRACKED_CLIENT_IPS: usize = 10_000;
const MESSAGE_RATE_WINDOW: Duration = Duration::from_secs(10);
const MAX_MESSAGES_PER_CONNECTION_PER_WINDOW: usize = 5;
const CLEANUP_INTERVAL: Duration = Duration::from_secs(5);
const TICKET_BYTES: usize = 32;
pub const CHAT_DELIVERY_CAPACITY: usize = 256;

#[derive(Clone)]
pub struct ChatService {
    inner: Arc<ChatServiceInner>,
}

struct ChatServiceInner {
    nicknames: NicknameConfig,
    commands: ChatCommandConfig,
    trusted_proxies: HashSet<IpAddr>,
    state: Mutex<ChatState>,
    messages: broadcast::Sender<ChatMessage>,
    presence: broadcast::Sender<usize>,
}

#[derive(Default)]
struct ChatState {
    tickets: HashMap<String, TicketRecord>,
    active_aliases: HashSet<String>,
    connections: HashMap<u64, ConnectionRecord>,
    ip_limits: HashMap<IpAddr, IpLimits>,
    nickname_sequences: HashMap<String, u64>,
    nickname_sequence_date: Option<NaiveDate>,
    next_connection_id: u64,
}

struct TicketRecord {
    expires_at: Instant,
    client_ip: IpAddr,
    alias: String,
}

struct ConnectionRecord {
    client_ip: IpAddr,
    alias: String,
    message_attempts: VecDeque<Instant>,
}

#[derive(Default)]
struct IpLimits {
    pending_tickets: usize,
    active_connections: usize,
    ticket_attempts: VecDeque<Instant>,
}

#[derive(Debug, Serialize)]
pub struct ChatTicketResponse {
    pub ticket: String,
    pub nickname: String,
    pub expires_in_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChatCommandDefinition {
    pub command: String,
    pub response: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ChatActionDefinition {
    pub action_id: String,
    pub command: String,
    pub aliases: Vec<String>,
    pub example: String,
}

pub struct ChatConnection {
    inner: Weak<ChatServiceInner>,
    id: u64,
    alias: String,
}

impl ChatService {
    pub fn load_from_env() -> Result<Self, ChatError> {
        let nickname_path = nickname_config_path()?;
        let nicknames = NicknameConfig::from_file(&nickname_path)?;
        let command_path = chat_commands_config_path()?;
        let commands = ChatCommandConfig::from_file(&command_path)?;
        let trusted_proxies = trusted_proxy_ips_from_env()?;
        tracing::info!(
            path = %nickname_path.display(),
            nickname_count = nicknames.words.len(),
            "chat nickname config loaded"
        );
        tracing::info!(
            path = %command_path.display(),
            command_count = commands.len(),
            "chat command config loaded"
        );

        Ok(Self::new(nicknames, commands, trusted_proxies))
    }

    fn new(
        nicknames: NicknameConfig,
        commands: ChatCommandConfig,
        trusted_proxies: HashSet<IpAddr>,
    ) -> Self {
        let (messages, _) = broadcast::channel(CHAT_DELIVERY_CAPACITY);
        let (presence, _) = broadcast::channel(CHAT_DELIVERY_CAPACITY);
        Self {
            inner: Arc::new(ChatServiceInner {
                nicknames,
                commands,
                trusted_proxies,
                state: Mutex::new(ChatState::default()),
                messages,
                presence,
            }),
        }
    }

    // NOTE (stabilization, 2026-11-19): the two `unwrap()`s that used to be here were
    // the only `unwrap()`s in this file outside `#[cfg(test)]` test bodies.
    // They lived in this `#[cfg(test)]` helper only — the production path
    // `load_from_env()` above already propagates errors with `?`, and
    // `NicknameConfig` / `ChatCommandConfig::from_file()` already return a
    // human-readable `ChatError::Configuration("... is invalid JSON: ...")`.
    // So production startup never panics on a bad config file: it fails fast
    // with a clear error. This helper keeps its `-> Self` signature so the
    // existing tests don't need to change, but it now panics with the real
    // parse error attached instead of a bare `unwrap()` (which hides the cause).
    #[cfg(test)]
    pub(crate) fn new_for_test(nickname_json: &str) -> Self {
        let nicknames = NicknameConfig::from_json(nickname_json)
            .unwrap_or_else(|error| panic!("test nickname config JSON must be valid: {error}"));
        let commands =
            ChatCommandConfig::from_json(r#"{"version":1,"commands":{"/test":"{nickname} test"}}"#)
                .unwrap_or_else(|error| {
                    panic!("test chat command config JSON must be valid: {error}")
                });
        Self::new(nicknames, commands, HashSet::new())
    }

    pub fn issue_ticket(
        &self,
        peer_ip: IpAddr,
        forwarded_for: Option<&str>,
    ) -> Result<ChatTicketResponse, ChatError> {
        let client_ip = self.resolve_client_ip(peer_ip, forwarded_for)?;
        self.issue_ticket_at(client_ip, Instant::now())
    }

    fn issue_ticket_at(
        &self,
        client_ip: IpAddr,
        now: Instant,
    ) -> Result<ChatTicketResponse, ChatError> {
        self.issue_ticket_at_with_date(client_ip, now, None)
    }

    fn issue_ticket_at_kst_date(
        &self,
        client_ip: IpAddr,
        now: Instant,
        kst_date: NaiveDate,
    ) -> Result<ChatTicketResponse, ChatError> {
        self.issue_ticket_at_with_date(client_ip, now, Some(kst_date))
    }

    fn issue_ticket_at_with_date(
        &self,
        client_ip: IpAddr,
        now: Instant,
        kst_date: Option<NaiveDate>,
    ) -> Result<ChatTicketResponse, ChatError> {
        let mut state = self.lock_state()?;
        prune_expired(&mut state, now);
        if !state.ip_limits.contains_key(&client_ip)
            && state.ip_limits.len() >= MAX_TRACKED_CLIENT_IPS
        {
            return Err(ChatError::RateLimited);
        }

        {
            let limits = state.ip_limits.entry(client_ip).or_default();
            prune_window(&mut limits.ticket_attempts, now, TICKET_REQUEST_WINDOW);
            if limits.ticket_attempts.len() >= MAX_TICKETS_PER_IP_PER_WINDOW {
                return Err(ChatError::RateLimited);
            }
            if limits.pending_tickets + limits.active_connections >= MAX_PENDING_AND_ACTIVE_PER_IP {
                return Err(ChatError::ConnectionLimitReached);
            }
            limits.ticket_attempts.push_back(now);
        }

        let kst_date = kst_date.unwrap_or_else(current_kst_date);
        if state.nickname_sequence_date != Some(kst_date) {
            state.nickname_sequences.clear();
            state.nickname_sequence_date = Some(kst_date);
        }

        let alias = {
            let ChatState {
                nickname_sequences,
                active_aliases,
                ..
            } = &mut *state;
            select_alias(&self.inner.nicknames, nickname_sequences, active_aliases)?
        };
        let token = generate_ticket(&state.tickets);
        state.active_aliases.insert(alias.clone());
        state
            .ip_limits
            .entry(client_ip)
            .or_default()
            .pending_tickets += 1;
        state.tickets.insert(
            token.clone(),
            TicketRecord {
                expires_at: now + TICKET_TTL,
                client_ip,
                alias: alias.clone(),
            },
        );

        Ok(ChatTicketResponse {
            ticket: token,
            nickname: alias,
            expires_in_seconds: TICKET_TTL.as_secs(),
        })
    }

    pub fn consume_ticket(&self, ticket: &str) -> Result<ChatConnection, ChatError> {
        self.consume_ticket_at(ticket, Instant::now())
    }

    fn consume_ticket_at(&self, ticket: &str, now: Instant) -> Result<ChatConnection, ChatError> {
        let mut state = self.lock_state()?;
        let ticket_record = state.tickets.remove(ticket);
        prune_expired(&mut state, now);

        let Some(record) = ticket_record else {
            return Err(ChatError::InvalidTicket);
        };
        if record.expires_at <= now {
            release_pending_ticket(&mut state, &record);
            prune_ip_limits(&mut state);
            return Err(ChatError::ExpiredTicket);
        }

        let Some(connection_id) = state.next_connection_id.checked_add(1) else {
            release_pending_ticket(&mut state, &record);
            prune_ip_limits(&mut state);
            return Err(ChatError::ServiceUnavailable);
        };
        state.next_connection_id = connection_id;
        if let Some(limits) = state.ip_limits.get_mut(&record.client_ip) {
            limits.pending_tickets = limits.pending_tickets.saturating_sub(1);
            limits.active_connections += 1;
        } else {
            release_alias(&mut state, &record.alias);
            return Err(ChatError::ServiceUnavailable);
        }
        state.connections.insert(
            connection_id,
            ConnectionRecord {
                client_ip: record.client_ip,
                alias: record.alias.clone(),
                message_attempts: VecDeque::new(),
            },
        );
        let online_count = state.connections.len();
        let _ = self.inner.presence.send(online_count);

        Ok(ChatConnection {
            inner: Arc::downgrade(&self.inner),
            id: connection_id,
            alias: record.alias,
        })
    }

    pub fn resolve_client_ip(
        &self,
        peer_ip: IpAddr,
        forwarded_for: Option<&str>,
    ) -> Result<IpAddr, ChatError> {
        if !self.inner.trusted_proxies.contains(&peer_ip) {
            return Ok(peer_ip);
        }

        let forwarded_for = forwarded_for.ok_or(ChatError::InvalidClientAddress)?;
        forwarded_for
            .split(',')
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .and_then(|value| value.parse::<IpAddr>().ok())
            .ok_or(ChatError::InvalidClientAddress)
    }

    pub fn cleanup_expired(&self) -> Result<(), ChatError> {
        let mut state = self.lock_state()?;
        prune_expired(&mut state, Instant::now());
        Ok(())
    }

    pub fn subscribe_messages(&self) -> broadcast::Receiver<ChatMessage> {
        self.inner.messages.subscribe()
    }

    /// 입장 알림을 모든 접속자에게 뿌립니다.
    /// - handle_socket에서 구독(subscribe) 직후에 불러야
    ///   들어온 본인도 자기 입장 메시지를 받습니다.
    /// - consume_ticket에서 보내면, 아직 구독 전이라 본인은 못 받습니다.
    pub fn broadcast_entry(&self, connection: &ChatConnection) {
        let leave = create_system_message(
            connection.nickname(),
            format!("{}님이 입장하셨습니다.", connection.nickname()),
        );
        let _ = self.inner.messages.send(leave);
    }

    pub fn subscribe_presence(&self) -> broadcast::Receiver<usize> {
        self.inner.presence.subscribe()
    }

    pub fn online_count(&self) -> usize {
        self.inner
            .state
            .lock()
            .map(|state| state.connections.len())
            .unwrap_or(0)
    }

    pub fn list_commands(&self) -> Vec<ChatCommandDefinition> {
        self.inner
            .commands
            .entries()
            .into_iter()
            .map(|(command, response)| ChatCommandDefinition { command, response })
            .collect()
    }

    pub fn list_actions(&self) -> Vec<ChatActionDefinition> {
        let mut actions = self
            .inner
            .commands
            .actions
            .iter()
            .map(|(action_id, action)| ChatActionDefinition {
                action_id: action_id.clone(),
                command: action.command.clone(),
                aliases: action.aliases.clone(),
                example: action.template_no_arg.replace("{nickname}", "누군가"),
            })
            .collect::<Vec<_>>();
        actions.sort_by(|left, right| left.command.cmp(&right.command));
        actions
    }

    pub fn publish_message(
        &self,
        connection: &ChatConnection,
        text: String,
    ) -> Result<(), ChatError> {
        connection.check_message_rate_limit()?;
        let (text, action_id) = if let Some(expanded) =
            self.inner.commands.expand(&text, connection.nickname())
        {
            (expanded, None)
        } else if let Some(matched) = parse_social_action(&text, &self.inner.commands.actions) {
            let action = self
                .inner
                .commands
                .actions
                .get(&matched.action_id)
                .ok_or(ChatError::ServiceUnavailable)?;
            let target_match =
                if action.template_target.is_some() && action.template_target_no_arg.is_some() {
                    matched.argument.as_deref().and_then(|argument| {
                        let state = self.inner.state.lock().ok()?;
                        state.connections.values().find_map(|online| {
                            let remainder = argument.strip_prefix(&online.alias)?;
                            if remainder.is_empty() {
                                Some((online.alias.clone(), None))
                            } else if remainder.chars().next().is_some_and(char::is_whitespace) {
                                let rest = remainder.trim_start();
                                Some((
                                    online.alias.clone(),
                                    (!rest.is_empty()).then(|| rest.to_string()),
                                ))
                            } else {
                                None
                            }
                        })
                    })
                } else {
                    None
                };
            let rendered = if let Some((target, argument)) = target_match {
                render_target_social_action(
                    action,
                    connection.nickname(),
                    &target,
                    argument.as_deref(),
                )?
            } else {
                render_social_action(action, connection.nickname(), matched.argument.as_deref())?
            };
            (rendered, Some(matched.action_id))
        } else {
            (text, None)
        };
        let message = create_message(connection.nickname(), text, action_id);
        self.inner
            .messages
            .send(message)
            .map(|_| ())
            .map_err(|_| ChatError::ServiceUnavailable)
    }

    pub fn spawn_cleanup_task(&self, shutdown: Arc<Notify>) -> JoinHandle<()> {
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
            loop {
                tokio::select! {
                    _ = shutdown.notified() => {
                        tracing::info!("종료 신호 수신 - chat cleanup 태스크 종료");
                        break;
                    }
                    _ = interval.tick() => {
                        if let Err(error) = service.cleanup_expired() {
                            tracing::error!(error = %error, "chat ticket cleanup failed");
                        }
                    }
                }
            }
        })
    }

    fn lock_state(&self) -> Result<std::sync::MutexGuard<'_, ChatState>, ChatError> {
        self.inner
            .state
            .lock()
            .map_err(|_| ChatError::ServiceUnavailable)
    }
}

impl ChatConnection {
    pub fn nickname(&self) -> &str {
        &self.alias
    }

    pub fn check_message_rate_limit(&self) -> Result<(), ChatError> {
        let inner = self.inner.upgrade().ok_or(ChatError::ServiceUnavailable)?;
        let now = Instant::now();
        let mut state = inner
            .state
            .lock()
            .map_err(|_| ChatError::ServiceUnavailable)?;

        let connection = state
            .connections
            .get_mut(&self.id)
            .ok_or(ChatError::InvalidTicket)?;
        prune_window(&mut connection.message_attempts, now, MESSAGE_RATE_WINDOW);
        if connection.message_attempts.len() >= MAX_MESSAGES_PER_CONNECTION_PER_WINDOW {
            return Err(ChatError::RateLimited);
        }
        connection.message_attempts.push_back(now);
        Ok(())
    }
}

impl Drop for ChatConnection {
    fn drop(&mut self) {
        let Some(inner) = self.inner.upgrade() else {
            return;
        };
        let mut state = match inner.state.lock() {
            Ok(state) => state,
            Err(poisoned) => poisoned.into_inner(),
        };
        let removed = if let Some(connection) = state.connections.remove(&self.id) {
            release_alias(&mut state, &self.alias);
            if let Some(limits) = state.ip_limits.get_mut(&connection.client_ip) {
                limits.active_connections = limits.active_connections.saturating_sub(1);
            }
            true
        } else {
            false
        };
        prune_ip_limits(&mut state);
        let online_count = state.connections.len();
        drop(state);
        if removed {
            let _ = inner.presence.send(online_count);
            // 퇴장 알림: 브라우저를 끄거나 다른 사이트로 가면
            // WebSocket이 끊기고, 이 Drop이 바로 실행됩니다.
            // 여기서 남은 사람들한테 "멍충이-1님이 퇴장하셨습니다."를 뿌립니다.
            // self.alias를 여기서 쓰는 이유: 별명은 이 연결에 붙은 익명 닉네임이라
            // Drop 시점에도 그대로 살아있습니다.
            let leave =
                create_system_message(&self.alias, format!("{}님이 퇴장하셨습니다.", self.alias));
            let _ = inner.messages.send(leave);
        }
    }
}

fn select_alias(
    config: &NicknameConfig,
    nickname_sequences: &mut HashMap<String, u64>,
    active_aliases: &HashSet<String>,
) -> Result<String, ChatError> {
    let word_index = rand::rng().random_range(0..config.words.len());
    next_alias_for_word(
        nickname_sequences,
        active_aliases,
        &config.words[word_index],
    )
}

fn next_alias_for_word(
    nickname_sequences: &mut HashMap<String, u64>,
    active_aliases: &HashSet<String>,
    word: &str,
) -> Result<String, ChatError> {
    let sequence = nickname_sequences.entry(word.to_string()).or_default();
    loop {
        *sequence = sequence
            .checked_add(1)
            .ok_or(ChatError::NicknameSequenceExhausted)?;
        let alias = format!("{word}-{sequence}");
        if !active_aliases.contains(&alias) {
            return Ok(alias);
        }
    }
}

fn current_kst_date() -> NaiveDate {
    kst_date_at(Utc::now())
}

fn kst_date_at(utc_time: chrono::DateTime<Utc>) -> NaiveDate {
    (utc_time + chrono::Duration::hours(9)).date_naive()
}

fn generate_ticket(existing: &HashMap<String, TicketRecord>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut rng = rand::rng();

    loop {
        let mut bytes = [0_u8; TICKET_BYTES];
        rng.fill(&mut bytes);
        let mut token = String::with_capacity(TICKET_BYTES * 2);
        for byte in bytes {
            token.push(HEX[(byte >> 4) as usize] as char);
            token.push(HEX[(byte & 0x0f) as usize] as char);
        }
        if !existing.contains_key(&token) {
            return token;
        }
    }
}

fn prune_expired(state: &mut ChatState, now: Instant) {
    let expired_tickets = state
        .tickets
        .iter()
        .filter_map(|(ticket, record)| (record.expires_at <= now).then(|| ticket.clone()))
        .collect::<Vec<_>>();
    for ticket in expired_tickets {
        if let Some(record) = state.tickets.remove(&ticket) {
            release_pending_ticket(state, &record);
        }
    }

    for limits in state.ip_limits.values_mut() {
        prune_window(&mut limits.ticket_attempts, now, TICKET_REQUEST_WINDOW);
    }
    for connection in state.connections.values_mut() {
        prune_window(&mut connection.message_attempts, now, MESSAGE_RATE_WINDOW);
    }
    prune_ip_limits(state);
}

fn prune_window(queue: &mut VecDeque<Instant>, now: Instant, window: Duration) {
    while queue
        .front()
        .is_some_and(|timestamp| now.duration_since(*timestamp) >= window)
    {
        queue.pop_front();
    }
}

fn release_pending_ticket(state: &mut ChatState, record: &TicketRecord) {
    if let Some(limits) = state.ip_limits.get_mut(&record.client_ip) {
        limits.pending_tickets = limits.pending_tickets.saturating_sub(1);
    }
    release_alias(state, &record.alias);
}

fn release_alias(state: &mut ChatState, alias: &str) {
    state.active_aliases.remove(alias);
}

fn prune_ip_limits(state: &mut ChatState) {
    state.ip_limits.retain(|_, limits| {
        limits.pending_tickets > 0
            || limits.active_connections > 0
            || !limits.ticket_attempts.is_empty()
    });
}

// Split (method A, 2026-11-19): tests live in the sibling file
// `service_tests.rs` and are included here. Production code above is
// unchanged. `service_tests.rs` uses `use super::*;`, so it can still
// reach this file's private helpers (e.g. `ChatService::new`,
// `issue_ticket_at`) because it is a child module of this file.
#[cfg(test)]
#[path = "service_tests.rs"]
mod tests;
