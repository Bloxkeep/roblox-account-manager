// Quick search (Ctrl+K): one box for everything on the page, ranked by a small fuzzy match. app.js builds the entries
// (paletteEntries: accounts, presets, games, tabs, Settings sections, actions, each { text, hint, keywords, icon |
// img | avatar, run, alts }) each time it opens; this draws the box and the list. Enter runs the highlighted entry;
// an account's other actions (Play, Edit) are on Ctrl+Enter and Shift+Enter, and on the highlighted row.
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { Icon, useHost } from './host';
import { fold } from './text';
import { Avatar } from './Avatar';

const PAL_MAX = 60;
const PAL_ALT_KEYS = ['Ctrl+Enter', 'Shift+Enter'];

export type PaletteEntry = {
  id: string; kind: string; text: string; hint?: string; keywords?: string[];
  icon?: string; img?: string; avatar?: { avatarUrl?: string | null; username?: string; label?: string };
  run: () => void; alts?: { text: string; run: () => void }[];
};

/**
 * How well a query matches a text, both folded: the start of the text beats the start of a word, which beats the
 * middle of one, then every word somewhere, then the letters in order (fewer gaps first). -1 for no match.
 */
const PAL_WORD = /[\s@_\-.·(/:[\]]/;
const wordStart = (s: string, j: number) => j === 0 || PAL_WORD.test(s[j - 1]);
function fuzzyScore(q: string, s: string) {
  if (!s) return -1;
  const at = s.indexOf(q);
  if (at === 0) return 1000 - Math.min(s.length, 200);
  if (at > 0) return (wordStart(s, at) ? 800 : 600) - Math.min(at, 100);
  const words = q.split(/\s+/).filter(Boolean);
  if (words.length > 1 && words.every(w => s.includes(w))) return 500;
  // Letters in order, the first one starting a word: "clall" finds "Close all", "anti" doesn't find "sang tieng".
  const letters = q.replace(/\s+/g, '');
  let first = s.indexOf(letters[0]);
  while (first >= 0 && !wordStart(s, first)) first = s.indexOf(letters[0], first + 1);
  if (first < 0) return -1;
  let score = 306, from = first + 1, last = first;
  for (const c of letters.slice(1)) {
    const j = s.indexOf(c, from);
    if (j < 0) return -1;
    if (j !== last + 1) score -= 5 + Math.min(j - from, 15);
    if (wordStart(s, j)) score += 6;
    last = j;
    from = j + 1;
  }
  return Math.max(1, score);
}
/** The entries for a query, best first (ties keep the registry's order); the text counts most, keywords only whole. */
function rank(all: PaletteEntry[], query: string): { rows: PaletteEntry[]; more: number } {
  const q = fold(query.trim());
  if (!q) return { rows: all.slice(0, PAL_MAX), more: Math.max(0, all.length - PAL_MAX) };
  const hits = all.map((e, i) => {
    const byText = fuzzyScore(q, fold(e.text));
    const byOther = Math.max(-1, ...[e.hint, ...(e.keywords || [])].filter(Boolean).map(k => {
      const s = fuzzyScore(q, fold(k));
      return s >= 500 ? s - 300 : -1;
    }));
    return { e, i, score: Math.max(byText, byOther) };
  }).filter(x => x.score > 0).sort((a, b) => b.score - a.score || a.i - b.i);
  // Only the best ones are listed: the rest are counted, so a long list doesn't read as all there is.
  return { rows: hits.slice(0, PAL_MAX).map(x => x.e), more: Math.max(0, hits.length - PAL_MAX) };
}

export function Palette({ entries, close }: { entries: PaletteEntry[]; close: () => void }) {
  const { t } = useHost();
  const [query, setQuery] = useState('');
  const [index, setIndex] = useState(0);
  const { rows, more } = rank(entries, query);
  const input = useRef<HTMLInputElement>(null), list = useRef<HTMLDivElement>(null);
  const pointer = useRef({ x: -1, y: -1 });
  useEffect(() => { list.current?.querySelector('.pal-row.on')?.scrollIntoView({ block: 'nearest' }); }, [index, query]);

  // First the palette closes (the focus goes back), then the entry may open a dialog of its own or move it.
  const runEntry = (e: PaletteEntry | undefined, alt = -1) => {
    const run = alt >= 0 ? e?.alts?.[alt]?.run : e?.run;
    if (!run) return;
    close();
    run();
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    const n = rows.length;
    if (['ArrowDown', 'ArrowUp', 'PageDown', 'PageUp'].includes(e.key)) {
      e.preventDefault();
      if (!n) return;
      const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : e.key === 'PageDown' ? 8 : -8;
      setIndex(e.key.startsWith('Page') ? Math.max(0, Math.min(n - 1, index + step)) : (index + step + n) % n);
    } else if (e.key === 'Enter' && !e.nativeEvent.isComposing) {
      e.preventDefault();
      runEntry(rows[index], e.ctrlKey ? 0 : e.shiftKey ? 1 : -1);
    } else if (e.key === 'Tab') {
      e.preventDefault(); // the box keeps the focus: the arrows walk the list
    } else if (e.key === 'Escape') {
      // The dialog leaves Esc to a box whose list is open (aria-expanded), as this one's always is: closed here, and
      // not passed on to the page (where Esc would clear the selection).
      e.stopPropagation();
      close();
    }
  };
  // The kind of each row ("Action", "Account"…) only when the list mixes kinds; a list of one kind needs no label.
  const mixed = new Set(rows.map(e => e.kind)).size > 1;

  return <div className="pal">
    <label className="inwrap pal-search"><Icon name="search" /><input className="pal-input" ref={input} type="text" autoComplete="off" spellCheck={false} maxLength={100}
      placeholder={t('pal.placeholder')} aria-label={t('pal.title')} role="combobox" aria-expanded="true" aria-autocomplete="list" aria-controls="pal-list"
      aria-activedescendant={rows.length ? `pal-${index}` : undefined} value={query} onChange={e => { setQuery(e.target.value); setIndex(0); }} onKeyDown={keys} /></label>
    {/* The focus stays in the box whatever is clicked in the list. */}
    <div className="pal-list" id="pal-list" role="listbox" aria-label={t('pal.title')} ref={list} onMouseDown={e => e.preventDefault()}
      onMouseMove={e => {
        // Only a real move: rows scrolled under a resting pointer (the arrow keys) don't take the highlight.
        if (e.clientX === pointer.current.x && e.clientY === pointer.current.y) return;
        pointer.current = { x: e.clientX, y: e.clientY };
        const row = (e.target as HTMLElement).closest<HTMLElement>('.pal-row');
        if (row && +row.dataset.pi! !== index) setIndex(+row.dataset.pi!);
      }}>
      {!rows.length ? <div className="pal-empty">{t('pal.none', { q: query.trim() })}</div> : <>
        {rows.map((e, i) => {
          const on = i === index;
          return <div key={`${e.id}|${i}`} className={`pal-row ${on ? 'on' : ''}`} role="option" id={`pal-${i}`} aria-selected={on} data-pi={i}
            onClick={ev => {
              const alt = (ev.target as HTMLElement).closest<HTMLElement>('.pal-alt');
              runEntry(e, alt ? +alt.dataset.alt! : -1);
            }}>
            <Lead e={e} />
            <span className="pal-txt"><b>{e.text}</b>{e.hint ? <small>{e.hint}</small> : null}</span>
            {/* The other actions show on the highlighted row only, with their keys. */}
            {on && e.alts ? e.alts.map((x, j) => <button key={j} type="button" className="pal-alt" data-alt={j} tabIndex={-1}>{x.text}<kbd>{PAL_ALT_KEYS[j]}</kbd></button>) : null}
            {mixed ? <em className="pal-cat">{t(`pal.cat.${e.kind}`)}</em> : null}
          </div>;
        })}
        {more ? <div className="pal-more">{t('pal.more', { n: more })}</div> : null}
      </>}
    </div>
    <div className="pal-foot" aria-hidden="true">
      <span><kbd>↑</kbd><kbd>↓</kbd>{t('pal.move')}</span><span><kbd>Enter</kbd>{t('pal.open')}</span><span><kbd>Esc</kbd>{t('pal.close')}</span>
    </div>
  </div>;
}

/** A row's picture: an account's avatar, a game's icon (the gamepad when it doesn't load), or the entry's icon. */
function Lead({ e }: { e: PaletteEntry }) {
  const [failed, setFailed] = useState(false);
  if (e.avatar) return <span className="avatar sm"><Avatar a={e.avatar} /></span>;
  if (e.img) return <span className="pal-ic">{failed ? <span className="gph"><Icon name="gamepad" /></span> : <img src={e.img} alt="" onError={() => setFailed(true)} />}</span>;
  return <span className="pal-ic"><Icon name={e.icon || 'search'} /></span>;
}
