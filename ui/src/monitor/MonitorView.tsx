// The game monitor tab's tiles: every Roblox window on this PC with its account, game, RAM and PID, a button to minimise
// it or open it again, and a box where its live picture goes. A page can't show other programs' windows, so the app lays
// each one's picture (a DWM thumbnail) exactly on its tile's box: app.js reports the boxes ([data-pic]) and withdraws the
// ones a menu or a toast would lie under, since Windows draws the pictures above the whole page. Until 2026-11 the whole
// tab was hand-drawn WinForms (WindowThumbnails); now only the pictures are the app's.
// The app sends the windows every 1.5 s while the tab shows (the "monitor" event, app.js keeps the last one).
// The windows are laid out in a lane per game: its icon, name, windows and RAM, a strip with one segment per window
// coloured by how it is doing, and buttons for all of them (fold and "only this one" with two games or more). A lane
// with a window cut off comes first. A folded lane has no picture boxes, so the app stops drawing (and Windows stops copying) its pictures.
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Icon, useHost } from '../host';
import type { MenuItem } from '../Overlays';

/** One Roblox window, as the app sends it. id: its window handle as a number; name as shown (Hide names holds). */
export interface MonitorTile { id: string; pid: number; name: string; game: string; ramMb: number; ours: boolean; minimized: boolean;
  /** As the accounts' cards have it: ingame, joining, menu, disconnected, signedout; "" when nothing is known. */
  status?: string;
  /** The server it is in (windows sharing one are marked with the same letter), and the game's icon once fetched. */
  jobId?: string | null; iconUrl?: string | null;
  /** The preview only (ui/dev/mock.js): a stand-in picture where the app would lay the live one. */
  picture?: string }
export interface MonitorData { tiles: MonitorTile[] }

/** Spacing and the smallest tile, in CSS pixels, as the WinForms tab had them (its MinTileWidth/Height). */
const GAP = 12, MIN_W = 200, MIN_H = 150;
/** The caption's two lines over the picture, and the picture's inset from the tile's edge. */
const CAPTION = 50, INSET = 6;
/** The grid's padding (app.css .mon-grid): room for the focus ring inside the scrolling area. */
const PAD = 4;
/** A lane's heading with its strip (app.css .mon-lane-head, .mon-strip) and the space between lanes (.mon-lanes). */
const HEAD = 66, LANE_GAP = 18;
/** The least height a lane's tiles get: more lanes than fit scroll, rather than shrink to slivers. */
const LANE_MIN = 240;

/** The columns that show the biggest 16:9 picture in the area as it is now (two windows in a tall area stack, so they
 * aren't banded by empty space); none narrower than MIN_W. */
export function columns(count: number, width: number, height: number): number {
  let best = 1, bestWidth = 0;
  for (let cols = 1; cols <= count; cols++) {
    const w = (width - GAP * (cols - 1)) / cols;
    if (cols > 1 && w < MIN_W) break;
    const rows = Math.ceil(count / cols);
    const h = Math.max(MIN_H, (height - GAP * (rows - 1)) / rows);
    const picture = Math.min(w - INSET * 2, (h - CAPTION - INSET) * 16 / 9);
    // As big with more columns (both held by the height): fewer rows, less to scroll.
    if (picture > bestWidth - .5) { best = cols; bestWidth = Math.max(bestWidth, picture); }
  }
  return best;
}

/** How a window is doing, as its strip segment and dot show it. */
export function health(status = ''): 'ok' | 'join' | 'bad' | 'idle' {
  return status === 'ingame' ? 'ok' : status === 'joining' ? 'join' : status === 'disconnected' || status === 'signedout' ? 'bad' : 'idle';
}

export interface Lane { game: string; iconUrl?: string | null; tiles: MonitorTile[]; ramMb: number; ok: number; join: number; bad: number;
  /** A letter for each server two or more of its windows share. */
  servers: Map<string, string> }

/** The windows by game, in the order the app sent them within each: lanes with a window cut off first, then those
 * still joining, then the most windows; "No game" (the Roblox menu) last. */
