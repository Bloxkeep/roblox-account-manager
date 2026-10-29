// The tray icon's quick panel (left click) and menu (right click), in the app's own look in a small window of their own
// (TrayWindow.cs, tray.html); until 2026-10-26 they were hand-drawn WinForms, which the owner found ugly. Whatever is
// clicked goes to the app (api.act); the window goes away by itself once something else is clicked.
import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { Avatar } from './Avatar';
import { Icon, useHost } from './host';

/** What the app sends each time the tray opens, and every two seconds while it is open (AppController.TrayState). */
export interface TrayState {
  open: boolean;
  multi: 'off' | 'ready' | 'notReady';
  multiOnly: boolean;
  robloxCount: number;
  ramMb: number;
  miniOpen: boolean;
  presets: { id: string; name: string }[];
  accounts: { id: string; name: string; running: boolean; avatar: string | null }[];
  clients: { id: string; name: string; game: string; pid: number; ramMb: number; avatar: string | null }[];
}
export interface TrayApi {
  /** A command: open, mini, multi ({ on }), account / preset ({ id }), newRoblox, closeAll, show ({ pid }), closeClient ({ id }), quit, hide. */
  act(what: string, args?: Record<string, unknown>): void;
  /** The page's size in its own pixels, for the window around it, and which opening it has drawn. */
  size(width: number, height: number, opened: number): void;
}

/** What tray.js holds: show draws the given part with the given state (opened: a new count each time it opens). */
export interface TrayViewApi { show(mode: 'panel' | 'menu', state: TrayState, opened: number): void }

export function TrayView({ api, handle }: { api: TrayApi; handle: Ref<TrayViewApi> }) {
  const [now, setNow] = useState<{ mode: 'panel' | 'menu'; state: TrayState; opened: number } | null>(null);
  useImperativeHandle(handle, () => ({ show: (mode, state, opened) => setNow({ mode, state, opened }) }), []);
  return now ? <Tray {...now} api={api} /> : null;
}

const MAX_CLIENTS = 6;
const ram = (mb: number) => (mb >= 1024 ? `${(mb / 1024).toFixed(1)} GB` : `${Math.max(0, mb)} MB`);

/** Switched from here: with Multi-Roblox only, both ways; with a vault, only off (on asks first, in Bloxkeep). */
const canSwitch = (s: TrayState, on: boolean) => s.multiOnly || (on && s.open);

export function Tray({ mode, state, opened, api }: { mode: 'panel' | 'menu'; state: TrayState; opened: number; api: TrayApi }) {
  const [page, setPage] = useState('home');
  const [confirmUntil, setConfirmUntil] = useState(0);
  const root = useRef<HTMLDivElement>(null);
  // Each opening: its first page, the keyboard on the card itself (Tab then reaches the first button), so a click shows no focus ring.
  useEffect(() => { setPage('home'); setConfirmUntil(0); root.current?.focus({ preventScroll: true }); }, [opened, mode]);
  // Windows hands the window the keyboard once it shows, and the browser gives it to the first button then: back to
  // the card, unless a key was pressed since.
  useEffect(() => {
    let keyed = false;
    const key = () => { keyed = true; };
    const focus = () => { if (!keyed) root.current?.focus({ preventScroll: true }); };
    document.addEventListener('keydown', key);
    window.addEventListener('focus', focus);
    return () => { document.removeEventListener('keydown', key); window.removeEventListener('focus', focus); };
  }, [opened]);
  // A page whose list emptied meanwhile (the vault locked, the last preset deleted): back to the first.
  useEffect(() => {
    if (page === 'accounts' && !(state.open && state.accounts.length) || page === 'presets' && !(state.open && state.presets.length)) setPage('home');
  }, [state, page]);
  // The window takes the page's size: measured after each drawing.
  useLayoutEffect(() => {
    const el = root.current!;
    const report = () => api.size(Math.ceil(el.offsetWidth), Math.ceil(el.offsetHeight), opened);
    report();
    const watch = new ResizeObserver(report);
    watch.observe(el);
    return () => watch.disconnect();
  });
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.preventDefault();
      if (page !== 'home') setPage('home'); else api.act('hide');
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [page]);
  const go = (p: string) => { setPage(p); setConfirmUntil(0); };
  return <div ref={root} tabIndex={-1} className={`tray tray-${mode}`} role={mode === 'menu' ? 'menu' : 'dialog'} aria-label="Bloxkeep">
    {mode === 'panel'
      ? <Panel s={state} page={page} go={go} api={api} confirmUntil={confirmUntil} setConfirmUntil={setConfirmUntil} />
      : <Menu s={state} page={page} go={go} api={api} />}
  </div>;
}

