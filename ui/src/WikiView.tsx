// The tables the Wiki part draws from a game's wiki (WikiSite.tsx) and its price list (Blox Fruits', read by Bloxkeep's
// bot). What the wiki can't do: every line of an item's box of facts is a column to filter and sort by ("weapons needing
// 40 or less, most damage first"), two to four items side by side with the better number of each line marked, and the
// items most like one (and by how much each line differs).
import { useEffect, useMemo, useState } from 'react';
import { Icon, useHost, type Host } from './host';
import { PriceList, type Prices } from './PriceList';

import type { WikiItem as Item } from './wikiText';
/** A kind's items as a table: what it's called, the wiki it's from. */
export type ItemList = { name: string; source: { name: string; url: string }; items: Item[] };
type PriceGame = Prices & { id: string; name: string; kind: 'prices' };
export type Word = (f: string) => string;
export type WikiInfo = { id: string; name: string; kind: 'wiki' | 'prices' | 'game'; count: number; at: number | null; place: number; wiki?: string | null };
type T = (k: string, v?: Record<string, string | number>) => string;
/** A line's filter: some of its values, or a number range. */
type Filter = { field: string; values?: string[]; min?: number | null; max?: number | null };

const COMPARE_MAX = 4, COLUMNS = 5, SIMILAR = 6;
/** Kept between visits to the tab: the games, and each game asked for once. */
let gamesNow: WikiInfo[] | null = null;
const loaded = new Map<string, PriceGame>();

/** Lower case, without accents (đ as d). */
export const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd');
/** The number a value starts with ("35 HVY / 75 HVY" 35, "0.85x" 0.85, "+12" 12, "1,200" 1200), or null. */
export const numberOf = (v: string | undefined) => {
  const m = v ? /^[+]?(-?\d[\d,]*(?:\.\d+)?)/.exec(v.trim()) : null;
  return m ? Number(m[1].replace(/,/g, '')) : null;
};
/** Lines where less is better (an attack's duration, a cost, what it asks for). */
export const lessIsBetter = (field: string) => /duration|endlag|startup|cost|cooldown|requirement|weight|delay|cast time|mana|ether|price|level/i.test(field);
export const valueOf = (i: Item, f: string) => i.fields.find(([k]) => k === f)?.[1];
/** A line's words in Vietnamese, for the lines most wikis have (the rest stay as the wiki says them). */
export const VI: Record<string, string> = {
  Damage: 'Sát thương', Rarity: 'Độ hiếm', Type: 'Loại', Obtainment: 'Cách lấy', 'Obtained from': 'Lấy từ', Requirements: 'Yêu cầu', 'Selling price': 'Giá bán',
  Sell: 'Giá bán', Range: 'Tầm đánh', 'Swing speed': 'Tốc độ vung', 'Posture damage': 'Sát thương posture', Location: 'Vị trí', Drops: 'Rơi ra', Defense: 'Phòng thủ',
  Gold: 'Vàng', Element: 'Nguyên tố', Cost: 'Giá', Cooldown: 'Hồi chiêu', 'Innate stats': 'Chỉ số gốc', 'Attack duration': 'Thời gian đánh', Penetration: 'Xuyên giáp',
  'Chip damage': 'Sát thương xuyên đỡ', 'Mana cost': 'Tốn mana', Class: 'Lớp', Subtype: 'Loại phụ', Droppable: 'Rơi khi chết', Bankable: 'Gửi kho được',
  Health: 'Máu', Weight: 'Trọng lượng', 'Damage type': 'Kiểu sát thương', 'Range type': 'Kiểu tầm', Endlag: 'Độ trễ sau đòn', Effect: 'Hiệu ứng', Talent: 'Talent',
  'Innate talent': 'Talent sẵn có', 'Special effect': 'Hiệu ứng đặc biệt', Species: 'Loài', Affiliation: 'Phe', 'Damage taken': 'Sát thương nhận', Scaling: 'Scaling',
  'Ether cost': 'Tốn ether', 'Mantra type': 'Loại mantra', Faction: 'Phe', Usage: 'Cách dùng', Rewards: 'Phần thưởng', 'Quest giver': 'Người giao nhiệm vụ',
  Value: 'Giá trị', Stack: 'Chồng tối đa', Knockback: 'Đẩy lùi', 'Crit chance': 'Tỉ lệ chí mạng', Velocity: 'Tốc độ bay', 'Heavy cooldown': 'Hồi đòn nặng',
};

