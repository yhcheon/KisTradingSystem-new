// ============================================================
// 파일명: pages/Analysis.tsx
// 설명: 분석 탭 데이터 로딩 및 목록/상세 화면 조립
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef, useState } from 'react';
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
  const detailControllerRef = useRef<AbortController | null>(null);

  // 글 목록 로드: 화면 이탈 시 요청을 취소하고 언마운트 후 상태 변경을 방지
  useEffect(() => {
    const controller = new AbortController();

    const fetchList = async () => {
      try {
        const res = await authenticatedFetch('/api/notes', {
          signal: controller.signal,
        });
        if (controller.signal.aborted) return;

        if (!res) {
          setError('로그인 필요');
          return;
        }
        if (res.status === 403) {
          setError('무료 회원은 오후 2시부터 열람 가능합니다');
          return;
        }
        if (!res.ok) {
          setError('글을 불러올 수 없습니다');
          return;
        }

        const json = await res.json();
        if (!controller.signal.aborted) {
          setList(Array.isArray(json.data) ? json.data : []);
        }
      } catch (requestError) {
        if (!controller.signal.aborted) {
          console.error('분석 글 목록 조회 실패:', requestError);
          setError('글을 불러올 수 없습니다');
        }
      } finally {
        if (!controller.signal.aborted) setLoading(false);
      }
    };

    void fetchList();
    return () => {
      controller.abort();
      detailControllerRef.current?.abort();
    };
  }, []);

  // 새 글 선택 시 이전 상세 요청을 취소해 늦게 도착한 응답이 선택 상태를 덮지 않도록 처리
  const handleSelect = async (id: string) => {
    detailControllerRef.current?.abort();
    const controller = new AbortController();
    detailControllerRef.current = controller;

    try {
      const res = await authenticatedFetch(
        `/api/notes/${encodeURIComponent(id)}`,
        { signal: controller.signal },
      );
      if (controller.signal.aborted) return;

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
      if (!controller.signal.aborted) setSelected(result.data as Note);
    } catch (requestError) {
      if (!controller.signal.aborted) {
        console.error('분석 글 상세 조회 실패:', requestError);
        alert('글 상세를 불러오지 못했습니다. 다시 시도해 주세요.');
      }
    }
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
