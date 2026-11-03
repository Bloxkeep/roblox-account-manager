// The macro panel's controls (PanelView.tsx): a number stepper, a switch, pick one, pick several, a key, a colour, a
// spot on the game, a heading. Each shows the value the app sent and sends what the person does as an "act"; the app
// keeps the maths and sends the result back. The lists open in the page's own popover, kept inside the panel's window:
// below the button, above it when there is more room there, scrolling when neither side fits.
import { createContext, useContext, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { Icon, useHost } from '../host';
import { keyFromEvent, keyName } from '../macros';
import type { PanelOn, PanelOption } from './panelHost';

/** What every row reaches: the app, a hint under a row, and whether something holds the keyboard (typing, a key, a list). */
export interface PanelCtx {
  on: PanelOn;
  /** A changed value (the keyboard may go back to the game once the pointer leaves). */
  changed(): void;
  hint(name: string, text: string, ms?: number): void;
  hints: Record<string, string>;
  /** Something takes the keyboard (Esc doesn't hide the panel, it isn't handed back). */
  hold(key: string, on: boolean): void;
  hasWindow: boolean;
}
export const Ctx = createContext<PanelCtx | null>(null);
const usePanel = () => useContext(Ctx)!;

// ───────────────────────── the popover ─────────────────────────

type Box = { left: number; top: number; right: number; bottom: number; width: number; height: number };
const MAX_ROWS = 8, ROW = 32;

/**
 * A list over the panel, anchored under (or over) a box, as wide as it at least, never past the window's edges: the
 * window is the panel's own and nothing outside it can show. A press outside or the window losing the keyboard closes it.
 */
export function Popover({ at, opener, close, children, className = '', label, role = 'listbox', rows }: {
  at: Box; opener?: HTMLElement | null; close: () => void; children: ReactNode; className?: string; label: string; role?: string; rows?: number;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ left: number; top: number; maxHeight: number; minWidth: number; maxWidth: number } | null>(null);
  useLayoutEffect(() => {
    const el = ref.current!;
    const pad = 6, gap = 4;
    // The panel's window is all there is: in it the card fills the page; in a browser preview, the card's box stands for it.
    const card = document.querySelector('.pn')?.getBoundingClientRect();
    const T = Math.max(0, card?.top ?? 0), B = Math.min(innerHeight, card?.bottom ?? innerHeight);
    const L = Math.max(0, card?.left ?? 0), R = Math.min(innerWidth, card?.right ?? innerWidth);
    const cap = rows === undefined ? Infinity : Math.min(rows, MAX_ROWS) * ROW + 10;
    const want = Math.min(el.scrollHeight, cap);
    const below = B - at.bottom - gap - pad, above = at.top - gap - T - pad;
    const up = want > below && above > below;
    const maxHeight = Math.max(ROW + 10, Math.min(want, up ? above : below));
    const minWidth = Math.min(Math.max(at.width, 150), R - L - pad * 2);
    const width = Math.min(Math.max(el.offsetWidth, minWidth), R - L - pad * 2);
    const left = Math.max(L + pad, Math.min(at.left, R - pad - width));
    const top = up ? Math.max(T + pad, at.top - gap - maxHeight) : Math.min(at.bottom + gap, B - pad - maxHeight);
    setPos({ left, top, maxHeight, minWidth, maxWidth: R - L - pad * 2 });
  }, [at.left, at.top, at.bottom, at.width]);
  useEffect(() => {
    const down = (e: PointerEvent) => {
      const n = e.target as Node;
      if (!ref.current?.contains(n) && !opener?.contains(n)) close();
    };
    const blur = () => close();
    document.addEventListener('pointerdown', down, true);
    window.addEventListener('blur', blur);
    return () => { document.removeEventListener('pointerdown', down, true); window.removeEventListener('blur', blur); };
  }, [close, opener]);
  return createPortal(<div ref={ref} className={`menu pn-pop ${className}`} role={role} aria-label={label}
    style={pos ? { left: pos.left, top: pos.top, maxHeight: pos.maxHeight, minWidth: pos.minWidth, maxWidth: pos.maxWidth } : { left: 0, top: 0, visibility: 'hidden' }}>
    {children}</div>, document.getElementById('panel')?.parentElement ?? document.body);
}

const boxOf = (el: Element): Box => {
  const r = el.getBoundingClientRect();
  return { left: r.left, top: r.top, right: r.right, bottom: r.bottom, width: r.width, height: r.height };
};

/** Up/Down, Home/End, typing a name's first letters: the row to go to in a list. */
function useListKeys(count: number, start: number, names: string[]) {
  const [active, setActive] = useState(start);
  const typed = useRef({ text: '', at: 0 });
  const key = (e: React.KeyboardEvent): boolean => {
    const k = e.key;
    if (k === 'ArrowDown') setActive(i => (i + 1) % count);
    else if (k === 'ArrowUp') setActive(i => (i - 1 + count) % count);
    else if (k === 'Home') setActive(0);
    else if (k === 'End') setActive(count - 1);
    else if (k.length === 1 && !e.ctrlKey && !e.altKey && !e.metaKey && k !== ' ') {
      const now = Date.now();
      typed.current = { text: (now - typed.current.at < 700 ? typed.current.text : '') + k.toLowerCase(), at: now };
      const find = (from: number) => names.findIndex((n, i) => i >= from && n.toLowerCase().startsWith(typed.current.text));
      const i = find(0);
      if (i >= 0) setActive(i);
    } else return false;
    e.preventDefault();
    return true;
  };
  return { active, setActive, key };
}

/** The list's row with the keyboard on it stays in view. */
function useScrollActive(list: React.RefObject<HTMLElement | null>, active: number) {
  useLayoutEffect(() => {
    (list.current?.querySelectorAll('[data-row]')[active] as HTMLElement | undefined)?.scrollIntoView({ block: 'nearest' });
  }, [active]);
}

// ───────────────────────── pick one, pick several ─────────────────────────

/** A button showing the choice, opening its list (≤ 8 rows, then scrolling); a second click closes it; Esc changes nothing. */
export function PickOne({ o, id }: { o: PanelOption; id: string }) {
  const { on, changed, hold } = usePanel();
  const btn = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const choices = o.choices || [];
  const now = typeof o.value === 'string' ? o.value : '';
  const close = (focus = true) => { setOpen(false); hold('list', false); if (focus) btn.current?.focus({ preventScroll: true }); };
  const pick = (c: string) => {
    close();
    if (c !== now) { on.act('set', { name: o.name, value: c }); changed(); }
  };
  return <>
    <button type="button" ref={btn} id={id} className={`pn-select${open ? ' open' : ''}`} aria-haspopup="listbox" aria-expanded={open}
      title={now} onClick={() => { if (open) close(); else { setOpen(true); hold('list', true); } }}>
      <span>{now || '—'}</span><Icon name="chevronDown" /></button>
    {open && btn.current ? <OneList choices={choices} now={now} at={boxOf(btn.current)} opener={btn.current} label={o.label}
      pick={pick} close={() => close()} /> : null}
  </>;
}

function OneList({ choices, now, at, opener, label, pick, close }: {
  choices: string[]; now: string; at: Box; opener: HTMLElement; label: string; pick: (c: string) => void; close: () => void;
}) {
  const list = useRef<HTMLDivElement>(null);
  const { active, setActive, key } = useListKeys(choices.length, Math.max(0, choices.indexOf(now)), choices);
  useScrollActive(list, active);
  useEffect(() => { list.current?.focus({ preventScroll: true }); }, []);
  return <Popover at={at} opener={opener} close={close} label={label} rows={choices.length}>
    <div ref={list} className="pn-list" tabIndex={-1} aria-activedescendant={`pn-o-${active}`}
      onKeyDown={e => {
        if (key(e)) return;
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); pick(choices[active]); }
        else if (e.key === 'Escape' || e.key === 'Tab') { e.preventDefault(); e.stopPropagation(); close(); }
      }}>
      {choices.map((c, i) => <button type="button" key={c} id={`pn-o-${i}`} data-row="" role="option" aria-selected={c === now} tabIndex={-1}
        className={`${i === active ? 'on' : ''}${c === now ? ' checked' : ''}`} onPointerMove={() => setActive(i)} onClick={() => pick(c)}>
        <span title={c}>{c}</span>{c === now ? <Icon name="check" /> : null}</button>)}
    </div>
  </Popover>;
}