/** The same list asked for from elsewhere (Game info's Wiki button), kept like the tab's; null when the bot can't be asked. */
export async function wikiGamesNow(call: Host['call']) {
  if (!gamesNow) {
    const d = await (call('wikiGames', {}, { silent: true }) as Promise<{ games: WikiInfo[] } | undefined>).catch(() => undefined);
    if (d) gamesNow = d.games;
  }
  return gamesNow;
}

/** The games with a wiki or a price list, asked for once the tab is first shown. */
export function useWikiGames(shown: boolean) {
  const { call } = useHost();
  const [games, setGames] = useState<WikiInfo[] | null>(gamesNow);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!shown) return;
    // Asked already from elsewhere (Game info's Wiki button) since this was drawn.
    if (gamesNow) { if (games !== gamesNow) setGames(gamesNow); return; }
    (call('wikiGames', {}, { silent: true }) as Promise<{ games: WikiInfo[] } | undefined>)
      .then(d => { if (d) { gamesNow = d.games; setGames(d.games); } else setFailed(true); }, () => setFailed(true));
  }, [shown]);
  return { games, failed };
}

/** A game's price list (the bot's), asked for the first time it is shown. */
export function PricePanel({ info, shown }: { info: WikiInfo; shown: boolean }) {
  const { t, call, locale } = useHost();
  const [game, setGame] = useState<PriceGame | null>(loaded.get(info.id) || null);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    if (!shown || game) return;
    (call('wikiGame', { id: info.id }, { silent: true }) as Promise<PriceGame | null | undefined>)
      .then(d => { if (d && d.kind === 'prices') { loaded.set(info.id, d); setGame(d); } else setFailed(true); }, () => setFailed(true));
  }, [shown, info.id]);
  const open = (url: string) => { call('wikiOpen', { url }); };
  // Not drawn while its part is hidden: an empty page drops the heading's tile (app.css).
  if (!game) return shown ? <div className="empty news-empty"><h2>{failed ? t('wiki.offline') : t('news.connecting')}</h2></div> : null;
  return <PriceList list={game} t={t} locale={locale} now={Date.now()} open={open} />;
}

/** The words for a line in the app's language. */
export function useWord(): Word {
  const { lang } = useHost();
  return (f: string) => (lang === 'vi' && VI[f]) || f;
}

/** A kind's items as a wikitable: found by words, filtered by lines, sorted by a column, picked to compare; a name opens
 * its page (onOpen). More may still be coming in (loading). */
