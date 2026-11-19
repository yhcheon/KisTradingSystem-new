// ============================================================
// 파일명: features/prices/types.ts
// 설명: 시세 전용 타입 — src/types.ts에서 이동 (2026-11-19 분리)
//       차트용 CandleData / SymbolKind / INTERVALS 포함
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
  reference_price: string;
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

export interface SessionRange {
  session: 'day' | 'night';
  date: string | null;
  status: 'waiting' | 'collecting' | 'monitoring' | 'unavailable';
  high: number | null;
  low: number | null;
  high_alerted: boolean;
  low_alerted: boolean;
}

export interface RangeMonitorSnapshot {
  day: SessionRange;
  night: SessionRange;
}

export interface RangeAlert {
  session: 'day' | 'night';
  direction: 'high' | 'low';
  price: number;
  reference_price: number;
  message: string;
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
  range?: RangeMonitorSnapshot;
  range_alert?: RangeAlert | null;
}

export interface CandleData {
  date: string;
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export type SymbolKind = 'regular' | 'mini';

export const INTERVALS = ['1m', '2m', '5m', '10m', '30m', '60m'];
