// ============================================================
// 파일명: core/db/mod.rs
// 설명: Supabase 클라이언트 공개 인터페이스
// ============================================================

mod cleanup;
mod client;
mod mutations;
mod query;

pub use client::SupabaseClient;
