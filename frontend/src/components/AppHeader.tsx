import { supabase } from '../lib/supabase';
import type { Session } from '@supabase/supabase-js';
import type { ActiveModal } from './AppModal';

interface AppHeaderProps {
  activeModal: ActiveModal;
  isAdmin: boolean;
  session: Session | null;
  onOpenModal: (modal: ActiveModal) => void;
}

export default function AppHeader({
  activeModal,
  isAdmin,
  session,
  onOpenModal,
}: AppHeaderProps) {
  return (
    <header className="top-bar">
      <h1>KOSPI200 선물 시세</h1>
      <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
        <button
          onClick={() => onOpenModal(null)}
          className={!activeModal ? 'active' : ''}
        >
          시세
        </button>
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
      </div>
    </header>
  );
}
