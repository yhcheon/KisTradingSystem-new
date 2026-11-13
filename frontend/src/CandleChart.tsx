
// ============================================================
// 파일명: CandleChart.tsx
// 설명: TradingView Lightweight Charts 캔들 차트
//       - 컨테이너 크기 변화에 반응 (ResizeObserver)
//       - KST 입력을 UTC로 변환해 축 표시
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef } from 'react';
import { createChart, CandlestickSeries } from 'lightweight-charts';

interface Candle {
  time: string;
  hour: string;
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

interface Props {
  data: Candle[];
}

export default function CandleChart({ data }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 560,
      layout: {
        background: { color: '#0f172a' },
        textColor: '#e2e8f0',
      },
      grid: {
        vertLines: { color: '#1e293b' },
        horzLines: { color: '#1e293b' },
      },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
      },
    });

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#ef4444',
      downColor: '#3b82f6',
      borderVisible: false,
      wickUpColor: '#ef4444',
      wickDownColor: '#3b82f6',
    });

    const chartData = data.map((c) => {
      const [y, m, d] = c.time.split('-').map(Number);
      const hh = Number(c.hour.slice(0, 2));
      const mm = Number(c.hour.slice(2, 4));
      const ts = Math.floor(Date.UTC(y, m - 1, d, hh, mm) / 1000);
      return {
        time: ts as any,
        open: c.open,
        high: c.high,
        low: c.low,
        close: c.close,
      };
    });

    candleSeries.setData(chartData);
    chart.timeScale().fitContent();

    // 컨테이너 크기 변화 감지 → 차트 너비 자동 갱신
    const resizeObserver = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width } = entry.contentRect;
        if (width > 0) {
          chart.applyOptions({ width });
        }
      }
    });
    resizeObserver.observe(containerRef.current);

    return () => {
      resizeObserver.disconnect();
      chart.remove();
    };
  }, [data]);

  return <div ref={containerRef} style={{ width: '100%', height: 560 }} />;
}

