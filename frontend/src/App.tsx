// ============================================================
// 파일명: App.tsx
// 설명: 인증 상태와 메인 시세 화면의 생명주기를 관리하는 애플리케이션 셸
//       - 시세 화면은 항상 마운트하여 모달 전환 중에도 상태와 WebSocket 유지
//       - OAuth 세션 공유 및 로그인 후 모달 전환 처리
// ============================================================

import { useEffect, useRef, useState } from 'react';
import { useAuthSession } from './features/auth/hooks/useAuthSession';
import { useAdminRole } from './features/auth/hooks/useAdminRole';
import PricesTab from './features/prices/PricesTab';
import ChatPage from './features/chat/ChatPage';
import AppHeader from './components/AppHeader';
import AppModal, { type ActiveModal } from './components/AppModal';
import './App.css';

export default function App() {
  const { session, authLoading } = useAuthSession();
  const [activePage, setActivePage] = useState<'prices' | 'chat'>('prices');
  const [activeModal, setActiveModal] = useState<ActiveModal>(null);
  const previousSessionRef = useRef(session);
  const isAdmin = useAdminRole(session);
  const selectPage = (page: 'prices' | 'chat') => {
    setActivePage(page);
    setActiveModal(null);
  };

  // Google OAuth 팝업에서 인증이 완료되면 이 창의 세션도 공유되고, 팝업은 자동으로 닫습니다.
  useEffect(() => {
    if (window.opener && session) {
      window.opener.postMessage({ type: 'supabase-auth-complete' }, window.location.origin);
      window.close();
    }
  }, [session]);

  // 비회원 상태에서 로그인에 성공하면 로그인 유도 모달만 닫고 시세 화면에 머뭅니다.
  // 분석 화면은 사용자가 분석 버튼을 다시 눌렀을 때만 엽니다.
  useEffect(() => {
    const justSignedIn = !previousSessionRef.current && !!session;
    previousSessionRef.current = session;

    if (justSignedIn && (activeModal === 'login' || activeModal === 'analysis')) {
      setActiveModal(null);
    }
  }, [session, activeModal]);

  useEffect(() => {
    if (!activeModal) return;

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') setActiveModal(null);
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [activeModal]);

  // OAuth 팝업은 인증 세션을 확인한 뒤 부모 창으로 결과를 전달하고 닫힙니다.
  if (!authLoading && window.opener && session) {
    return <p style={{ padding: 20, color: '#e2e8f0' }}>로그인 완료. 창을 닫는 중...</p>;
  }

  return (
    <div className="app">
      <AppHeader
        activeModal={activeModal}
        isAdmin={isAdmin}
        session={session}
        authLoading={authLoading}
        activePage={activePage}
        onSelectPage={selectPage}
        onOpenModal={setActiveModal}
      />

      <div
        id="prices-panel"
        role="tabpanel"
        aria-labelledby="prices-tab"
        hidden={activePage !== 'prices'}
      >
        <PricesTab />
      </div>
      <div
        id="chat-panel"
        role="tabpanel"
        aria-labelledby="chat-tab"
        hidden={activePage !== 'chat'}
      >
        <ChatPage />
      </div>

      {!authLoading && (
        <AppModal
          activeModal={activeModal}
          session={session}
          isAdmin={isAdmin}
          onClose={() => setActiveModal(null)}
        />
      )}
    </div>
  );
}
