// ============================================================
// 파일명: auth/jwt/claims.rs
// 설명: Supabase JWT 클레임 타입
// ============================================================

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub email: Option<String>,
    pub role: String,
    pub exp: usize,
    pub iat: usize,
    pub iss: Option<String>,
}
