use super::ChatError;
use super::message::MAX_MESSAGE_CHARACTERS;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

const MAX_NICKNAME_WORD_LENGTH: usize = 32;
const MAX_NICKNAME_ALIAS_LENGTH: usize = MAX_NICKNAME_WORD_LENGTH + 1 + 20;

#[derive(Clone, Debug)]
pub(super) struct NicknameConfig {
    pub words: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNicknameConfig {
    words: Vec<String>,
}

#[derive(Clone, Debug)]
pub(super) struct ChatCommandConfig {
    commands: HashMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChatCommandConfig {
    version: u8,
    commands: HashMap<String, String>,
}

impl NicknameConfig {
    pub fn from_file(path: &Path) -> Result<Self, ChatError> {
        let contents = fs::read_to_string(path).map_err(|error| {
            ChatError::Configuration(format!(
                "nickname config {} could not be read: {error}",
                path.display()
            ))
        })?;
        Self::from_json(&contents)
    }

    pub(super) fn from_json(contents: &str) -> Result<Self, ChatError> {
        let raw: RawNicknameConfig = serde_json::from_str(contents).map_err(|error| {
            ChatError::Configuration(format!("nickname config is invalid JSON: {error}"))
        })?;

        if raw.words.is_empty() {
            return Err(ChatError::Configuration(
                "nickname config words must not be empty".to_string(),
            ));
        }
        let mut words = Vec::with_capacity(raw.words.len());
        let mut seen = HashSet::with_capacity(raw.words.len());
        for word in raw.words {
            let word = word.trim();
            if word.is_empty() {
                return Err(ChatError::Configuration(
                    "nickname config words must not contain empty entries".to_string(),
                ));
            }
            if word.chars().count() > MAX_NICKNAME_WORD_LENGTH {
                return Err(ChatError::Configuration(format!(
                    "nickname config words must be at most {MAX_NICKNAME_WORD_LENGTH} characters"
                )));
            }
            if word.chars().any(char::is_control) {
                return Err(ChatError::Configuration(
                    "nickname config words must not contain control characters".to_string(),
                ));
            }
            if !seen.insert(word.to_string()) {
                return Err(ChatError::Configuration(
                    "nickname config words must be unique after trimming".to_string(),
                ));
            }
            words.push(word.to_string());
        }

        Ok(Self { words })
    }
}

impl ChatCommandConfig {
    pub fn from_file(path: &Path) -> Result<Self, ChatError> {
        let contents = fs::read_to_string(path).map_err(|error| {
            ChatError::Configuration(format!(
                "chat command config {} could not be read: {error}",
                path.display()
            ))
        })?;
        Self::from_json(&contents)
    }

    pub(super) fn from_json(contents: &str) -> Result<Self, ChatError> {
        let raw: RawChatCommandConfig = serde_json::from_str(contents).map_err(|error| {
            ChatError::Configuration(format!("chat command config is invalid JSON: {error}"))
        })?;
        if raw.version != 1 {
            return Err(ChatError::Configuration(
                "chat command config version must be 1".to_string(),
            ));
        }
        if raw.commands.is_empty() {
            return Err(ChatError::Configuration(
                "chat command config commands must not be empty".to_string(),
            ));
        }

        let max_nickname = "n".repeat(MAX_NICKNAME_ALIAS_LENGTH);
        for (command, response) in &raw.commands {
            let command_length = command.chars().count();
            if command_length < 2
                || command_length > MAX_MESSAGE_CHARACTERS
                || !command.starts_with('/')
                || command
                    .chars()
                    .any(|character| character.is_whitespace() || character.is_control())
            {
                return Err(ChatError::Configuration(
                    "chat command config keys must be slash-prefixed commands without whitespace"
                        .to_string(),
                ));
            }
            if response.trim().is_empty() {
                return Err(ChatError::Configuration(
                    "chat command config responses must not be empty".to_string(),
                ));
            }
            let max_expanded_response = response.replace("{nickname}", &max_nickname);
            if max_expanded_response.chars().count() > MAX_MESSAGE_CHARACTERS {
                return Err(ChatError::Configuration(format!(
                    "chat command config responses must expand to at most {MAX_MESSAGE_CHARACTERS} characters"
                )));
            }
        }

        Ok(Self {
            commands: raw.commands,
        })
    }

