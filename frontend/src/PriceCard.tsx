
// ============================================================
// 파일명: PriceCard.tsx
// 설명: 선물 시세 카드 (전일종가/시가/고가/저가/현재가)
// 작성일: 2026-11-14
// ============================================================

import type { PriceResult } from './types';

interface Props {
  title: string;
  result: PriceResult | null;
}

export default function PriceCard({ title, result }: Props) {
  if (!result) {
    return (
      <div className="card">
        <h2>{title}</h2>
        <p className="loading">불러오는 중...</p>
      </div>
    );
  }
  if (!result.ok || !result.data) {
    return (
      <div className="card">
        <h2>{title}</h2>
        <p className="error">오류: {result.error ?? '알 수 없음'}</p>
      </div>
    );
  }

  const d = result.data;
  const changeNum = parseFloat(d.change);
  const colorClass = changeNum > 0 ? 'up' : changeNum < 0 ? 'down' : 'flat';

  return (
    <div className="card">
      <h2>{title}</h2>
      <div className="symbol">{d.name} <span className="code">({d.code})</span></div>

      <div className={`price ${colorClass}`}>{d.price}</div>
      <div className={`change ${colorClass}`}>
        {changeNum > 0 ? '▲' : changeNum < 0 ? '▼' : '-'} {d.change} ({d.change_rate}%)
      </div>

      <div className="grid">
        <div><span className="label">전일종가</span><span className="value">{d.prev_close}</span></div>
        <div><span className="label">시가</span><span className="value">{d.open}</span></div>
        <div><span className="label">고가</span><span className="value high">{d.high}</span></div>
        <div><span className="label">저가</span><span className="value low">{d.low}</span></div>
      </div>

      <div className="volume">거래량: {d.volume}</div>
      <div className="updated">조회: {new Date(d.cached_at * 1000).toLocaleTimeString('ko-KR')}</div>
    </div>
  );
}