/** Ticks several: "All" and "None" first; the list stays open while ticking and the choice is sent as it closes. */
export function PickSeveral({ o, id }: { o: PanelOption; id: string }) {
  const { t } = useHost();
  const { on, changed, hold } = usePanel();
  const btn = useRef<HTMLButtonElement>(null);
  const choices = o.choices || [];
  const sent = Array.isArray(o.value) ? o.value : [];
  const [open, setOpen] = useState(false);
  const [ticked, setTicked] = useState<string[]>(sent);
  useEffect(() => { if (!open) setTicked(sent); }, [JSON.stringify(sent), open]);
  const close = (focus = true) => {
    setOpen(false);
    hold('list', false);
    if (focus) btn.current?.focus({ preventScroll: true });
    const now = choices.filter(c => ticked.includes(c));
    if (now.join('\n') !== choices.filter(c => sent.includes(c)).join('\n')) { on.act('set', { name: o.name, value: now }); changed(); }
  };
  const shown = open ? ticked : sent;
  const count = t('panel.of', { n: choices.filter(c => shown.includes(c)).length, all: choices.length });
  return <>
    <button type="button" ref={btn} id={id} className={`pn-select${open ? ' open' : ''}`} aria-haspopup="listbox" aria-expanded={open}
      title={choices.filter(c => shown.includes(c)).join(', ')} onClick={() => { if (open) close(); else { setOpen(true); hold('list', true); } }}>
      <span>{count}</span><Icon name="chevronDown" /></button>
    {open && btn.current ? <ManyList choices={choices} ticked={ticked} setTicked={setTicked} at={boxOf(btn.current)} opener={btn.current}
      label={o.label} close={() => close()} /> : null}
  </>;
}