export function lanes(tiles: MonitorTile[], noGame: string): Lane[] {
  const by = new Map<string, Lane>();
  for (const tile of tiles) {
    let lane = by.get(tile.game);
    if (!lane) by.set(tile.game, lane = { game: tile.game, tiles: [], ramMb: 0, ok: 0, join: 0, bad: 0, servers: new Map() });
    lane.tiles.push(tile);
    lane.iconUrl ||= tile.iconUrl;
    lane.ramMb += Math.max(0, tile.ramMb);
    const h = health(tile.status);
    if (h !== 'idle') lane[h]++;
  }
  for (const lane of by.values()) {
    const seen = new Map<string, number>();
    for (const tile of lane.tiles) if (tile.jobId) seen.set(tile.jobId, (seen.get(tile.jobId) ?? 0) + 1);
    for (const [job, n] of seen) if (n > 1) lane.servers.set(job, String.fromCharCode(65 + lane.servers.size % 26));
  }
  const rank = (l: Lane) => l.bad ? 0 : l.join ? 1 : l.game === noGame || !l.game ? 3 : 2;
  return [...by.values()].sort((a, b) => rank(a) - rank(b) || b.tiles.length - a.tiles.length || a.game.localeCompare(b.game));
}

/** The lanes folded and the one shown alone, kept while the tab is away (the page stays open). */
const folded = new Set<string>();
let soloGame: string | null = null;

/** Glyphs of their own: a window's minimise and restore, and "only this one" (four corners). */
const GLYPH = {
  min: <path d="M6 12h12" />,
  restore: <rect x="6.5" y="6.5" width="11" height="11" rx="1.5" />,
  solo: <path d="M8 3H5a2 2 0 0 0-2 2v3M21 8V5a2 2 0 0 0-2-2h-3M3 16v3a2 2 0 0 0 2 2h3M16 21h3a2 2 0 0 0 2-2v-3" />,
};
const Glyph = ({ d }: { d: keyof typeof GLYPH }) => <svg className="i" viewBox="0 0 24 24" aria-hidden="true">{GLYPH[d]}</svg>;

/** The game's icon, or the gamepad until it comes (or if it fails). */
function GameMark({ url }: { url?: string | null }) {
  const [failed, setFailed] = useState('');
  return url && failed !== url ? <img className="mon-gicon" src={url} alt="" decoding="async" onError={() => setFailed(url)} />
    : <span className="mon-gicon"><Icon name="gamepad" /></span>;
}

