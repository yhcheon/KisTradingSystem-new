mod config;
mod message;
mod service;

use std::fmt;

pub use message::validate_message_frame;
pub use service::{ChatConnection, ChatService, ChatTicketResponse};

#[derive(Debug, PartialEq, Eq)]
pub enum ChatError {
    Configuration(String),
    InvalidClientAddress,
    InvalidTicket,
    ExpiredTicket,
    RateLimited,
    ConnectionLimitReached,
    AliasPoolExhausted,
    InvalidMessage,
    MessageTooLong,
    FrameTooLarge,
    ServiceUnavailable,
}

impl fmt::Display for ChatError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Configuration(message) => write!(formatter, "{message}"),
            Self::InvalidClientAddress => write!(formatter, "invalid client address"),
            Self::InvalidTicket => write!(formatter, "invalid or already used chat ticket"),
            Self::ExpiredTicket => write!(formatter, "chat ticket expired"),
            Self::RateLimited => write!(formatter, "chat rate limit exceeded"),
            Self::ConnectionLimitReached => write!(formatter, "chat connection limit reached"),
            Self::AliasPoolExhausted => write!(formatter, "no unused chat nickname is available"),
            Self::InvalidMessage => write!(formatter, "invalid chat message"),
            Self::MessageTooLong => write!(formatter, "chat message is too long"),
            Self::FrameTooLarge => write!(formatter, "chat message frame is too large"),
            Self::ServiceUnavailable => write!(formatter, "chat state is unavailable"),
        }
    }
}

impl std::error::Error for ChatError {}
