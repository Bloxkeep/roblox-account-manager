// What Bloxkeep's users play now, as the online count's bot tells every app back with each beat (AppController.Online.cs,
// services/DiscordBot/worker/online.js communityOf): the games played most (a game shows with 3 PCs on it or more), the
// radar (a game where 3 PCs or more dropped out the same way in the last 15 minutes) and the alts board (the PCs running
// the most accounts, under a made-up name or the one they chose). With the count off, asked for apart (communityNow).
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Icon, useAppState, useHost, usePage } from './host';
import { TextBox } from './settings/parts';

export type PlayingGame = { game: number; name: string | null; place: number | null; pcs: number };
export type RadarLine = PlayingGame & { why: 'kick' | 'lost' | 'shutdown' | 'join'; minutes: number };
export type Community = {
  pcs: number; alts: number; games: PlayingGame[]; radar: RadarLine[]; board: { name: string; alts: number }[];
  /** This PC on the board: its place among how many; null when it isn't on it. */
  you: { rank: number | null; of: number } | null;
};
type T = (k: string, v?: Record<string, string | number>) => string;

/** The sums; while the count is off and this is on screen, asked for once a minute (with the count on, each beat brings them). */
export function useCommunity(shown: boolean): Community | null {
  const app = useAppState();
  const { call } = useHost();
  const off = app.countOnline === false;
  useEffect(() => {
    if (!shown || !off)
      return;
    const ask = () => { call('communityNow', {}, { silent: true }).catch(() => {}); };
    ask();
    const timer = setInterval(ask, 60_000);
    return () => clearInterval(timer);
  }, [shown, off]);
  return app.community?.data || null;
}

const sameName = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');
/** A game card's line in it: by its start place, else by its name. */
export function playingOf<L extends PlayingGame>(list: L[] | undefined, place: number, name: string) {
  return (list || []).filter(g => (place && g.place === place) || (g.name && sameName(g.name) === sameName(name)));
}

/** "Radar": games where several Bloxkeep users dropped out the same way just now, as a strip of warnings. */
export function RadarStrip({ lines, t, named = true }: { lines: RadarLine[]; t: T; named?: boolean }) {
  if (!lines.length) return null;
  return <div className="cm-radar" role="status">
    {lines.map(r => <p key={`${r.game}|${r.why}`}><Icon name="alert" />
      <span>{named ? <b>{r.name || t('cm.aGame')}</b> : null}{named ? ': ' : ''}{t(`cm.radar.${r.why}`, { n: r.pcs, m: r.minutes })}</span></p>)}
  </div>;
}

/** Faces' sizes to try, largest first: the first row's, the others', this PC's at the foot (240 px wide, as in app.css). A row's faces may take two lines at most, this PC's three. */
const FIRST_SIZES = [30, 24, 20, 16, 13, 10, 8], ROW_SIZES = [20, 16, 13, 10, 8], FOOT_SIZES = [14, 12, 10, 8, 6];
const gapOf = (s: number) => Math.max(2, Math.round(s / 5));
function fitSize(n: number, width: number, sizes: number[], lines = 2) {
  for (const s of sizes) {
    const perLine = Math.max(1, Math.floor((width + gapOf(s)) / (s + gapOf(s))));
    if (Math.ceil(n / perLine) <= lines) return s;
  }
  return sizes[sizes.length - 1];
}
/** One alt: a little block head, like Bloxkeep's own. */
const Face = () => <svg className="cm-face" aria-hidden="true"><use href="#cm-head" /></svg>;
const faces = (n: number) => Array.from({ length: n }, (_, i) => <Face key={i} />);
/** "{name}" and the like in a sentence filled with bold parts. */
const fill = (s: string, parts: Record<string, React.ReactNode>) =>
  s.split(/\{(\w+)\}/).map((x, i) => i % 2 ? <b key={i}>{parts[x] ?? x}</b> : x);

