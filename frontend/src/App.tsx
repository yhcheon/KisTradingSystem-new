// ============================================================
// 파일명: App.tsx
// 설명: 메인 대시보드
//       - 최초 1회 REST(/api/prices)로 스냅샷 조회
//       - 이후 /ws WebSocket으로 실시간 체결 수신
//       - 체결 시 현재가/전일대비/거래량 갱신
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef, useState } from 'react';
import PriceCard from './PriceCard';
import type { CachedPrice, PricesResponse, RealtimeTick } from './types';
import './App.css';

export default function App() {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [lastFetch, setLastFetch] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wsStatus, setWsStatus] = useState<'connecting' | 'open' | 'closed'>('connecting');
  const wsRef = useRef<WebSocket | null>(null);

  // 1) 최초 REST 조회
  useEffect(() => {
    const fetchPrices = async () => {
      try {
        const res = await fetch('/api/prices');
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const json: PricesResponse = await res.json();
        setData(json);
        setLastFetch(new Date());
        setError(null);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    };
    fetchPrices();
  }, []);

  // 2) WebSocket 연결
  useEffect(() => {
    const url = `ws://${window.location.hostname}:8080/ws`;
    const ws = new WebSocket(url);
    wsRef.current = ws;

    ws.onopen = () => {
      setWsStatus('open');
      console.log('✅ WebSocket connected:', url);
    };

    ws.onmessage = (e) => {
      try {
        const tick: RealtimeTick = JSON.parse(e.data);
        if (tick.type !== 'tick') return;

        // 해당 종목의 현재가/전일대비/거래량만 갱신
        setData((prev) => {
          if (!prev) return prev;
          const updateOne = (result: PricesResponse['regular']) => {
            if (!result.ok || !result.data) return result;
            if (result.data.code !== tick.code) return result;
            const updated: CachedPrice = {
              ...result.data,
              price: tick.price,
              change: tick.change,
              change_rate: tick.change_rate,
              volume: tick.volume,
              cached_at: Math.floor(Date.now() / 1000),
            };
            return { ok: true, data: updated };
          };
          return {
            regular: updateOne(prev.regular),
            mini: updateOne(prev.mini),
          };
        });
      } catch (err) {
        console.warn('WS 메시지 파싱 실패:', e.data);
      }
    };

    ws.onerror = (e) => console.error('❌ WebSocket error', e);
    ws.onclose = () => {
      setWsStatus('closed');
      console.log('WebSocket closed');
    };

    return () => {
      ws.close();
    };
  }, []);

  return (
    <div className="app">
      <header>
        <h1>KOSPI200 선물 시세</h1>
        <div className="status">
          {error && <span className="error">연결 오류: {error}</span>}
          {!error && lastFetch && (
            <span className="ok">최초 조회: {lastFetch.toLocaleTimeString('ko-KR')}</span>
          )}
          {' · '}
          <span className={wsStatus === 'open' ? 'ok' : wsStatus === 'connecting' ? '' : 'error'}>
            실시간: {wsStatus === 'open' ? '연결됨' : wsStatus === 'connecting' ? '연결 중...' : '끊김'}
          </span>
        </div>
      </header>

      <main className="cards">
        <PriceCard title="정규선물" result={data?.regular ?? null} />
        <PriceCard title="미니선물" result={data?.mini ?? null} />
      </main>

      <footer>
        <p>데이터 출처: 한국투자증권 KIS Open API</p>
      </footer>
    </div>
  );
}


