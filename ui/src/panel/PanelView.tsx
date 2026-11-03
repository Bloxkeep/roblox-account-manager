// A macro's panel beside the game (PanelWindow.cs draws panel.html in a small window of its own): its name, Run/Stop with
// the hotkey, one status line with the counters, its tabs, the settings the maker let the player change (PanelControls),
// Move areas and Reset all. Folded, a pill: the dot, the name, Run/Stop. Until 2026-11 it was hand-drawn WinForms.
// The app keeps every value and its maths; what is clicked goes to it as an "act" and comes back in the next "panel".
import { useCallback, useEffect, useImperativeHandle, useLayoutEffect, useMemo, useRef, useState, type Ref } from 'react';
import { Icon, useHost } from '../host';
import { Ctx, MenuAt, Row, type PanelCtx } from './PanelControls';
import { elapsed, type PanelData, type PanelError, type PanelMacro, type PanelNow, type PanelOn, type PanelViewApi } from './panelHost';

const ERROR_FOR = 5000, RESET_FOR = 6000, AWAY_AFTER = 400, DRAG_FROM = 3;

/** What panel.js holds: the app's latest "panel" event drawn, a refused change said. */
export function PanelRoot({ on, handle }: { on: PanelOn; handle: Ref<PanelViewApi> }) {
  const [data, setData] = useState<PanelData | null>(null);
  const [error, setError] = useState<{ e: PanelError; n: number } | null>(null);
  const count = useRef(0);
  useImperativeHandle(handle, () => ({
    show: d => setData(d),
    error: e => setError({ e, n: ++count.current }),
  }), []);
  if (!data?.macro || !data.now) return null;
  return <PanelView key={data.macro.id} model={data.macro} now={data.now} pill={data.pill} error={error} mode="live" on={on} />;
}

const tabKey = (id: string) => `bk.panel.tab.${id}`;
function savedTab(id: string): string | null {
  try { return localStorage.getItem(tabKey(id)); } catch { return null; }
}
function saveTab(id: string, tab: string) {
  try { localStorage.setItem(tabKey(id), tab); } catch { /* storage off: the first tab next time */ }
}

