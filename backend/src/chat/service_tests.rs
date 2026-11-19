
// Tests for `chat/service.rs` (split out by method A, 2026-11-19).
// This file is NOT a standalone module — it is included from `service.rs` via:
//     #[cfg(test)]
//     #[path = "service_tests.rs"]
//     mod tests;
// So `use super::*;` below still refers to `service.rs` and can reach its
// private helpers. Do not add `mod tests { ... }` wrapper here.
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
    fn publishes_social_actions_and_preserves_unmatched_messages() {
        const SOCIAL_COMMANDS: &str = r#"{
            "version": 2,
            "commands": {
                "/wave": "{nickname} waves"
            },
            "actions": {
                "laugh": {
                    "command": "웃",
                    "aliases": ["웃음"],
                    "template": "{nickname}님이{argument} 웃습니다. ^^",
                    "template_no_arg": "{nickname}님이 웃습니다. ^^"
                }
            }
        }"#;
        let service = service_with_commands(r#"{"words":["guest"]}"#, SOCIAL_COMMANDS);
        let mut messages = service.subscribe_messages();
        let ticket = service.issue_ticket(ip(1), None).unwrap();
        let connection = service.consume_ticket(&ticket.ticket).unwrap();

        for (input, expected) in [
            ("즐겁게 신나서 웃", "guest-1님이 즐겁게 신나서 웃습니다. ^^"),
            ("웃", "guest-1님이 웃습니다. ^^"),
            ("신나서 웃음", "guest-1님이 신나서 웃습니다. ^^"),
            ("즐겁게 웃는다", "즐겁게 웃는다"),
            ("/wave", "guest-1 waves"),
        ] {
            service
                .publish_message(&connection, input.to_string())
                .unwrap();
            let message = messages.try_recv().unwrap();
            assert_eq!(message.nickname, "guest-1");
            assert_eq!(message.text, expected, "input: {input}");
        }
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

    #[tokio::test]
    async fn targets_only_online_nicknames_and_tracks_presence() {
        const SOCIAL_COMMANDS: &str = r#"{
            "version": 2,
            "commands": {"/wave": "{nickname} waves"},
            "actions": {
                "laugh": {
                    "command": "웃",
                    "aliases": ["웃음"],
                    "template": "{nickname}님이{argument} 웃습니다. ^^",
                    "template_no_arg": "{nickname}님이 웃습니다. ^^",
                    "template_target": "{nickname}님이 {target}님을 보고{argument} 웃습니다. ^^",
                    "template_target_no_arg": "{nickname}님이 {target}님을 보고 웃습니다. ^^"
                },
                "greet": {
                    "command": "인사",
                    "aliases": [],
                    "template": "{nickname}님이{argument} 인사합니다.",
                    "template_no_arg": "{nickname}님이 인사합니다.",
                    "template_target": "{nickname}님이 {target}님에게{argument} 인사합니다.",
                    "template_target_no_arg": "{nickname}님이 {target}님에게 인사합니다."
                }
            }
        }"#;
        let service = service_with_commands(r#"{"words":["guest"]}"#, SOCIAL_COMMANDS);
        let mut presence = service.subscribe_presence();
        let mut messages = service.subscribe_messages();
        let first_ticket = service.issue_ticket(ip(1), None).unwrap();
        let first = service.consume_ticket(&first_ticket.ticket).unwrap();
        assert_eq!(presence.recv().await.unwrap(), 1);
        let second_ticket = service.issue_ticket(ip(2), None).unwrap();
        let second = service.consume_ticket(&second_ticket.ticket).unwrap();
        assert_eq!(presence.recv().await.unwrap(), 2);
        assert_eq!(service.online_count(), 2);

        service
            .publish_message(&first, format!("{} 웃", second.nickname()))
            .unwrap();
        let message = messages.recv().await.unwrap();
        assert_eq!(
            message.text,
            format!(
                "{}님이 {}님을 보고 웃습니다. ^^",
                first.nickname(),
                second.nickname()
            )
        );
        assert_eq!(message.action_id.as_deref(), Some("laugh"));

        service
            .publish_message(&first, format!("{} 인사", second.nickname()))
            .unwrap();
        let greeting = messages.recv().await.unwrap();
        assert_eq!(
            greeting.text,
            format!(
                "{}님이 {}님에게 인사합니다.",
                first.nickname(),
                second.nickname()
            )
        );

        service
            .publish_message(&first, "offline-999 웃".to_string())
            .unwrap();
        let fallback = messages.recv().await.unwrap();
        assert_eq!(
            fallback.text,
            format!("{}님이 offline-999 웃습니다. ^^", first.nickname())
        );

        drop(second);
        assert_eq!(presence.recv().await.unwrap(), 1);
        assert_eq!(service.online_count(), 1);
    }
