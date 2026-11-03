// The game monitor tab's tiles: every Roblox window on this PC with its account, game, RAM and PID, a button to minimise
// it or open it again, and a box where its live picture goes. A page can't show other programs' windows, so the app lays
// each one's picture (a DWM thumbnail) exactly on its tile's box: app.js reports the boxes ([data-pic]) and withdraws the
// ones a menu or a toast would lie under, since Windows draws the pictures above the whole page. Until 2026-11 the whole
// tab was hand-drawn WinForms (WindowThumbnails); now only the pictures are the app's.
// The app sends the windows every 1.5 s while the tab shows (the "monitor" event, app.js keeps the last one).
import { useEffect, useLayoutEffect, useRef, useState } from 'react';
import { Icon, useHost } from '../host';
import type { MenuItem } from '../Overlays';

/** One Roblox window, as the app sends it. id: its window handle as a number; name as shown (Hide names holds). */
export interface MonitorTile { id: string; pid: number; name: string; game: string; ramMb: number; ours: boolean; minimized: boolean;
  /** The preview only (ui/dev/mock-p7.js): a stand-in picture where the app would lay the live one. */
  picture?: string }
export interface MonitorData { tiles: MonitorTile[] }

/** Spacing and the smallest tile, in CSS pixels, as the WinForms tab had them (its MinTileWidth/Height). */
const GAP = 12, MIN_W = 200, MIN_H = 150;
/** The caption's two lines over the picture, and the picture's inset from the tile's edge. */
const CAPTION = 50, INSET = 6;
/** The grid's padding (app.css .mon-grid): room for the focus ring inside the scrolling area. */
const PAD = 4;

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
    if (picture > bestWidth) { best = cols; bestWidth = picture; }
  }
  return best;
}

export function MonitorView({ preview }: { preview: boolean }) {
  const host = useHost();
  const { t, fmtMb } = host;
  const [data, setData] = useState<MonitorData | null>(() => host.monitor());
  const [size, setSize] = useState({ w: 0, h: 0 });
  const [focus, setFocus] = useState<string | null>(null);
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

  if (!data) {
    // In the browser preview there is no app to draw the pictures: it says what goes here.
    return <div className={`mon-area ${preview ? 'preview' : ''}`} id="mon-area" ref={area}>{preview ? <p>{t('mon.preview')}</p> : null}</div>;
  }
  const tiles = data.tiles;
  if (!tiles.length) {
    return <div className="mon-area none" id="mon-area" ref={area}><div className="mon-empty"><span className="pv-empty-icon"><Icon name="screens" /></span><p>{t('mon.empty')}</p></div></div>;
  }

  const cols = size.w ? columns(tiles.length, size.w, size.h) : 1;
  const rows = Math.ceil(tiles.length / cols);
  const tileH = Math.max(MIN_H, Math.floor((size.h - GAP * (rows - 1)) / rows));
  const stop = tiles.some(x => x.id === focus) ? focus : tiles[0].id;

  const act = (tile: MonitorTile, what: string) => { void host.call('monitorAct', { id: tile.id, what }, { silent: true }); };
  const items = (tile: MonitorTile): MenuItem[] => [
    { icon: 'external', text: t('mon.show'), run: () => act(tile, 'show') },
    tile.minimized ? { icon: 'screens', text: t('mon.restore'), run: () => act(tile, 'restore') } : { icon: 'screens', text: t('mon.minimize'), run: () => act(tile, 'minimize') },
    { icon: 'copy', text: t('mon.copyPid'), run: () => act(tile, 'copyPid') },
    '-',
    { icon: 'x', text: t('mon.close'), danger: true, run: () => act(tile, 'close') },
  ];
  const moveFocus = (from: number, to: number) => {
    if (to < 0 || to >= tiles.length || to === from) return;
    setFocus(tiles[to].id);
    area.current?.querySelector<HTMLElement>(`[data-tile="${tiles[to].id}"]`)?.focus();
  };
  const onKey = (e: React.KeyboardEvent, tile: MonitorTile, i: number) => {
    if (e.target !== e.currentTarget) return; // the minimise button's own keys
    const key = e.key;
    if (key === 'ArrowRight') moveFocus(i, i + 1);
    else if (key === 'ArrowLeft') moveFocus(i, i - 1);
    else if (key === 'ArrowDown') moveFocus(i, Math.min(tiles.length - 1, i + cols));
    else if (key === 'ArrowUp') moveFocus(i, i - cols);
    else if (key === 'Home') moveFocus(i, 0);
    else if (key === 'End') moveFocus(i, tiles.length - 1);
    else if (key === 'Enter' || key === ' ') act(tile, 'show');
    else if (key.toLowerCase() === 'm' && !e.ctrlKey && !e.altKey && !e.metaKey) act(tile, tile.minimized ? 'restore' : 'minimize');
    else if (key === 'ContextMenu' || (key === 'F10' && e.shiftKey)) {
      const r = e.currentTarget.getBoundingClientRect();
      host.menuAt(r.left + 12, r.top + CAPTION, items(tile), { icons: true });
    } else return;
    e.preventDefault();
  };

  return <div className="mon-area mon-tiles" id="mon-area" ref={area} onScroll={() => host.monitorMoved()}>
    <div className="mon-grid" role="list" style={{ gridTemplateColumns: `repeat(${cols}, minmax(${MIN_W}px, 1fr))`, gridAutoRows: `${tileH}px` }}>
      {tiles.map((tile, i) => {
        const ram = tile.ramMb > 0 ? fmtMb(tile.ramMb) : '';
        return <div key={tile.id} role="listitem" className={`mon-tile ${tile.minimized ? 'min' : ''}`} data-tile={tile.id}
          tabIndex={tile.id === stop ? 0 : -1} aria-label={`${tile.name}, ${tile.game}`}
          onFocus={() => setFocus(tile.id)}
          onClick={e => { if (!(e.target as Element).closest('.mon-min')) act(tile, 'show'); }}
          onContextMenu={e => { e.preventDefault(); host.menuAt(e.clientX, e.clientY, items(tile), { icons: true }); }}
          onKeyDown={e => onKey(e, tile, i)}>
          <div className="mon-cap">
            <div className="mon-lines">
              <div className="mon-line"><b className="mon-name">{tile.name}</b>{ram ? <span className="mon-tag">{ram}</span> : null}</div>
              <div className="mon-line"><span className="mon-game">{tile.game}</span><span className="mon-tag">PID {tile.pid}</span></div>
            </div>
            <button type="button" className="icon-btn mon-min" tabIndex={-1}
              title={t(tile.minimized ? 'mon.restore' : 'mon.minimize')} aria-label={t(tile.minimized ? 'mon.restore' : 'mon.minimize')}
              onClick={() => act(tile, tile.minimized ? 'restore' : 'minimize')}>
              <svg className="i" viewBox="0 0 24 24" aria-hidden="true">{tile.minimized ? <rect x="6.5" y="6.5" width="11" height="11" rx="1.5" /> : <path d="M6 12h12" />}</svg>
            </button>
          </div>
          <div className="mon-pic" data-pic={tile.id}>
            {tile.minimized ? <p>{t('mon.minimized')}</p> : preview && tile.picture ? <img src={tile.picture} alt="" draggable={false} /> : null}
          </div>
        </div>;
      })}
    </div>
  </div>;
}