function ManyList({ choices, ticked, setTicked, at, opener, label, close }: {
  choices: string[]; ticked: string[]; setTicked: (f: (x: string[]) => string[]) => void; at: Box; opener: HTMLElement; label: string; close: () => void;
}) {
  const { t } = useHost();
  const list = useRef<HTMLDivElement>(null);
  const rows = [t('panel.all'), t('panel.none'), ...choices];
  const { active, setActive, key } = useListKeys(rows.length, 2, rows);
  useScrollActive(list, active);
  useEffect(() => { list.current?.focus({ preventScroll: true }); }, []);
  const flip = (i: number) => {
    if (i === 0) setTicked(() => [...choices]);
    else if (i === 1) setTicked(() => []);
    else { const c = choices[i - 2]; setTicked(x => x.includes(c) ? x.filter(y => y !== c) : [...x, c]); }
  };
  return <Popover at={at} opener={opener} close={close} label={label} rows={rows.length + 1}>
    <div ref={list} className="pn-list" tabIndex={-1} role="listbox" aria-multiselectable="true" aria-activedescendant={`pn-m-${active}`}
      onKeyDown={e => {
        if (key(e)) return;
        if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); flip(active); }
        else if (e.key === 'Escape' || e.key === 'Tab') { e.preventDefault(); e.stopPropagation(); close(); }
      }}>
      {rows.map((r, i) => {
        const tick = i >= 2 ? ticked.includes(r) : null;
        return <div key={`${i}:${r}`} className="pn-list-row" data-row="">
          {i === 2 ? <hr /> : null}
          <button type="button" id={`pn-m-${i}`} role="option" aria-selected={tick ?? undefined} tabIndex={-1} className={i === active ? 'on' : ''}
            onPointerMove={() => setActive(i)} onClick={() => flip(i)}>
            {tick === null ? <Icon name={i === 0 ? 'check' : 'x'} className="pn-all" />
              : <input type="checkbox" checked={tick} readOnly tabIndex={-1} aria-hidden="true" />}
            <span title={r}>{r}</span></button>
        </div>;
      })}
    </div>
  </Popover>;
}

