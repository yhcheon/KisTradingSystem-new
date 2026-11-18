use super::{
    ChatError,
    config::SocialActionConfig,
    message::MAX_MESSAGE_CHARACTERS,
};
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


/// Renders a matched social action without changing the parser's input semantics.
/// The argument placeholder receives one separating space when an argument exists.
pub(super) fn render_social_action(
    action: &SocialActionConfig,
    nickname: &str,
    argument: Option<&str>,
) -> Result<String, ChatError> {
    let rendered = match argument {
        Some(argument) if !argument.is_empty() => action
            .template
            .replace("{nickname}", nickname)
            .replace("{argument}", &format!(" {argument}")),
        _ => action.template_no_arg.replace("{nickname}", nickname),
    };

    if rendered.chars().any(char::is_control) {
        return Err(ChatError::InvalidMessage);
    }
    if rendered.chars().count() > MAX_MESSAGE_CHARACTERS {
        return Err(ChatError::MessageTooLong);
    }

    Ok(rendered)
}

pub(super) fn render_target_social_action(
    action: &SocialActionConfig,
    nickname: &str,
    target: &str,
    argument: Option<&str>,
) -> Result<String, ChatError> {
    let argument = argument.filter(|value| !value.is_empty());
    let template = match argument {
        Some(_) => action.template_target.as_deref().ok_or(ChatError::ServiceUnavailable)?,
        None => action.template_target_no_arg.as_deref().ok_or(ChatError::ServiceUnavailable)?,
    };
    let argument_text = argument.map(|value| format!(" {value}")).unwrap_or_default();
    let rendered = template
        .replace("{nickname}", nickname)
        .replace("{target}", target)
        .replace("{argument}", &argument_text);
    if rendered.chars().any(char::is_control) {
        return Err(ChatError::InvalidMessage);
    }
    if rendered.chars().count() > MAX_MESSAGE_CHARACTERS {
        return Err(ChatError::MessageTooLong);
    }
    Ok(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn laugh_action() -> SocialActionConfig {
        SocialActionConfig {
            command: "웃".to_string(),
            aliases: vec!["웃음".to_string()],
            template: "{nickname}님이{argument} 웃습니다. ^^".to_string(),
            template_no_arg: "{nickname}님이 웃습니다. ^^".to_string(),
            template_target: Some("{nickname}님이 {target}님을 보고{argument} 웃습니다. ^^".to_string()),
            template_target_no_arg: Some("{nickname}님이 {target}님을 보고 웃습니다. ^^".to_string()),
        }
    }

    fn actions() -> HashMap<String, SocialActionConfig> {
        HashMap::from([("laugh".to_string(), laugh_action())])
    }

    #[test]
    fn renders_social_action_with_and_without_argument() {
        let action = laugh_action();
        assert_eq!(render_social_action(&action, "작성자", None).unwrap(), "작성자님이 웃습니다. ^^");
        assert_eq!(render_social_action(&action, "작성자", Some("즐겁게")).unwrap(), "작성자님이 즐겁게 웃습니다. ^^");
    }

    #[test]
    fn renders_target_action_in_confirmed_form() {
        let action = laugh_action();
        assert_eq!(
            render_target_social_action(&action, "작성자", "홍길동", None).unwrap(),
            "작성자님이 홍길동님을 보고 웃습니다. ^^"
        );
        assert_eq!(
            render_target_social_action(&action, "작성자", "홍길동", Some("즐겁게")).unwrap(),
            "작성자님이 홍길동님을 보고 즐겁게 웃습니다. ^^"
        );
    }

    #[test]
    fn matches_only_the_final_token_and_preserves_argument() {
        let matched = parse_social_action("신나게   ㅋㅋㅋ 웃", &actions()).unwrap();
        assert_eq!(matched.action_id, "laugh");
        assert_eq!(matched.argument.as_deref(), Some("신나게   ㅋㅋㅋ"));
        assert_eq!(parse_social_action("웃음 뒤에", &actions()), None);
        assert_eq!(parse_social_action("즐겁게 웃는다", &actions()), None);
    }

    #[test]
    fn rejects_invalid_rendered_messages() {
        let action = laugh_action();
        assert_eq!(
            render_social_action(&action, "작성자", Some("줄1\n줄2")),
            Err(ChatError::InvalidMessage)
        );
    }
}
