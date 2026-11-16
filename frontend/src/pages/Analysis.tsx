// ============================================================
// 파일명: pages/Analysis.tsx
// 설명: 분석 탭 데이터 로딩 및 목록/상세 화면 조립
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useState } from 'react';
import { authenticatedFetch } from '../lib/apiClient';
import type { NoteListItem, Note } from '../types';
import NewsLinks from '../features/news/NewsLinks';
import AnalysisNoteList from '../features/analysis/AnalysisNoteList';
import AnalysisNoteDetail from '../features/analysis/AnalysisNoteDetail';

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
      if (!res.ok) {
        setError('글을 불러올 수 없습니다');
        setLoading(false);
        return;
      }

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
        <AnalysisNoteList notes={list} selected={selected} onSelect={handleSelect} />
        <AnalysisNoteDetail note={selected} />
      </div>
    </div>
  );
}