/** Statistics' alts board: the PCs running the most accounts now, each account a face, this PC marked; joined here under a name. */
export function AltsBoard() {
  const { t, call, locale } = useHost();
  const app = useAppState();
  const c = useCommunity(usePage(p => p.view) === 'stats');
  const counted = app.countOnline !== false, name: string = app.boardName || '';
  const num = (n: number) => n.toLocaleString(locale);
  const isMe = (i: number) => !!name && c?.you?.rank === i + 1;
  const board = c?.board || [];
  // The faces shrink so a row takes two lines at most: measured on the faces' column, before the board paints.
  const rows = useRef<HTMLOListElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const ol = rows.current;
    if (!ol) return;
    const measure = () => setWidth((ol.querySelector('.cm-faces') as HTMLElement | null)?.clientWidth || 0);
    measure();
    const seen = new ResizeObserver(measure);
    seen.observe(ol);
    return () => seen.disconnect();
  }, [board.length > 0]);
  const restSize = fitSize(Math.max(0, ...board.slice(1).map(b => b.alts)), width, ROW_SIZES);
  const firstSize = Math.max(restSize, fitSize(board[0]?.alts || 0, width, FIRST_SIZES));
  const sizes = { '--fs': `${restSize}px`, '--fg': `${gapOf(restSize)}px`, '--fs1': `${firstSize}px`, '--fg1': `${gapOf(firstSize)}px` } as React.CSSProperties;
  // This PC at the foot: its accounts (as the board counts them when it's on it), its place and name.
  const rank = c?.you?.rank || 0, onBoard = !!name && rank > 0 && rank <= board.length;
  const mine = onBoard ? board[rank - 1].alts : (app.accounts || []).filter((a: { state?: string }) => a.state === 'running').length;
  const footSize = fitSize(mine, 240, FOOT_SIZES, 3);
  const perPc = c?.pcs ? (c.alts / c.pcs).toLocaleString(locale, { maximumFractionDigits: 1 }) : '0';
  return <section className="stat-card cm-board" id="alts-board" aria-labelledby="alts-board-h">
    <svg className="cm-defs" aria-hidden="true"><symbol id="cm-head" viewBox="0 0 20 20">
      <rect x="1" y="1" width="18" height="18" rx="4.5" fill="currentColor" />
      <rect x="6" y="7" width="2.2" height="3.4" rx="1.1" fill="#1d1a14" /><rect x="11.8" y="7" width="2.2" height="3.4" rx="1.1" fill="#1d1a14" />
      <path d="M6.6 12.6q3.4 2.6 6.8 0" stroke="#1d1a14" strokeWidth="1.6" fill="none" strokeLinecap="round" />
    </symbol></svg>
    <div className="cm-head">
      <svg className="cm-logo" aria-hidden="true"><use href="#cm-head" /></svg>
      <div className="cm-title">
        <h3 id="alts-board-h">{t('cm.board')}</h3>
        {c ? <span className="cm-live"><i />{t('cm.live')} · {t('cm.liveSub')}</span> : null}
      </div>
      {c ? <div className="cm-stats">
        <span><Face /><span><b>{num(c.alts)}</b><small>{t('cm.accounts')}</small></span></span>
        <span><Icon name="monitor" /><span><b>{num(c.pcs)}</b><small>{t('cm.pcs')}</small></span></span>
        <span><span><b>{perPc}</b><small>{t('cm.perPc')}</small></span></span>
      </div> : null}
    </div>
    {!c ? <p className="cm-note">{t('news.connecting')}</p>
      : !board.length ? <p className="cm-note">{t('cm.empty')}</p>
      : <ol className="cm-rows" ref={rows} style={sizes}>{board.map((b, i) => {
        const me = isMe(i);
        return <li key={i} className={`cm-row${i < 3 ? ` p${i + 1}` : ''}${me ? ' me' : ''}`} aria-label={`${i + 1}. ${b.name}: ${t('cm.altsN', { n: num(b.alts) })}`}>
          <span className="cm-rank">{i < 3 ? <span className="cm-medal">{i + 1}</span> : i + 1}</span>
          <span className="cm-name"><span title={b.name}>{b.name}</span>
            {me ? <em>{t('cm.you')}</em> : i === 0 ? <small>{t('cm.leader')}</small> : null}</span>
          <span className="cm-faces">{faces(b.alts)}</span>
          <b>{num(b.alts)}{i === 0 ? <small>{t('cm.alts')}</small> : null}</b>
        </li>;
      })}</ol>}
    {!counted ? <div className="cm-foot"><small>{t('cm.joinOff')}</small></div> : <div className="cm-foot">
      <div className="cm-me">
        {name && mine > 0 ? <span className="cm-faces" style={{ '--fs': `${footSize}px`, '--fg': `${gapOf(footSize)}px` } as React.CSSProperties}>{faces(mine)}</span> : null}
        {onBoard ? <p>{fill(t('cm.youLine'), { name, r: rank, n: num(c!.you!.of), a: num(mine) })}</p>
          : name && rank ? <p>{t('cm.yourRank', { r: rank, n: c!.you!.of })}</p>
          : name && c?.you ? <p>{t('cm.notYet')}</p> : null}
        <label><span>{t(name ? 'cm.nameOn' : 'cm.join')}</span>
          <TextBox className="field sm" saved={name} maxLength={20} spellCheck={false} placeholder={t('cm.namePh')} aria-label={t('cm.join')}
            commit={v => { call('setBoardName', { name: v }); }} /></label>
      </div>
      <small>{t('cm.joinNote')}</small>
    </div>}
  </section>;
}