/** A small menu at a box (the title row's right-click). */
export function MenuAt({ at, close, items, label }: { at: Box; close: () => void; label: string; items: { text: string; icon: string; run: () => void }[] }) {
  const list = useRef<HTMLDivElement>(null);
  const { active, setActive, key } = useListKeys(items.length, 0, items.map(i => i.text));
  useEffect(() => { list.current?.focus({ preventScroll: true }); }, []);
  return <Popover at={at} close={close} label={label} role="menu" className="pn-menu">
    <div ref={list} className="pn-list" tabIndex={-1} onKeyDown={e => {
      if (key(e)) return;
      if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); close(); items[active].run(); }
      else if (e.key === 'Escape' || e.key === 'Tab') { e.preventDefault(); e.stopPropagation(); close(); }
    }}>
      {items.map((it, i) => <button type="button" key={it.text} role="menuitem" data-row="" tabIndex={-1} className={i === active ? 'on' : ''}
        onPointerMove={() => setActive(i)} onClick={() => { close(); it.run(); }}><Icon name={it.icon} /><span>{it.text}</span></button>)}
    </div>
  </Popover>;
}

// ───────────────────────── a number ─────────────────────────

const REPEAT_AFTER = 400, REPEAT_EVERY = 40, WHEEL_REST = 300;

/**
 * [−][ value unit ][+]. The value is typed (Enter or leaving keeps it, Esc puts it back). Holding − or + repeats (after
 * 0.4 s, every 40 ms) and the wheel moves it a step a notch; meanwhile the value shown here moves at once and the app is
 * told once, with the steps added up, as the button is let go (or the wheel rests). Shift: ten steps.
 */
