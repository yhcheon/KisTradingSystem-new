// ============================================================
// 파일명: PricesTab.tsx
// 설명: 정규선물/미니선물 시세, 분봉 차트 및 실시간 시세 화면
// ============================================================

import { useState } from 'react';
import PriceCard from '../../PriceCard';
import CandleChart from '../../CandleChart';
import { usePrices } from './hooks/usePrices';
import { useMarketWebSocket } from './hooks/useMarketWebSocket';
import { INTERVALS, type SymbolKind } from './types';

export default function PricesTab() {
  const [interval, setInterval] = useState<string>('1m');
  const [selected, setSelected] = useState<SymbolKind>('regular');
  const { data, setData, candles, setCandles, selectedCode } = usePrices(
    interval,
    selected,
  );

  useMarketWebSocket({
    interval,
    selectedCode,
    setData,
    setCandles,
  });

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
            {INTERVALS.map((item) => (
              <button
                key={item}
                className={interval === item ? 'active' : ''}
                onClick={() => setInterval(item)}
              >
                {item.replace('m', '분')}
              </button>
            ))}
          </div>
        </div>
        {candles.length > 0 ? (
          <CandleChart
            data={candles}
            interval={interval}
            symbolKey={selectedCode ?? ''}
          />
        ) : (
          <p style={{ color: '#94a3b8' }}>분봉 데이터를 불러오는 중...</p>
        )}
      </section>
    </main>
  );
}