export function MonitorView({ preview }: { preview: boolean }) {
  const host = useHost();
  const { t, tn, fmtMb } = host;
  const [data, setData] = useState<MonitorData | null>(() => host.monitor());
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [focus, setFocus] = useState<string | null>(null);
  const [, redraw] = useState(0);
  const [closeAsk, setCloseAsk] = useState<string | null>(null);
  const area = useRef<HTMLDivElement>(null);

  useEffect(() => host.on('monitor', d => setData(d as MonitorData)), [host]);
  // The area's inside less the grid's padding (PAD): the grid is laid out for it.
  useLayoutEffect(() => {
    const el = area.current;
    if (!el) return;
    const measure = () => {
      const w = el.clientWidth - PAD * 2, h = el.clientHeight - PAD * 2;
      setSize(s => s.w === w && s.h === h ? s : { w, h });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [data === null, data?.tiles.length === 0]);
  // Every drawing may move a box: the app is told where they are now (app.js reports them once per frame at most).
  useLayoutEffect(() => { host.monitorMoved(); });
  // A lane's "Close all" asks on the first click and keeps asking a few seconds, as the tray's does.
  useEffect(() => {
    if (!closeAsk) return;
    const timer = setTimeout(() => setCloseAsk(null), 4000);
    return () => clearTimeout(timer);
  }, [closeAsk]);

  if (!data) {
    // In the browser preview there is no app to draw the pictures: it says what goes here.
    return <div className={`mon-area ${preview ? 'preview' : ''}`} id="mon-area" ref={area}>{preview ? <p>{t('mon.preview')}</p> : null}</div>;
  }
  const tiles = data.tiles;
  if (!tiles.length) {
    return <div className="mon-area none" id="mon-area" ref={area}><div className="mon-empty"><span className="pv-empty-icon"><Icon name="screens" /></span><p>{t('mon.empty')}</p></div></div>;
  }

  const all = lanes(tiles, t('mon.noGame'));
  // One game has its heading too, for its buttons for all the windows (owner, 2026-11-07: no Minimise all with one
  // game), but nothing to fold or show alone.
  const multi = all.length > 1;
  const solo = multi && soloGame !== null ? all.find(l => l.game === soloGame) ?? null : null;
  const shown = solo ? [solo] : all;
  // With one game a fold left from before is no matter: it shows.
  const open = multi ? shown.filter(l => solo || !folded.has(l.game)) : all;
  // Each open lane gets an equal share of the height, less the headings.
  const share = Math.max(LANE_MIN, (size.h - shown.length * (HEAD + LANE_GAP) + LANE_GAP) / Math.max(1, open.length));
  const layout = new Map(open.map(l => {
    const cols = size.w ? columns(l.tiles.length, size.w, share) : 1;
    const rows = Math.ceil(l.tiles.length / cols);
    return [l.game, { cols, tileH: Math.max(MIN_H, Math.floor((share - GAP * (rows - 1)) / rows)) }];
  }));
  const order = open.flatMap(l => l.tiles), index = new Map(order.map((x, i) => [x, i]));
  const stop = order.some(x => x.id === focus) ? focus : order[0]?.id;

  const act = (tile: MonitorTile, what: string) => { void host.call('monitorAct', { id: tile.id, what }, { silent: true }); };
  const items = (tile: MonitorTile): MenuItem[] => [
    { icon: 'external', text: t('mon.show'), run: () => act(tile, 'show') },
    tile.minimized ? { icon: 'screens', text: t('mon.restore'), run: () => act(tile, 'restore') } : { icon: 'screens', text: t('mon.minimize'), run: () => act(tile, 'minimize') },
    { icon: 'copy', text: t('mon.copyPid'), run: () => act(tile, 'copyPid') },
    '-',
    { icon: 'x', text: t('mon.close'), danger: true, run: () => act(tile, 'close') },
  ];
  const moveFocus = (from: number, to: number) => {
    if (to < 0 || to >= order.length || to === from) return;
    setFocus(order[to].id);
    area.current?.querySelector<HTMLElement>(`[data-tile="${order[to].id}"]`)?.focus();
  };
  const onKey = (e: React.KeyboardEvent, tile: MonitorTile, i: number, cols: number) => {
    if (e.target !== e.currentTarget) return; // the minimise button's own keys
    const key = e.key;
    if (key === 'ArrowRight') moveFocus(i, i + 1);
    else if (key === 'ArrowLeft') moveFocus(i, i - 1);
    else if (key === 'ArrowDown') moveFocus(i, Math.min(order.length - 1, i + cols));
    else if (key === 'ArrowUp') moveFocus(i, i - cols);
    else if (key === 'Home') moveFocus(i, 0);
    else if (key === 'End') moveFocus(i, order.length - 1);
    else if (key === 'Enter' || key === ' ') act(tile, 'show');
    else if (key.toLowerCase() === 'm' && !e.ctrlKey && !e.altKey && !e.metaKey) act(tile, tile.minimized ? 'restore' : 'minimize');
    else if (key === 'ContextMenu' || (key === 'F10' && e.shiftKey)) {
      const r = e.currentTarget.getBoundingClientRect();
      host.menuAt(r.left + 12, r.top + CAPTION, items(tile), { icons: true });
    } else return;
    e.preventDefault();
  };
  /** How the window is doing in words ("" when nothing is known). */
  const stLabel = (tile: MonitorTile) => {
    const h = health(tile.status);
    if (h === 'idle') return tile.status === 'menu' ? t('mon.st.menu') : '';
    return t(tile.status === 'signedout' ? 'mon.st.signedout' : `mon.st.${h}`);
  };

  const tileOf = (tile: MonitorTile, lane: Lane, cols: number) => {
    const i = index.get(tile) ?? -1;
    const ram = tile.ramMb > 0 ? fmtMb(tile.ramMb) : '';
    const h = health(tile.status), st = stLabel(tile);
    const letter = tile.jobId ? lane.servers.get(tile.jobId) : undefined;
    // Its lane's heading names the game: the second line says how the window is doing instead.
    const line = st || tile.game;
    return <div key={tile.id} role="listitem" className={`mon-tile ${tile.minimized ? 'min' : ''} ${h === 'bad' ? 'bad' : ''}`} data-tile={tile.id}
      tabIndex={tile.id === stop ? 0 : -1} aria-label={[tile.name, tile.game, st].filter(Boolean).join(', ')}
      onFocus={() => setFocus(tile.id)}
      onClick={e => { if (!(e.target as Element).closest('.mon-min')) act(tile, 'show'); }}
      onContextMenu={e => { e.preventDefault(); host.menuAt(e.clientX, e.clientY, items(tile), { icons: true }); }}
      onKeyDown={e => onKey(e, tile, i, cols)}>
      <div className="mon-cap">
        <div className="mon-lines">
          <div className="mon-line"><b className="mon-name" title={tile.name}>{tile.name}</b>{ram ? <span className="mon-tag">{ram}</span> : null}</div>
          <div className="mon-line">
            <span className="mon-game" title={line}><i className={`mon-dot ${h}`} />{line}</span>
            {letter ? <span className={`mon-tag mon-srv s${(letter.charCodeAt(0) - 65) % 4}`} title={t('mon.serverTip')}>{t('mon.server', { s: letter })}</span> : null}
            <span className="mon-tag">PID {tile.pid}</span>
          </div>
        </div>
        <button type="button" className="icon-btn mon-min" tabIndex={-1}
          title={t(tile.minimized ? 'mon.restore' : 'mon.minimize')} aria-label={t(tile.minimized ? 'mon.restore' : 'mon.minimize')}
          onClick={() => act(tile, tile.minimized ? 'restore' : 'minimize')}>
          <Glyph d={tile.minimized ? 'restore' : 'min'} />
        </button>
      </div>
      <div className="mon-pic" data-pic={tile.id}>
        {tile.minimized ? <p>{t('mon.minimized')}</p> : preview && tile.picture ? <img src={tile.picture} alt="" draggable={false} /> : null}
      </div>
    </div>;
  };
  const grid = (lane: Lane) => {
    const { cols, tileH } = layout.get(lane.game)!;
    return <div className="mon-grid in-lane" role="list" aria-label={lane.game || t('mon.noGame')}
      style={{ gridTemplateColumns: `repeat(${cols}, minmax(${MIN_W}px, 1fr))`, gridAutoRows: `${tileH}px` }}>
      {lane.tiles.map(tile => tileOf(tile, lane, cols))}
    </div>;
  };

  const lane = (l: Lane) => {
    const isOpen = open.includes(l);
    const anyUp = l.tiles.some(x => !x.minimized);
    const asking = closeAsk === l.game;
    const name = l.game || t('mon.noGame');
    const toggle = () => { if (folded.has(l.game)) folded.delete(l.game); else folded.add(l.game); redraw(n => n + 1); };
    const soloToggle = () => { soloGame = solo ? null : l.game; redraw(n => n + 1); };
    const meta = [tn('mon.windows', l.tiles.length), l.ramMb > 0 ? fmtMb(l.ramMb) : ''].filter(Boolean).join(' · ');
    return <section key={l.game} className={`mon-lane ${isOpen ? '' : 'folded'}`} aria-label={name}>
      <div className="mon-lane-head" onDoubleClick={e => { if (multi && !(e.target as Element).closest('button')) soloToggle(); }}>
        {!multi ? null : solo ? <button type="button" className="btn ghost sm mon-back" onClick={soloToggle}><Icon name="chevronLeft" /><span>{t('mon.allGames')}</span></button>
          : <button type="button" className="icon-btn mon-fold" aria-expanded={isOpen} title={t(isOpen ? 'mon.fold' : 'mon.unfold')}
            aria-label={t(isOpen ? 'mon.fold' : 'mon.unfold')} onClick={toggle}><Icon name={isOpen ? 'chevronDown' : 'chevronRight'} /></button>}
        <GameMark url={l.iconUrl} />
        <div className="mon-lane-title"><b title={name}>{name}</b><span className="mon-lane-meta">{meta}</span></div>
        <div className="mon-lane-states">
          {l.bad ? <span className="mon-st bad">{tn('mon.nBad', l.bad)}</span> : null}
          {l.join ? <span className="mon-st join">{tn('mon.nJoin', l.join)}</span> : null}
          {l.ok ? <span className="mon-st ok">{tn('mon.nOk', l.ok)}</span> : null}
        </div>
        <div className="mon-lane-acts">
          <button type="button" className="icon-btn" title={t(anyUp ? 'mon.minAll' : 'mon.restoreAll')} aria-label={t(anyUp ? 'mon.minAll' : 'mon.restoreAll')}
            onClick={() => l.tiles.filter(x => x.minimized !== anyUp).forEach(x => act(x, anyUp ? 'minimize' : 'restore'))}><Glyph d={anyUp ? 'min' : 'restore'} /></button>
          {solo || !multi ? null : <button type="button" className="icon-btn" title={t('mon.solo')} aria-label={t('mon.solo')} onClick={soloToggle}><Glyph d="solo" /></button>}
          <button type="button" className={`icon-btn mon-kill ${asking ? 'ask' : ''}`} title={t('mon.closeAll')}
            aria-label={asking ? tn('mon.closeAsk', l.tiles.length) : t('mon.closeAll')}
            onClick={() => { if (asking) { setCloseAsk(null); l.tiles.forEach(x => act(x, 'close')); } else setCloseAsk(l.game); }}>
            {asking ? <span>{tn('mon.closeAsk', l.tiles.length)}</span> : <Icon name="power" />}</button>
        </div>
      </div>
      <div className="mon-strip" aria-hidden="true">{l.tiles.map(x => <i key={x.id} className={health(x.status)} />)}</div>
      {isOpen ? grid(l) : null}
    </section>;
  };

  return <div className="mon-area mon-tiles" id="mon-area" ref={area} onScroll={() => host.monitorMoved()}>
    <div className="mon-lanes">{shown.map(lane)}</div>
  </div>;
}
