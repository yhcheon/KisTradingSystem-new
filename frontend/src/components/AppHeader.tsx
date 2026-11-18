import { supabase } from '../lib/supabase';
import type { Session } from '@supabase/supabase-js';
import type { ActiveModal } from './AppModal';

interface AppHeaderProps {
  activeModal: ActiveModal;
  activePage: 'prices' | 'chat';
  authLoading: boolean;
  isAdmin: boolean;
  session: Session | null;
  isMarketConnected: boolean;
  onSelectPage: (page: 'prices' | 'chat') => void;
  onOpenModal: (modal: ActiveModal) => void;
}

export default function AppHeader({
  activeModal,
  activePage,
  authLoading,
  isAdmin,
  session,
  isMarketConnected,
  onSelectPage,
  onOpenModal,
}: AppHeaderProps) {
  return (
    <header className="top-bar">
      <h1>KOSPI200 선물 시세</h1>
      <nav className="app-tabs" role="tablist" aria-label="주요 화면">
        <button
          id="prices-tab"
          type="button"
          role="tab"
          aria-selected={activePage === 'prices'}
          aria-controls="prices-panel"
          className={activePage === 'prices' ? 'active' : ''}
          onClick={() => onSelectPage('prices')}
        >
          차트
        </button>
        <button
          id="chat-tab"
          type="button"
          role="tab"
          aria-selected={activePage === 'chat'}
          aria-controls="chat-panel"
          className={activePage === 'chat' ? 'active' : ''}
          onClick={() => onSelectPage('chat')}
        >
          채팅
        </button>
      </nav>
      <div className="header-actions">
        <span className={`market-websocket-status ${isMarketConnected ? 'connected' : 'closed'}`} role="status">
          {isMarketConnected ? '연결됨' : '연결 종료'}
        </span>
        {authLoading ? (
          <span role="status">로딩 중...</span>
        ) : (
          <>
            <button
              onClick={() => onOpenModal('analysis')}
              className={activeModal === 'analysis' ? 'active' : ''}
              aria-haspopup="dialog"
              aria-expanded={activeModal === 'analysis'}
            >
              분석
            </button>
            {isAdmin && (
              <button
                onClick={() => onOpenModal('admin')}
                className={activeModal === 'admin' ? 'active' : ''}
                aria-haspopup="dialog"
                aria-expanded={activeModal === 'admin'}
              >
                관리자
              </button>
            )}
            {session ? (
              <button onClick={() => void supabase.auth.signOut()}>로그아웃</button>
            ) : (
              <button
                onClick={() => onOpenModal('login')}
                className={activeModal === 'login' ? 'active' : ''}
              >
                로그인
              </button>
            )}
          </>
        )}
      </div>
    </header>
  );
}
