

// ============================================================
// 파일명: PricesTab.tsx
// 설명: 시세 카드, 차트 및 세션별 시초 레인지 돌파 경보
// ============================================================

import { useCallback, useEffect, useMemo, useState } from 'react';
import PriceCard from './PriceCard';
import CandleChart from './CandleChart';
import ChatPage from '../chat/ChatPage';
import { usePrices } from './hooks/usePrices';
import { useMarketWebSocket } from './hooks/useMarketWebSocket';
import { INTERVALS, type SymbolKind, type RangeAlert, type RangeMonitorSnapshot } from './types';

interface PricesTabProps {
  activePage: 'prices' | 'chat';
  backendReady: boolean;
  onMarketConnectionChange: (connected: boolean) => void;
}

interface ChartRangeLine {
  label: string;
  price: number;
  color: string;
}

function rangeStatusLabel(status: string): string {
  switch (status) {
    case 'waiting':
      return '수집 대기';
    case 'collecting':
      return '기준 가격 수집 중';
    case 'monitoring':
      return '돌파 감시 중';
    default:
      return '오늘 감시 미제공';
  }
}

export default function PricesTab({ activePage, backendReady, onMarketConnectionChange }: PricesTabProps) {
  const [interval, setInterval] = useState<string>('1m');
  const [selected, setSelected] = useState<SymbolKind>('regular');
  const [rangeSnapshot, setRangeSnapshot] = useState<RangeMonitorSnapshot | null>(null);
  const [activeAlert, setActiveAlert] = useState<RangeAlert | null>(null);
  const { data, setData, candles, setCandles, selectedCode } = usePrices(
    interval,
    selected,
    backendReady,
  );

  const handleRangeUpdate = useCallback((next: RangeMonitorSnapshot) => {
    setRangeSnapshot((previous) =>
      previous && JSON.stringify(previous) === JSON.stringify(next) ? previous : next,
    );
  }, []);

  const handleRangeAlert = useCallback((alert: RangeAlert) => {
    setActiveAlert(alert);
    if ('speechSynthesis' in window) {
      window.speechSynthesis.cancel();
      const utterance = new SpeechSynthesisUtterance(alert.message);
      utterance.lang = 'ko-KR';
      window.speechSynthesis.speak(utterance);
    }
  }, []);

  // 이 WebSocket은 패널 표시 여부와 무관하게 계속 실행됩니다.
  const isMarketConnected = useMarketWebSocket({
    enabled: backendReady,
    interval,
    selectedCode,
    setData,
    setCandles,
    onRangeUpdate: handleRangeUpdate,
    onRangeAlert: handleRangeAlert,
  });

  useEffect(() => {
    onMarketConnectionChange(isMarketConnected);
  }, [isMarketConnected, onMarketConnectionChange]);

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

  const rangeLines = useMemo<ChartRangeLine[]>(() => {
    if (selected !== 'regular' || !rangeSnapshot) return [];
    const currentDate = candles[candles.length - 1]?.date;
    if (!currentDate) return [];

    const lines: ChartRangeLine[] = [];
    for (const session of [rangeSnapshot.day, rangeSnapshot.night]) {
      if (session.date !== currentDate || session.status === 'unavailable') continue;
      const prefix = session.session === 'day' ? '주간' : '야간';
      if (session.high !== null) {
        lines.push({
          label: `${prefix} 고가`,
          price: session.high,
          color: '#f59e0b',
        });
      }
      if (session.low !== null) {
        lines.push({
          label: `${prefix} 저가`,
          price: session.low,
          color: '#38bdf8',
        });
      }
    }
    return lines;
  }, [selected, rangeSnapshot, candles]);

  const alertTitle = activeAlert
    ? `${activeAlert.session === 'day' ? '주간장' : '야간장'} ${activeAlert.direction === 'high' ? '고가 돌파' : '저가 이탈'}`
    : '';

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

        {selected === 'regular' && (
          <div className="range-session-status" aria-live="polite">
            <span>주간장: {rangeSnapshot ? rangeStatusLabel(rangeSnapshot.day.status) : '상태 확인 중'}</span>
            <span>야간장: {rangeSnapshot ? rangeStatusLabel(rangeSnapshot.night.status) : '상태 확인 중'}</span>
          </div>
        )}

        {candles.length > 0 ? (
          <CandleChart
            data={candles}
            interval={interval}
            symbolKey={selectedCode ?? ''}
            rangeLines={rangeLines}
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
        <ChatPage active={activePage === 'chat'} backendReady={backendReady} />
      </div>

      {activeAlert && (
        <div className="range-alert-backdrop" role="alertdialog" aria-modal="true" aria-labelledby="range-alert-title">
          <div className="range-alert-dialog">
            <h2 id="range-alert-title">{alertTitle}</h2>
            <p>{activeAlert.message}</p>
            <dl>
              <div><dt>기준 가격</dt><dd>{activeAlert.reference_price.toFixed(2)}</dd></div>
              <div><dt>현재 체결가</dt><dd>{activeAlert.price.toFixed(2)}</dd></div>
            </dl>
            <button type="button" onClick={() => setActiveAlert(null)}>확인</button>
          </div>
        </div>
      )}
    </div>
  );
}



