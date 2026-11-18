// ============================================================
// 파일명: CandleChart.tsx
// 설명: TradingView Lightweight Charts 캔들 차트
//       - 배경 검정, 양봉 초록, 음봉 빨강
//       - 종목/interval 변경 시 차트 재생성
//       - 컨테이너 크기 변화에 반응 (ResizeObserver)
//       - date(YYYYMMDD) + time(HHMMSS) → UTC timestamp
// 작성일: 2026-11-14
// ============================================================

import { useEffect, useRef } from 'react';
import {
  createChart,
  CandlestickSeries,
  type ISeriesApi,
  type UTCTimestamp,
} from 'lightweight-charts';

interface Candle {
  date: string;   // "YYYYMMDD" (예: "20261113")
  time: string;   // "HHMMSS" (예: "093000")
  open: number;
  high: number;
  low: number;
  close: number;
  volume: number;
}

interface Props {
  data: Candle[];
  interval: string;
  symbolKey: string;
}

function toChartData(data: Candle[]) {
  return data.map((c) => {
    const y = Number(c.date.slice(0, 4));
    const m = Number(c.date.slice(4, 6));
    const d = Number(c.date.slice(6, 8));
    const hh = Number(c.time.slice(0, 2));
    const mm = Number(c.time.slice(2, 4));
    const ts = Math.floor(Date.UTC(y, m - 1, d, hh, mm) / 1000);
    return {
      time: ts as UTCTimestamp,
      open: c.open,
      high: c.high,
      low: c.low,
      close: c.close,
    };
  });
}

function sameCandle(
  left: ReturnType<typeof toChartData>[number],
  right: ReturnType<typeof toChartData>[number],
): boolean {
  return left.time === right.time
    && left.open === right.open
    && left.high === right.high
    && left.low === right.low
    && left.close === right.close;
}

export default function CandleChart({ data, interval, symbolKey }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const chartRef = useRef<ReturnType<typeof createChart> | null>(null);
  const candleSeriesRef = useRef<ISeriesApi<'Candlestick'> | null>(null);
  const chartDataRef = useRef<ReturnType<typeof toChartData> | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const chart = createChart(containerRef.current, {
      width: containerRef.current.clientWidth,
      height: 560,
      layout: {
        background: { color: '#000000' },
        textColor: '#e2e8f0',
      },
      grid: {
        vertLines: { color: '#1e293b' },
        horzLines: { color: '#1e293b' },
      },
      timeScale: {
        timeVisible: true,
        secondsVisible: false,
        shiftVisibleRangeOnNewBar: false,
      },
    });

    const candleSeries = chart.addSeries(CandlestickSeries, {
      upColor: '#66bb6a',       // 양봉(상승) 초록
      downColor: '#b71c1c',     // 음봉(하락) 빨강
      borderVisible: false,
      wickUpColor: '#66bb6a',
      wickDownColor: '#b71c1c',
    });

    chartRef.current = chart;
    candleSeriesRef.current = candleSeries;
    chartDataRef.current = null;

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
      chartRef.current = null;
      candleSeriesRef.current = null;
      chartDataRef.current = null;
    };
  }, [interval, symbolKey]);

  useEffect(() => {
    const chart = chartRef.current;
    const candleSeries = candleSeriesRef.current;
    if (!chart || !candleSeries || data.length === 0) return;

    const nextData = toChartData(data);
    const previousData = chartDataRef.current;
    if (!previousData) {
      candleSeries.setData(nextData);
      chart.timeScale().fitContent();
      chartDataRef.current = nextData;
      return;
    }

    const commonLength = Math.min(previousData.length, nextData.length);
    let unchangedPrefixLength = 0;
    while (
      unchangedPrefixLength < commonLength
      && sameCandle(previousData[unchangedPrefixLength], nextData[unchangedPrefixLength])
    ) {
      unchangedPrefixLength += 1;
    }

    if (
      unchangedPrefixLength === previousData.length
      || (
        previousData.length === nextData.length
        && unchangedPrefixLength === previousData.length - 1
      )
    ) {
      for (let index = unchangedPrefixLength; index < nextData.length; index += 1) {
        candleSeries.update(nextData[index]);
      }
    } else {
      const visibleRange = chart.timeScale().getVisibleLogicalRange();
      candleSeries.setData(nextData);
      if (visibleRange) {
        chart.timeScale().setVisibleLogicalRange(visibleRange);
      }
    }

    chartDataRef.current = nextData;
  }, [data]);

  return <div ref={containerRef} style={{ width: '100%', height: 560 }} />;
}

