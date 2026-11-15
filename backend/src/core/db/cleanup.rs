// ============================================================
// 파일명: core/db/cleanup.rs
// 설명: Supabase 오래된 데이터 정리
// ============================================================

use super::SupabaseClient;

impl SupabaseClient {
    /// notes와 news_links에서 생성 후 30일이 지난 데이터를 삭제합니다.
    pub async fn cleanup_old_data(&self) -> Result<(), String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        for table in ["notes", "news_links"] {
            let resp = self.http
                .delete(&format!("{}/rest/v1/{}", self.url, table))
                .query(&[("created_at", format!("lt.{}", cutoff))])
                .header("apikey", &self.service_role_key)
                .header("Authorization", format!("Bearer {}", self.service_role_key))
                .send()
                .await
                .map_err(|e| format!("Supabase {} 삭제 요청 실패: {}", table, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await
                    .unwrap_or_else(|e| format!("<응답 본문 읽기 실패: {}>", e));
                let body = body.chars().take(2000).collect::<String>();
                tracing::error!(
                    target: "supabase",
                    operation = "cleanup",
                    table = %table,
                    http_status = %status,
                    response_body = %body,
                    "30일 지난 데이터 삭제 실패"
                );
                return Err(format!("Supabase {} 삭제 HTTP {}: {}", table, status, body));
            }

            tracing::info!(table = %table, "30일 지난 데이터 정리 완료");
        }
        Ok(())
    }
}
