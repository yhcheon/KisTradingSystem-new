// ============================================================
// 파일명: lib/supabase.ts
// 설명: Supabase 클라이언트 (프론트엔드용, anon 키)
// 작성일: 2026-11-14
// ============================================================

import { createClient } from '@supabase/supabase-js';

const supabaseUrl = import.meta.env.VITE_SUPABASE_URL;
const supabaseAnonKey = import.meta.env.VITE_SUPABASE_ANON_KEY;

if (!supabaseUrl || !supabaseAnonKey) {
  throw new Error('Supabase 환경 변수가 없습니다. .env.local을 확인하세요.');
}

export const supabase = createClient(supabaseUrl, supabaseAnonKey);