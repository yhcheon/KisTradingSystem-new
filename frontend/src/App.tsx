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

type Tab = 'prices' | 'admin';

export default function App() {
  const { session, authLoading } = useAuthSession();
  const [tab, setTab] = useState<Tab>('prices');
  const [analysisOpen, setAnalysisOpen] = useState(false);
  const isAdmin = useAdminRole(session);

  useEffect(() => {
    if (!analysisOpen) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setAnalysisOpen(false);
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [analysisOpen]);

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
              setTab('prices');
              setAnalysisOpen(false);
            }}
            className={tab === 'prices' && !analysisOpen ? 'active' : ''}
          >
            시세
          </button>
          <button
            onClick={() => setAnalysisOpen(true)}
            className={analysisOpen ? 'active' : ''}
            aria-haspopup="dialog"
            aria-expanded={analysisOpen}
          >
            분석
          </button>
          {isAdmin && (
            <button
              onClick={() => {
                setTab('admin');
                setAnalysisOpen(false);
              }}
              className={tab === 'admin' ? 'active' : ''}
            >
              관리자
            </button>
          )}
          {session ? (
            <button onClick={() => supabase.auth.signOut()}>로그아웃</button>
          ) : (
            <button
              onClick={() => setAnalysisOpen(true)}
              className={analysisOpen ? 'active' : ''}
            >
              로그인
            </button>
          )}
        </div>
      </header>

      {/* 시세 화면은 숨겨져도 언마운트하지 않아 선택 종목/분봉/실시간 연결을 유지합니다. */}
      <div style={{ display: tab === 'prices' ? 'block' : 'none' }}>
        <PricesTab />
      </div>

      {tab === 'admin' && (
        !session ? (
          <Login />
        ) : isAdmin ? (
          <Admin />
        ) : (
          <p style={{ padding: 20 }}>관리자 권한이 없습니다</p>
        )
      )}

      {analysisOpen && (
        <div
          className="analysis-modal-backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) setAnalysisOpen(false);
          }}
        >
          <section
            className="analysis-modal"
            role="dialog"
            aria-modal="true"
            aria-labelledby="analysis-modal-title"
          >
            <header className="analysis-modal-header">
              <h2 id="analysis-modal-title">분석</h2>
              <button
                type="button"
                className="analysis-modal-close"
                onClick={() => setAnalysisOpen(false)}
                aria-label="분석 창 닫기"
              >
                닫기 ✕
              </button>
            </header>
            <div className="analysis-modal-content">
              {session ? <Analysis /> : <Login />}
            </div>
          </section>
        </div>
      )}
    </div>
  );
}
