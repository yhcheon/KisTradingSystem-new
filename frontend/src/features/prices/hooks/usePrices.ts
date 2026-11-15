import { useEffect, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { PricesResponse } from '../../../types';
import type { CandleData, SymbolKind } from '../types';
import { mapCandlesForChart } from '../utils/candleUtils';

interface UsePricesResult {
  data: PricesResponse | null;
  setData: Dispatch<SetStateAction<PricesResponse | null>>;
  candles: CandleData[];
  setCandles: Dispatch<SetStateAction<CandleData[]>>;
  selectedCode: string | null;
}

export function usePrices(interval: string, selected: SymbolKind): UsePricesResult {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candles, setCandles] = useState<CandleData[]>([]);

  useEffect(() => {
    const fetchPrices = async () => {
      try {
        const response = await fetch('/api/prices');
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const json: PricesResponse = await response.json();
        setData(json);
      } catch (error) {
        console.error('prices fetch 실패:', error);
      }
    };

    fetchPrices();
  }, []);

  const selectedResult = selected === 'regular' ? data?.regular : data?.mini;
  const selectedCode =
    selectedResult?.ok && selectedResult.data ? selectedResult.data.code : null;

  useEffect(() => {
    if (!selectedCode) return;

    const fetchCandles = async () => {
      try {
        const response = await fetch(
          `/api/candles?code=${selectedCode}&interval=${interval}&date=both`,
        );
        if (!response.ok) return;

        const json = await response.json();
        if (!json.ok) return;

        const raw: CandleData[] = json.data;
        setCandles(mapCandlesForChart(raw));
      } catch (error) {
        console.error('candles fetch 실패:', error);
      }
    };

    fetchCandles();
  }, [selectedCode, interval]);

  return { data, setData, candles, setCandles, selectedCode };
}
