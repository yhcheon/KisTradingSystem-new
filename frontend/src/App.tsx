// ============================================================
// 파일명: App.tsx
// 설명: 메인 대시보드 + 탭 ([시세] [분석])
//       - 게스트: 시세 탭만 (로그인 없이)
//       - 로그인: 분석 탭 접근 가능
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef, useState } from 'react';
import type { Session } from '@supabase/supabase-js';
import { supabase } from './lib/supabase';
import Login from './pages/Login';
import Analysis from './pages/Analysis';
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
type SymbolKind = 'regular' | 'mini';
type Tab = 'prices' | 'analysis';

function intervalToMinutes(interval: string): number {
  return parseInt(interval.replace('m'), 10);
}

export default function App() {
  const [session, setSession] = useState<Session | null>(null);
  const [authLoading, setAuthLoading] = useState(true);
  const [tab, setTab] = useState<Tab>('prices');

  useEffect(() => {
    supabase.auth.getSession().then(({ data }) => {
      setSession(data.session);
      setAuthLoading(false);
    });
    const { data: sub } = supabase.auth.onAuthStateChange((_event, s) => {
      setSession(s);
    });
    return () => sub.subscription.unsubscribe();
  }, []);

  if (authLoading) {
    return <p style={{ padding: 20, color: '#e2e8f0' }}>로딩 중...</p>;
  }

  return (
    <div className="app">
      <header className="top-bar">
        <h1>KOSPI200 선물 시세</h1>
        <div style={{ display: 'flex', gap: 8, alignItems: 'center' }}>
          <button
            onClick={() => setTab('prices')}
            className={tab === 'prices' ? 'active' : ''}
          >
            시세
          </button>
          <button
            onClick={() => setTab('analysis')}
            className={tab === 'analysis' ? 'active' : ''}
          >
            분석
          </button>
          {session ? (
            <button onClick={() => supabase.auth.signOut()}>로그아웃</button>
          ) : null}
        </div>
      </header>

      {tab === 'prices' ? (
        <PricesTab />
      ) : session ? (
        <Analysis />
      ) : (
        <Login />
      )}
    </div>
  );
}

// ── 시세 탭 (기존 App.tsx의 시세 로직 전체) ──
function PricesTab() {
  const [data, setData] = useState<PricesResponse | null>(null);
  const [candles, setCandles] = useState<CandleData[]>([]);
  const [interval, setInterval] = useState<string>('1m');
  const [selected, setSelected] = useState<SymbolKind>('regular');
  const [lastFetch, setLastFetch] = useState<Date | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [wsStatus, setWsStatus] = useState<'connecting' | 'open' | 'closed'>('connecting');
  const wsRef = useRef<WebSocket | null>(null);
  const selectedCodeRef = useRef<string | null>(null);

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

  // 2) 분봉 로드
  useEffect(() => {
    if (!data) return;
    const result = selected === 'regular' ? data.regular : data.mini;
    if (!result.ok || !result.data) return;

    const code = result.data.code;
    selectedCodeRef.current = code;

    const fetchCandles = async () => {
      try {
        const cres = await fetch(`/api/candles?code=${code}&interval=${interval}&date=both`);
        if (!cres.ok) return;
        const cjson = await cres.json();
        if (!cjson.ok) return;

        const raw: CandleData[] = cjson.data;
        if (raw.length === 0) { setCandles([]); return; }

        const todayStr = new Date(Date.now() + 9 * 3600 * 1000)
          .toISOString().slice(0, 10).replace(/-/g, '');

        const yesterday = raw.filter((c) => c.date !== todayStr);
        const today = raw.filter((c) => c.date === todayStr);

        const mapped: CandleData[] = [];
        let lastMin = 0;

        for (const c of yesterday) {
          const hh = Number(c.time.slice(0, 2));
          const mm = Number(c.time.slice(2, 4));
          lastMin = hh * 60 + mm;
          mapped.push(c);
        }
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
  }, [selected, interval, data]);

  // 3) WebSocket
  useEffect(() => {
    const url = `ws://${window.location.hostname}:8080/ws`;
    const ws = new WebSocket(url);
    wsRef.current = ws;
    ws.onopen = () => setWsStatus('open');
    ws.onclose = () => setWsStatus('closed');

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

        if (tick.code !== selectedCodeRef.current) return;

        const price = parseFloat(tick.price);
        const qty = parseInt(tick.last_qty, 10);
        if (isNaN(price) || price <= 0) return;

        const hhmmss = tick.time;
        if (hhmmss.length < 4) return;
        const hhmm = hhmmss.slice(0, 4);

        const n = intervalToMinutes(interval);
        const hh = Number(hhmm.slice(0, 2));
        const mm = Number(hhmm.slice(2, 4));
        const groupKey = Math.floor((hh * 60 + mm) / n);

        setCandles((prev) => {
          if (prev.length === 0) return prev;
          const updated = [...prev];
          const last = updated[updated.length - 1];

          const lastHh = Number(last.time.slice(0, 2));
          const lastMm = Number(last.time.slice(2, 4));
          const lastGroupKey = Math.floor((lastHh * 60 + lastMm) / n);

          if (groupKey === lastGroupKey) {
            updated[updated.length - 1] = {
              ...last,
              high: Math.max(last.high, price),
              low: Math.min(last.low, price),
              close: price,
              volume: last.volume + qty,
            };
          } else if (groupKey > lastGroupKey) {
            const newMinute = groupKey * n;
            const newHh = String(Math.floor(newMinute / 60)).padStart(2, '0');
            const newMm = String(newMinute % 60).padStart(2, '0');
            updated.push({
              date: last.date,
              time: `${newHh}${newMm}00`,
              open: price,
              high: price,
              low: price,
              close: price,
              volume: qty,
            });
          }
          return updated;
        });
      } catch {}
    };

    return () => ws.close();
  }, [interval]);

  const chartTitle = (() => {
    const target = selected === 'regular' ? data?.regular : data?.mini;
    if (!target?.ok || !target.data) {
      return selected === 'regular' ? '정규선물' : '미니선물';
    }
    const name = target.data.name;
    const monthMatch = name.match(/20(\d{2})(\d{2})/);
    const label = selected === 'regular' ? '정규선물' : '미니선물';
    if (monthMatch) {
      return `${label}(20${monthMatch[1]}년 ${monthMatch[2]}월)`;
    }
    return label;
  })();

  return (
    <main className="layout">
      <aside className="sidebar">
        <PriceCard
          title="정규선물"
          result={data?.regular ?? null}
          selected={selected === 'regular'}
          onClick={() => setSelected('regular')}
        />
        <PriceCard
          title="미니선물"
          result={data?.mini ?? null}
          selected={selected === 'mini'}
          onClick={() => setSelected('mini')}
        />
      </aside>

      <section className="chart-area">
        <div className="chart-header">
          <h2 className="chart-title">{chartTitle}</h2>
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
  );
}

