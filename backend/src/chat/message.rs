use super::ChatError;
use serde::Deserialize;

pub const MAX_MESSAGE_CHARACTERS: usize = 500;
pub const MAX_MESSAGE_FRAME_BYTES: usize = 4 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct IncomingMessage {
    #[serde(rename = "type")]
    kind: String,
    text: String,
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

    Ok(text.to_string())
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
