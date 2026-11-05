// A game's price list in the Game news tab: read once a day by Bloxkeep's bot from a values website
// (services/DiscordBot/worker/prices.js) and sent with the games. Searched here (accents and case aside), by kind, the
// most valuable first; each item's page and the website itself open in the browser, and the website is named under the
// list, since the prices are theirs.
import { useMemo, useState } from 'react';
import { timeText } from './DiscordBlocks';
import { Icon } from './host';

export type PriceItem = {
  id: string; name: string; category?: string | null; rarity?: string | null; type?: string | null; img?: string | null;
  value?: number | null; perm?: number | null; demand?: number | null; permDemand?: number | null; trend?: string | null; permTrend?: string | null;
  beli?: number | null; robux?: number | null; bestFor?: string | null; updated?: number | null; url?: string | null;
};
export type Prices = { source: { name: string; url: string }; at: number; items: PriceItem[] };
type T = (k: string, v?: Record<string, string | number>) => string;

const TRENDS = ['Stable', 'Underpaid', 'Overpaid', 'Fluctuating', 'Unstable'];
/** Lower case, without accents (đ as d). */
const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd');
/** "2.7B", "450M": how players say it, whatever the app's language. */
const short = new Intl.NumberFormat('en', { notation: 'compact', maximumFractionDigits: 2 });

export function PriceList({ list, t, locale, now, open }: { list: Prices; t: T; locale: string; now: number; open: (url: string) => void }) {
  const [query, setQuery] = useState('');
  const [kind, setKind] = useState('');
  const kinds = useMemo(() => [...new Set(list.items.map(i => i.category || '').filter(Boolean))], [list]);
  const shown = useMemo(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    return list.items
      .filter(i => (!kind || i.category === kind) && words.every(w => fold(`${i.name} ${i.rarity ?? ''} ${i.type ?? ''}`).includes(w)))
      .sort((a, b) => (b.value ?? b.perm ?? 0) - (a.value ?? a.perm ?? 0));
  }, [list, query, kind]);
  const full = (n: number) => n.toLocaleString(locale);
  const trend = (s?: string | null) => s ? <span className={`pl-trend pl-${fold(s).replace(/[^a-z]/g, '')}`}>{TRENDS.includes(s) ? t(`prices.trend.${s}`) : s}</span> : null;
  const [before, after] = t('prices.from', { t: timeText(list.at, 'R', locale, now) }).split('{site}');

  return <section className="pl" aria-label={t('prices.tab')}>
    <div className="pl-tools">
      <label className="pl-search"><Icon name="search" />
        <input className="field sm" type="search" value={query} placeholder={t('prices.search')} aria-label={t('prices.search')} maxLength={60}
          autoComplete="off" spellCheck={false} onChange={e => setQuery(e.target.value)} /></label>
      {kinds.length > 1 ? <div className="chips pl-kinds" role="group" aria-label={t('prices.kinds')}>
        <button type="button" className={`chip ${kind ? '' : 'on'}`} aria-pressed={!kind} onClick={() => setKind('')}>{t('news.all')}</button>
        {kinds.map(k => <button key={k} type="button" className={`chip ${kind === k ? 'on' : ''}`} aria-pressed={kind === k} onClick={() => setKind(k)}>{k}</button>)}
      </div> : null}
    </div>
    {shown.length ? <ul className="pl-list">
      {shown.map(i => {
        const sub = [i.rarity, i.type, i.beli ? `${short.format(i.beli)} Beli` : null, i.robux ? `${full(i.robux)} Robux` : null].filter(Boolean).join(' · ');
        return <li key={i.id} className="pl-row">
          {i.img ? <img className="pl-img" src={i.img} alt="" loading="lazy" /> : <span className="pl-img pl-mark">{i.name.slice(0, 1)}</span>}
          <div className="pl-name">
            {i.url ? <button type="button" className="pl-link" title={i.url} onClick={() => open(i.url!)}>{i.name}</button> : <b>{i.name}</b>}
            {sub ? <small>{sub}</small> : null}
          </div>
          <dl className="pl-stats">
            <div><dt>{t('prices.value')}</dt><dd title={i.value ? full(i.value) : undefined}>{i.value ? short.format(i.value) : '–'}</dd>{trend(i.trend)}</div>
            {i.perm ? <div><dt>{t('prices.perm')}</dt><dd title={full(i.perm)}>{short.format(i.perm)}</dd>{trend(i.permTrend)}</div> : <div className="pl-none" />}
            {i.demand ? <div><dt>{t('prices.demand')}</dt><dd>{i.demand}<span>/10</span></dd>
              <i className="pl-meter" aria-hidden="true"><i style={{ width: `${Math.min(10, i.demand) * 10}%` }} /></i></div> : null}
          </dl>
        </li>;
      })}
    </ul> : <div className="empty news-empty"><h2>{t('prices.none', { q: query.trim() })}</h2></div>}
    <p className="pl-credit">{before}<button type="button" className="pl-link" onClick={() => open(list.source.url)}>{list.source.name}</button>{after}</p>
  </section>;
}