export function ItemTable({ game, loading, onOpen }: { game: ItemList; loading: string | null; onOpen: (i: Item) => void }) {
  const { t, locale } = useHost();
  const word = useWord();
  const [query, setQuery] = useState('');
  const [filters, setFilters] = useState<Filter[]>([]);
  const [filtering, setFiltering] = useState(false);
  const [sort, setSort] = useState<{ field: string; down: boolean } | null>(null);
  const [picked, setPicked] = useState<string[]>([]);
  const [comparing, setComparing] = useState(false);
  const byId = useMemo(() => new Map(game.items.map(i => [i.id, i])), [game]);
  const inCat = game.items;

  // The category's lines, the most common first; a line is a number when most of its values start with one.
  const lines = useMemo(() => {
    const count = new Map<string, number>(), nums = new Map<string, number>(), values = new Map<string, Set<string>>();
    for (const i of inCat)
      for (const [k, v] of i.fields) {
        count.set(k, (count.get(k) || 0) + 1);
        if (numberOf(v) !== null) nums.set(k, (nums.get(k) || 0) + 1);
        if (!values.has(k)) values.set(k, new Set());
        if (!v.includes('\n') && v.length <= 40) values.get(k)!.add(v);
      }
    return [...count.entries()].filter(([, n]) => n >= Math.max(2, inCat.length * 0.15)).sort((a, b) => b[1] - a[1])
      .map(([field, n]) => ({ field, n, number: (nums.get(field) || 0) >= n * 0.7, values: [...(values.get(field) || [])].sort() }));
  }, [inCat]);
  const columns = lines.filter(l => l.number || l.values.length <= 30).slice(0, COLUMNS);
  // What can be filtered: a number's range, or a line with a few values (rarity, type, element…).
  const facets = lines.filter(l => l.number || (l.values.length >= 2 && l.values.length <= 24)).slice(0, 8);

  const shown = useMemo(() => {
    const words = fold(query).split(/\s+/).filter(Boolean);
    const list = inCat.filter(i => {
      if (words.length && !words.every(w => fold(`${i.name} ${i.fields.map(f => f[1]).join(' ')}`).includes(w))) return false;
      return filters.every(f => {
        const v = valueOf(i, f.field);
        if (f.values?.length) return v !== undefined && f.values.includes(v);
        const n = numberOf(v);
        if (f.min == null && f.max == null) return true;
        return n !== null && (f.min == null || n >= f.min) && (f.max == null || n <= f.max);
      });
    });
    if (sort) {
      const line = lines.find(l => l.field === sort.field);
      list.sort((a, b) => {
        const va = valueOf(a, sort.field), vb = valueOf(b, sort.field);
        if (va === undefined || vb === undefined) return va === undefined ? (vb === undefined ? 0 : 1) : -1;
        const d = line?.number ? (numberOf(va) ?? -Infinity) - (numberOf(vb) ?? -Infinity) : va.localeCompare(vb);
        return sort.down ? -d : d;
      });
    }
    return list;
  }, [inCat, query, filters, sort, lines]);

  const setFilter = (f: Filter) => setFilters(all => {
    const rest = all.filter(x => x.field !== f.field);
    const empty = !f.values?.length && f.min == null && f.max == null;
    return empty ? rest : [...rest, f];
  });
  const togglePick = (id: string) => setPicked(p => p.includes(id) ? p.filter(x => x !== id) : p.length >= COMPARE_MAX ? p : [...p, id]);
  const pickedItems = picked.map(id => byId.get(id)).filter(Boolean) as Item[];

  return <div className="wk">
    <div className="ws-body">
      <h1 className="ws-h1">{game.name}</h1>
      <p className="ws-tagline">{t('wiki.pages', { n: shown.length.toLocaleString(locale) })}{loading ? ` · ${loading}` : ''}
        {' · '}<label className="ws-find"><Icon name="search" /><input type="search" value={query} placeholder={t('wiki.search', { n: inCat.length })}
          aria-label={t('wiki.search', { n: inCat.length })} maxLength={60} autoComplete="off" spellCheck={false} onChange={e => setQuery(e.target.value)} /></label>
        {facets.length ? <> · <button type="button" className={`ws-link ${filtering || filters.length ? 'on' : ''}`} id="wk-filter" aria-expanded={filtering}
          onClick={() => setFiltering(f => !f)}>{t('wiki.filter')}{filters.length ? ` (${filters.length})` : ''}</button></> : null}
        {filters.length ? <> · <button type="button" className="ws-link" onClick={() => setFilters([])}>{t('wiki.clear')}</button></> : null}</p>
      {filtering ? <table className="ws-table ws-filters"><tbody>
        {facets.map(l => {
          const f = filters.find(x => x.field === l.field);
          return <tr key={l.field}><th scope="row">{word(l.field)}</th><td>
            {l.number ? <span className="wk-range">
              <input className="ws-input" type="text" inputMode="decimal" placeholder={t('wiki.min')} aria-label={`${word(l.field)} ${t('wiki.min')}`} value={f?.min ?? ''}
                onChange={e => setFilter({ field: l.field, min: numberOf(e.target.value), max: f?.max ?? null })} />
              <span aria-hidden="true">–</span>
              <input className="ws-input" type="text" inputMode="decimal" placeholder={t('wiki.max')} aria-label={`${word(l.field)} ${t('wiki.max')}`} value={f?.max ?? ''}
                onChange={e => setFilter({ field: l.field, min: f?.min ?? null, max: numberOf(e.target.value) })} />
            </span> : <span className="ws-values">{l.values.map(v => {
              const on = !!f?.values?.includes(v);
              return <button key={v} type="button" className={on ? 'on' : ''} aria-pressed={on}
                onClick={() => setFilter({ field: l.field, values: on ? f!.values!.filter(x => x !== v) : [...(f?.values || []), v] })}>{v}</button>;
            })}</span>}
          </td></tr>;
        })}
      </tbody></table> : null}
      {shown.length ? <div className="ws-table-wrap"><table className="ws-table ws-items">
        <thead><tr>
          <th className="ws-pick" aria-label={t('wiki.compare')} />
          <th>{t('wiki.name')}</th>
          {columns.map(c => {
            const on = sort?.field === c.field;
            return <th key={c.field} aria-sort={on ? (sort!.down ? 'descending' : 'ascending') : 'none'}>
              <button type="button" className={`wk-sort ${on ? 'on' : ''}`} title={t('wiki.sortBy', { f: word(c.field) })}
                onClick={() => setSort(on ? (sort!.down === (c.number) ? { field: c.field, down: !sort!.down } : null) : { field: c.field, down: c.number })}>
                {word(c.field)}<Icon name={on ? (sort!.down ? 'chevronDown' : 'chevronUp') : 'sort'} /></button></th>;
          })}
        </tr></thead>
        <tbody>{shown.slice(0, 400).map(i => <tr key={i.id} className={picked.includes(i.id) ? 'picked' : ''}>
          <td className="ws-pick">
            <label className="wk-check" title={t('wiki.compareAdd')}><input type="checkbox" checked={picked.includes(i.id)} disabled={!picked.includes(i.id) && picked.length >= COMPARE_MAX}
              aria-label={`${t('wiki.compareAdd')}: ${i.name}`} onChange={() => togglePick(i.id)} /><span /></label></td>
          <td className="ws-name"><button type="button" className="ws-item" onClick={() => onOpen(i)}><Pic item={i} /><span>{i.name}</span></button></td>
          {columns.map(c => <td key={c.field} className={c.number ? 'num' : ''}>{valueOf(i, c.field)?.split('\n')[0] ?? '–'}</td>)}
        </tr>)}</tbody>
      </table>
        {shown.length > 400 ? <p className="ws-more">{t('wiki.more', { n: shown.length - 400 })}</p> : null}
      </div> : <p className="ws-nothing">{t('wiki.nothing')}</p>}
    </div>
    {picked.length ? <div className="wk-tray" role="region" aria-label={t('wiki.compare')}>
      <span className="wk-tray-items">{pickedItems.map(i => <span key={i.id} className="wk-tray-item">
        {i.img ? <img src={i.img} alt="" referrerPolicy="no-referrer" /> : null}{i.name}
        <button type="button" className="icon-btn" title={t('wiki.compareRemove')} aria-label={`${t('wiki.compareRemove')}: ${i.name}`} onClick={() => togglePick(i.id)}><Icon name="x" /></button></span>)}</span>
      <button type="button" className="btn sm ghost" onClick={() => { setPicked([]); setComparing(false); }}>{t('wiki.clear')}</button>
      <button type="button" className="btn sm primary" id="wk-compare" disabled={picked.length < 2} onClick={() => setComparing(true)}>
        <Icon name="grid" /><span>{t('wiki.compareN', { n: picked.length })}</span></button>
    </div> : null}
    {comparing && pickedItems.length >= 2 ? <Compare items={pickedItems} t={t} word={word} onClose={() => setComparing(false)}
      onRemove={id => { togglePick(id); if (picked.length <= 2) setComparing(false); }} /> : null}
  </div>;
}

