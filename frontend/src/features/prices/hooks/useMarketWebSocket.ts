import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { PricesResponse, RangeAlert, RangeMonitorSnapshot, RealtimeTick } from '../../../types';
import type { CandleData } from '../types';
import { applyTickToCandles } from '../utils/candleUtils';

interface UseMarketWebSocketOptions {
  enabled: boolean;
  interval: string;
  selectedCode: string | null;
  setData: Dispatch<SetStateAction<PricesResponse | null>>;
  setCandles: Dispatch<SetStateAction<CandleData[]>>;
  onRangeUpdate: (range: RangeMonitorSnapshot) => void;
  onRangeAlert: (alert: RangeAlert) => void;
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
  enabled,
  interval,
  selectedCode,
  setData,
  setCandles,
  onRangeUpdate,
  onRangeAlert,
}: UseMarketWebSocketOptions): boolean {
  const [isConnected, setIsConnected] = useState(false);
  const selectedCodeRef = useRef<string | null>(selectedCode);
  const intervalRef = useRef(interval);
  useLayoutEffect(() => {
    intervalRef.current = interval;
  }, [interval]);

  useEffect(() => {
    selectedCodeRef.current = selectedCode;
  }, [selectedCode]);

  useEffect(() => {
    if (!enabled) {
      setIsConnected(false);
      return;
    }

    let socket: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof window.setTimeout> | null = null;
    let reconnectDelay = RECONNECT_INITIAL_DELAY_MS;
    let disposed = false;
    const url = getWebSocketUrl();

    const connect = () => {
      if (disposed || !enabled) return;

      setIsConnected(false);
      const nextSocket = new WebSocket(url);
      socket = nextSocket;

      nextSocket.onopen = () => {
        if (disposed || socket !== nextSocket) {
          nextSocket.close();
          return;
        }
        reconnectDelay = RECONNECT_INITIAL_DELAY_MS;
        setIsConnected(true);
      };

      nextSocket.onmessage = (event) => {
        try {
          const message: unknown = JSON.parse(event.data);
          if (
            typeof message === 'object' &&
            message !== null &&
            'type' in message &&
            message.type === 'snapshot' &&
            'data' in message &&
            typeof message.data === 'object' &&
            message.data !== null &&
            'regular' in message.data &&
            'mini' in message.data
          ) {
            if ('range' in message && typeof message.range === 'object' && message.range !== null) {
              onRangeUpdate(message.range as RangeMonitorSnapshot);
            }
            // 유효한 가격이 포함된 스냅샷만 적용합니다. 빈 스냅샷이면 REST 재시도를 계속합니다.
            const snapshot = message.data as PricesResponse;
            if (snapshot.regular?.ok || snapshot.mini?.ok) {
              setData(snapshot);
            }
            return;
          }

          if (
            typeof message !== 'object' ||
            message === null ||
            !('type' in message) ||
            message.type !== 'tick'
          ) return;
          const tick = message as RealtimeTick;
          if (tick.range) onRangeUpdate(tick.range);
          if (tick.range_alert) onRangeAlert(tick.range_alert);

          setData((previous) => {
            if (!previous) return previous;

            const updateOne = (result: PricesResponse['regular']) => {
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

              if (!result.ok || !result.data || result.data.code !== tick.code) {
                return result;
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

          // 선택 종목의 주간/야간 체결을 모두 현재 차트에 반영합니다.
          if (tick.code !== selectedCodeRef.current) return;
          setCandles((previous) =>
            applyTickToCandles(previous, tick, intervalRef.current),
          );
        } catch {
          // Ignore malformed or unexpected WebSocket messages.
        }
      };

      nextSocket.onclose = () => {
        if (disposed || !enabled || socket !== nextSocket) return;

        socket = null;
        setIsConnected(false);
        reconnectTimer = window.setTimeout(connect, reconnectDelay);
        reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_MAX_DELAY_MS);
      };

      nextSocket.onerror = () => {
        if (disposed || socket !== nextSocket) return;

        setIsConnected(false);
        nextSocket.close();
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
  }, [enabled, setData, setCandles, onRangeUpdate, onRangeAlert]);

  return isConnected;
}