export function PanelView({ model, now, pill: pillSent, error, on }: {
  model: PanelMacro; now: PanelNow; pill: boolean; error: { e: PanelError; n: number } | null; mode: 'live'; on: PanelOn;
}) {
  const { t } = useHost();
  const [pill, setPill] = useState(pillSent);
  const [hints, setHints] = useState<Record<string, string>>({});
  const [status, setStatus] = useState<{ text: string; kind: 'error' | 'reset' } | null>(null);
  const [menu, setMenu] = useState<{ x: number; y: number } | null>(null);
  const tabs = model.tabs.length >= 2 ? model.tabs : [];
  const [tab, setTabNow] = useState(() => { const s = savedTab(model.id); return s && tabs.includes(s) ? s : tabs[0] ?? null; });
  const shownTab = tab && tabs.includes(tab) ? tab : tabs[0] ?? null;
  const setTab = (x: string) => { setTabNow(x); saveTab(model.id, x); };

  const root = useRef<HTMLDivElement>(null);
  const rows = useRef<HTMLDivElement>(null);
  const timers = useRef<Record<string, number>>({});
  const holds = useRef(new Set<string>());
  const dirty = useRef(false);
  const away = useRef<number | undefined>(undefined);
  /** The size last said, and for which "panel" from the app: said again only when it changed, or once per new one. */
  const sent = useRef<{ now: PanelNow | null; key: string }>({ now: null, key: '' });

  const later = (key: string, ms: number, fn: () => void) => {
    clearTimeout(timers.current[key]);
    timers.current[key] = window.setTimeout(fn, ms);
  };
  useEffect(() => () => { Object.values(timers.current).forEach(clearTimeout); clearTimeout(away.current); }, []);

  const hint = useCallback((name: string, text: string, ms = ERROR_FOR) => {
    setHints(h => ({ ...h, [name]: text }));
    later(`h:${name}`, ms, () => setHints(h => { const { [name]: _, ...rest } = h; return rest; }));
  }, []);
  const say = (text: string, kind: 'error' | 'reset', ms: number) => {
    setStatus({ text, kind });
    later('status', ms, () => setStatus(null));
  };
  // A refused change, under its row (or in the status line).
  useEffect(() => {
    if (!error) return;
    if (error.e.name) hint(error.e.name, error.e.text);
    else say(error.e.text, 'error', ERROR_FOR);
  }, [error?.n]);

  const ctx: PanelCtx = useMemo(() => ({
    on,
    changed: () => { dirty.current = true; },
    hint,
    hints,
    hold: (key, h) => { if (h) holds.current.add(key); else holds.current.delete(key); },
    hasWindow: now.hasWindow,
  }), [on, hint, hints, now.hasWindow]);

  // The window takes the page's size: its natural height, the rows not scrolled (the app caps it at the screen).
  // Said when it changed, and once after each "panel" from the app (the window shows on that answer); not on every drawing.
  useLayoutEffect(() => {
    const el = root.current;
    if (!el) return;
    const report = () => {
      const r = rows.current;
      const w = Math.ceil(el.offsetWidth), h = Math.ceil(el.offsetHeight + (r ? r.scrollHeight - r.clientHeight : 0));
      const key = `${w}x${h}${pill ? ' pill' : ''}`;
      if (sent.current.key === key && sent.current.now === now) return;
      sent.current = { now, key };
      on.size(w, h, pill);
    };
    report();
    const watch = new ResizeObserver(report);
    watch.observe(el);
    if (rows.current?.firstElementChild) watch.observe(rows.current.firstElementChild);
    return () => watch.disconnect();
  });

  // Esc hides it, unless a number is being typed, a key waited for or a list open (those take Esc first).
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' || e.defaultPrevented || holds.current.size || menu) return;
      e.preventDefault();
      if (now.editingAreas) on.act('areas'); else on.act('hide');
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [menu, now.editingAreas]);

  // After a change, the pointer leaving the panel hands the keyboard back to the game (the app checks it is still here).
  useEffect(() => {
    const leave = () => {
      clearTimeout(away.current);
      if (!dirty.current) return;
      away.current = window.setTimeout(() => {
        if (holds.current.size || menu) return;
        dirty.current = false;
        on.act('away');
      }, AWAY_AFTER);
    };
    const enter = () => clearTimeout(away.current);
    document.documentElement.addEventListener('pointerleave', leave);
    document.documentElement.addEventListener('pointerenter', enter);
    return () => { document.documentElement.removeEventListener('pointerleave', leave); document.documentElement.removeEventListener('pointerenter', enter); };
  }, [menu]);

  // The title row moves the window once the pointer has moved a little with the button down (a click stays a click).
  const dragStart = (e: React.PointerEvent) => {
    if (e.button !== 0 || (e.target as Element).closest('button, a, input')) return;
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
  const fold = (to: boolean) => { setPill(to); setMenu(null); };

  const running = now.state === 'running' || now.state === 'paused';
  const blocked = now.state === 'recording' ? t('panel.recordingWait') : now.state === 'noGame' ? t('panel.noGame') : undefined;
  const run = () => {
    if (running) on.act('stop');
    else if (!blocked) on.act('run');
  };
  const hotkey = now.hotkeysOn && model.hotkey !== null ? `Ctrl+Alt+${model.hotkey}` : null;
  const dot = <i className={`pn-dot s-${now.editingAreas ? 'editing' : now.state}`} aria-hidden="true" />;
  const titleMenu = (x: number, y: number) => setMenu({ x, y });

  if (pill) {
    return <div ref={root} className="pn pn-pill" onPointerDown={dragStart} onDoubleClick={e => { if (!(e.target as Element).closest('button')) fold(false); }}
      onContextMenu={e => { e.preventDefault(); titleMenu(e.clientX, e.clientY); }}>
      {dot}
      <b className="pn-name" title={model.name}>{model.name}</b>
      <button type="button" className={`btn sm${running ? ' danger' : ' go'}`} aria-disabled={!running && blocked ? true : undefined} title={running ? undefined : blocked}
        onClick={run}><Icon name={running ? 'stop' : 'play'} />{t(running ? 'macro.stop' : 'macro.run')}</button>
      <button type="button" className="pn-icon" title={t('panel.unfold')} aria-label={t('panel.unfold')} onClick={() => fold(false)}><Icon name="chevronDown" /></button>
      {menu ? <TitleMenu at={menu} close={() => setMenu(null)} on={on} lastStep={now.lastError?.step} /> : null}
    </div>;
  }

  const options = tabs.length ? model.options.filter(o => (o.tab && tabs.includes(o.tab) ? o.tab : tabs[0]) === shownTab) : model.options;
  const counters = model.counters.filter(c => c.name in now.counters);
  const fix = now.state === 'stopped' && now.lastError
    ? <button type="button" className="pn-link" onClick={() => on.act('edit', { step: now.lastError!.step })}>{t('panel.fix')}</button> : null;

  return <Ctx.Provider value={ctx}>
    <div ref={root} className={`pn s-${now.state}`}>
      <div className="pn-title" onPointerDown={dragStart} onContextMenu={e => { e.preventDefault(); titleMenu(e.clientX, e.clientY); }}>
        {dot}
        <b className="pn-name" title={model.name}>{model.name}</b>
        <span className="pn-beta" title={t('macro.betaTip')}>{t('panel.beta')}</span>
        <button type="button" className="pn-icon" title={t('panel.fold')} aria-label={t('panel.fold')} onClick={() => fold(true)}><Icon name="chevronUp" /></button>
        <button type="button" className="pn-icon" title={t('panel.hide')} aria-label={t('panel.hide')} onClick={() => on.act('hide')}><Icon name="x" /></button>
      </div>

      <div className="pn-top">
        <button type="button" className={`btn pn-run${running ? ' danger' : ' go'}`} aria-disabled={!running && blocked ? true : undefined}
          title={running ? undefined : blocked} onClick={run}>
          <Icon name={running ? 'stop' : 'play'} /><span>{t(running ? 'macro.stop' : 'macro.run')}</span>
          {hotkey ? <kbd>{hotkey}</kbd> : null}
        </button>
        <p className={`pn-status${status?.kind === 'error' ? ' bad' : ''}`} aria-live="polite">
          {status ? <>{status.kind === 'reset' ? <><span>{status.text}</span>{now.undoable ? <><i className="pn-sep" aria-hidden="true">·</i><button type="button" className="pn-link"
            onClick={() => { on.act('undoReset'); setStatus(null); }}>{t('panel.undo')}</button></> : null}</> : <span>{status.text}</span>}</>
            : <><span className="pn-say" title={statusText(t, now)}>{statusText(t, now)}</span>{fix ? <><i className="pn-sep" aria-hidden="true">·</i>{fix}</> : null}</>}
        </p>
        {counters.length || (running && now.startedAt) ? <p className="pn-chips">
          {counters.map(c => <span key={c.name} className="pn-chip"><span>{c.label}</span><b>{now.counters[c.name]}</b></span>)}
          {running && now.startedAt ? <span className="pn-chip clock"><Icon name="clock" /><Elapsed since={now.startedAt} /></span> : null}
        </p> : null}
        {tabs.length ? <Tabs tabs={tabs} tab={shownTab!} setTab={setTab} /> : null}
      </div>

      <div ref={rows} className="pn-rows">
        <div className="pn-rows-in">
          {options.map(o => <Row key={o.name} o={o} macroId={model.id} />)}
        </div>
      </div>

      {model.areasCount > 0 || model.options.some(o => o.kind !== 'heading') ? <div className="pn-foot">
        {model.areasCount > 0 ? <button type="button" className={`btn sm${now.editingAreas ? ' primary' : ''}`}
          aria-disabled={!now.editingAreas && !now.hasWindow ? true : undefined} title={!now.editingAreas && !now.hasWindow ? t('panel.openGame') : undefined}
          onClick={() => { if (now.editingAreas || now.hasWindow) on.act('areas'); }}>{t(now.editingAreas ? 'panel.editingStop' : 'panel.areas')}</button> : null}
        {model.options.some(o => o.kind !== 'heading') ? <button type="button" className="btn sm ghost pn-reset" aria-disabled={!now.changed || undefined}
          title={now.changed ? undefined : t('panel.resetNone')}
          onClick={() => { if (!now.changed) return; on.act('reset'); say(t('panel.resetDone'), 'reset', RESET_FOR); }}>{t('panel.reset')}</button> : null}
      </div> : null}

      {menu ? <TitleMenu at={menu} close={() => setMenu(null)} on={on} lastStep={undefined} /> : null}
    </div>
  </Ctx.Provider>;
}