/** Bloxkeep's logo and name, with what is going on under it (the menu's own line, with Multi-Roblox's light). */
function Head({ s, children, line, dot }: { s: TrayState; children?: React.ReactNode; line?: string; dot?: boolean }) {
  const { t, tn, logo } = useHost();
  const status = line ?? (!s.open && !s.multiOnly ? t('tray.locked')
    : s.robloxCount > 0 ? tn('tray.status', s.robloxCount, { ram: ram(s.ramMb) }) : t('tray.none'));
  return <div className="tray-head">
    <img className="tray-logo" src={logo} alt="" />
    <div className="tray-name"><b>Bloxkeep</b><span>{dot ? <i className={`tray-dot ${s.multi}`} /> : null}{status}</span></div>
    {children}
  </div>;
}

function Panel({ s, page, go, api, confirmUntil, setConfirmUntil }: {
  s: TrayState; page: string; go: (p: string) => void; api: TrayApi; confirmUntil: number; setConfirmUntil: (n: number) => void;
}) {
  const { t, tn } = useHost();
  const [, tick] = useState(0);
  // "Close N windows?" goes back to "Close all" after four seconds.
  useEffect(() => {
    if (!confirmUntil) return;
    const timer = setTimeout(() => { setConfirmUntil(0); tick(n => n + 1); }, Math.max(0, confirmUntil - Date.now()));
    return () => clearTimeout(timer);
  }, [confirmUntil]);
  const head = <Head s={s}>
    <button type="button" className={`tray-icon${s.miniOpen ? ' on' : ''}`} title={t('tray.mini')} aria-label={t('tray.mini')} aria-pressed={s.miniOpen}
      onClick={() => api.act('mini')}><Icon name="screens" /></button>
    <button type="button" className="tray-icon" title={t('tray.open')} aria-label={t('tray.open')} onClick={() => api.act('open')}><Icon name="external" /></button>
  </Head>;

  if (page !== 'home') {
    const accounts = page === 'accounts';
    return <>
      {head}
      <button type="button" className="tray-back" onClick={() => go('home')}><Icon name="chevronLeft" />{t(accounts ? 'tray.playAs' : 'tray.runPreset')}</button>
      <div className="tray-list">{accounts
        ? s.accounts.map(a => <button key={a.id} type="button" className="tray-row" disabled={a.running} onClick={() => api.act('account', { id: a.id })}>
          <span className="tray-av"><Avatar a={{ avatarUrl: a.avatar, label: a.name }} /></span>
          <span className="tray-row-text"><b>{a.name}</b></span>
          {a.running ? <span className="tray-playing">{t('tray.playing')}</span> : null}
        </button>)
        : s.presets.map(p => <button key={p.id} type="button" className="tray-row" onClick={() => api.act('preset', { id: p.id })}>
          <span className="tray-av glyph"><Icon name="layers" /></span><span className="tray-row-text"><b>{p.name}</b></span>
        </button>)}</div>
    </>;
  }

  const on = s.multi !== 'off';
  const locked = !s.open && !s.multiOnly;
  const switchable = canSwitch(s, on) && !(s.multi === 'notReady' && s.open);
  const sub = !canSwitch(s, on) && locked ? t('tray.unlockToChange')
    : t(s.multi === 'ready' ? 'tray.multiReady' : s.multi === 'notReady' ? 'tray.multiNotReady' : 'tray.multiOff');
  const confirming = confirmUntil > Date.now();
  const canClose = s.robloxCount > 0 && (s.open || s.multiOnly);
  const clients = s.clients;
  const shown = clients.length > MAX_CLIENTS ? clients.slice(0, MAX_CLIENTS - 1) : clients;
  return <>
    {head}
    {/* Switched here when it can be; otherwise Bloxkeep opens: its switch asks first, its card fixes "not ready". */}
    <button type="button" className={`tray-multi ${s.multi}`} role="switch" aria-checked={on}
      onClick={() => switchable ? api.act('multi', { on }) : api.act('open')}>
      <Icon name="layers" />
      <span className="tray-multi-text"><b>Multi-Roblox</b><span>{sub}</span></span>
      <span className={`tray-switch${on ? ' on' : ''}`} aria-hidden="true"><i /></span>
    </button>
    {locked
      ? <div className="tray-locked"><Icon name="lock" /><span>{t('tray.isLocked')}</span>
        <button type="button" className="tray-btn" onClick={() => api.act('open')}>{t('tray.unlock')}</button></div>
      : <section className="tray-running">
        <div className="tray-label"><span>{t('tray.running')}</span>
          {canClose ? <button type="button" className={`tray-link danger${confirming ? ' confirm' : ''}`}
            onClick={() => { if (confirming) { setConfirmUntil(0); api.act('closeAll'); } else setConfirmUntil(Date.now() + 4000); }}>
            {confirming ? tn('tray.closeAsk', s.robloxCount) : t('tray.closeAll')}</button> : null}
        </div>
        {clients.length === 0
          ? <div className="tray-empty">{s.robloxCount > 0 ? tn('tray.openCount', s.robloxCount) : t('tray.none')}</div>
          : <div className="tray-list">
            {shown.map(c => <div key={c.id} className="tray-row client" title={`PID ${c.pid}`}>
              <button type="button" className="tray-row-main" onClick={() => api.act('show', { pid: c.pid })}>
                <span className="tray-av"><Avatar a={{ avatarUrl: c.avatar, label: c.name }} /></span>
                <span className="tray-row-text"><b>{c.name}</b><span>{c.game} · {ram(c.ramMb)}</span></span>
              </button>
              <span className="tray-row-acts">
                <button type="button" className="tray-icon sm" title={t('tray.show')} aria-label={t('tray.show')} onClick={() => api.act('show', { pid: c.pid })}><Icon name="eye" /></button>
                <button type="button" className="tray-icon sm danger" title={t('tray.close')} aria-label={t('tray.close')} onClick={() => api.act('closeClient', { id: c.id })}><Icon name="x" /></button>
              </span>
            </div>)}
            {clients.length > MAX_CLIENTS
              ? <button type="button" className="tray-more" onClick={() => api.act('open')}>{t('tray.more', { n: clients.length - MAX_CLIENTS + 1 })}</button> : null}
          </div>}
      </section>}
    <div className="tray-tiles">
      {s.open && s.accounts.length ? <button type="button" className="tray-tile primary" onClick={() => go('accounts')}><Icon name="play" /><span>{t('tray.play')}</span></button> : null}
      {s.open && s.presets.length ? <button type="button" className="tray-tile" onClick={() => go('presets')}><Icon name="layers" /><span>{t('tray.presets')}</span></button> : null}
      <button type="button" className="tray-tile" title={t('tray.newTip')} onClick={() => api.act('newRoblox')}><Icon name="userPlus" /><span>{t('tray.new')}</span></button>
    </div>
  </>;
}

