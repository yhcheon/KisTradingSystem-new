// ============================================================
// 파일명: core/db/query.rs
// 설명: Supabase 데이터 조회
// ============================================================

use super::SupabaseClient;

impl SupabaseClient {
    /// 테이블 조회 (예: "notes?category=eq.us_market&order=created_at.desc")
    pub async fn select(&self, table_query: &str) -> Result<serde_json::Value, String> {
        let full_url = format!("{}/rest/v1/{}", self.url, table_query);
        let resp = self.http
            .get(&full_url)
            .header("apikey", &self.service_role_key)
            .header("Authorization", format!("Bearer {}", self.service_role_key))
            .send()
            .await
            .map_err(|e| format!("Supabase select 실패: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await
                .unwrap_or_else(|error| format!("<응답 본문 읽기 실패: {}>", error));
            let body = body.chars().take(2000).collect::<String>();
            tracing::error!(
                target: "supabase",
                operation = "select",
                http_status = %status,
                response_body = %body,
                "Supabase SELECT 실패"
            );
            return Err(format!("Supabase select HTTP {}: {}", status, body));
        }
        resp.json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Supabase JSON 파싱 실패: {}", e))
    }
}
