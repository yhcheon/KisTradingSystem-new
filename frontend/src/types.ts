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

export interface PriceResult {
  ok: boolean;
  data?: CachedPrice;
  error?: string;
}

export interface PricesResponse {
  regular: PriceResult;
  mini: PriceResult;
}

export interface RealtimeTick {
  type: 'tick';
  code: string;
  date: string;
  price: string;
  change: string;
  change_rate: string;
  volume: string;
  last_qty: string;
  time: string;
}

