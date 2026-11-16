import { useEffect, useLayoutEffect, useRef } from 'react';
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

function numeric(value: string): number {
  return Number.parseFloat(value.replace(/,/g, '').trim());
}

export function useMarketWebSocket({
  interval,
  selectedCode,
  setData,
  setCandles,
}: UseMarketWebSocketOptions): void {
  const selectedCodeRef = useRef<string | null>(selectedCode);
  const intervalRef = useRef(interval);
  useLayoutEffect(() => {
    intervalRef.current = interval;
  }, [interval]);

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

              if (tick.market_session === 'night') {
                const night = result.night;
                if (!night?.ok || !night.data || night.data.code !== tick.code) {
                  return result;
                }
                const current = numeric(tick.price);
                const high = numeric(night.data.high);
                const low = numeric(night.data.low);
                return {
                  ...result,
                  night: {
                    ...night,
                    data: {
                      ...night.data,
                      price: tick.price,
                      change: tick.change,
                      change_rate: tick.change_rate,
                      high: Number.isFinite(high) ? String(Math.max(high, current)) : tick.price,
                      low: Number.isFinite(low) ? String(Math.min(low, current)) : tick.price,
                      volume: tick.volume,
                    },
                  },
                };
              }

              return {
                ...result,
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

          // 야간 체결은 좌측 야간 시세만 갱신하고 주간 차트 캔들에는 반영하지 않습니다.
          if (tick.market_session === 'night' || tick.code !== selectedCodeRef.current) return;
          setCandles((previous) =>
            applyTickToCandles(previous, tick, intervalRef.current),
          );
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
  }, [setData, setCandles]);
}
