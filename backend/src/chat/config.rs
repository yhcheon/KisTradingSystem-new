use super::ChatError;
use serde::Deserialize;
use std::{
    collections::HashSet,
    fs,
    net::IpAddr,
    path::{Path, PathBuf},
};

const MAX_NICKNAME_WORD_LENGTH: usize = 32;

#[derive(Clone, Debug)]
pub(super) struct NicknameConfig {
    pub words: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNicknameConfig {
    words: Vec<String>,
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
