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
            return Err(format!("Supabase select HTTP {}", resp.status()));
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
            return Err(format!("Supabase insert HTTP {}", resp.status()));
        }
        Ok(())
    }

    /// 30일 지난 데이터 삭제
    pub async fn cleanup_old_data(&self) -> Result<(), String> {
        let cutoff = (chrono::Utc::now() - chrono::Duration::days(30)).to_rfc3339();
        for table in ["notes", "news_links"] {
            let full_url = format!("{}/rest/v1/{}?created_at=lt.{}", self.url, table, cutoff);
            let resp = self
                .http
                .delete(&full_url)
                .header("apikey", &self.service_role_key)
                .header("Authorization", format!("Bearer {}", self.service_role_key))
                .send()
                .await
                .map_err(|e| format!("Supabase delete 실패: {}", e))?;

            if !resp.status().is_success() {
                tracing::warn!("{} 삭제 실패: HTTP {}", table, resp.status());
            }
        }
        Ok(())
    }
}
