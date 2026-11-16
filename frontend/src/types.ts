// ============================================================
// 파일명: types.ts
// 설명: 백엔드 API + WebSocket 메시지 타입 정의
// 작성일: 2026-11-14
// ============================================================

export interface CachedPrice {
  code: string;
  name: string;
  price: string;
  prev_close: string;
  open: string;
  high: string;
  low: string;
  change: string;
  change_rate: string;
  volume: string;
  cached_at: number;
}

export interface SessionQuote {
  code: string;
  name: string;
  price: string;
  open: string;
  high: string;
  low: string;
  change: string;
  change_rate: string;
  volume: string;
}

export interface SessionPriceResult {
  ok: boolean;
  data?: SessionQuote;
  error?: string;
}

export interface PriceResult {
  ok: boolean;
  data?: CachedPrice;
  error?: string;
  night?: SessionPriceResult;
}

export interface PricesResponse {
  regular: PriceResult;
  mini: PriceResult;
}

export interface RealtimeTick {
  type: 'tick';
  market_session: 'day' | 'night';
  code: string;
  date: string;
  price: string;
  change: string;
  change_rate: string;
  volume: string;
  last_qty: string;
  time: string;
}


// 기존 코드 아래에 추가
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
