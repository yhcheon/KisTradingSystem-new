// ============================================================
// 파일명: PriceCard.tsx
// 설명: 선물 시세 카드 (클릭 시 해당 종목 차트 선택)
//       - 제목 + 월물 표시, 전일종가/고가/시가/저가 + 전일 대비
// 작성일: 2026-11-14
// ============================================================

import type { PriceResult } from './types';

interface Props {
  title: string;
  result: PriceResult | null;
  selected: boolean;
  onClick: () => void;
}

// 숫자 + 기준가 → { 부호, 차이(6자리 고정, 앞자리 0은 공백) } 반환
function diffFrom(value: string, base: string) {
  const v = parseFloat(value);
  const b = parseFloat(base);
  if (isNaN(v) || isNaN(b)) return null;
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
  const changeNum = parseFloat(d.change);
  const colorClass = changeNum > 0 ? 'up' : changeNum < 0 ? 'down' : 'flat';

  const highDiff = diffFrom(d.high, d.prev_close);
  const openDiff = diffFrom(d.open, d.prev_close);
  const lowDiff = diffFrom(d.low, d.prev_close);

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
          {changeNum > 0 ? '▲' : changeNum < 0 ? '▼' : '-'} {d.change} ({d.change_rate}%)
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
          <span
            className={`value ${
              parseFloat(d.open) > parseFloat(d.prev_close) ? 'up'
                : parseFloat(d.open) < parseFloat(d.prev_close) ? 'down' : ''
            }`}
          > {d.open}
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