/** The clock of a run going on, ticked here (the app sends when it started): only this redraws each second. */
function Elapsed({ since }: { since: string }) {
  const [clock, setClock] = useState(() => Date.now());
  useEffect(() => {
    const timer = setInterval(() => setClock(Date.now()), 1000);
    return () => clearInterval(timer);
  }, []);
  return <b>{elapsed(since, clock)}</b>;
}

/** The status line: what the macro does now. */
function statusText(t: (k: string, v?: Record<string, string | number>) => string, now: PanelNow): string {
  if (now.state === 'recording') return t('panel.recording', { account: now.recording?.account ?? '' });
  if (now.editingAreas) return t('panel.editing');
  if (now.state === 'running' || now.state === 'paused') {
    const first = now.runs[0];
    if (!first) return t('panel.ready');
    const one = first.status === 'paused' ? t('panel.paused', { name: first.name, reason: first.reason ?? '' })
      : first.status === 'waiting' ? t('panel.waits', { name: first.name })
      : first.status === 'recovering' ? t('panel.unstuck', { name: first.name })
      : t('panel.runs', { name: first.name });
    return now.runs.length > 1 ? `${one} · ${t('panel.more', { n: now.runs.length - 1 })}` : one;
  }
  if (now.state === 'stopped') return t('panel.stopped', { reason: now.lastError?.text ?? '' });
  if (now.state === 'noGame') return t('panel.noGame');
  return t('panel.ready');
}