function Pic({ item, big }: { item: Item; big?: boolean }) {
  return item.img ? <img src={item.img} alt="" loading="lazy" referrerPolicy="no-referrer" />
    : <span className={`wk-img-none ${big ? 'big' : ''}`}>{item.name.slice(0, 1)}</span>;
}

/** The items most like one: same kind, nearest on the number lines it has that most of the kind has too (and by how
 * much each differs), a different Type counting against. */
export function similarTo(item: Item, items: Item[]) {
  const peers = items.filter(i => i !== item && i.id !== item.id);
  const lines = item.fields.map(([k, v]) => ({ k, n: numberOf(v) })).filter(l => l.n !== null)
    .map(l => ({ ...l, have: peers.map(p => numberOf(valueOf(p, l.k))).filter((n): n is number => n !== null) }))
    .filter(l => l.have.length >= peers.length * 0.5 && l.have.length >= 2)
    .slice(0, 3)
    .map(l => ({ field: l.k, at: l.n!, span: Math.max(...l.have, l.n!) - Math.min(...l.have, l.n!) || 1 }));
  const type = valueOf(item, 'Type');
  const near = peers.map(p => {
    let d = 0, known = 0;
    for (const l of lines) {
      const n = numberOf(valueOf(p, l.field));
      if (n !== null) { d += Math.abs(n - l.at) / l.span; known++; }
    }
    if (!known) return null;
    return { p, d: d / known + (lines.length - known) * 0.5 + (type && valueOf(p, 'Type') !== type ? 0.25 : 0) };
  }).filter(Boolean) as { p: Item; d: number }[];
  return { lines, items: near.sort((a, b) => a.d - b.d).slice(0, SIMILAR).map(x => x.p) };
}

