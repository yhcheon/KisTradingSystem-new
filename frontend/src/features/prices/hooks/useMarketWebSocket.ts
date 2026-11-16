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

const RECONNECT_INITIAL_DELAY_MS = 1_000;
const RECONNECT_MAX_DELAY_MS = 30_000;

function getWebSocketUrl(): string {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  return `${protocol}//${window.location.host}/ws`;
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
    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof window.setTimeout> | null = null;
    let reconnectDelay = RECONNECT_INITIAL_DELAY_MS;
    let disposed = false;
    const url = getWebSocketUrl();

    const connect = () => {
      if (disposed) return;

      socket = new WebSocket(url);

      socket.onopen = () => {
        reconnectDelay = RECONNECT_INITIAL_DELAY_MS;
      };

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

      socket.onclose = () => {
        if (disposed) return;

        reconnectTimer = window.setTimeout(connect, reconnectDelay);
        reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_MAX_DELAY_MS);
      };

      socket.onerror = () => {
        socket?.close();
      };
    };

    connect();

    return () => {
      disposed = true;
      if (reconnectTimer !== null) {
        window.clearTimeout(reconnectTimer);
      }
      socket?.close();
    };
  }, [interval, setData, setCandles]);
}
