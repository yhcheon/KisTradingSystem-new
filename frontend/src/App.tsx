
// ============================================================
// 파일명: App.tsx
// 설명: 메인 대시보드 (최초 1회 REST 조회, 폴링 없음)
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useState } from 'react';
import PriceCard from './PriceCard';
import type { PricesResponse } from './types';
import './App.css';

export default function App() {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [lastFetch, setLastFetch] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);

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

  return (
    <div className="app">
      <header>
        <h1>KOSPI200 선물 시세</h1>
        <div className="status">
          {error && <span className="error">연결 오류: {error}</span>}
          {!error && lastFetch && (
            <span className="ok">조회 시각: {lastFetch.toLocaleTimeString('ko-KR')}</span>
          )}
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