/** The items most like one, as a wikitable: each differing line with + or −, green where it's better. */
export function Similar({ item, items, onOpen }: { item: Item; items: Item[]; onOpen: (i: Item) => void }) {
  const { t } = useHost();
  const word = useWord();
  const similar = useMemo(() => similarTo(item, items), [item, items]);
  if (!similar.items.length) return null;
  return <section data-sec="similar">
    <h2 className="ws-h2">{t('wiki.similar')}</h2>
    <div className="ws-table-wrap"><table className="ws-table">
      <thead><tr><th>{t('wiki.name')}</th>{similar.lines.map(l => <th key={l.field}>{word(l.field)}</th>)}</tr></thead>
      <tbody>{similar.items.map(p => <tr key={p.id}>
        <td className="ws-name"><button type="button" className="ws-item" onClick={() => onOpen(p)}><Pic item={p} /><span>{p.name}</span></button></td>
        {similar.lines.map(l => {
          const v = valueOf(p, l.field), n = numberOf(v), d = n === null ? null : Math.round((n - l.at) * 100) / 100;
          const better = d !== null && d !== 0 && (lessIsBetter(l.field) ? d < 0 : d > 0);
          return <td key={l.field} className="num">{v?.split('\n')[0] ?? '–'}{d ? <em className={better ? 'up' : 'down'}> ({d > 0 ? '+' : '−'}{Math.abs(d)})</em> : null}</td>;
        })}
      </tr>)}</tbody>
    </table></div>
  </section>;
}

/** Rarities from lowest to highest, as most Roblox wikis name them; a rarity line's value ranks by the highest it names. */
const RARITIES = [['common'], ['uncommon'], ['rare'], ['epic'], ['legendary'], ['mythic', 'mythical'], ['godly', 'divine', 'secret', 'exotic', 'celestial']];
/** What a value ranks as on its line: its number, or on a rarity line its rarity; null when it can't be ranked. */
const rankOf = (field: string, v: string | undefined) => {
  const n = numberOf(v);
  if (n !== null || !v || !/rarity|tier/i.test(field)) return n;
  const words = fold(v).split(/[^a-z]+/);
  let rank: number | null = null;
  RARITIES.forEach((names, r) => { if (names.some(x => words.includes(x))) rank = r; });
  return rank;
};

/** Items face to face. Two: a duel, each on its own side in its own colour with "VS" between, every number line a bar
 * growing out from the middle (the better side's full, the other's faint), and how many lines each wins on top. Three
 * or four: a column each, the bars under the values. Lines where one is better come first, then the ones all of them
 * have; the lines only some have wait folded at the bottom. */
