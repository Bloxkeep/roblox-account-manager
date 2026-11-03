// Times of day and lengths of time as the app writes them: 24-hour "07:00" in Vietnamese, "7 AM" or "7:30 AM" in English;
// "35 min", "2 h 5 min". One formatter per kind and language, made once: a page of hour cells would otherwise build hundreds.
import type { Host } from './host';

const formatters = new Map<string, Intl.DateTimeFormat>();

/** A moment's time of day; without minutes, English drops them ("7 AM") and Vietnamese keeps "07:00". */
export function clockText(locale: string, lang: string, at: Date | number, minutes = true): string {
  const key = `${locale}|${lang}|${minutes}`;
  let f = formatters.get(key);
  if (!f) {
    f = new Intl.DateTimeFormat(locale, lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }
      : minutes ? { hour: 'numeric', minute: '2-digit' } : { hour: 'numeric' });
    formatters.set(key, f);
  }
  return f.format(at);
}

/** Hour h of any day, as a mark on a scale. */
export const hourText = (locale: string, lang: string, h: number) => clockText(locale, lang, new Date(2000, 0, 1, h), false);

/** "35 min", "2 h 5 min", "2 h", "1,234 h": hours grouped like other numbers, no "0 min". */
export function playedText({ t, fmtNum }: Pick<Host, 't' | 'fmtNum'>, seconds: number): string {
  const m = Math.floor((seconds || 0) / 60), h = Math.floor(m / 60);
  return m < 60 ? t('dur.min', { m }) : m % 60 ? t('dur.hour', { h: fmtNum(h), m: m % 60 }) : t('dur.hours', { h: fmtNum(h) });
}
