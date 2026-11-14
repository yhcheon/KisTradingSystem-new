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
