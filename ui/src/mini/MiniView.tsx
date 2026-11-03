// The mini window (MiniWindow.cs draws mini.html in a small always-on-top window): the accounts running now, each with
// its state, game, RAM, Show and Close; Close all (asked first, as the main page asks) and Open Bloxkeep at the foot.
// The same chrome as a macro's panel beside the game. Until 2026-11 it was hand-drawn WinForms.
// The app sends the list every two seconds (the "mini" event); rows keep their account, values are patched in place.
import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { Icon, useHost } from '../host';

/** One running account, as the app sends it: its name as shown (Hide names holds), its game in words, its window's state. */
export interface MiniClient { id: string; name: string; game: string; pid: number; ramMb: number; state: 'game' | 'menu' | 'disconnected' }

/** The app's "mini" event. */
export interface MiniData {
  unlocked: boolean;
  clients: MiniClient[];
  /** Every Roblox on the PC, Bloxkeep's or not (Close all closes those too). */
  robloxCount: number;
  /** "Close all" asks first (the person hasn't ticked "Don't ask again"). */
  askCloseAll: boolean;
  theme: string;
  customTheme: unknown;
  language: string;
}

/** What the mini window's page sends the app. */
export interface MiniOn {
  /** show {id}, close {id}, closeAll {remember}, open (Bloxkeep's window), hide. */
  act(what: string, args?: Record<string, unknown>): void;
  /** The page's size in CSS pixels, after each drawing (the window shows once the page has drawn). */
  size(width: number, height: number): void;
  /** The title row is being dragged: the window moves with the pointer. */
  drag(): void;
  /** The bottom edge is being dragged: the window grows or shrinks with the pointer. */
  resize(): void;
}

export interface MiniViewApi { show(data: MiniData): void }

/** No click this soon after a row appeared or moved: it was meant for what was there before. */
const SETTLE_MS = 500;
/** After Close, its buttons stay off this long; Roblox normally ends well within it, and the row with it. */
const CLOSE_WAIT_MS = 10_000;
const DRAG_FROM = 3;

export function MiniRoot({ on, handle }: { on: MiniOn; handle: Ref<MiniViewApi> }) {
  const [data, setData] = useState<MiniData | null>(null);
  useImperativeHandle(handle, () => ({ show: d => setData(d) }), []);
  if (!data) return null;
  return <MiniView data={data} on={on} />;
}