export function Stepper({ o, id, label }: { o: PanelOption; id: string; label: string }) {
  const { t, locale } = useHost();
  const { on, changed, hold } = usePanel();
  const raw = typeof o.value === 'number' ? o.value : 0;
  const step = o.step || 1, scale = o.scale || 1, min = o.min ?? -Infinity, max = o.max ?? Infinity;
  // Steps moved here and not yet answered by the app.
  const [moved, setMoved] = useState(0);
  const movedRef = useRef(0);
  const [text, setText] = useState<string | null>(null);
  const timers = useRef<{ repeat?: number; wheel?: number }>({});
  // The app's answer replaces what was shown meanwhile.
  useEffect(() => { movedRef.current = 0; setMoved(0); }, [raw, o.shown]);
  const clamp = (v: number) => Math.min(max, Math.max(min, v));
  const valueAt = (by: number) => clamp(Math.round((raw + by * step) / step) * step);
  const fmt = (v: number) => (v / scale).toLocaleString(locale, { maximumFractionDigits: 2, useGrouping: false }) + (o.suffix || '');
  const shown = moved ? fmt(valueAt(moved)) : o.shown;
  const now = moved ? valueAt(moved) : raw;
  const send = () => {
    const by = movedRef.current;
    if (!by) return;
    on.act('nudge', { name: o.name, by });
    changed();
  };
  const move = (by: number) => {
    const next = movedRef.current + by;
    if (valueAt(next) === valueAt(movedRef.current)) return false; // at its end
    movedRef.current = next;
    setMoved(next);
    return true;
  };
  const stop = () => {
    clearTimeout(timers.current.repeat);
    clearInterval(timers.current.repeat);
    timers.current.repeat = undefined;
  };
  const press = (dir: number, shift: boolean) => {
    const by = dir * (shift ? 10 : 1);
    move(by);
    stop();
    timers.current.repeat = window.setTimeout(() => {
      timers.current.repeat = window.setInterval(() => { if (!move(by)) stop(); }, REPEAT_EVERY);
    }, REPEAT_AFTER);
  };
  const release = () => { if (timers.current.repeat === undefined && !movedRef.current) return; stop(); send(); };
  useEffect(() => () => { stop(); clearTimeout(timers.current.wheel); }, []);
  const range = o.minShown && o.maxShown
    ? t('panel.range', { min: o.suffix && o.minShown.endsWith(o.suffix) ? o.minShown.slice(0, -o.suffix.length) : o.minShown, max: o.maxShown }) : undefined;
  const btn = (dir: number) => <button type="button" className="pn-step" tabIndex={-1}
    aria-label={`${dir < 0 ? '−' : '+'} ${label}`} disabled={dir < 0 ? now <= min : now >= max}
    onPointerDown={e => { if (e.button === 0) { e.currentTarget.setPointerCapture(e.pointerId); press(dir, e.shiftKey); } }}
    onPointerUp={release} onPointerCancel={release} onLostPointerCapture={release}
    onKeyDown={e => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); move(dir * (e.shiftKey ? 10 : 1)); send(); } }}>
    {dir < 0 ? <svg className="i" viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14" /></svg> : <Icon name="plus" />}</button>;
  return <span className="pn-stepper" title={range}
    onWheel={e => {
      if (text !== null) return;
      move(e.deltaY < 0 ? (e.shiftKey ? 10 : 1) : (e.shiftKey ? -10 : -1));
      clearTimeout(timers.current.wheel);
      timers.current.wheel = window.setTimeout(send, WHEEL_REST);
    }}>
    {btn(-1)}
    <input id={id} className="pn-num" inputMode="decimal" spellCheck={false} autoComplete="off" aria-label={label} title={range}
      value={text ?? shown}
      onFocus={e => { setText(shown); hold('type', true); requestAnimationFrame(() => e.target.select()); }}
      onChange={e => setText(e.target.value)}
      onBlur={() => {
        hold('type', false);
        if (text !== null && text.trim() !== shown.trim()) { on.act('typed', { name: o.name, text }); changed(); }
        setText(null);
      }}
      onKeyDown={e => {
        if (e.key === 'Enter') { e.preventDefault(); e.currentTarget.blur(); }
        else if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); setText(null); hold('type', false); setText(shown); requestAnimationFrame(() => { setText(null); (e.target as HTMLInputElement).blur(); }); }
        else if (e.key === 'ArrowUp' || e.key === 'ArrowDown') { e.preventDefault(); setText(null); move(e.key === 'ArrowUp' ? 1 : -1); clearTimeout(timers.current.wheel); timers.current.wheel = window.setTimeout(send, WHEEL_REST); }
      }} />
    {btn(1)}
  </span>;
}

// ───────────────────────── a key, a colour, a spot, a switch ─────────────────────────

/** The key in a cap and Change; while it waits the cap says "Press a key…"; Esc or leaving it changes nothing. */
export function KeyField({ o, id, label }: { o: PanelOption; id: string; label: string }) {
  const { t } = useHost();
  const { on, changed, hint, hold } = usePanel();
  const [wait, setWait] = useState(false);
  const set = (w: boolean) => { setWait(w); hold('key', w); };
  const key = typeof o.value === 'string' ? o.value : '';
  return <>
    <kbd className={`pn-key${wait ? ' wait' : ''}`} aria-live="polite">{wait ? t('panel.pressKey') : keyName(t, key)}</kbd>
    <button type="button" id={id} className={`btn sm${wait ? ' on' : ''}`} aria-pressed={wait} aria-label={`${t('panel.change')}: ${label}`}
      onClick={() => set(!wait)} onBlur={() => set(false)}
      onKeyDown={e => {
        if (!wait) return;
        if (e.key === 'Tab') return set(false);
        e.preventDefault();
        e.stopPropagation();
        if (e.key === 'Escape') return set(false);
        const k = keyFromEvent(e);
        if (!k) { hint(o.name, t('panel.badKey'), 3000); return; }
        set(false);
        if (k !== key) { on.act('set', { name: o.name, key: k }); changed(); }
      }}>{t('panel.change')}</button>
  </>;
}

