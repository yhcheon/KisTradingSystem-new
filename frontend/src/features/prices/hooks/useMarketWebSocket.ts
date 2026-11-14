import { useEffect, useRef } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { PricesResponse, RealtimeTick } from '../../../types';
import type { CandleData } from '../types';
import { applyTickToCandles } from '../utils/candleUtils';

interface UseMarketWebSocketOptions {
  interval: string;
  selectedCode: string | null;
  setData: Dispatch<SetStateAction<PricesResponse | null>>;
  setCandles: Dispatch<SetStateAction<CandleData[]>>;
}

export function useMarketWebSocket({
  interval,
  selectedCode,
  setData,
  setCandles,
}: UseMarketWebSocketOptions): void {
  const selectedCodeRef = useRef<string | null>(selectedCode);

  useEffect(() => {
    selectedCodeRef.current = selectedCode;
  }, [selectedCode]);

  useEffect(() => {
    const url = `ws://${window.location.hostname}:8080/ws`;
    const socket = new WebSocket(url);

    socket.onmessage = (event) => {
      try {
        const tick: RealtimeTick = JSON.parse(event.data);
        if (tick.type !== 'tick') return;

        setData((previous) => {
          if (!previous) return previous;

          const updateOne = (result: PricesResponse['regular']) => {
            if (!result.ok || !result.data || result.data.code !== tick.code) {
              return result;
            }

            return {
              ok: true,
              data: {
                ...result.data,
                price: tick.price,
                change: tick.change,
                change_rate: tick.change_rate,
                volume: tick.volume,
                cached_at: Math.floor(Date.now() / 1000),
              },
            };
          };

          return {
            regular: updateOne(previous.regular),
            mini: updateOne(previous.mini),
          };
        });

        if (tick.code !== selectedCodeRef.current) return;
        setCandles((previous) => applyTickToCandles(previous, tick, interval));
      } catch {
        // Ignore malformed or unexpected WebSocket messages.
      }
    };

    return () => socket.close();
  }, [interval, setData, setCandles]);
}