export function MiniView({ data, on }: { data: MiniData; on: MiniOn }) {
  const { t, tn, fmtMb, logo } = useHost();
  const root = useRef<HTMLDivElement>(null);
  const [asking, setAsking] = useState(false);
  const [remember, setRemember] = useState(false);
  const [closing, setClosing] = useState<Record<string, number>>({});
  const [, tick] = useState(0);
  // Where each row was last drawn, and since when: a row that moved under the pointer ignores clicks for a moment.
  const places = useRef(new Map<string, { index: number; since: number }>());
  const now = performance.now();
  data.clients.forEach((c, index) => {
    const was = places.current.get(c.id);
    if (!was || was.index !== index) places.current.set(c.id, { index, since: now });
  });
  const settled = (id: string) => performance.now() - (places.current.get(id)?.since ?? 0) >= SETTLE_MS;
  /** The size last said, and for which "mini" from the app. */
  const sent = useRef<{ data: MiniData | null; key: string }>({ data: null, key: '' });

  // The window shows once the page has drawn: it says its size once after each "mini" from the app, and when it changed.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const w = Math.ceil(el.offsetWidth), h = Math.ceil(el.scrollHeight), key = `${w}x${h}`;
    if (sent.current.key === key && sent.current.data === data) return;
    sent.current = { data, key };
    on.size(w, h);
  });

  // A close that didn't take (Roblox asked something, or hung): its buttons come back after a while.
  useEffect(() => {
    const ids = Object.keys(closing);
    if (!ids.length) return;
    const timer = setTimeout(() => {
      setClosing(c => Object.fromEntries(Object.entries(c).filter(([id, at]) => performance.now() - at < CLOSE_WAIT_MS && data.clients.some(x => x.id === id))));
      tick(n => n + 1);
    }, 1000);
    return () => clearTimeout(timer);
  }, [closing, data.clients]);

  // Esc hides the window, or first takes back the question.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.defaultPrevented) return;
      e.preventDefault();
      if (asking) setAsking(false); else on.act('hide');
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [asking]);

  // The title row moves the window once the pointer has moved a little with the button down (a click stays a click).
  const dragStart = (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as Element).closest('button')) return;
    const x = e.clientX, y = e.clientY;
    const move = (m: PointerEvent) => {
      if (!(m.buttons & 1)) return stop();
      if (Math.abs(m.clientX - x) + Math.abs(m.clientY - y) < DRAG_FROM) return;
      stop();
      on.drag();
    };
    const stop = () => { document.removeEventListener('pointermove', move); document.removeEventListener('pointerup', stop); };
    document.addEventListener('pointermove', move);
    document.addEventListener('pointerup', stop);
  };

  const n = data.clients.length;
  const canCloseAll = data.unlocked && (n > 0 || data.robloxCount > 0);
  const closeAll = () => {
    if (!canCloseAll) return;
    if (data.askCloseAll) { setRemember(false); setAsking(true); } else on.act('closeAll');
  };
  const close = (c: MiniClient) => {
    if (!settled(c.id) || closing[c.id]) return;
    setClosing(x => ({ ...x, [c.id]: performance.now() }));
    on.act('close', { id: c.id });
  };

  return <div ref={root} className="pn mini">
    <div className="pn-title" onPointerDown={dragStart}>
      <img className="mini-logo" src={logo} alt="" />
      <b className="pn-name">Bloxkeep</b>
      {data.unlocked && n ? <span className="mini-count" title={tn('tray.windows', n)}>{n}</span> : null}
      <button type="button" className="pn-icon" title={t('mini.hide')} aria-label={t('mini.hide')} onClick={() => on.act('hide')}><Icon name="x" /></button>
    </div>

    <div className="mini-list" role="list">
      {!data.unlocked
        ? <div className="mini-empty"><Icon name="lock" /><p>{t('mini.locked')}</p></div>
        : !n
          ? <div className="mini-empty"><Icon name="screens" /><p>{t('mini.none')}</p>
            {data.robloxCount ? <span>{tn('tray.openCount', data.robloxCount)}</span> : null}</div>
          : data.clients.map(c => {
            const off = !!closing[c.id];
            return <div key={c.id} className="mini-row" role="listitem" title={`PID ${c.pid}`}>
              <i className={`pn-dot s-${c.state === 'game' ? 'ready' : c.state === 'menu' ? 'idle' : 'stopped'}`}
                aria-label={t(c.state === 'game' ? 'mini.inGame' : c.state === 'menu' ? 'mini.inMenu' : 'mini.dropped')} />
              <span className="mini-text">
                <b>{c.name}</b>
                <span><span className="mini-game">{c.game}</span>{c.ramMb > 0 ? <><i className="pn-sep" aria-hidden="true">·</i><span className="mini-ram">{fmtMb(c.ramMb)}</span></> : null}</span>
              </span>
              <span className="mini-acts">
                <button type="button" className="btn sm" disabled={off} onClick={() => { if (settled(c.id)) on.act('show', { id: c.id }); }}>{t('tray.show')}</button>
                <button type="button" className="btn sm ghost" disabled={off} onClick={() => close(c)}>{t(off ? 'mini.closing' : 'tray.close')}</button>
              </span>
            </div>;
          })}
    </div>

    <div className="pn-foot mini-foot">
      <button type="button" className="btn sm ghost danger" aria-disabled={!canCloseAll || undefined}
        title={canCloseAll ? undefined : t(data.unlocked ? 'mini.none' : 'mini.locked')} onClick={closeAll}>{t('sys.closeAll')}</button>
      <button type="button" className="btn sm mini-open" onClick={() => on.act('open')}>{t('tray.open')}</button>
    </div>
    <div className="mini-grip" title={t('mini.resize')} onPointerDown={e => { if (e.button === 0) { e.preventDefault(); on.resize(); } }}><i /></div>

    {asking ? <div className="mini-ask" role="alertdialog" aria-modal="true" aria-labelledby="mini-ask-q">
      <p id="mini-ask-q">{t('t.closeAllQ')}</p>
      <label className="mini-remember"><input type="checkbox" checked={remember} onChange={e => setRemember(e.target.checked)} /><span>{t('common.dontAsk')}</span></label>
      <div className="mini-ask-btns">
        <button type="button" className="btn sm ghost" onClick={() => setAsking(false)}>{t('common.cancel')}</button>
        <button type="button" className="btn sm danger" autoFocus onClick={() => { setAsking(false); on.act('closeAll', { remember }); }}>{t('t.closeAllOk')}</button>
      </div>
    </div> : null}
  </div>;
}
