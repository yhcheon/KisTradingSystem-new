// ============================================================
// 파일명: App.tsx
// 설명: 메인 대시보드 (좌: 시세 카드 세로, 우: 차트)
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef, useState } from 'react';
import PriceCard from './PriceCard';
import CandleChart from './CandleChart';
import type { PricesResponse, RealtimeTick } from './types';
import './App.css';

interface CandleData {
  time: string;
  hour: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

export default function App() {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candles, setCandles] = useState<CandleData[]>([]);
  const [lastFetch, setLastFetch] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wsStatus, setWsStatus] = useState<'connecting' | 'open' | 'closed'>('connecting');
  const wsRef = useRef<WebSocket | null>(null);

  // 최초 REST 조회 + 분봉 조회
  useEffect(() => {
    const fetchPrices = async () => {
      try {
        const res = await fetch('/api/prices');
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
        const json: PricesResponse = await res.json();
        setData(json);
        setLastFetch(new Date());
        setError(null);

        if (json.regular.ok && json.regular.data) {
          const code = json.regular.data.code;
          const cres = await fetch(`/api/candles?code=${code}`);
          if (cres.ok) {
            const cjson = await cres.json();
            if (cjson.ok) setCandles(cjson.data);
          }
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    };
    fetchPrices();
  }, []);

  // WebSocket
  useEffect(() => {
    const url = `ws://${window.location.hostname}:8080/ws`;
    const ws = new WebSocket(url);
    wsRef.current = ws;
    ws.onopen = () => setWsStatus('open');
    ws.onmessage = (e) => {
      try {
        const tick: RealtimeTick = JSON.parse(e.data);
        if (tick.type !== 'tick') return;
        setData((prev) => {
          if (!prev) return prev;
          const updateOne = (result: PricesResponse['regular']) => {
            if (!result.ok || !result.data) return result;
            if (result.data.code !== tick.code) return result;
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
          return { regular: updateOne(prev.regular), mini: updateOne(prev.mini) };
        });
      } catch {}
    };
    ws.onclose = () => setWsStatus('closed');
    return () => ws.close();
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

      <main className="layout">
        <aside className="sidebar">
          <PriceCard title="정규선물" result={data?.regular ?? null} />
          <PriceCard title="미니선물" result={data?.mini ?? null} />
        </aside>

        <section className="chart-area">
          <h2 className="chart-title">
            전일 1분봉 (정규선물) {candles.length > 0 && `— ${candles.length}건`}
          </h2>
          {candles.length > 0 ? (
            <CandleChart data={candles} />
          ) : (
            <p style={{ color: '#94a3b8' }}>분봉 데이터를 불러오는 중...</p>
          )}
        </section>
      </main>

      <footer>
        <p>데이터 출처: 한국투자증권 KIS Open API</p>
      </footer>
    </div>
  );
}

