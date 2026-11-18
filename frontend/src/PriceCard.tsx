// ============================================================
// 파일명: PriceCard.tsx
// 설명: 주간/야간 선물 시세 카드 (클릭 시 해당 종목 차트 선택)
// ============================================================

import type { PriceResult, SessionQuote } from './types';
import { candleColors } from './candleColors';

interface Props {
  title: string;
  result: PriceResult | null;
  selected: boolean;
  onClick: () => void;
}

function toNumber(value: string): number {
  return Number.parseFloat(value.replace(/,/g, '').trim());
}

function SignedAmount({ value }: { value: number }) {
  const [integer, fraction] = Math.abs(value).toFixed(2).split('.');
  const sign = value > 0 ? '+' : value < 0 ? '-' : '';
  return (
    <span className="delta-amount">
      <span className="delta-amount-sign">{sign}</span>
      <span className="delta-amount-integer">{integer}</span>
      <span className="delta-amount-decimal">.</span>
      <span className="delta-amount-fraction">{fraction}</span>
    </span>
  );
}

// 기준가 대비 차이를 부호와 함께 계산합니다.
function diffFrom(value: string, base: string) {
  const v = toNumber(value);
  const b = toNumber(base);
  if (!Number.isFinite(v) || !Number.isFinite(b)) return null;
  const diff = v - b;
  return {
    sign: diff > 0 ? '▲' : diff < 0 ? '▼' : '-',
    value: diff,
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

function DiffValue({ value, base, showRate = false, useCandleColors = false }: {
  value: string;
  base: string;
  showRate?: boolean;
  useCandleColors?: boolean;
}) {
  const diff = diffFrom(value, base);
  if (!diff) return null;
  const directionClass = diff.sign === '▲' ? 'up' : diff.sign === '▼' ? 'down' : 'flat';
  const color = useCandleColors
    ? directionClass === 'up'
      ? candleColors.up
      : directionClass === 'down'
        ? candleColors.down
        : undefined
    : undefined;
  const baseNumber = toNumber(base);
  const rate = Number.isFinite(baseNumber) && baseNumber !== 0
    ? (diff.value / baseNumber) * 100
    : Number.NaN;
  const rateText = rate > 0 ? `+${rate.toFixed(2)}` : rate.toFixed(2);
  return (
    <span className={`diff ${directionClass}`} style={{ color }}>
      <span className="delta-direction">{diff.sign}</span>
      <SignedAmount value={diff.value} />
      {showRate && Number.isFinite(rate) && (
        <span className="delta-rate">({rateText}%)</span>
      )}
    </span>
  );
}

function SessionStats({ quote, baseLabel, baseValue }: {
  quote: { open: string; high: string; low: string };
  baseLabel: string;
  baseValue?: string;
}) {
  const rows = [
    { label: baseLabel, value: baseValue ?? '-', isBase: true },
    { label: '시가', value: quote.open, isBase: false },
    { label: '고가', value: quote.high, isBase: false },
    { label: '저가', value: quote.low, isBase: false },
  ];
  return (
    <div className="session-stats">
      {rows.map((row) => {
        const isComparedValue = !row.isBase && !!baseValue;
        return (
          <div className="session-stat-row" key={row.label}>
            <span className="label">{row.label}</span>
            <span className="value">
              {formatPrice(row.value)}
            </span>
            {isComparedValue && (
              <DiffValue value={row.value} base={baseValue!} useCandleColors />
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
        {quote.reference_price && (
          <DiffValue
            value={quote.price}
            base={quote.reference_price}
            showRate
            useCandleColors
          />
        )}
      </div>
      <SessionStats quote={quote} baseLabel="기준가" baseValue={quote.reference_price} />
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
  const changeColor = priceDiff > 0
    ? candleColors.up
    : priceDiff < 0 ? candleColors.down : undefined;
  const changeSign = priceDiff > 0 ? '▲' : priceDiff < 0 ? '▼' : '-';
  const changeRate =
    Number.isFinite(priceDiff) && Number.isFinite(previousClose) && previousClose !== 0
      ? ((priceDiff / previousClose) * 100).toFixed(2)
      : day.change_rate;
  const changeRateText = Number.isFinite(Number(changeRate)) && Number(changeRate) > 0
    ? `+${changeRate}`
    : changeRate;

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
          <span className="session-price">{day.price}</span>
          <span className={`session-change ${colorClass}`} style={{ color: changeColor }}>
            <span className="delta-direction">{changeSign}</span>
            {Number.isFinite(priceDiff) ? <SignedAmount value={priceDiff} /> : day.change}
            <span className="delta-rate">({changeRateText}%)</span>
          </span>
        </div>
        <SessionStats
          quote={{ open: day.open, high: day.high, low: day.low }}
          baseLabel="전일종가"
          baseValue={day.prev_close}
        />
      </section>

      {result.night && <NightSession night={result.night} />}
    </div>
  );
}
