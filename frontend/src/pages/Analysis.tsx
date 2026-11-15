// ============================================================
// 파일명: pages/Analysis.tsx
// 설명: 분석 탭 (관리자 글 목록 + Markdown 본문)
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useState } from 'react';
import ReactMarkdown from 'react-markdown';
import { authenticatedFetch } from '../lib/apiClient';
import type { NoteListItem, Note } from '../types';
import NewsLinks from '../features/news/NewsLinks';

export default function Analysis() {
  const [list, setList] = useState<NoteListItem[]>([]);
  const [selected, setSelected] = useState<Note | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(true);

  // 글 목록 로드
  useEffect(() => {
    const fetchList = async () => {
      const res = await authenticatedFetch('/api/notes');
      if (!res) {
        setError('로그인 필요');
        setLoading(false);
        return;
      }
      if (res.status === 403) {
        setError('무료 회원은 오후 2시부터 열람 가능합니다');
        setLoading(false);
        return;
      }
      if (!res.ok) { setError('글을 불러올 수 없습니다'); setLoading(false); return; }

      const json = await res.json();
      setList(json.data || []);
      setLoading(false);
    };
    fetchList();
  }, []);

  // 글 상세 로드
  const handleSelect = async (id: string) => {
    const res = await authenticatedFetch(`/api/notes/${encodeURIComponent(id)}`);
    if (!res) {
      alert('로그인이 필요합니다');
      return;
    }
    if (res.status === 403) {
      alert('무료 회원은 오후 2시부터 열람 가능합니다');
      return;
    }
    if (res.status === 404) {
      alert('글을 찾을 수 없습니다');
      return;
    }
    if (!res.ok) {
      alert('글 상세를 불러올 수 없습니다');
      return;
    }
    const result = await res.json();
    setSelected(result.data as Note);
  };

  if (loading) return <p style={{ color: '#94a3b8', padding: 20 }}>불러오는 중...</p>;
  if (error) return <p style={{ color: '#ef4444', padding: 20 }}>{error}</p>;

  return (
    <div>
      <NewsLinks />
      <div style={{ display: 'grid', gridTemplateColumns: '300px 1fr', gap: 20, padding: 20 }}>
      <aside style={{ display: 'flex', flexDirection: 'column', gap: 8 }}>
        {list.length === 0 && <p style={{ color: '#94a3b8' }}>글이 없습니다</p>}
        {list.map((n) => (
          <div
            key={n.id}
            onClick={() => handleSelect(n.id)}
            style={{
              padding: 12,
              background: selected?.id === n.id ? '#334155' : '#1e293b',
              border: '1px solid #334155',
              borderRadius: 8,
              cursor: 'pointer',
            }}
          >
            <div style={{ fontSize: 12, color: '#94a3b8' }}>
              {n.category === 'us_market' ? '미국장 분석' : '코스피 대응'}
            </div>
            <div style={{ fontSize: 14, color: '#e2e8f0' }}>{n.title}</div>
            <div style={{ fontSize: 11, color: '#64748b' }}>
              {new Date(n.created_at).toLocaleDateString('ko-KR')}
            </div>
          </div>
        ))}
      </aside>

      <section style={{ background: '#1e293b', padding: 20, borderRadius: 12 }}>
        {selected ? (
          <>
            <h2 style={{ color: '#f8fafc', marginBottom: 12 }}>{selected.title}</h2>
            <div style={{ color: '#94a3b8', fontSize: 12, marginBottom: 20 }}>
              {new Date(selected.created_at).toLocaleString('ko-KR')}
            </div>
            <div className="markdown-body">
              <ReactMarkdown>{selected.content}</ReactMarkdown>
            </div>
          </>
        ) : (
          <p style={{ color: '#94a3b8' }}>왼쪽에서 글을 선택하세요</p>
        )}
      </section>
      </div>
    </div>
  );
}