/** The colour's swatch (not a button), its hex, and Pick (on the game; on the screen when no game is open). */
export function ColourField({ o, id, label }: { o: PanelOption; id: string; label: string }) {
  const { t } = useHost();
  const { on, changed } = usePanel();
  const hex = typeof o.value === 'string' ? o.value : '#ffffff';
  return <>
    <i className="pn-swatch" style={{ background: hex }} aria-hidden="true" />
    <span className="pn-hex">{hex}</span>
    <button type="button" id={id} className="btn sm" aria-label={`${t('panel.pick')}: ${label}`}
      onClick={() => { on.act('pickColour', { name: o.name }); changed(); }}>{t('panel.pick')}</button>
  </>;
}

/** Where on the game: a small map with the dot, the place in percent, and Pick spot (needs a game window). */
export function SpotField({ o, id, label }: { o: PanelOption; id: string; label: string }) {
  const { t, locale } = useHost();
  const { on, changed, hasWindow } = usePanel();
  const p = o.value && typeof o.value === 'object' && !Array.isArray(o.value) ? o.value : { x: 0.5, y: 0.5 };
  const pct = (v: number) => `${Math.round(v * 100).toLocaleString(locale)} %`;
  return <>
    <span className="pn-map" aria-hidden="true"><i style={{ left: `${p.x * 100}%`, top: `${p.y * 100}%` }} /></span>
    <span className="pn-hex">{pct(p.x)}, {pct(p.y)}</span>
    <button type="button" id={id} className="btn sm" aria-disabled={!hasWindow || undefined} aria-label={`${t('panel.pickSpot')}: ${label}`}
      title={hasWindow ? undefined : t('panel.openGame')}
      onClick={() => { if (hasWindow) { on.act('pickPoint', { name: o.name }); changed(); } }}>{t('panel.pickSpot')}</button>
  </>;
}

export function SwitchField({ o, id, label }: { o: PanelOption; id: string; label: string }) {
  const { on, changed } = usePanel();
  const value = typeof o.value === 'number' && o.value !== 0;
  return <label className="switch"><input type="checkbox" id={id} checked={value} aria-label={label}
    onChange={e => { on.act('set', { name: o.name, value: e.target.checked ? 1 : 0 }); changed(); }} /><span /></label>;
}

// ───────────────────────── a row ─────────────────────────

/** One setting: its label (cut with … and whole in its title) left, its control right; a control wider than 55 % of the
 * row goes under the label at full width. A refused change says why under it for a few seconds. */
export function Row({ o, macroId }: { o: PanelOption; macroId: string }) {
  const { hints } = usePanel();
  const row = useRef<HTMLDivElement>(null);
  const inner = useRef<HTMLSpanElement>(null);
  const [stack, setStack] = useState(false);
  const id = `pn-${macroId.slice(0, 8)}-${o.name}`;
  const flex = o.kind === 'choice' || o.kind === 'list';
  useLayoutEffect(() => {
    if (flex || !row.current || !inner.current) return;
    const fits = inner.current.offsetWidth <= row.current.clientWidth * 0.55 + 0.5;
    if (fits === stack) setStack(!fits);
  });
  if (o.kind === 'heading') return <h4 className="pn-head" title={o.label}>{o.label}</h4>;
  const control = o.kind === 'number' ? <Stepper o={o} id={id} label={o.label} />
    : o.kind === 'check' ? <SwitchField o={o} id={id} label={o.label} />
    : o.kind === 'choice' ? <PickOne o={o} id={id} />
    : o.kind === 'list' ? <PickSeveral o={o} id={id} />
    : o.kind === 'key' ? <KeyField o={o} id={id} label={o.label} />
    : o.kind === 'colour' ? <ColourField o={o} id={id} label={o.label} />
    : o.kind === 'point' ? <SpotField o={o} id={id} label={o.label} />
    : <span className="pn-hex">{o.shown}</span>;
  const hint = hints[o.name];
  return <div ref={row} className={`pn-row k-${o.kind}${stack ? ' stack' : ''}`}>
    <label className="pn-label" htmlFor={id} title={o.label}>{o.label}</label>
    <span className={`pn-ctl${flex ? ' flex' : ''}`}><span ref={inner} className="pn-ctl-in">{control}</span></span>
    {hint ? <p className="pn-hint" role="alert">{hint}</p> : null}
  </div>;
}
