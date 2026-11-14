// ============================================================
// 파일명: CandleChart.tsx
// 설명: TradingView Lightweight Charts 캔들 차트
//       - 배경 검정, 양봉 초록, 음봉 빨강
//       - interval 변경 시 차트 재생성
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
  interval: string;
}

export default function CandleChart({ data, interval }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 560,
      layout: {
        background: { color: '#000000' },   // ← 배경 검정
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
      upColor: '#66bb6a',       // ← 양봉(상승) 초록
      downColor: '#b71c1c',     // ← 음봉(하락) 빨강
      borderVisible: false,
      wickUpColor: '#66bb6a',   // ← 양봉 꼬리 초록
      wickDownColor: '#b71c1c', // ← 음봉 꼬리 빨강
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
  }, [data, interval]);

  return <div ref={containerRef} style={{ width: '100%', height: 560 }} />;
}

