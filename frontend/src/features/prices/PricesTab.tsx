// ============================================================
// 파일명: PricesTab.tsx
// 설명: 왼쪽 시세 영역과 오른쪽 차트/채팅 패널을 함께 유지하는 메인 화면
// ============================================================

import { useState } from 'react';
import PriceCard from '../../PriceCard';
import CandleChart from '../../CandleChart';
import ChatPage from '../chat/ChatPage';
import { usePrices } from './hooks/usePrices';
import { useMarketWebSocket } from './hooks/useMarketWebSocket';
import { INTERVALS, type SymbolKind } from './types';

interface PricesTabProps {
  activePage: 'prices' | 'chat';
}

export default function PricesTab({ activePage }: PricesTabProps) {
  const [interval, setInterval] = useState<string>('1m');
  const [selected, setSelected] = useState<SymbolKind>('regular');
  const { data, setData, candles, setCandles, selectedCode } = usePrices(
    interval,
    selected,
  );

  // 이 WebSocket은 패널 표시 여부와 무관하게 계속 실행됩니다.
  const isMarketConnected = useMarketWebSocket({
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
    <div className="layout">
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

      {/* hidden은 표시만 제어합니다. 두 패널의 React 생명주기는 유지됩니다. */}
      <section
        id="prices-panel"
        className="chart-area"
        role="tabpanel"
        aria-labelledby="prices-tab"
        hidden={activePage !== 'prices'}
      >
        <div className="chart-header">
          <div className="chart-heading">
            <h2 className="chart-title">{chartTitle}</h2>
            <span
              className={`market-websocket-status ${isMarketConnected ? 'connected' : 'closed'}`}
              role="status"
            >
              {isMarketConnected ? '연결중' : '연결종료'}
            </span>
          </div>
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

      <div
        id="chat-panel"
        role="tabpanel"
        aria-labelledby="chat-tab"
        hidden={activePage !== 'chat'}
      >
        <ChatPage active={activePage === 'chat'} />
      </div>
    </div>
  );
}
