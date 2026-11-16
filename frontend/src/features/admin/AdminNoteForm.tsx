import type { CSSProperties } from 'react';
import type { Category } from './types';

const panelStyle: CSSProperties = {
  background: '#111827', border: '1px solid #334155', borderRadius: 8,
  padding: 16, marginBottom: 12, color: '#e2e8f0',
};
const fieldStyle: CSSProperties = {
  width: '100%', boxSizing: 'border-box', background: '#1e293b',
  color: '#e2e8f0', border: '1px solid #334155', borderRadius: 6,
  padding: '10px 12px', fontSize: 14,
};
const buttonStyle: CSSProperties = {
  border: 0, borderRadius: 6, padding: '9px 15px',
  color: 'white', cursor: 'pointer', fontSize: 14,
};

interface AdminNoteFormProps {
  title: string;
  content: string;
  category: Category;
  saving: boolean;
  editing: boolean;
  message: string | null;
  onTitleChange: (value: string) => void;
  onContentChange: (value: string) => void;
  onCategoryChange: (value: Category) => void;
  onSubmit: () => void;
  onCancel: () => void;
}

export default function AdminNoteForm({
  title, content, category, saving, editing, message,
  onTitleChange, onContentChange, onCategoryChange, onSubmit, onCancel,
}: AdminNoteFormProps) {
  return (
    <section style={panelStyle}>
      <h3 style={{ marginTop: 0, marginBottom: 16 }}>
        {editing ? '글 수정' : '새 글 작성'}
      </h3>

      <div style={{ marginBottom: 12 }}>
        <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>카테고리</label>
        <select value={category} onChange={event => onCategoryChange(event.target.value as Category)} style={fieldStyle}>
          <option value="us_market">미국장 분석</option>
          <option value="kospi_plan">코스피 대응</option>
        </select>
      </div>

      <div style={{ marginBottom: 12 }}>
        <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>제목</label>
        <input value={title} onChange={event => onTitleChange(event.target.value)}
          placeholder="제목을 입력하세요" style={fieldStyle} />
      </div>

      <div style={{ marginBottom: 12 }}>
        <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>본문 (Markdown)</label>
        <textarea value={content} onChange={event => onContentChange(event.target.value)}
          placeholder={'## 소제목\n\n- 항목1\n- 항목2\n\n**굵게** 표시도 가능'}
          rows={15} style={{ ...fieldStyle, fontFamily: 'Consolas, monospace', resize: 'vertical' }} />
      </div>

      <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
        <button onClick={onSubmit} disabled={saving}
          style={{ ...buttonStyle, background: '#2563eb', opacity: saving ? 0.6 : 1 }}>
          {saving ? '처리 중...' : editing ? '수정 저장' : '글 저장'}
        </button>
        {editing && (
          <button onClick={onCancel} disabled={saving}
            style={{ ...buttonStyle, background: '#475569' }}>수정 취소</button>
        )}
        {message && <span role="status" style={{ color: message.includes('완료') ? '#10b981' : '#f87171', fontSize: 13 }}>{message}</span>}
      </div>
    </section>
  );
}
