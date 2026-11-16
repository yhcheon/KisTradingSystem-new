// ============================================================
// 파일명: PriceCard.tsx
// 설명: 주간/야간 선물 시세 카드 (클릭 시 해당 종목 차트 선택)
// ============================================================

import type { PriceResult, SessionQuote } from './types';

interface Props {
  title: string;
  result: PriceResult | null;
  selected: boolean;
  onClick: () => void;
}

function toNumber(value: string): number {
  return Number.parseFloat(value.replace(/,/g, '').trim());
}

// 가격을 전일종가와 비교해 방향 및 차이 포인트를 계산합니다.
function diffFrom(value: string, base: string) {
  const v = toNumber(value);
  const b = toNumber(base);
  if (!Number.isFinite(v) || !Number.isFinite(b)) return null;

  const diff = v - b;
  const abs = Math.abs(diff);
  const formatted = abs.toFixed(2).padStart(6, '0').replace(/^0+/, (m) => ' '.repeat(m.length));
  return {
    sign: diff > 0 ? '▲' : diff < 0 ? '▼' : '-',
    abs: formatted,
  };
}

function extractMonth(name: string): string {
  const m = name.match(/20(\d{2})(\d{2})/);
  if (m) return `20${m[1]}년 ${m[2]}월`;
  return '';
}

function formatPrice(value?: string): string {
  return value && value.trim() ? value : '-';
}

function DiffValue({ value, base }: { value: string; base: string }) {
  const diff = diffFrom(value, base);
  if (!diff) return null;

  const directionClass =
    diff.sign === '▲' ? 'up' : diff.sign === '▼' ? 'down' : 'flat';

  return (
    <span className={`diff ${directionClass}`}>
      {diff.sign} {diff.abs}
    </span>
  );
}

function SessionStats({ quote, includePreviousClose }: {
  quote: {
    open: string;
    high: string;
    low: string;
    prev_close?: string;
  };
  includePreviousClose: boolean;
}) {
  const base = quote.prev_close;
  const rows = [
    ...(includePreviousClose ? [{ label: '전일종가', value: quote.prev_close ?? '-' }] : []),
    { label: '시가', value: quote.open },
    { label: '고가', value: quote.high },
    { label: '저가', value: quote.low },
  ];

  return (
    <div className="session-stats">
      {rows.map((row) => {
        const isComparedValue = row.label !== '전일종가' && includePreviousClose && !!base;
        const openValue = row.label === '시가' ? toNumber(row.value) : Number.NaN;
        const previousClose = base ? toNumber(base) : Number.NaN;
        const openColor =
          row.label === '시가' && Number.isFinite(openValue) && Number.isFinite(previousClose)
            ? openValue > previousClose ? 'up' : openValue < previousClose ? 'down' : ''
            : '';

        return (
          <div className="session-stat-row" key={row.label}>
            <span className="label">{row.label}</span>
            <span className={`value ${row.label === '고가' ? 'high' : row.label === '저가' ? 'low' : openColor}`}>
              {formatPrice(row.value)}
            </span>
            {isComparedValue && (
              <DiffValue value={row.value} base={base!} />
            )}
          </div>
        );
      })}
    </div>
  );
}

function NightSession({ night }: { night: NonNullable<PriceResult['night']> }) {
  if (!night.ok || !night.data) {
    return (
      <section className="session-section night-session">
        <div className="session-heading">
          <span>야간</span>
          <span className="session-status">시세 확인 대기</span>
        </div>
        <p className="session-message">{night.error ?? '야간 시세를 불러오지 못했습니다.'}</p>
      </section>
    );
  }

  const quote: SessionQuote = night.data;
  return (
    <section className="session-section night-session">
      <div className="session-heading">
        <span>야간</span>
        <span className="session-status">실시간</span>
      </div>
      <div className="session-price-row">
        <span className="session-price">{formatPrice(quote.price)}</span>
      </div>
      <SessionStats quote={quote} includePreviousClose={false} />
    </section>
  );
}

export default function PriceCard({ title, result, selected, onClick }: Props) {
  const baseClass = `card clickable ${selected ? 'selected' : ''}`;

  if (!result) {
    return (
      <div className={baseClass} onClick={onClick}>
        <div className="card-title-row"><h2>{title}</h2></div>
        <p className="loading">불러오는 중...</p>
      </div>
    );
  }

  if (!result.ok || !result.data) {
    return (
      <div className={baseClass} onClick={onClick}>
        <div className="card-title-row"><h2>{title}</h2></div>
        <p className="error">오류: {result.error ?? '알 수 없음'}</p>
      </div>
    );
  }

  const day = result.data;
  const currentPrice = toNumber(day.price);
  const previousClose = toNumber(day.prev_close);
  const priceDiff =
    Number.isFinite(currentPrice) && Number.isFinite(previousClose)
      ? currentPrice - previousClose
      : toNumber(day.change);
  const colorClass = priceDiff > 0 ? 'up' : priceDiff < 0 ? 'down' : 'flat';
  const changeSign = priceDiff > 0 ? '▲' : priceDiff < 0 ? '▼' : '-';
  const changeText = Number.isFinite(priceDiff) ? priceDiff.toFixed(2) : day.change;
  const changeRate =
    Number.isFinite(priceDiff) && Number.isFinite(previousClose) && previousClose !== 0
      ? ((priceDiff / previousClose) * 100).toFixed(2)
      : day.change_rate;

  return (
    <div className={baseClass} onClick={onClick}>
      <div className="card-title-row">
        <h2>{title}</h2>
        <span className="month">{extractMonth(day.name)}</span>
      </div>

      <section className="session-section day-session">
        <div className="session-heading">
          <span>주간</span>
          <span className="session-status">정규장</span>
        </div>
        <div className="session-price-row">
          <span className={`session-price ${colorClass}`}>{day.price}</span>
          <span className={`session-change ${colorClass}`}>
            {changeSign} {changeText} ({changeRate}%)
          </span>
        </div>
        <SessionStats
          quote={{
            prev_close: day.prev_close,
            open: day.open,
            high: day.high,
            low: day.low,
          }}
          includePreviousClose
        />
      </section>

      {result.night && <NightSession night={result.night} />}
    </div>
  );
}
