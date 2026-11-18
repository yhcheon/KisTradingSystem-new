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

export function usePrices(interval: string, selected: SymbolKind, backendReady: boolean): UsePricesResult {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candleState, setCandleState] = useState<CandleState>({
    key: '',
    data: [],
  });

  useEffect(() => {
    if (!backendReady || data?.regular?.ok || data?.mini?.ok) return;
    const controller = new AbortController();

    const fetchPrices = async () => {
      let retryDelay = 1_000;

      while (!controller.signal.aborted) {
        try {
          const response = await fetch('/api/prices', {
            signal: controller.signal,
          });
          if (!response.ok) throw new Error(`HTTP ${response.status}`);

          const json: PricesResponse = await response.json();
          const hasQuote = Boolean(json.regular?.ok || json.mini?.ok);
          if (hasQuote && !controller.signal.aborted) {
            // WebSocket 스냅샷이 먼저 도착했다면 REST 응답이 실시간 값을 덮어쓰지 않습니다.
            setData((previous) => previous ?? json);
            return;
          }
        } catch (error) {
          if (controller.signal.aborted) return;
          console.error('prices fetch 실패, 재시도합니다:', error);
        }

        await new Promise((resolve) => window.setTimeout(resolve, retryDelay));
        retryDelay = Math.min(retryDelay * 2, 10_000);
      }
    };

    void fetchPrices();
    return () => controller.abort();
  }, [backendReady, data]);

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
    if (!backendReady || !selectedCode) return;

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
        setCandleState((previous) => {
          const current = previous.key === candleKey ? previous.data : [];
          const merged = new Map<string, CandleData>();
          for (const candle of raw) {
            merged.set(`${candle.date}:${candle.time}`, candle);
          }
          // REST 누적 거래량과 조회 이후 실시간 체결을 함께 보존합니다.
          for (const candle of current) {
            const key = `${candle.date}:${candle.time}`;
            const apiCandle = merged.get(key);
            merged.set(
              key,
              apiCandle
                ? {
                    ...apiCandle,
                    high: Math.max(apiCandle.high, candle.high),
                    low: Math.min(apiCandle.low, candle.low),
                    close: candle.close,
                    volume: Math.max(apiCandle.volume, candle.volume),
                  }
                : candle,
            );
          }
          return {
            key: candleKey,
            data: mapCandlesForChart([...merged.values()]),
          };
        });
      } catch (error) {
        if (!controller.signal.aborted) {
          console.error('candles fetch 실패:', error);
        }
      }
    };

    void fetchCandles();
    // 서버가 18시 이후 REST 야간 분봉을 비동기로 채우는 경우를 반영하고,
    // 조회 사이에 새로 생성된 분봉도 주기적으로 동기화합니다.
    const refreshTimer = window.setInterval(() => {
      void fetchCandles();
    }, 10_000);

    return () => {
      controller.abort();
      window.clearInterval(refreshTimer);
    };
  }, [backendReady, selectedCode, interval, candleKey]);

  return { data, setData, candles, setCandles, selectedCode };
}
