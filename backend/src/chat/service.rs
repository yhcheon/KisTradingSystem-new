use super::{
    ChatError,
    config::{
        ChatCommandConfig, NicknameConfig, chat_commands_config_path, nickname_config_path,
        trusted_proxy_ips_from_env,
    },
    message::{ChatMessage, create_message},
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
        Self {
            inner: Arc::new(ChatServiceInner {
                nicknames,
                commands,
                trusted_proxies,
                state: Mutex::new(ChatState::default()),
                messages,
            }),
        }
    }

    #[cfg(test)]
    pub(crate) fn new_for_test(nickname_json: &str) -> Self {
        let nicknames = NicknameConfig::from_json(nickname_json).unwrap();
        let commands =
            ChatCommandConfig::from_json(r#"{"version":1,"commands":{"/test":"{nickname} test"}}"#)
                .unwrap();
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
                message_attempts: VecDeque::new(),
            },
        );

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

    pub fn publish_message(
        &self,
        connection: &ChatConnection,
        text: String,
    ) -> Result<(), ChatError> {
        connection.check_message_rate_limit()?;
        let text = self
            .inner
            .commands
            .expand(&text, connection.nickname())
            .unwrap_or(text);
        let message = create_message(connection.nickname(), text);
        self.inner
            .messages
            .send(message)
            .map(|_| ())
            .map_err(|_| ChatError::ServiceUnavailable)
    }

    pub fn spawn_cleanup_task(&self) {
        let service = self.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(CLEANUP_INTERVAL);
            loop {
                interval.tick().await;
                if let Err(error) = service.cleanup_expired() {
                    tracing::error!(error = %error, "chat ticket cleanup failed");
                }
            }
        });
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
        if let Some(connection) = state.connections.remove(&self.id) {
            release_alias(&mut state, &self.alias);
            if let Some(limits) = state.ip_limits.get_mut(&connection.client_ip) {
                limits.active_connections = limits.active_connections.saturating_sub(1);
            }
        }
        prune_ip_limits(&mut state);
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::{net::Ipv4Addr, str::FromStr};

    const TEST_COMMANDS: &str =
        r#"{"version":1,"commands":{"/wave":"{nickname} waves","/hello":"{nickname} says hello"}}"#;

    fn service(words: &[&str]) -> ChatService {
        let words = words
            .iter()
            .map(|word| format!("\"{word}\""))
            .collect::<Vec<_>>()
            .join(",");
        let json = format!(r#"{{"words":[{words}]}}"#);
        service_with_commands(&json, TEST_COMMANDS)
    }

    fn service_with_commands(nickname_json: &str, command_json: &str) -> ChatService {
        ChatService::new(
            NicknameConfig::from_json(nickname_json).unwrap(),
            ChatCommandConfig::from_json(command_json).unwrap(),
            HashSet::new(),
        )
    }

    fn ip(last: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(192, 0, 2, last))
    }

    #[test]
    fn issues_random_alias_and_single_use_ticket() {
        let service = service(&["one", "two"]);
        let issued = service.issue_ticket(ip(1), None).unwrap();
        let (word, sequence) = issued.nickname.rsplit_once('-').unwrap();
        assert!(["one", "two"].contains(&word));
        assert_eq!(sequence, "1");
        assert_eq!(issued.ticket.len(), 64);
        assert_eq!(issued.expires_in_seconds, TICKET_TTL.as_secs());

        let connection = service.consume_ticket(&issued.ticket).unwrap();
        assert_eq!(connection.nickname(), issued.nickname);
        assert_eq!(
            service.consume_ticket(&issued.ticket).err(),
            Some(ChatError::InvalidTicket)
        );
    }

    #[test]
    fn increments_the_sequence_when_a_ticket_expires_unused() {
        let service = service(&["only"]);
        let now = Instant::now();
        let issued = service.issue_ticket_at(ip(1), now).unwrap();
        assert_eq!(issued.nickname, "only-1");
        assert_eq!(
            service
                .consume_ticket_at(&issued.ticket, now + TICKET_TTL)
                .err(),
            Some(ChatError::ExpiredTicket)
        );

        let replacement = service.issue_ticket_at(ip(1), now + TICKET_TTL).unwrap();
        assert_eq!(replacement.nickname, "only-2");
    }

    #[test]
    fn expands_an_exact_command_using_the_server_assigned_nickname() {
        let service = service(&["guest"]);
        let mut messages = service.subscribe_messages();
        let ticket = service.issue_ticket(ip(1), None).unwrap();
        let connection = service.consume_ticket(&ticket.ticket).unwrap();

        service
            .publish_message(&connection, "/wave".to_string())
            .unwrap();

        let message = messages.try_recv().unwrap();
        assert_eq!(message.nickname, "guest-1");
        assert_eq!(message.text, "guest-1 waves");
    }

    #[test]
    fn does_not_expand_a_command_prefix_or_unknown_command() {
        let service = service(&["guest"]);
        let mut messages = service.subscribe_messages();
        let ticket = service.issue_ticket(ip(1), None).unwrap();
        let connection = service.consume_ticket(&ticket.ticket).unwrap();

        for text in ["/wave now", "/unknown"] {
            service
                .publish_message(&connection, text.to_string())
                .unwrap();
            assert_eq!(messages.try_recv().unwrap().text, text);
        }
    }

    #[test]
    fn passes_ordinary_messages_through_unchanged() {
        let service = service(&["guest"]);
        let mut messages = service.subscribe_messages();
        let ticket = service.issue_ticket(ip(1), None).unwrap();
        let connection = service.consume_ticket(&ticket.ticket).unwrap();

        service
            .publish_message(&connection, "hello everyone".to_string())
            .unwrap();

        assert_eq!(messages.try_recv().unwrap().text, "hello everyone");
    }

    #[test]
    fn continues_the_sequence_after_a_connection_disconnects() {
        let service = service(&["one"]);
        let first = service.issue_ticket(ip(1), None).unwrap();
        assert_eq!(first.nickname, "one-1");
        let first_connection = service.consume_ticket(&first.ticket).unwrap();
        let second = service.issue_ticket(ip(2), None).unwrap();
        assert_eq!(second.nickname, "one-2");
        let second_connection = service.consume_ticket(&second.ticket).unwrap();
        assert_eq!(second_connection.nickname(), "one-2");

        drop(first_connection);
        assert_eq!(service.issue_ticket(ip(3), None).unwrap().nickname, "one-3");
    }

    #[test]
    fn increments_each_word_sequence_independently_when_interleaved() {
        let mut sequences = HashMap::new();
        let active_aliases = HashSet::new();
        assert_eq!(
            next_alias_for_word(&mut sequences, &active_aliases, "one").unwrap(),
            "one-1"
        );
        assert_eq!(
            next_alias_for_word(&mut sequences, &active_aliases, "two").unwrap(),
            "two-1"
        );
        assert_eq!(
            next_alias_for_word(&mut sequences, &active_aliases, "one").unwrap(),
            "one-2"
        );
        assert_eq!(
            next_alias_for_word(&mut sequences, &active_aliases, "two").unwrap(),
            "two-2"
        );
    }

    #[test]
    fn resets_sequences_at_kst_midnight() {
        let before_midnight_utc = Utc
            .with_ymd_and_hms(2026, 11, 18, 14, 59, 59)
            .single()
            .unwrap();
        let after_midnight_utc = Utc
            .with_ymd_and_hms(2026, 11, 18, 15, 0, 0)
            .single()
            .unwrap();
        let before_midnight = kst_date_at(before_midnight_utc);
        let after_midnight = kst_date_at(after_midnight_utc);
        assert_eq!(
            before_midnight,
            NaiveDate::from_ymd_opt(2026, 11, 18).unwrap()
        );
        assert_eq!(
            after_midnight,
            NaiveDate::from_ymd_opt(2026, 11, 19).unwrap()
        );

        let service = service(&["one"]);
        let now = Instant::now();
        let first = service
            .issue_ticket_at_kst_date(ip(1), now, before_midnight)
            .unwrap();
        assert_eq!(first.nickname, "one-1");
        drop(service.consume_ticket_at(&first.ticket, now).unwrap());

        let second = service
            .issue_ticket_at_kst_date(ip(2), now, before_midnight)
            .unwrap();
        assert_eq!(second.nickname, "one-2");
        drop(service.consume_ticket_at(&second.ticket, now).unwrap());

        let next_day = service
            .issue_ticket_at_kst_date(ip(3), now, after_midnight)
            .unwrap();
        assert_eq!(next_day.nickname, "one-1");
    }

    #[test]
    fn skips_previous_day_aliases_that_are_still_active() {
        let service = service(&["one"]);
        let now = Instant::now();
        let yesterday = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 11, 19).unwrap();
        let yesterday_ticket = service
            .issue_ticket_at_kst_date(ip(1), now, yesterday)
            .unwrap();
        assert_eq!(yesterday_ticket.nickname, "one-1");
        let active_connection = service
            .consume_ticket_at(&yesterday_ticket.ticket, now)
            .unwrap();

        let first_today = service.issue_ticket_at_kst_date(ip(2), now, today).unwrap();
        assert_eq!(first_today.nickname, "one-2");
        let second_today = service.issue_ticket_at_kst_date(ip(3), now, today).unwrap();
        assert_eq!(second_today.nickname, "one-3");

        drop(active_connection);
    }

    #[test]
    fn reports_sequence_overflow_instead_of_reusing_a_nickname() {
        let mut sequences = HashMap::from([("one".to_string(), u64::MAX)]);
        let active_aliases = HashSet::new();
        assert_eq!(
            next_alias_for_word(&mut sequences, &active_aliases, "one").err(),
            Some(ChatError::NicknameSequenceExhausted)
        );
    }

    #[test]
    fn reserves_unique_aliases_for_concurrent_ticket_requests() {
        let service = service(&["word"]);
        let barrier = Arc::new(std::sync::Barrier::new(16));
        let threads = (0..16)
            .map(|_| {
                let service = service.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait();
                    service.issue_ticket(ip(1), None).unwrap().nickname
                })
            })
            .collect::<Vec<_>>();
        let aliases = threads
            .into_iter()
            .map(|thread| thread.join().unwrap())
            .collect::<HashSet<_>>();
        assert_eq!(aliases.len(), 16);
    }

    #[test]
    fn enforces_ticket_connection_and_message_limits() {
        let service = service(&["word"]);
        let now = Instant::now();
        let mut tickets = Vec::new();
        for _ in 0..MAX_PENDING_AND_ACTIVE_PER_IP {
            tickets.push(service.issue_ticket_at(ip(1), now).unwrap());
        }
        assert_eq!(
            service.issue_ticket_at(ip(1), now).err(),
            Some(ChatError::ConnectionLimitReached)
        );

        let connections = tickets
            .into_iter()
            .map(|ticket| service.consume_ticket(&ticket.ticket).unwrap())
            .collect::<Vec<_>>();
        assert_eq!(
            service.issue_ticket_at(ip(1), now).err(),
            Some(ChatError::ConnectionLimitReached)
        );
        let mut messages = service.subscribe_messages();
        for _ in 0..MAX_MESSAGES_PER_CONNECTION_PER_WINDOW {
            service
                .publish_message(&connections[0], "hello".to_string())
                .unwrap();
        }
        assert_eq!(
            connections[0].check_message_rate_limit().err(),
            Some(ChatError::RateLimited)
        );
        assert_eq!(
            service
                .publish_message(&connections[0], "hello".to_string())
                .err(),
            Some(ChatError::RateLimited)
        );
        for _ in 0..MAX_MESSAGES_PER_CONNECTION_PER_WINDOW {
            assert_eq!(messages.try_recv().unwrap().text, "hello");
        }
    }

    #[test]
    fn limits_ticket_requests_per_ip_and_resets_after_window() {
        let service = service(&["word"]);
        let now = Instant::now();
        for _ in 0..MAX_TICKETS_PER_IP_PER_WINDOW {
            let ticket = service.issue_ticket_at(ip(1), now).unwrap();
            drop(service.consume_ticket_at(&ticket.ticket, now).unwrap());
        }
        assert_eq!(
            service.issue_ticket_at(ip(1), now).err(),
            Some(ChatError::RateLimited)
        );
        assert!(
            service
                .issue_ticket_at(ip(1), now + TICKET_REQUEST_WINDOW)
                .is_ok()
        );
    }

    #[test]
    fn honors_forwarded_ip_only_for_configured_proxies() {
        let proxy_ip = IpAddr::from_str("127.0.0.1").unwrap();
        let trusted = ChatService::new(
            NicknameConfig::from_json(r#"{"words":["word"]}"#).unwrap(),
            ChatCommandConfig::from_json(TEST_COMMANDS).unwrap(),
            HashSet::from([proxy_ip]),
        );
        assert_eq!(
            trusted
                .resolve_client_ip(proxy_ip, Some("192.0.2.10, 10.0.0.1"))
                .unwrap(),
            ip(10)
        );
        assert_eq!(
            trusted.resolve_client_ip(proxy_ip, None),
            Err(ChatError::InvalidClientAddress)
        );
        assert_eq!(
            trusted
                .resolve_client_ip(ip(9), Some("192.0.2.10"))
                .unwrap(),
            ip(9)
        );
    }
}
