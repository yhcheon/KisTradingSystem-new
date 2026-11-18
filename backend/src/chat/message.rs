use super::ChatError;
use rand::Rng;
use serde::{Deserialize, Serialize};

pub const MAX_MESSAGE_CHARACTERS: usize = 500;
pub const MAX_MESSAGE_FRAME_BYTES: usize = 4 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomingMessage {
    #[serde(rename = "type")]
    kind: String,
    text: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub struct ChatMessage {
    #[serde(rename = "type")]
    pub kind: String,
    pub id: String,
    pub nickname: String,
    pub text: String,
    #[serde(default = "default_message_kind")]
    pub message_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action_id: Option<String>,
    pub sent_at: String,
}

pub fn validate_message_frame(frame: &[u8]) -> Result<String, ChatError> {
    if frame.len() > MAX_MESSAGE_FRAME_BYTES {
        return Err(ChatError::FrameTooLarge);
    }

    let message: IncomingMessage =
        serde_json::from_slice(frame).map_err(|_| ChatError::InvalidMessage)?;
    if message.kind != "send" {
        return Err(ChatError::InvalidMessage);
    }

    let text = message.text.trim();
    if text.is_empty() {
        return Err(ChatError::InvalidMessage);
    }
    if text.chars().count() > MAX_MESSAGE_CHARACTERS {
        return Err(ChatError::MessageTooLong);
    }
    if text.chars().any(char::is_control) {
        return Err(ChatError::InvalidMessage);
    }

    Ok(text.to_string())
}

fn default_message_kind() -> String {
    "chat".to_string()
}

pub fn create_message(
    nickname: &str,
    text: String,
    action_id: Option<String>,
) -> ChatMessage {
    let mut bytes = [0_u8; 16];
    rand::rng().fill(&mut bytes);
    let id = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();

    ChatMessage {
        kind: "message".to_string(),
        id,
        nickname: nickname.to_string(),
        text,
        message_kind: if action_id.is_some() { "social" } else { "chat" }.to_string(),
        action_id,
        sent_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(text: &str) -> Vec<u8> {
        serde_json::to_vec(&serde_json::json!({ "type": "send", "text": text })).unwrap()
    }

    #[test]
    fn trims_valid_text_and_counts_unicode_code_points() {
        assert_eq!(
            validate_message_frame(&frame("  hello  ")).unwrap(),
            "hello"
        );
        assert!(validate_message_frame(&frame(&"한".repeat(500))).is_ok());
        assert_eq!(
            validate_message_frame(&frame(&"한".repeat(501))),
            Err(ChatError::MessageTooLong)
        );
    }

    #[test]
    fn rejects_control_characters_in_messages() {
        for text in ["line1\nline2", "tab\there", "nul\0byte"] {
            assert_eq!(
                validate_message_frame(&frame(text)),
                Err(ChatError::InvalidMessage),
                "control characters must be rejected: {text:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_invalid_and_oversized_messages() {
        assert_eq!(
            validate_message_frame(&frame(" \n ")),
            Err(ChatError::InvalidMessage)
        );
        assert_eq!(
            validate_message_frame(br#"{"type":"join","text":"hello"}"#),
            Err(ChatError::InvalidMessage)
        );
        assert_eq!(
            validate_message_frame(br#"{"type":"send","text":"hello","sender":"forged"}"#),
            Err(ChatError::InvalidMessage)
        );
        assert_eq!(
            validate_message_frame(&vec![b'a'; MAX_MESSAGE_FRAME_BYTES + 1]),
            Err(ChatError::FrameTooLarge)
        );
    }
}
