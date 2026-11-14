// ============================================================
// 파일명: App.tsx
// 설명: 인증 상태 및 메인 탭 전환을 담당하는 애플리케이션 셸
//       - 게스트: 시세 탭만
//       - 로그인: 분석 탭
//       - admin: 관리자 탭 추가
// ============================================================

import { useEffect, useState } from 'react';
import type { Session } from '@supabase/supabase-js';
import { supabase } from './lib/supabase';
import Login from './pages/Login';
import Analysis from './pages/Analysis';
import Admin from './pages/Admin';
import PricesTab from './features/prices/PricesTab';
import './App.css';

type Tab = 'prices' | 'analysis' | 'admin';

export default function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [tab, setTab] = useState<Tab>('prices');
  const [isAdmin, setIsAdmin] = useState(false);

  useEffect(() => {
    supabase.auth.getSession().then(({ data }) => {
      setSession(data.session);
      setAuthLoading(false);
    });
    const { data: sub } = supabase.auth.onAuthStateChange((_event, s) => {
      setSession(s);
    });
    return () => sub.subscription.unsubscribe();
  }, []);

  // 로그인 시 admin 여부 확인
  useEffect(() => {
    if (!session) { setIsAdmin(false); return; }
    const checkAdmin = async () => {
      const { data } = await supabase
        .from('user_profiles')
        .select('tier')
        .eq('user_id', session.user.id)
        .maybeSingle();
      setIsAdmin(data?.tier === 'admin');
    };
    checkAdmin();
  }, [session]);

  if (authLoading) {
    return <p style={{ padding: 20, color: '#e2e8f0' }}>로딩 중...</p>;
  }

  return (
    <div className="app">
      <header className="top-bar">
        <h1>KOSPI200 선물 시세</h1>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button
            onClick={() => setTab('prices')}
            className={tab === 'prices' ? 'active' : ''}
          >
            시세
          </button>
          <button
            onClick={() => setTab('analysis')}
            className={tab === 'analysis' ? 'active' : ''}
          >
            분석
          </button>
          {isAdmin && (
            <button
              onClick={() => setTab('admin')}
              className={tab === 'admin' ? 'active' : ''}
            >
              관리자
            </button>
          )}
          {session ? (
            <button onClick={() => supabase.auth.signOut()}>로그아웃</button>
          ) : null}
        </div>
      </header>

      {tab === 'prices' ? (
        <PricesTab />
      ) : tab === 'analysis' ? (
        session ? <Analysis /> : <Login />
      ) : tab === 'admin' ? (
        isAdmin ? <Admin /> : <p style={{ padding: 20 }}>관리자 권한이 없습니다</p>
      ) : null}
    </div>
  );
}
