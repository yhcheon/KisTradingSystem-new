import { useCallback, useEffect, useState } from 'react';
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

interface CandleState {
  key: string;
  data: CandleData[];
}

export function usePrices(interval: string, selected: SymbolKind): UsePricesResult {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candleState, setCandleState] = useState<CandleState>({
    key: '',
    data: [],
  });

  useEffect(() => {
    const controller = new AbortController();

    const fetchPrices = async () => {
      try {
        const response = await fetch('/api/prices', {
          signal: controller.signal,
        });
        if (!response.ok) throw new Error(`HTTP ${response.status}`);
        const json: PricesResponse = await response.json();
        if (!controller.signal.aborted) setData(json);
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error('prices fetch 실패:', error);
        }
      }
    };

    void fetchPrices();
    return () => controller.abort();
  }, []);

  const selectedResult = selected === 'regular' ? data?.regular : data?.mini;
  const selectedCode =
    selectedResult?.ok && selectedResult.data ? selectedResult.data.code : null;
  const candleKey = selectedCode ? `${selectedCode}:${interval}` : '';

  // 현재 종목/주기와 일치하는 데이터만 차트에 전달해 이전 요청의 캔들을 숨긴다.
  const candles = candleState.key === candleKey && candleKey
    ? candleState.data
    : [];

  const setCandles = useCallback<Dispatch<SetStateAction<CandleData[]>>>(
    (action) => {
      setCandleState((previous) => {
        const current = previous.key === candleKey ? previous.data : [];
        const next = typeof action === 'function' ? action(current) : action;
        return { key: candleKey, data: next };
      });
    },
    [candleKey],
  );

  useEffect(() => {
    if (!selectedCode) return;

    const controller = new AbortController();
    const params = new URLSearchParams({
      code: selectedCode,
      interval,
      date: 'both',
    });

    const fetchCandles = async () => {
      try {
        const response = await fetch(`/api/candles?${params.toString()}`, {
          signal: controller.signal,
        });
        if (!response.ok) return;

        const json = await response.json();
        if (!json.ok || controller.signal.aborted) return;

        const raw: CandleData[] = json.data;
        setCandleState({
          key: candleKey,
          data: mapCandlesForChart(raw),
        });
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error('candles fetch 실패:', error);
        }
      }
    };

    void fetchCandles();
    return () => controller.abort();
  }, [selectedCode, interval, candleKey]);

  return { data, setData, candles, setCandles, selectedCode };
}
