// ============================================================
// 파일명: core/db/mutations.rs
// 설명: Supabase 데이터 삽입, 수정, 삭제
// ============================================================

use super::SupabaseClient;

impl SupabaseClient {
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
            tracing::error!(
                target: "supabase",
                operation = "update",
                table = %table,
                http_status = %status,
                response_body = %response_body,
                "Supabase UPDATE 실패"
            );
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
            tracing::error!(
                target: "supabase",
                operation = "delete",
                table = %table,
                http_status = %status,
                response_body = %response_body,
                "Supabase DELETE 실패"
            );
            return Err(format!(
                "Supabase delete HTTP {}: {}",
                status, response_body
            ));
        }
        Ok(())
    }
}