/** The tabs as the app's segmented control: the arrows move between them. */
function Tabs({ tabs, tab, setTab }: { tabs: string[]; tab: string; setTab: (x: string) => void }) {
  const box = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => { (box.current?.querySelector('.on') as HTMLElement | null)?.scrollIntoView({ block: 'nearest', inline: 'nearest' }); }, [tab]);
  return <div ref={box} className="seg pn-tabs" role="tablist" onKeyDown={e => {
    const i = tabs.indexOf(tab);
    const to = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? tabs.length - 1 : -2;
    if (to === -2) return;
    e.preventDefault();
    const next = tabs[(to + tabs.length) % tabs.length];
    setTab(next);
    requestAnimationFrame(() => (box.current?.querySelector('.on') as HTMLElement | null)?.focus());
  }}>
    {tabs.map(x => <button type="button" key={x} role="tab" aria-selected={x === tab} tabIndex={x === tab ? 0 : -1} className={x === tab ? 'on' : ''}
      title={x} onClick={() => setTab(x)}>{x}</button>)}
  </div>;
}

function TitleMenu({ at, close, on, lastStep }: { at: { x: number; y: number }; close: () => void; on: PanelOn; lastStep: number | undefined }) {
  const { t } = useHost();
  return <MenuAt at={{ left: at.x, top: at.y, right: at.x, bottom: at.y, width: 0, height: 0 }} close={close} label={t('panel.editMacro')} items={[
    { text: t('panel.snap'), icon: 'screens', run: () => on.act('snap') },
    { text: t('panel.editMacro'), icon: 'edit', run: () => on.act('edit', lastStep === undefined ? {} : { step: lastStep }) },
    { text: t('panel.hide'), icon: 'eyeOff', run: () => on.act('hide') },
  ]} />;
}
