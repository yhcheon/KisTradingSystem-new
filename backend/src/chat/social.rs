use super::config::SocialActionConfig;
use std::collections::HashMap;

/// A successful match against the final whitespace-delimited input token.
///
/// The argument is the complete input prefix before that token. Internal
/// whitespace and punctuation are preserved; boundary whitespace is removed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct MatchedSocialAction {
    pub(super) action_id: String,
    pub(super) matched_token: String,
    pub(super) argument: Option<String>,
}

/// Matches only the final whitespace-delimited token against registered
/// commands and aliases. Earlier tokens are never searched.
///
/// Callers pass normalized chat text (the WebSocket message validator trims
/// leading and trailing whitespace before this function is reached).
pub(super) fn parse_social_action(
    input: &str,
    actions: &HashMap<String, SocialActionConfig>,
) -> Option<MatchedSocialAction> {
    if input.is_empty() {
        return None;
    }

    let separator = input
        .char_indices()
        .rev()
        .find(|(_, character)| character.is_whitespace())
        .map(|(index, character)| (index, character.len_utf8()));

    let (argument, final_token) = match separator {
        Some((index, width)) => {
            let token_start = index + width;
            if token_start == input.len() {
                return None;
            }
            let argument = input[..index].trim();
            let argument = (!argument.is_empty()).then(|| argument.to_string());
            (argument, &input[token_start..])
        }
        None => (None, input),
    };

    for (action_id, action) in actions {
        let is_match = action.command == final_token
            || action.aliases.iter().any(|alias| alias == final_token);
        if is_match {
            return Some(MatchedSocialAction {
                action_id: action_id.clone(),
                matched_token: final_token.to_string(),
                argument,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actions() -> HashMap<String, SocialActionConfig> {
        HashMap::from([
            (
                "laugh".to_string(),
                SocialActionConfig {
                    command: "웃".to_string(),
                    aliases: vec!["웃음".to_string()],
                    template: "{nickname}님이{argument} 웃습니다.".to_string(),
                    template_no_arg: "{nickname}님이 웃습니다. ^^".to_string(),
                },
            ),
            (
                "go".to_string(),
                SocialActionConfig {
                    command: "가".to_string(),
                    aliases: vec!["가다".to_string()],
                    template: "{nickname}님이{argument} 갑니다.".to_string(),
                    template_no_arg: "{nickname}님이 갑니다.".to_string(),
                },
            ),
        ])
    }

    #[test]
    fn matches_a_command_as_the_only_token() {
        assert_eq!(
            parse_social_action("웃", &actions()),
            Some(MatchedSocialAction {
                action_id: "laugh".to_string(),
                matched_token: "웃".to_string(),
                argument: None,
            })
        );
    }

    #[test]
    fn preserves_the_complete_multiword_argument() {
        assert_eq!(
            parse_social_action("즐겁게 신나서 ㅋㅋㅋ 웃", &actions()),
            Some(MatchedSocialAction {
                action_id: "laugh".to_string(),
                matched_token: "웃".to_string(),
                argument: Some("즐겁게 신나서 ㅋㅋㅋ".to_string()),
            })
        );
    }

    #[test]
    fn matches_aliases_only_when_the_alias_is_the_final_token() {
        assert_eq!(
            parse_social_action("학교로 가다", &actions()),
            Some(MatchedSocialAction {
                action_id: "go".to_string(),
                matched_token: "가다".to_string(),
                argument: Some("학교로".to_string()),
            })
        );
        assert_eq!(parse_social_action("웃음 뒤에", &actions()), None);
    }

    #[test]
    fn does_not_match_substrings_or_commands_in_middle_tokens() {
        for input in [
            "가방",
            "가방을 샀다",
            "학교 가방",
            "즐겁게 웃는다",
            "ㅎㅎ 엉",
            "웃 뒤에",
        ] {
            assert_eq!(parse_social_action(input, &actions()), None, "{input}");
        }
    }

    #[test]
    fn preserves_internal_whitespace_and_symbols_in_argument() {
        assert_eq!(
            parse_social_action("신나게   ㅋㅋㅋ 웃", &actions())
                .unwrap()
                .argument
                .as_deref(),
            Some("신나게   ㅋㅋㅋ")
        );
    }

    #[test]
    fn does_not_match_an_empty_final_token() {
        assert_eq!(parse_social_action("", &actions()), None);
        assert_eq!(parse_social_action("웃 ", &actions()), None);
    }
}
