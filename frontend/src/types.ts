// ============================================================
// 파일명: src/types.ts
// 설명: 전역 공용 타입 — 분석 글(Note)만 남김
//       시세 타입은 features/prices/types.ts 로 이동 (2026-11-19 분리)
// 작성일: 2026-11-14
// ============================================================

export interface Note {
  id: string;
  category: string;
  title: string;
  content: string;
  author_email: string;
  created_at: string;
}

export interface NoteListItem {
  id: string;
  category: string;
  title: string;
  created_at: string;
}
