import type { RealtimeTick } from '../../../types';
import type { CandleData } from '../types';

export function intervalToMinutes(interval: string): number {
  return parseInt(interval.replace('m', ''), 10);
}

export function mapCandlesForChart(raw: CandleData[]): CandleData[] {
  if (raw.length === 0) return [];

  const todayStr = new Date(Date.now() + 9 * 3600 * 1000)
    .toISOString()
    .slice(0, 10)
    .replace(/-/g, '');

  const yesterday = raw.filter((c) => c.date !== todayStr);
  const today = raw.filter((c) => c.date === todayStr);

  const mapped: CandleData[] = [];
  let lastMin = 0;

  for (const candle of yesterday) {
    const hh = Number(candle.time.slice(0, 2));
    const mm = Number(candle.time.slice(2, 4));
    lastMin = hh * 60 + mm;
    mapped.push(candle);
  }

  if (today.length > 0) {
    const firstTodayMin =
      Number(today[0].time.slice(0, 2)) * 60 + Number(today[0].time.slice(2, 4));
    const offset = (yesterday.length > 0 ? lastMin + 1 : 0) - firstTodayMin;

    for (const candle of today) {
      const hh = Number(candle.time.slice(0, 2));
      const mm = Number(candle.time.slice(2, 4));
      const virtualMinute = hh * 60 + mm + offset;
      const virtualHour = String(Math.floor(virtualMinute / 60)).padStart(2, '0');
      const virtualMinutePart = String(virtualMinute % 60).padStart(2, '0');
      mapped.push({ ...candle, time: `${virtualHour}${virtualMinutePart}00` });
    }
  }

  return mapped;
}

export function applyTickToCandles(
  candles: CandleData[],
  tick: RealtimeTick,
  interval: string,
): CandleData[] {
  const price = parseFloat(tick.price);
  const quantity = parseInt(tick.last_qty, 10);

  if (Number.isNaN(price) || price <= 0 || tick.time.length < 4 || candles.length === 0) {
    return candles;
  }

  const hhmm = tick.time.slice(0, 4);
  const minutes = intervalToMinutes(interval);
  const hour = Number(hhmm.slice(0, 2));
  const minute = Number(hhmm.slice(2, 4));
  const groupKey = Math.floor((hour * 60 + minute) / minutes);

  const updated = [...candles];
  const last = updated[updated.length - 1];
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
    const newMinute = groupKey * minutes;
    const newHour = String(Math.floor(newMinute / 60)).padStart(2, '0');
    const newMinutePart = String(newMinute % 60).padStart(2, '0');
    updated.push({
      date: last.date,
      time: `${newHour}${newMinutePart}00`,
      open: price,
      high: price,
      low: price,
      close: price,
      volume: quantity,
    });
  }

  return updated;
}
