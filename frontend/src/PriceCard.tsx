// ============================================================
// 파일명: PriceCard.tsx
// 설명: 선물 시세 카드 (클릭 시 해당 종목 차트 선택)
//       - 제목 + 월물 표시, 전일종가/고가/시가/저가 + 전일 대비
// ============================================================

import type { PriceResult } from './types';

interface Props {
  title: string;
  result: PriceResult | null;
  selected: boolean;
  onClick: () => void;
}

// API 숫자 문자열의 천 단위 쉼표 등을 제거해 숫자로 변환합니다.
function toNumber(value: string): number {
  return Number.parseFloat(value.replace(/,/g, '').trim());
}

// 숫자 + 기준가 → { 부호, 차이(6자리 고정, 앞자리 0은 공백) } 반환
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

// 종목명에서 월물 추출 (예: "미니F 202612" → "2026년 12월")
function extractMonth(name: string): string {
  const m = name.match(/20(\d{2})(\d{2})/);
  if (m) return `20${m[1]}년 ${m[2]}월`;
  return '';
}

export default function PriceCard({ title, result, selected, onClick }: Props) {
  const baseClass = `card clickable ${selected ? 'selected' : ''}`;

  if (!result) {
    return (
      <div className={baseClass} onClick={onClick}>
        <div className="card-title-row">
          <h2>{title}</h2>
        </div>
        <p className="loading">불러오는 중...</p>
      </div>
    );
  }

  if (!result.ok || !result.data) {
    return (
      <div className={baseClass} onClick={onClick}>
        <div className="card-title-row">
          <h2>{title}</h2>
        </div>
        <p className="error">오류: {result.error ?? '알 수 없음'}</p>
      </div>
    );
  }

  const d = result.data;
  const currentPrice = toNumber(d.price);
  const previousClose = toNumber(d.prev_close);

  // 등락 방향은 KIS change 필드의 부호가 아니라 현재가와 전일종가를 직접 비교합니다.
  // 가격 데이터가 숫자로 변환되지 않는 경우에만 API change 값을 보조로 사용합니다.
  const priceDiff =
    Number.isFinite(currentPrice) && Number.isFinite(previousClose)
      ? currentPrice - previousClose
      : toNumber(d.change);

  const colorClass = priceDiff > 0 ? 'up' : priceDiff < 0 ? 'down' : 'flat';
  const changeSign = priceDiff > 0 ? '▲' : priceDiff < 0 ? '▼' : '-';
  const changeText = Number.isFinite(priceDiff) ? priceDiff.toFixed(2) : d.change;

  // 등락률도 동일한 가격 차이를 기준으로 계산해 방향과 색상이 어긋나지 않게 합니다.
  const changeRate =
    Number.isFinite(priceDiff) && Number.isFinite(previousClose) && previousClose !== 0
      ? ((priceDiff / previousClose) * 100).toFixed(2)
      : d.change_rate;

  const highDiff = diffFrom(d.high, d.prev_close);
  const openDiff = diffFrom(d.open, d.prev_close);
  const lowDiff = diffFrom(d.low, d.prev_close);
  const openNum = toNumber(d.open);

  return (
    <div className={baseClass} onClick={onClick}>
      {/* 제목 + 월물 */}
      <div className="card-title-row">
        <h2>{title}</h2>
        <span className="month">{extractMonth(d.name)}</span>
      </div>

      <div className="price-row">
        <div className={`price ${colorClass}`}>{d.price}</div>
        <div className={`change ${colorClass}`}>
          {changeSign} {changeText} ({changeRate}%)
        </div>
      </div>

      <div className="stats-list">
        <div className="stat-row">
          <span className="label">전일종가</span>
          <span className="value">{d.prev_close}</span>
        </div>

        <div className="stat-row">
          <span className="label">고가</span>
          <span className="value high">{d.high}</span>
          {highDiff && (
            <span className={`diff ${highDiff.sign === '▲' ? 'up' : highDiff.sign === '▼' ? 'down' : 'flat'}`}>
              {highDiff.sign} {highDiff.abs}
            </span>
          )}
        </div>

        <div className="stat-row">
          <span className="label">시가</span>
          <span className={`value ${openNum > previousClose ? 'up' : openNum < previousClose ? 'down' : ''}`}>
            {d.open}
          </span>
          {openDiff && (
            <span className={`diff ${openDiff.sign === '▲' ? 'up' : openDiff.sign === '▼' ? 'down' : 'flat'}`}>
              {openDiff.sign} {openDiff.abs}
            </span>
          )}
        </div>

        <div className="stat-row">
          <span className="label">저가</span>
          <span className="value low">{d.low}</span>
          {lowDiff && (
            <span className={`diff ${lowDiff.sign === '▲' ? 'up' : lowDiff.sign === '▼' ? 'down' : 'flat'}`}>
              {lowDiff.sign} {lowDiff.abs}
            </span>
          )}
        </div>
      </div>
    </div>
  );
}
