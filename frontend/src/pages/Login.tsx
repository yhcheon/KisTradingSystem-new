// ============================================================
// 파일명: pages/Login.tsx
// 설명: 로그인 유도 컴포넌트 (분석 탭 안에 삽입됨)
// 작성일: 2026-11-14
// ============================================================

import { supabase } from '../lib/supabase';

export default function Login() {
  const handleGoogleLogin = async () => {
    const { error } = await supabase.auth.signInWithOAuth({
      provider: 'google',
      options: {
        redirectTo: window.location.origin,
        queryParams: { prompt: 'select_account' },
      },
    });
    if (error) alert(`로그인 실패: ${error.message}`);
  };

  return (
    <div style={{ padding: 40, textAlign: 'center' }}>
      <p style={{ color: '#94a3b8', marginBottom: 24, fontSize: 15 }}>
        분석 콘텐츠는 로그인 후 이용 가능합니다
      </p>
      <button
        onClick={handleGoogleLogin}
        style={{
          padding: '10px 20px',
          background: '#3b82f6',
          color: 'white',
          border: 'none',
          borderRadius: 8,
          cursor: 'pointer',
          fontSize: 14,
        }}
      >
        구글 로그인
      </button>
    </div>
  );
}

