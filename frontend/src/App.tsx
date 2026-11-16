// ============================================================
// 파일명: App.tsx
// 설명: 인증 상태 및 메인 화면/분석 모달 전환을 담당하는 애플리케이션 셸
//       - 시세 화면은 항상 마운트하여 분석 모달을 열고 닫아도 상태와 WebSocket 유지
//       - 게스트: 분석 모달에서 로그인 유도
//       - admin: 관리자 화면
// ============================================================

import { useEffect, useState } from 'react';
import { supabase } from './lib/supabase';
import { useAuthSession } from './features/auth/hooks/useAuthSession';
import { useAdminRole } from './features/auth/hooks/useAdminRole';
import Login from './pages/Login';
import Analysis from './pages/Analysis';
import Admin from './pages/Admin';
import PricesTab from './features/prices/PricesTab';
import './App.css';


export default function App() {
  const { session, authLoading } = useAuthSession();
  const [activeModal, setActiveModal] = useState<'analysis' | 'admin' | 'login' | null>(null);
  const isAdmin = useAdminRole(session);

  // Google OAuth 팝업에서 인증이 완료되면 이 창의 세션도 공유되고, 팝업은 자동으로 닫습니다.
  useEffect(() => {
    if (window.opener && session) {
      window.opener.postMessage({ type: 'supabase-auth-complete' }, window.location.origin);
      window.close();
    }
  }, [session]);

  useEffect(() => {
    if (!activeModal) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setActiveModal(null);
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [activeModal]);

  if (authLoading) {
    return <p style={{ padding: 20, color: '#e2e8f0' }}>로딩 중...</p>;
  }

  return (
    <div className="app">
      <header className="top-bar">
        <h1>KOSPI200 선물 시세</h1>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button
            onClick={() => {
              setActiveModal(null);
            }}
            className={!activeModal ? 'active' : ''}
          >
            시세
          </button>
          <button
            onClick={() => setActiveModal('analysis')}
            className={activeModal === 'analysis' ? 'active' : ''}
            aria-haspopup="dialog"
            aria-expanded={activeModal === 'analysis'}
          >
            분석
          </button>
          {isAdmin && (
            <button
              onClick={() => setActiveModal('admin')}
              className={activeModal === 'admin' ? 'active' : ''}
              aria-haspopup="dialog"
              aria-expanded={activeModal === 'admin'}
            >
              관리자
            </button>
          )}
          {session ? (
            <button onClick={() => supabase.auth.signOut()}>로그아웃</button>
          ) : (
            <button
              onClick={() => setActiveModal('login')}
              className={activeModal === 'login' ? 'active' : ''}
            >
              로그인
            </button>
          )}
        </div>
      </header>

      {/* 시세 화면은 항상 마운트하여 모달을 열고 닫아도 상태와 WebSocket을 유지합니다. */}
      <PricesTab />

      {activeModal && (
        <div
          className="analysis-modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setActiveModal(null);
          }}
        >
          <section
            className={`analysis-modal ${activeModal === 'login' || (activeModal === 'analysis' && !session) ? 'login-modal' : ''}`}
            role="dialog"
            aria-modal="true"
            aria-labelledby="app-modal-title"
          >
            <header className="analysis-modal-header">
              <h2 id="app-modal-title">
                {activeModal === 'analysis' ? (session ? '분석' : '로그인') : activeModal === 'login' ? '로그인' : '관리자'}
              </h2>
              <button
                type="button"
                className="analysis-modal-close"
                onClick={() => setActiveModal(null)}
                aria-label="창 닫기"
              >
                닫기 ✕
              </button>
            </header>
            <div className="analysis-modal-content">
              {activeModal === 'login' ? (
                <Login />
              ) : activeModal === 'analysis' ? (
                session ? <Analysis /> : <Login />
              ) : !session ? (
                <Login />
              ) : isAdmin ? (
                <Admin />
              ) : (
                <p style={{ padding: 20 }}>관리자 권한이 없습니다</p>
              )}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