function Menu({ s, page, go, api }: { s: TrayState; page: string; go: (p: string) => void; api: TrayApi }) {
  const { t, tn } = useHost();
  const on = s.multi !== 'off';
  const item = (icon: string, label: string, act: () => void, { value, sub, strong, checked, disabled, danger, title, id }: {
    value?: string; sub?: boolean; strong?: boolean; checked?: boolean; disabled?: boolean; danger?: boolean; title?: string; id?: string;
  } = {}) => <button type="button" role="menuitem" className={`tray-item${strong ? ' strong' : ''}${danger ? ' danger' : ''}`} disabled={disabled}
    title={title} data-item={id} onClick={act}>
    <Icon name={icon} /><span className="tray-item-text">{label}</span>
    {checked ? <Icon name="check" className="tray-check" /> : null}
    {value ? <span className="tray-value">{value}</span> : null}
    {sub ? <Icon name="chevronRight" className="tray-sub" /> : null}
  </button>;
  const back = (label: string) => <button type="button" className="tray-back" onClick={() => go('home')}><Icon name="chevronLeft" />{label}</button>;

  if (page === 'accounts') return <>{back(t('tray.playAs'))}
    <div className="tray-list">{s.accounts.map(a => <button key={a.id} type="button" role="menuitem" className="tray-row" disabled={a.running}
      onClick={() => api.act('account', { id: a.id })}>
      <span className="tray-av sm"><Avatar a={{ avatarUrl: a.avatar, label: a.name }} /></span><span className="tray-row-text"><b>{a.name}</b></span>
      {a.running ? <span className="tray-playing">{t('tray.playing')}</span> : null}
    </button>)}</div></>;
  if (page === 'presets') return <>{back(t('tray.runPreset'))}
    <div className="tray-list">{s.presets.map(p => item('layers', p.name, () => api.act('preset', { id: p.id })))}</div></>;
  if (page === 'multi') return <>{back('Multi-Roblox')}
    {s.multi === 'notReady' && s.open && !s.multiOnly ? item('wrench', t('tray.fix'), () => api.act('open')) : null}
    {canSwitch(s, on) ? item('power', t(on ? 'tray.turnOff' : 'tray.turnOn'), () => api.act('multi', { on }))
      : item(!s.open ? 'lock' : 'external', t(!s.open ? 'tray.unlockToChange' : 'tray.openToTurnOn'), () => api.act('open'))}</>;
  if (page === 'close') return <>{back(t('tray.closeAllRoblox'))}
    {item('x', tn('tray.closeN', s.robloxCount), () => api.act('closeAll'), { danger: true })}</>;

  const short = t(s.multi === 'ready' ? 'tray.mOn' : s.multi === 'notReady' ? 'tray.mNotReady' : 'tray.mOff');
  const canClose = s.robloxCount > 0 && (s.open || s.multiOnly);
  return <>
    <Head s={s} dot line={!s.open && !s.multiOnly ? `${t('tray.locked')} · ${short}`
      : s.robloxCount > 0 ? `${short} · ${tn('tray.windows', s.robloxCount)}` : short} />
    <div className="tray-sep" />
    {item('external', t('tray.open'), () => api.act('open'), { strong: true })}
    {item('screens', t('tray.mini'), () => api.act('mini'), { checked: s.miniOpen })}
    <div className="tray-sep" />
    {s.open && s.accounts.length ? item('play', t('tray.playAs'), () => go('accounts'), { sub: true }) : null}
    {s.open && s.presets.length ? item('layers', t('tray.runPreset'), () => go('presets'), { sub: true }) : null}
    {item('userPlus', t('tray.newWindow'), () => api.act('newRoblox'), { title: t('tray.newTip') })}
    {item('layers', 'Multi-Roblox', () => go('multi'), { sub: true, id: 'multi', value: t(s.multi === 'ready' ? 'tray.on' : s.multi === 'notReady' ? 'tray.notReady' : 'tray.off') })}
    <div className="tray-sep" />
    {canClose ? item('x', t('tray.closeAllRoblox'), () => go('close'), { sub: true, value: String(s.robloxCount) }) : null}
    {item('power', t('tray.quit'), () => api.act('quit'))}
  </>;
}
