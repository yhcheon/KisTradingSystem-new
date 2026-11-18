import type { RealtimeTick } from '../../../types';
import type { CandleData } from '../types';

export function intervalToMinutes(interval: string): number {
  return parseInt(interval.replace('m', ''), 10);
}

export function mapCandlesForChart(raw: CandleData[]): CandleData[] {
  // CM 야간 분봉은 자정 이후 시간을 24시 이상으로 표현합니다.
  // 영업일 날짜 + 확장 시각을 그대로 사용해야 주간장 뒤에 연속해서 정렬됩니다.
  return [...raw].sort((a, b) =>
    a.date === b.date
      ? a.time.localeCompare(b.time)
      : a.date.localeCompare(b.date),
  );
}

export function applyTickToCandles(
  candles: CandleData[],
  tick: RealtimeTick,
  interval: string,
): CandleData[] {
  const price = parseFloat(tick.price);
  const quantity = parseInt(tick.last_qty, 10);

  if (
    Number.isNaN(price) ||
    price <= 0 ||
    tick.time.length < 4 ||
    !/^\d{8}$/.test(tick.date)
  ) {
    return candles;
  }

  const hhmm = tick.time.slice(0, 4);
  const minutes = intervalToMinutes(interval);
  const hour = Number(hhmm.slice(0, 2));
  const minute = Number(hhmm.slice(2, 4));
  const groupKey = Math.floor((hour * 60 + minute) / minutes);
  // CandleStore도 각 interval 그룹의 첫 체결 분을 봉 시각으로 사용합니다.
  const candleTime = `${hhmm}00`;

  const updated = [...candles];
  if (updated.length === 0) {
    return [{
      date: tick.date,
      time: candleTime,
      open: price,
      high: price,
      low: price,
      close: price,
      volume: quantity,
    }];
  }

  const last = updated[updated.length - 1];
  if (last.date !== tick.date) {
    updated.push({
      date: tick.date,
      time: candleTime,
      open: price,
      high: price,
      low: price,
      close: price,
      volume: quantity,
    });
    return updated;
  }

  const lastHour = Number(last.time.slice(0, 2));
  const lastMinute = Number(last.time.slice(2, 4));
  const lastGroupKey = Math.floor((lastHour * 60 + lastMinute) / minutes);

  if (groupKey === lastGroupKey) {
    updated[updated.length - 1] = {
      ...last,
      high: Math.max(last.high, price),
      low: Math.min(last.low, price),
      close: price,
      volume: last.volume + quantity,
    };
  } else if (groupKey > lastGroupKey) {
    updated.push({
      date: tick.date,
      time: candleTime,
      open: price,
      high: price,
      low: price,
      close: price,
      volume: quantity,
    });
  }

  return updated;
}
