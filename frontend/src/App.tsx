// ============================================================
// 파일명: App.tsx
// 설명: 메인 대시보드 (좌: 시세 카드, 우: 차트 + interval 선택)
//       - 어제 + 오늘 봉을 가상 시간으로 이어붙임
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef, useState } from 'react';
import PriceCard from './PriceCard';
import CandleChart from './CandleChart';
import type { PricesResponse, RealtimeTick } from './types';
import './App.css';

interface CandleData {
  date: string;
  time: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

const INTERVALS = ['1m', '2m', '5m', '10m', '30m', '60m'];

export default function App() {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candles, setCandles] = useState<CandleData[]>([]);
  const [interval, setInterval] = useState<string>('1m');
  const [lastFetch, setLastFetch] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wsStatus, setWsStatus] = useState<'connecting' | 'open' | 'closed'>('connecting');
  const wsRef = useRef<WebSocket | null>(null);
  const regularCodeRef = useRef<string | null>(null);

  // 최초 REST 조회
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
          regularCodeRef.current = json.regular.data.code;
        }
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      }
    };
    fetchPrices();
  }, []);

  // interval 변경 시 분봉 재조회
  useEffect(() => {
    const code = regularCodeRef.current;
    if (!code) return;

    const fetchCandles = async () => {
      try {
        const cres = await fetch(`/api/candles?code=${code}&interval=${interval}&date=both`);
        if (!cres.ok) return;
        const cjson = await cres.json();
        if (!cjson.ok) return;

        const raw: CandleData[] = cjson.data;
        if (raw.length === 0) { setCandles([]); return; }

        // 오늘 날짜 (KST, YYYYMMDD)
        const todayStr = new Date(Date.now() + 9 * 3600 * 1000)
          .toISOString().slice(0, 10).replace(/-/g, '');

        const yesterday = raw.filter((c) => c.date !== todayStr);
        const today = raw.filter((c) => c.date === todayStr);

        const mapped: CandleData[] = [];
        let lastMin = 0;

        // 어제 봉: 실제 시각 그대로
        for (const c of yesterday) {
          const hh = Number(c.time.slice(0, 2));
          const mm = Number(c.time.slice(2, 4));
          lastMin = hh * 60 + mm;
          mapped.push(c);
        }

        // 오늘 봉: 어제 마지막 + 1분부터 이어붙임
        if (today.length > 0) {
          const firstTodayMin =
            Number(today[0].time.slice(0, 2)) * 60 + Number(today[0].time.slice(2, 4));
          const offset = (yesterday.length > 0 ? lastMin + 1 : 0) - firstTodayMin;
          for (const c of today) {
            const hh = Number(c.time.slice(0, 2));
            const mm = Number(c.time.slice(2, 4));
            const virtMin = hh * 60 + mm + offset;
            const vhh = String(Math.floor(virtMin / 60)).padStart(2, '0');
            const vmm = String(virtMin % 60).padStart(2, '0');
            mapped.push({ ...c, time: `${vhh}${vmm}00` });
          }
        }

        setCandles(mapped);
      } catch (e) {
        console.error('candles fetch 실패:', e);
      }
    };
    fetchCandles();
  }, [interval, data]);

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
          <div className="chart-header">
            <h2 className="chart-title">
              분봉 차트 (정규선물) {candles.length > 0 && `— ${candles.length}건`}
            </h2>
            <div className="interval-buttons">
              {INTERVALS.map((iv) => (
                <button
                  key={iv}
                  className={interval === iv ? 'active' : ''}
                  onClick={() => setInterval(iv)}
                >
                  {iv.replace('m', '분')}
                </button>
              ))}
            </div>
          </div>
          {candles.length > 0 ? (
            <CandleChart data={candles} interval={interval} />
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