    pub fn expand(&self, command: &str, nickname: &str) -> Option<String> {
        self.commands
            .get(command)
            .map(|response| response.replace("{nickname}", nickname))
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }
}

pub(super) fn nickname_config_path() -> Result<PathBuf, ChatError> {
    if let Some(configured_path) = std::env::var_os("CHAT_NICKNAMES_CONFIG") {
        let path = PathBuf::from(configured_path);
        return Ok(if path.is_absolute() {
            path
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
        });
    }

    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ChatError::Configuration("could not resolve repository root for nickname config".into())
        })?;
    Ok(repository_root.join("config").join("nicknames.json"))
}

pub(super) fn chat_commands_config_path() -> Result<PathBuf, ChatError> {
    if let Some(configured_path) = std::env::var_os("CHAT_COMMANDS_CONFIG") {
        let path = PathBuf::from(configured_path);
        return Ok(if path.is_absolute() {
            path
        } else {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path)
        });
    }

    let repository_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| {
            ChatError::Configuration(
                "could not resolve repository root for chat command config".into(),
            )
        })?;
    Ok(repository_root.join("config").join("chat_commands.json"))
}

pub(super) fn trusted_proxy_ips_from_env() -> Result<HashSet<IpAddr>, ChatError> {
    let value = match std::env::var("TRUSTED_PROXY_IPS") {
        Ok(value) => value,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(error) => {
            return Err(ChatError::Configuration(format!(
                "TRUSTED_PROXY_IPS could not be read: {error}"
            )));
        }
    };

    value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            value.parse::<IpAddr>().map_err(|error| {
                ChatError::Configuration(format!(
                    "TRUSTED_PROXY_IPS contains an invalid IP address: {error}"
                ))
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_versioned_chat_commands() {
        let config = ChatCommandConfig::from_json(
            r#"{"version":1,"commands":{"/hello":"{nickname} says hello"}}"#,
        )
        .unwrap();

        assert_eq!(
            config.expand("/hello", "guest-1").as_deref(),
            Some("guest-1 says hello")
        );
        assert_eq!(config.expand("/hello extra", "guest-1"), None);
    }

    #[test]
    fn loads_the_supplied_chat_command_file() {
        let config =
            ChatCommandConfig::from_json(include_str!("../../../config/chat_commands.json"))
                .unwrap();

        assert_eq!(config.len(), 94);
    }

    #[test]
    fn rejects_invalid_chat_command_config() {
        for json in [
            "",
            r#"{"version":2,"commands":{"/hello":"hello"}}"#,
            r#"{"version":1,"commands":{}}"#,
            r#"{"version":1,"commands":{"hello":"hello"}}"#,
            r#"{"version":1,"commands":{"/":"hello"}}"#,
            r#"{"version":1,"commands":{"/hello":"  "}}"#,
            r#"{"version":1,"commands":{"/hello":"hello"},"fallback":"no"}"#,
            r#"not json"#,
        ] {
            assert!(
                ChatCommandConfig::from_json(json).is_err(),
                "expected invalid config: {json}"
            );
        }
    }

    #[test]
    fn rejects_missing_chat_command_config_file() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "chat-commands-missing-{}-{unique}.json",
            std::process::id()
        ));
        assert!(ChatCommandConfig::from_file(&path).is_err());
    }

    #[test]
    fn accepts_operator_managed_words() {
        let config = NicknameConfig::from_json(r#"{"words":["name one","name two"]}"#).unwrap();

        assert_eq!(config.words, ["name one", "name two"]);
    }

    #[test]
    fn rejects_missing_or_empty_words_and_unrecognized_fields() {
        for json in [
            "",
            r#"{"words":[]}"#,
            r#"{"words":["  "]}"#,
            r#"{"words":["same"," same "]}"#,
            r#"{"words":["name"],"number_range":{"min":1,"max":2}}"#,
            r#"{"words":["name"],"fallback":"no"}"#,
            r#"not json"#,
        ] {
            assert!(
                NicknameConfig::from_json(json).is_err(),
                "expected invalid config: {json}"
            );
        }
    }

    #[test]
    fn rejects_missing_config_file() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "chat-nicknames-missing-{}-{unique}.json",
            std::process::id()
        ));
        assert!(NicknameConfig::from_file(&path).is_err());
    }
}
