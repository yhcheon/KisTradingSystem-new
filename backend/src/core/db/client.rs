// ============================================================
// 파일명: core/db/client.rs
// 설명: Supabase PostgREST 클라이언트 기본 타입
// ============================================================

#[derive(Debug, Clone)]
pub struct SupabaseClient {
    pub url: String,
    pub service_role_key: String,
    pub http: reqwest::Client,
}

impl SupabaseClient {
    pub fn new(url: String, service_role_key: String, http: reqwest::Client) -> Self {
        Self {
            url,
            service_role_key,
            http,
        }
    }
}
