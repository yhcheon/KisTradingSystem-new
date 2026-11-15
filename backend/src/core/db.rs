// ============================================================
// 파일명: core/db.rs
// 설명: Supabase PostgREST API 클라이언트
//       - service_role 키로 RLS 우회
//       - notes, news_links, user_profiles 조회/삽입
// 작성일: 2026-11-14
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

    /// 테이블 조회 (예: "notes?category=eq.us_market&order=created_at.desc")
    pub async fn select(&self, table_query: &str) -> Result<serde_json::Value, String> {
        let full_url = format!("{}/rest/v1/{}", self.url, table_query);
        let resp = self
            .http
            .get(&full_url)
            .header("apikey", &self.service_role_key)
            .header("Authorization", format!("Bearer {}", self.service_role_key))
            .send()
            .await
            .map_err(|e| format!("Supabase select 실패: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
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

    /// 테이블 삽입
    pub async fn insert(&self, table: &str, body: &serde_json::Value) -> Result<(), String> {
        let full_url = format!("{}/rest/v1/{}", self.url, table);
        let resp = self
            .http
            .post(&full_url)
            .header("apikey", &self.service_role_key)
            .header("Authorization", format!("Bearer {}", self.service_role_key))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=minimal")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Supabase insert 실패: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp
                .text()
                .await
                .unwrap_or_else(|error| format!("<응답 본문 읽기 실패: {}>", error));
            let body = body.chars().take(2000).collect::<String>();
            tracing::error!(
                target: "supabase",
                operation = "insert",
                table = %table,
                http_status = %status,
                response_body = %body,
                "Supabase INSERT 실패"
            );
            return Err(format!("Supabase insert HTTP {}: {}", status, body));
        }
        Ok(())
    }

    /// 글 수정 (id 기준)
    pub async fn update_by_id(
        &self,
        table: &str,
        id: &str,
        body: &serde_json::Value,
    ) -> Result<(), String> {
        let full_url = format!("{}/rest/v1/{}", self.url, table);
        let resp = self
            .http
            .patch(&full_url)
            .query(&[("id", format!("eq.{}", id))])
            .header("apikey", &self.service_role_key)
            .header("Authorization", format!("Bearer {}", self.service_role_key))
            .header("Content-Type", "application/json")
            .header("Prefer", "return=minimal")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Supabase update 실패: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let response_body = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("<응답 본문 읽기 실패: {}>", e));
            let response_body = response_body.chars().take(2000).collect::<String>();
            tracing::error!(target: "supabase", operation = "update", table = %table, http_status = %status, response_body = %response_body, "Supabase UPDATE 실패");
            return Err(format!(
                "Supabase update HTTP {}: {}",
                status, response_body
            ));
        }
        Ok(())
    }

    /// 글 삭제 (id 기준)
    pub async fn delete_by_id(&self, table: &str, id: &str) -> Result<(), String> {
        let full_url = format!("{}/rest/v1/{}", self.url, table);
        let resp = self
            .http
            .delete(&full_url)
            .query(&[("id", format!("eq.{}", id))])
            .header("apikey", &self.service_role_key)
            .header("Authorization", format!("Bearer {}", self.service_role_key))
            .send()
            .await
            .map_err(|e| format!("Supabase delete 실패: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let response_body = resp
                .text()
                .await
                .unwrap_or_else(|e| format!("<응답 본문 읽기 실패: {}>", e));
            let response_body = response_body.chars().take(2000).collect::<String>();
            tracing::error!(target: "supabase", operation = "delete", table = %table, http_status = %status, response_body = %response_body, "Supabase DELETE 실패");
            return Err(format!(
                "Supabase delete HTTP {}: {}",
                status, response_body
            ));
        }
        Ok(())
    }

    /// notes와 news_links에서 생성 후 30일이 지난 데이터를 삭제합니다.
    pub async fn cleanup_old_data(&self) -> Result<(), String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        for table in ["notes", "news_links"] {
            let resp = self
                .http
                .delete(&format!("{}/rest/v1/{}", self.url, table))
                .query(&[("created_at", format!("lt.{}", cutoff))])
                .header("apikey", &self.service_role_key)
                .header("Authorization", format!("Bearer {}", self.service_role_key))
                .send()
                .await
                .map_err(|e| format!("Supabase {} 삭제 요청 실패: {}", table, e))?;

            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp
                    .text()
                    .await
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