function Compare({ items, t, word, onClose, onRemove }: { items: Item[]; t: T; word: Word; onClose: () => void; onRemove: (id: string) => void }) {
  const fields: string[] = [];
  for (const i of items) for (const [k] of i.fields) if (!fields.includes(k)) fields.push(k);
  const best = (f: string) => {
    const have = items.map(i => rankOf(f, valueOf(i, f))).filter((n): n is number => n !== null);
    if (have.length < 2 || new Set(have).size < 2) return null;
    return lessIsBetter(f) ? Math.min(...have) : Math.max(...have);
  };
  const all = (f: string) => items.every(i => valueOf(i, f) !== undefined);
  const ranked = fields.filter(f => best(f) !== null), shared = fields.filter(f => best(f) === null && all(f)), some = fields.filter(f => best(f) === null && !all(f));
  const wins = items.map(i => ranked.filter(f => rankOf(f, valueOf(i, f)) === best(f)).length);
  const top = Math.max(...wins);
  const duel = items.length === 2;
  useEffect(() => {
    const key = (e: KeyboardEvent) => { if (e.key === 'Escape') { e.preventDefault(); onClose(); } };
    addEventListener('keydown', key, true);
    return () => removeEventListener('keydown', key, true);
  }, [onClose]);

  /** Each item's bar on a number line: its share of the biggest (a sliver at least), none on a line of words. */
  const bars = (f: string) => {
    const nums = items.map(i => numberOf(valueOf(i, f)));
    const max = Math.max(...nums.map(n => Math.abs(n ?? 0)));
    return nums.filter(n => n !== null).length >= 2 && max > 0 ? nums.map(n => n === null ? null : Math.max(0.04, Math.abs(n) / max)) : null;
  };
  const row = (f: string, at: number, quiet = false) => {
    const b = best(f), fill = bars(f);
    const label = <div className="cmp-label"><span>{word(f)}</span>{b !== null && lessIsBetter(f) ? <small>{t('wiki.lessBetter')}</small> : null}</div>;
    // By how much the better one is ahead of the next: on number lines only.
    const nums = items.map(i => numberOf(valueOf(i, f)));
    const gap = (n: number) => {
      const own = nums[n], rest = nums.filter((x, k) => k !== n && x !== null) as number[];
      if (own === null || !rest.length || !fill) return null;
      const d = Math.round(Math.min(...rest.map(x => Math.abs(own - x))) * 100) / 100;
      return d ? `${lessIsBetter(f) ? '−' : '+'}${d.toLocaleString()}` : null;
    };
    const cells = items.map((i, n) => {
      const v = valueOf(i, f), win = b !== null && rankOf(f, v) === b, ahead = win ? gap(n) : null;
      return <div key={i.id} className={`cmp-cell s${n} ${win ? 'win' : ''} ${b !== null && !win ? 'lose' : ''}`}>
        <span className="cmp-v">{win ? <Icon name="check" /> : null}{v ? v.split('\n').map((l, k) => <span key={k} className="wk-line">{l}</span>) : <span className="wk-dash">–</span>}{ahead ? <small className="cmp-gap">{ahead}</small> : null}</span>
        {fill && fill[n] !== null ? <i className="cmp-bar" style={{ '--f': fill[n] } as React.CSSProperties} /> : null}
      </div>;
    });
    return <div key={f} className={`cmp-row ${quiet ? 'quiet' : ''}`} style={{ '--i': at } as React.CSSProperties}>
      {duel ? <>{cells[0]}{label}{cells[1]}</> : <>{label}{cells}</>}
    </div>;
  };

  return <div className="overlay wk-overlay" onMouseDown={e => { if (e.target === e.currentTarget) onClose(); }}>
    <div className={`wk-compare ${duel ? 'duel' : 'many'}`} role="dialog" aria-modal="true" aria-label={t('wiki.compare')} style={{ '--n': items.length } as React.CSSProperties}>
      <button type="button" className="icon-btn cmp-close" title={t('wiki.close')} aria-label={t('wiki.close')} onClick={onClose}><Icon name="x" /></button>
      <div className="cmp-head">
        {duel ? null : <div className="cmp-corner"><h2>{t('wiki.compare')}</h2></div>}
        {items.map((i, n) => <div key={i.id} className={`cmp-side s${n} ${ranked.length && wins[n] === top ? 'lead' : ''}`}>
          <span className="cmp-pic">{i.img ? <img src={i.img} alt="" referrerPolicy="no-referrer" /> : <span>{i.name.slice(0, 1)}</span>}</span>
          <b className="cmp-name">{i.name}</b>
          <small className="cmp-cat">{i.cat}</small>
          {ranked.length ? <span className="cmp-score"><em>{wins[n]}</em>/{ranked.length} {t('wiki.winsOf')}</span> : null}
          <button type="button" className="icon-btn cmp-drop" title={t('wiki.compareRemove')} aria-label={`${t('wiki.compareRemove')}: ${i.name}`} onClick={() => onRemove(i.id)}><Icon name="x" /></button>
          {duel && n === 0 ? <span className="cmp-vs" aria-hidden="true">VS</span> : null}
        </div>)}
      </div>
      <div className="cmp-body">
        {[...ranked, ...shared].map((f, n) => row(f, n))}
        {some.length ? <details className="cmp-some">
          <summary><Icon name="chevronRight" />{t('wiki.onlySome')} <span>{some.length}</span></summary>
          {some.map((f, n) => row(f, n, true))}
        </details> : null}
      </div>
    </div>
  </div>;
}

