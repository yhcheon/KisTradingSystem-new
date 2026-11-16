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
    const controller = new AbortController();

    const fetchPrices = async () => {
      try {
        const response = await fetch('/api/prices', {
          signal: controller.signal,
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const json: PricesResponse = await response.json();
        setData(json);
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error('prices fetch 실패:', error);
        }
      }
    };

    fetchPrices();
    return () => controller.abort();
  }, []);

  const selectedResult = selected === 'regular' ? data?.regular : data?.mini;
  const selectedCode =
    selectedResult?.ok && selectedResult.data ? selectedResult.data.code : null;

  useEffect(() => {
    if (!selectedCode) {
      setCandles([]);
      return;
    }

    const controller = new AbortController();
    const params = new URLSearchParams({
      code: selectedCode,
      interval,
      date: 'both',
    });

    setCandles([]);

    const fetchCandles = async () => {
      try {
        const response = await fetch(`/api/candles?${params.toString()}`, {
          signal: controller.signal,
        });
        if (!response.ok) return;

        const json = await response.json();
        if (!json.ok) return;

        const raw: CandleData[] = json.data;
        setCandles(mapCandlesForChart(raw));
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error('candles fetch 실패:', error);
        }
      }
    };

    fetchCandles();
    return () => controller.abort();
  }, [selectedCode, interval]);

  return { data, setData, candles, setCandles, selectedCode };
}
