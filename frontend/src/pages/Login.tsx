// ============================================================
// 파일명: pages/Login.tsx
// 설명: 로그인 유도 컴포넌트 (분석 탭 안에 삽입됨)
// 작성일: 2026-11-14
// ============================================================

import { supabase } from '../lib/supabase';

export default function Login() {
  const handleGoogleLogin = async () => {
    // 팝업 차단을 피하려고 사용자 클릭 시점에 빈 창을 먼저 엽니다.
    const authPopup = window.open(
      'about:blank',
      'google-oauth',
      'popup=yes,width=520,height=700,left=200,top=100',
    );

    if (!authPopup) {
      alert('로그인 팝업이 차단되었습니다. 브라우저에서 팝업을 허용해 주세요.');
      return;
    }

    const { data, error } = await supabase.auth.signInWithOAuth({
      provider: 'google',
      options: {
        redirectTo: window.location.origin,
        skipBrowserRedirect: true,
        queryParams: { prompt: 'select_account' },
      },
    });

    if (error || !data.url) {
      authPopup.close();
      alert(`로그인 실패: ${error?.message ?? '인증 주소를 가져오지 못했습니다.'}`);
      return;
    }

    authPopup.location.href = data.url;
  };

  return (
    <div style={{ padding: 40, textAlign: 'center' }}>
      <p style={{ color: '#94a3b8', marginBottom: 24, fontSize: 15 }}>
        로그인하면 분석 기능을 이용할 수 있습니다
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

