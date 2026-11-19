
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import type { Dispatch, SetStateAction } from 'react';
import type { CandleData, PricesResponse, RangeAlert, RangeMonitorSnapshot, RealtimeTick } from '../types';
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
  const alertedRangeKeysRef = useRef(new Set<string>());
  const rangeSnapshotRef = useRef<RangeMonitorSnapshot | null>(null);
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

    // A: 시세 WS도 health를 보고 붙입니다. 백엔드 초기화 중에는 WS를 열지 않고
    // Retry-After만큼 기다렸다가 다시 시도합니다. 그래야 초기화 구간에
    // WS가 바로 끊기고 재연결이 몰리는 일을 막을 수 있습니다.
    const checkHealth = async (): Promise<boolean> => {
      try {
        const response = await fetch('/api/health', { cache: 'no-store' });
        if (response.ok) {
          const payload = (await response.json()) as { ready?: boolean; status?: string };
          return payload.ready === true || payload.status === 'ok';
        }
        const retryAfter = Number(response.headers.get('Retry-After'));
        if (Number.isFinite(retryAfter) && retryAfter > 0) {
          reconnectDelay = Math.min(Math.max(retryAfter * 1000, RECONNECT_INITIAL_DELAY_MS), RECONNECT_MAX_DELAY_MS);
        }
        return false;
      } catch {
        return false;
      }
    };

    const scheduleReconnect = () => {
      if (disposed || !enabled) return;
      // 지터: 탭 여러 개가 같은 타이밍에 재연결하지 않게 0~500ms 랜덤을 더합니다.
      const delayWithJitter = reconnectDelay + Math.floor(Math.random() * 500);
      reconnectTimer = window.setTimeout(() => {
        reconnectTimer = null;
        void connect();
      }, delayWithJitter);
      reconnectDelay = Math.min(reconnectDelay * 2, RECONNECT_MAX_DELAY_MS);
    };

    const connect = async () => {
      if (disposed || !enabled) return;

      setIsConnected(false);
      // health가 ready가 아니면 WS를 열지 않습니다.
      const ready = await checkHealth();
      if (!ready || disposed || !enabled) {
        scheduleReconnect();
        return;
      }
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
              const range = message.range as RangeMonitorSnapshot;
              onRangeUpdate(range);
              rangeSnapshotRef.current = range;
              for (const session of [range.day, range.night]) {
                if (!session.date) continue;
                if (session.high_alerted) alertedRangeKeysRef.current.add(`${session.date}:${session.session}:high`);
                if (session.low_alerted) alertedRangeKeysRef.current.add(`${session.date}:${session.session}:low`);
              }
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
          if (tick.range) {
            const previousRange = rangeSnapshotRef.current;
            for (const session of [tick.range.day, tick.range.night]) {
              const previousSession = previousRange?.[session.session];
              for (const direction of ['high', 'low'] as const) {
                const isAlerted = direction === 'high' ? session.high_alerted : session.low_alerted;
                const wasAlerted = direction === 'high'
                  ? previousSession?.high_alerted
                  : previousSession?.low_alerted;
                if (!isAlerted || wasAlerted) continue;

                const alertKey = `${session.date ?? tick.date}:${session.session}:${direction}`;
                if (alertedRangeKeysRef.current.has(alertKey)) continue;
                alertedRangeKeysRef.current.add(alertKey);

                const matchingEvent = tick.range_alert
                  && tick.range_alert.session === session.session
                  && tick.range_alert.direction === direction
                  ? tick.range_alert
                  : null;
                const referencePrice = direction === 'high' ? session.high : session.low;
                const fallbackAlert: RangeAlert = {
                  session: session.session,
                  direction,
                  price: numeric(tick.price),
                  reference_price: referencePrice ?? numeric(tick.price),
                  message: direction === 'high'
                    ? '시세가 고가 돌파 했습니다. 숏 포지션은 위험 관리 하세요.'
                    : '시세가 저가를 깼습니다. 롱 포지션은 위험 관리 하세요.',
                };
                onRangeAlert(matchingEvent ?? fallbackAlert);
              }
            }
            rangeSnapshotRef.current = tick.range;
            onRangeUpdate(tick.range);
          } else if (tick.range_alert) {
            const alertKey = `${tick.date}:${tick.range_alert.session}:${tick.range_alert.direction}`;
            if (!alertedRangeKeysRef.current.has(alertKey)) {
              alertedRangeKeysRef.current.add(alertKey);
              onRangeAlert(tick.range_alert);
            }
          }

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
        scheduleReconnect();
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



