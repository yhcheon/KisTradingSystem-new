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
    actions: HashMap<String, SocialActionConfig>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SocialActionConfig {
    pub(super) command: String,
    pub(super) aliases: Vec<String>,
    pub(super) template: String,
    pub(super) template_no_arg: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawChatCommandConfig {
    version: u8,
    #[serde(default)]
    commands: HashMap<String, String>,
    #[serde(default)]
    actions: HashMap<String, RawSocialActionConfig>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSocialActionConfig {
    command: String,
    #[serde(default)]
    aliases: Vec<String>,
    template: String,
    template_no_arg: String,
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


fn validate_social_token(token: &str) -> Result<(), ChatError> {
    let length = token.chars().count();
    if token.trim() != token
        || length == 0
        || length > MAX_MESSAGE_CHARACTERS
        || token.starts_with('/')
        || token
            .chars()
            .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(ChatError::Configuration(
            "social commands and aliases must be non-empty single tokens without whitespace, control characters or a leading slash"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_social_template(template: &str, requires_argument: bool) -> Result<(), ChatError> {
    if template.trim().is_empty()
        || template.chars().count() > MAX_MESSAGE_CHARACTERS
        || template.chars().any(char::is_control)
    {
        return Err(ChatError::Configuration(
            "social templates must be non-empty, control-character-free and within the message length limit"
                .to_string(),
        ));
    }

    let mut has_nickname = false;
    let mut has_argument = false;
    let mut chars = template.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '{' => {
                let mut placeholder = String::new();
                let mut closed = false;
                for next in chars.by_ref() {
                    if next == '}' {
                        closed = true;
                        break;
                    }
                    placeholder.push(next);
                }
                if !closed || !matches!(placeholder.as_str(), "nickname" | "argument") {
                    return Err(ChatError::Configuration(
                        "social templates may use only {nickname} and {argument} placeholders"
                            .to_string(),
                    ));
                }
                match placeholder.as_str() {
                    "nickname" => has_nickname = true,
                    "argument" => has_argument = true,
                    _ => unreachable!(),
                }
            }
            '}' => {
                return Err(ChatError::Configuration(
                    "social templates contain an unmatched closing brace".to_string(),
                ));
            }
            _ => {}
        }
    }

    if !has_nickname || has_argument != requires_argument {
        return Err(ChatError::Configuration(if requires_argument {
            "social templates must contain both {nickname} and {argument}".to_string()
        } else {
            "social no-argument templates must contain {nickname} and must not contain {argument}"
                .to_string()
        }));
    }
    Ok(())
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

        let actions = match raw.version {
            1 => {
                if !raw.actions.is_empty() {
                    return Err(ChatError::Configuration(
                        "version 1 chat command config must contain only commands".to_string(),
                    ));
                }
                HashMap::new()
            }
            2 => {
                if raw.actions.is_empty() {
                    return Err(ChatError::Configuration(
                        "version 2 chat command config actions must not be empty".to_string(),
                    ));
                }
                let mut registered_tokens = HashSet::new();
                let mut actions = HashMap::with_capacity(raw.actions.len());
                for (action_id, action) in raw.actions {
                    if action_id.is_empty()
                        || action_id.len() > 64
                        || !action_id
                            .chars()
                            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '_' | '-'))
                    {
                        return Err(ChatError::Configuration(
                            "social action ids must use 1-64 ASCII letters, digits, '_' or '-'"
                                .to_string(),
                        ));
                    }

                    validate_social_token(&action.command)?;
                    if !registered_tokens.insert(action.command.clone()) {
                        return Err(ChatError::Configuration(format!(
                            "social command or alias is duplicated: {}",
                            action.command
                        )));
                    }
                    for alias in &action.aliases {
                        validate_social_token(alias)?;
                        if !registered_tokens.insert(alias.clone()) {
                            return Err(ChatError::Configuration(format!(
                                "social command or alias is duplicated: {alias}"
                            )));
                        }
                    }
                    validate_social_template(&action.template, true)?;
                    validate_social_template(&action.template_no_arg, false)?;
                    let expanded_no_arg = action
                        .template_no_arg
                        .replace("{nickname}", &"n".repeat(MAX_NICKNAME_ALIAS_LENGTH));
                    if expanded_no_arg.chars().count() > MAX_MESSAGE_CHARACTERS {
                        return Err(ChatError::Configuration(format!(
                            "social action {action_id} no-argument template exceeds {MAX_MESSAGE_CHARACTERS} characters after nickname expansion"
                        )));
                    }
                    actions.insert(
                        action_id,
                        SocialActionConfig {
                            command: action.command,
                            aliases: action.aliases,
                            template: action.template,
                            template_no_arg: action.template_no_arg,
                        },
                    );
                }
                actions
            }
            _ => {
                return Err(ChatError::Configuration(
                    "chat command config version must be 1 or 2".to_string(),
                ));
            }
        };

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
            actions,
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

    pub fn entries(&self) -> Vec<(String, String)> {
        let mut entries = self
            .commands
            .iter()
            .map(|(command, response)| (command.clone(), response.clone()))
            .collect::<Vec<_>>();
        entries.sort_by(|left, right| left.0.cmp(&right.0));
        entries
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

        assert_eq!(
            config.len(),
            152,
            "the existing operator-managed slash command set must be preserved"
        );
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
    fn loads_v2_social_actions_without_changing_slash_commands() {
        let config = ChatCommandConfig::from_json(
            r#"{
                "version":2,
                "commands":{"/hello":"{nickname} says hello"},
                "actions":{
                    "laugh":{
                        "command":"웃",
                        "aliases":["웃음"],
                        "template":"{nickname}님이{argument} 웃습니다.",
                        "template_no_arg":"{nickname}님이 웃습니다. ^^"
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(config.expand("/hello", "guest-1").as_deref(), Some("guest-1 says hello"));
        assert_eq!(config.commands.len(), 1);
        let laugh = config.actions.get("laugh").unwrap();
        assert_eq!(laugh.command, "웃");
        assert_eq!(laugh.aliases, ["웃음"]);
        assert!(laugh.template.contains("{argument}"));
        assert!(laugh.template_no_arg.contains("웃습니다"));
    }

    #[test]
    fn rejects_invalid_v2_actions_and_templates() {
        let cases = [
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{}}"#,
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{"a":{"command":"웃","template":"{nickname}님이 웃습니다.","template_no_arg":"{nickname}"}}}"#,
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{"a":{"command":"웃","template":"{argument}","template_no_arg":"{nickname}"}}}"#,
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{"a":{"command":"웃","aliases":["웃"],"template":"{nickname}{argument}","template_no_arg":"{nickname}"}}}"#,
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{"a":{"command":"웃","template":"{nickname}{unknown}","template_no_arg":"{nickname}"}}}"#,
            r#"{"version":2,"commands":{"/hello":"hello"},"actions":{"a":{"command":"즐겁게 웃","template":"{nickname}{argument}","template_no_arg":"{nickname}"}}}"#,
        ];
        for json in cases {
            assert!(ChatCommandConfig::from_json(json).is_err(), "expected invalid config: {json}");
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
