import type { Session } from '@supabase/supabase-js';
import Login from '../pages/Login';
import Analysis from '../pages/Analysis';
import Admin from '../pages/Admin';

export type ActiveModal = 'analysis' | 'admin' | 'login' | null;

interface AppModalProps {
  activeModal: ActiveModal;
  session: Session | null;
  isAdmin: boolean;
  onClose: () => void;
}

export default function AppModal({
  activeModal,
  session,
  isAdmin,
  onClose,
}: AppModalProps) {
  if (!activeModal) return null;

  const title =
    activeModal === 'analysis'
      ? (session ? '분석' : '로그인')
      : activeModal === 'login'
        ? '로그인'
        : '관리자';

  const isLoginModal =
    activeModal === 'login' || (activeModal === 'analysis' && !session);

  return (
    <div
      className="analysis-modal-backdrop"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <section
        className={`analysis-modal ${isLoginModal ? 'login-modal' : ''}`}
        role="dialog"
        aria-modal="true"
        aria-labelledby="app-modal-title"
      >
        <header className="analysis-modal-header">
          <h2 id="app-modal-title">{title}</h2>
          <button
            type="button"
            className="analysis-modal-close"
            onClick={onClose}
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
  );
}
