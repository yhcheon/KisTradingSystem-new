// ============================================================
// 파일명: src/core/env.rs  (신규)
// 3번: 환경변수 한 번에 검증
// - bootstrap 시작하자마자 validate_env()를 한 번만 부르면,
//   빠진 변수를 하나씩이 아니라 전부 모아서 한 번에 보여줍니다.
// - 값 자체는 노출하지 않습니다 (키 이름만).
// ============================================================
use std::collections::HashMap;

/// 시작에 반드시 필요한 환경변수 목록.
/// TOKEN_CACHE_FILE / BACKEND_BIND_ADDR / CORS_ALLOWED_ORIGINS / KIS_REST_DOMAIN 은
/// 기본값이 있어서 필수가 아닙니다. 그래서 여기서는 빼두었습니다.
pub const REQUIRED_ENV_VARS: &[&str] = &[
    "KIS_APP_KEY",
    "KIS_APP_SECRET",
    "SUPABASE_URL",
    "SUPABASE_SERVICE_ROLE_KEY",
    "SUPABASE_JWT_SECRET",
];

/// 순수 함수: 주어진 맵에서 빠진(없거나 빈 값인) 변수만 반환.
/// 테스트하기 위해 std::env에 직접 의존하지 않습니다.
pub fn missing_vars(vars: &HashMap<String, String>) -> Vec<&'static str> {
    REQUIRED_ENV_VARS
        .iter()
        .copied()
        .filter(|key| vars.get(*key).map(|v| v.trim().is_empty()).unwrap_or(true))
        .collect()
}

/// 실제 환경에서 검증. 빠진 게 있으면 전부 나열한 에러를 반환합니다.
pub fn validate_env() -> Result<(), String> {
    let vars: HashMap<String, String> = REQUIRED_ENV_VARS
        .iter()
        .map(|key| (key.to_string(), std::env::var(key).unwrap_or_default()))
        .collect();
    // ChatService::load_from_env()가 따로 검증하는 변수도 여기서 같이 잡히는지 확인용:
    // chat 관련 변수가 있다면 여기에 추가하세요. 예: "CHAT_..." — 지금은 KIS/Supabase 5개만 필수.
    let missing = missing_vars(&vars);
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "필수 환경변수 {}개가 없습니다: {}",
            missing.len(),
            missing.join(", ")
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn full_map() -> HashMap<String, String> {
        REQUIRED_ENV_VARS
            .iter()
            .map(|k| (k.to_string(), "dummy".to_string()))
            .collect()
    }

    #[test]
    fn all_present_passes() {
        assert!(missing_vars(&full_map()).is_empty());
    }

    #[test]
    fn reports_all_missing_at_once_not_one_by_one() {
        // 기존 방식의 문제: KIS_APP_KEY만 먼저 터지고, 고치면 SUPABASE_URL이 터지는 식.
        // 이 테스트는 빠진 3개를 한 번에 다 보고하는지 검증합니다.
        let mut vars = full_map();
        vars.remove("KIS_APP_KEY");
        vars.remove("SUPABASE_URL");
        vars.remove("SUPABASE_JWT_SECRET");
        assert_eq!(
            missing_vars(&vars),
            vec!["KIS_APP_KEY", "SUPABASE_URL", "SUPABASE_JWT_SECRET"]
        );
    }

    #[test]
    fn empty_string_counts_as_missing() {
        let mut vars = full_map();
        vars.insert("KIS_APP_SECRET".to_string(), "   ".to_string()); // 공백만 있어도 빠진 것
        assert_eq!(missing_vars(&vars), vec!["KIS_APP_SECRET"]);
    }
}
