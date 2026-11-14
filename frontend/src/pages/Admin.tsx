// ============================================================
// 파일명: pages/Admin.tsx
// 설명: 관리자 글 작성 / 목록 / 수정 / 삭제 페이지
// ============================================================

import { useCallback, useEffect, useState } from 'react';
import { supabase } from '../lib/supabase';

type Category = 'us_market' | 'kospi_plan';
type Note = {
  id: string | number;
  title: string;
  content: string;
  category: Category | string;
  created_at?: string;
};

const panelStyle: React.CSSProperties = {
  background: '#111827', border: '1px solid #334155', borderRadius: 8,
  padding: 16, marginBottom: 12, color: '#e2e8f0',
};
const fieldStyle: React.CSSProperties = {
  width: '100%', boxSizing: 'border-box', background: '#1e293b',
  color: '#e2e8f0', border: '1px solid #334155', borderRadius: 6,
  padding: '10px 12px', fontSize: 14,
};
const buttonStyle: React.CSSProperties = {
  border: 0, borderRadius: 6, padding: '9px 15px',
  color: 'white', cursor: 'pointer', fontSize: 14,
};

export default function Admin() {
  const [title, setTitle] = useState('');
  const [content, setContent] = useState('');
  const [category, setCategory] = useState<Category>('us_market');
  const [notes, setNotes] = useState<Note[]>([]);
  const [loading, setLoading] = useState(false);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const [editingId, setEditingId] = useState<string | number | null>(null);

  const authorizedFetch = useCallback(async (method: string, body?: object) => {
    const { data: sessionData, error: sessionError } = await supabase.auth.getSession();
    if (sessionError) throw sessionError;
    const token = sessionData.session?.access_token;
    if (!token) throw new Error('로그인이 필요합니다.');

    const response = await fetch('/api/admin/notes', {
      method,
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${token}`,
      },
      ...(body ? { body: JSON.stringify(body) } : {}),
    });

    if (!response.ok) {
      const detail = await response.text();
      throw new Error(detail || `HTTP ${response.status}`);
    }
    if (response.status === 204) return null;
    return response.json();
  }, []);

  const loadNotes = useCallback(async () => {
    setLoading(true);
    try {
      const result = await authorizedFetch('GET');
      setNotes(Array.isArray(result?.data) ? result.data : []);
    } catch (error) {
      setMessage(`목록 조회 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setLoading(false);
    }
  }, [authorizedFetch]);

  useEffect(() => {
    void loadNotes();
  }, [loadNotes]);

  const resetForm = () => {
    setTitle('');
    setContent('');
    setCategory('us_market');
    setEditingId(null);
  };

  const handleSubmit = async () => {
    if (!title.trim() || !content.trim()) {
      setMessage('제목과 본문을 입력하세요.');
      return;
    }
    setSaving(true);
    setMessage(null);
    try {
      if (editingId !== null) {
        await authorizedFetch('PUT', { id: String(editingId), title, content, category });
        setMessage('수정 완료');
      } else {
        await authorizedFetch('POST', { title, content, category });
        setMessage('저장 완료');
      }
      resetForm();
      await loadNotes();
    } catch (error) {
      setMessage(`처리 실패: ${error instanceof Error ? error.message : String(error)}`);
    } finally {
      setSaving(false);
    }
  };

  const startEdit = (note: Note) => {
    setEditingId(note.id);
    setTitle(note.title);
    setContent(note.content);
    setCategory(note.category === 'kospi_plan' ? 'kospi_plan' : 'us_market');
    setMessage(null);
    window.scrollTo({ top: 0, behavior: 'smooth' });
  };

  const handleDelete = async (note: Note) => {
    if (!window.confirm(`'${note.title}' 글을 삭제하시겠습니까? 삭제 후 복구할 수 없습니다.`)) return;
    setMessage(null);
    try {
      await authorizedFetch('DELETE', { id: String(note.id) });
      if (editingId === note.id) resetForm();
      setMessage('삭제 완료');
      await loadNotes();
    } catch (error) {
      setMessage(`삭제 실패: ${error instanceof Error ? error.message : String(error)}`);
    }
  };

  const categoryName = (value: string) =>
    value === 'kospi_plan' ? '코스피 대응' : '미국장 분석';

  return (
    <div style={{ padding: 24, maxWidth: 1000, margin: '0 auto', color: '#f8fafc' }}>
      <h2 style={{ marginBottom: 20 }}>관리자 글 관리</h2>

      <section style={panelStyle}>
        <h3 style={{ marginTop: 0, marginBottom: 16 }}>
          {editingId !== null ? '글 수정' : '새 글 작성'}
        </h3>

        <div style={{ marginBottom: 12 }}>
          <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>카테고리</label>
          <select value={category} onChange={e => setCategory(e.target.value as Category)} style={fieldStyle}>
            <option value="us_market">미국장 분석</option>
            <option value="kospi_plan">코스피 대응</option>
          </select>
        </div>

        <div style={{ marginBottom: 12 }}>
          <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>제목</label>
          <input value={title} onChange={e => setTitle(e.target.value)}
            placeholder="제목을 입력하세요" style={fieldStyle} />
        </div>

        <div style={{ marginBottom: 12 }}>
          <label style={{ display: 'block', color: '#94a3b8', marginBottom: 6 }}>본문 (Markdown)</label>
          <textarea value={content} onChange={e => setContent(e.target.value)}
            placeholder={'## 소제목\n\n- 항목1\n- 항목2\n\n**굵게** 표시도 가능'}
            rows={15} style={{ ...fieldStyle, fontFamily: 'Consolas, monospace', resize: 'vertical' }} />
        </div>

        <div style={{ display: 'flex', gap: 10, alignItems: 'center', flexWrap: 'wrap' }}>
          <button onClick={handleSubmit} disabled={saving}
            style={{ ...buttonStyle, background: '#2563eb', opacity: saving ? 0.6 : 1 }}>
            {saving ? '처리 중...' : editingId !== null ? '수정 저장' : '글 저장'}
          </button>
          {editingId !== null && (
            <button onClick={resetForm} disabled={saving}
              style={{ ...buttonStyle, background: '#475569' }}>수정 취소</button>
          )}
          {message && <span role="status" style={{ color: message.includes('완료') ? '#10b981' : '#f87171', fontSize: 13 }}>{message}</span>}
        </div>
      </section>

      <section>
        <div style={{ display: 'flex', justifyContent: 'space-between', alignItems: 'center', marginBottom: 12 }}>
          <h3 style={{ margin: 0 }}>저장된 글 ({notes.length})</h3>
          <button onClick={() => void loadNotes()} disabled={loading}
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
                <button onClick={() => startEdit(note)}
                  style={{ ...buttonStyle, background: '#2563eb' }}>수정</button>
                <button onClick={() => void handleDelete(note)}
                  style={{ ...buttonStyle, background: '#b91c1c' }}>삭제</button>
              </div>
            </div>
          </article>
        ))}
      </section>
    </div>
  );
}
