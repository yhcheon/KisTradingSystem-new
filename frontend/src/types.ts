
// ============================================================
// 파일명: types.ts
// 설명: 백엔드 API 응답 타입 정의
// 작성일: 2026-11-14
// ============================================================

export interface CachedPrice {
  code: string;
  name: string;
  price: string;        // 현재가
  prev_close: string;   // 전일 종가
  open: string;         // 시가
  high: string;         // 고가
  low: string;          // 저가
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


