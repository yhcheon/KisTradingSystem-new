import type { NoteListItem, Note } from '../../types';

interface AnalysisNoteListProps {
  notes: NoteListItem[];
  selected: Note | null;
  onSelect: (id: string) => void;
}

export default function AnalysisNoteList({ notes, selected, onSelect }: AnalysisNoteListProps) {
  return (
    <aside style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
      {notes.length === 0 && <p style={{ color: '#94a3b8' }}>글이 없습니다</p>}
      {notes.map(note => (
        <div
          key={note.id}
          onClick={() => onSelect(note.id)}
          style={{
            padding: 12,
            background: selected?.id === note.id ? '#334155' : '#1e293b',
            border: '1px solid #334155',
            borderRadius: 8,
            cursor: 'pointer',
          }}
        >
          <div style={{ fontSize: 12, color: '#94a3b8' }}>
            {note.category === 'us_market' ? '미국장 분석' : '코스피 대응'}
          </div>
          <div style={{ fontSize: 14, color: '#e2e8f0' }}>{note.title}</div>
          <div style={{ fontSize: 11, color: '#64748b' }}>
            {new Date(note.created_at).toLocaleDateString('ko-KR')}
          </div>
        </div>
      ))}
    </aside>
  );
}
