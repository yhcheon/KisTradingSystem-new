import ReactMarkdown from 'react-markdown';
import type { Note } from '../../types';

interface AnalysisNoteDetailProps {
  note: Note | null;
}

export default function AnalysisNoteDetail({ note }: AnalysisNoteDetailProps) {
  return (
    <section style={{ background: '#1e293b', padding: 20, borderRadius: 12 }}>
      {note ? (
        <>
          <h2 style={{ color: '#f8fafc', marginBottom: 12 }}>{note.title}</h2>
          <div style={{ color: '#94a3b8', fontSize: 12, marginBottom: 20 }}>
            {new Date(note.created_at).toLocaleString('ko-KR')}
          </div>
          <div className="markdown-body">
            <ReactMarkdown>{note.content}</ReactMarkdown>
          </div>
        </>
      ) : (
        <p style={{ color: '#94a3b8' }}>왼쪽에서 글을 선택하세요</p>
      )}
    </section>
  );
}
