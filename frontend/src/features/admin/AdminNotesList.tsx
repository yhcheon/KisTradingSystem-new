import type { CSSProperties } from 'react';
import type { Note } from './types';

const panelStyle: CSSProperties = {
  background: '#111827', border: '1px solid #334155', borderRadius: 8,
  padding: 16, marginBottom: 12, color: '#e2e8f0',
};
const buttonStyle: CSSProperties = {
  border: 0, borderRadius: 6, padding: '9px 15px',
  color: 'white', cursor: 'pointer', fontSize: 14,
};

interface AdminNotesListProps {
  notes: Note[];
  loading: boolean;
  onRefresh: () => void;
  onEdit: (note: Note) => void;
  onDelete: (note: Note) => void;
}

function categoryName(value: string) {
  return value === 'kospi_plan' ? '코스피 대응' : '미국장 분석';
}

export default function AdminNotesList({
  notes, loading, onRefresh, onEdit, onDelete,
}: AdminNotesListProps) {
  return (
    <section>
      <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
        <h3 style={{ margin: 0 }}>저장된 글 ({notes.length})</h3>
        <button onClick={onRefresh} disabled={loading}
          style={{ ...buttonStyle, background: '#475569' }}>
          {loading ? '불러오는 중...' : '새로고침'}
        </button>
      </div>

      {loading && notes.length === 0 && <p style={{ color: '#94a3b8' }}>글 목록을 불러오는 중입니다.</p>}
      {!loading && notes.length === 0 && (
        <div style={{ ...panelStyle, color: '#94a3b8' }}>저장된 글이 없습니다.</div>
      )}
      {notes.map(note => (
        <article key={String(note.id)} style={panelStyle}>
          <div style={{ display: 'flex', justifyContent: 'space-between', gap: 12, flexWrap: 'wrap' }}>
            <div style={{ minWidth: 0, flex: 1 }}>
              <div style={{ color: '#60a5fa', fontSize: 12, marginBottom: 6 }}>
                {categoryName(note.category)}
                {note.created_at ? ` · ${new Date(note.created_at).toLocaleString('ko-KR')}` : ''}
              </div>
              <h4 style={{ margin: '0 0 8px', overflowWrap: 'anywhere' }}>{note.title}</h4>
              <p style={{
                margin: 0, color: '#cbd5e1', fontSize: 13, whiteSpace: 'pre-wrap',
                overflowWrap: 'anywhere', maxHeight: 100, overflow: 'hidden',
              }}>{note.content}</p>
            </div>
            <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start' }}>
              <button onClick={() => onEdit(note)}
                style={{ ...buttonStyle, background: '#2563eb' }}>수정</button>
              <button onClick={() => onDelete(note)}
                style={{ ...buttonStyle, background: '#b91c1c' }}>삭제</button>
            </div>
          </div>
        </article>
      ))}
    </section>
  );
}