/** An item's chance to drop each try, from its "drop chance" line: "1%", "0.5 %", "1/250", "1 in 40". */
export function dropChance(item: Pick<Item, 'fields'>): number | null {
  for (const [k, v] of item.fields) {
    if (!/drop|chance|rate|odds/i.test(k)) continue;
    const one = /\b1\s*(?:\/|in)\s*([\d,]+(?:\.\d+)?)/i.exec(v), pct = /([\d.]+)\s*%/.exec(v);
    const p = one ? 1 / Number(one[1].replace(/,/g, '')) : pct ? Number(pct[1]) / 100 : null;
    if (p !== null && p > 0 && p < 1) return p;
  }
  return null;
}
/** Tries for a given chance to have had it at least once. */
const triesFor = (p: number, want: number) => Math.ceil(Math.log(1 - want) / Math.log(1 - p));

/** "How long to farm it": for an item with a drop chance, the tries it takes for half, nine in ten and 99 of 100 players
 * to have it, on a track, and how lucky (or not) someone is after the tries they say they made. The wiki says only "1%". */
export function DropOdds({ item }: { item: Pick<Item, 'fields'> }) {
  const { t, locale } = useHost();
  const [tried, setTried] = useState('');
  const p = dropChance(item);
  if (p === null) return null;
  const marks = [0.5, 0.9, 0.99].map(c => ({ c, n: triesFor(p, c) }));
  const end = marks[2].n;
  const n = Math.min(Number(tried.replace(/\D/g, '')) || 0, 1e7);
  const had = n ? 1 - Math.pow(1 - p, n) : null;
  const pct = (x: number) => `${(x * 100).toLocaleString(locale, { maximumFractionDigits: x < 0.995 ? 0 : 1 })}%`;
  const num = (x: number) => x.toLocaleString(locale);
  return <section className="ws-odds" aria-label={t('wiki.odds')}>
    <div className="ws-odds-top">
      <b>{t('wiki.odds')}</b>
      <span className="ws-dim">{t('wiki.oddsEach', { p: `${(p * 100).toLocaleString(locale, { maximumSignificantDigits: 3 })}%`, n: num(Math.round(1 / p)) })}</span>
    </div>
    <div className="ws-odds-track">
      {had !== null ? <i className="ws-odds-you" style={{ '--f': Math.min(1, n / end) } as React.CSSProperties} /> : null}
      {marks.map(m => <span key={m.c} className="ws-odds-mark" style={{ '--f': m.n / end } as React.CSSProperties}>
        <b>{num(m.n)}</b><small>{t('wiki.oddsFor', { c: pct(m.c) })}</small></span>)}
    </div>
    <label className="ws-odds-ask">{t('wiki.oddsTried')}
      <input className="ws-input" type="text" inputMode="numeric" value={tried} maxLength={8} placeholder="0" onChange={e => setTried(e.target.value)} />
      {had !== null ? <span className={had >= 0.9 ? 'unlucky' : ''}>{t(had >= 0.5 ? 'wiki.oddsUnlucky' : 'wiki.oddsNormal', { c: pct(had) })}</span> : null}
    </label>
  </section>;
}
