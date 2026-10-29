// The sidebar: window buttons (macOS themes), the logo with the Discord button, the tabs, then Roblox on this PC,
// Multi-Roblox's card, a waiting update and Lock. Buttons keep their data-act for app.js's one click handler.
import { useLocal } from './settings/parts';
import { Icon, useAppState, useHost, useMetrics, usePage } from './host';
import { VaultFace, type VaultInfo } from './Vaults';

/** The watch found the lock changed in the build Multi-Roblox meets next, and it hasn't worked on it here since. */
export const watchWarns = (app: Record<string, any>) => {
  const w = app.robloxUpdates?.watch;
  return !!w && ['changed', 'renamed', 'gone'].includes(w.check) && !w.worked;
};

export function Sidebar() {
  const { t, logo } = useHost();
  const app = useAppState();
  const view = usePage(p => p.view);
  const accounts: any[] = app.accounts || [];
  // The mini window's buttons read as pressed while it is open (the tray menu and Esc can change that too).
  const mini = t(app.miniOpen ? 'sys.miniHide' : 'sys.mini');
  const nav = (v: string, icon: string, extra?: React.ReactNode) =>
    <button className={`nav ${view === v ? 'on' : ''}`} data-act="view" data-view={v} title={t(`nav.${v}`)}><Icon name={icon} /><span>{t(`nav.${v}`)}</span>{extra}</button>;
  const u = app.update;
  const vault: VaultInfo | null = (app.vaults?.list || []).find((v: VaultInfo) => v.id === app.vaults?.open) || null;
  return <aside className="side">
    <div className="lights">{(['close', 'min', 'zoom'] as const).map(w =>
      <button key={w} className={`light l-${w}`} data-act={`win-${w}`} title={t(`win.${w}`)} aria-label={t(`win.${w}`)} />)}</div>
    <div className="brand"><img src={logo} alt="" /><div className="brand-text">
      <div className="brand-name">Bloxkeep</div><div className="brand-sub">{t('brand.sub')}</div></div></div>
    <div className="side-nav">
      {nav('accounts', 'users', <em id="nav-count">{accounts.length}</em>)}
      {nav('presets', 'layers')}
      {nav('monitor', 'screens')}
      {nav('stats', 'clock')}
      {nav('settings', 'sliders')}
      <button className="nav nav-mini" data-act="mini" title={mini} aria-pressed={!!app.miniOpen}><Icon name="monitor" /><span>{mini}</span></button>
    </div>
    <div className="side-bottom">
      <div id="side-warn">{app.robloxInstalled ? null : <div className="warn-card" title={t('side.noRoblox')}><Icon name="alert" /><span>{t('side.noRoblox')}</span></div>}</div>
      <SysCard mini={mini} />
      <div id="multi-card"><MultiCard /></div>
      <div className="side-foot">
        <div id="update-slot">{u?.status === 'ready' && !u.quiet
          ? <button className="btn primary w update-btn" data-act="apply-update" title={t(u.auto ? 'upd.restartTipAuto' : 'upd.restartTip', { v: u.version })}>
            <Icon name="refresh" /><span><b>{t('upd.restart')}</b><small>{t('upd.versionN', { v: u.version })}</small></span></button> : null}</div>
        {/* Switch vault (its face): the open vault locks and the lock screen's picker shows (Vaults.tsx). Bloxkeep's
            Discord server is under Settings › Bloxkeep › Help & feedback. */}
        <div className="side-lock">
          <button className="btn ghost w" data-act="lock" title={t('side.lockTip')}><Icon name="lock" /><span>{t('side.lock')}</span></button>
          {vault ? <button className="btn ghost side-sq vault-btn" data-act="vault-pick" title={t('vault.switchTip', { name: vault.name })} aria-label={t('vault.switchTip', { name: vault.name })}>
            <VaultFace vault={vault} className="xs" /></button> : null}
        </div>
      </div>
    </div>
  </aside>;
}

/** Roblox on this PC: its windows (whose is which, by process id), their memory against the whole PC's, its version. */
function SysCard({ mini }: { mini: string }) {
  const { t, fmtMb } = useHost();
  const app = useAppState();
  const m = useMetrics();
  const pids = (app.accounts || []).filter((a: any) => a.state === 'running' && a.session?.pid > 0).map((a: any) => `${a.label} · PID ${a.session.pid}`);
  // The bar is the whole PC's RAM: Roblox's share first, then everything else in use. Transforms only: moving the
  // bars never makes the page re-run layout.
  const robloxPct = m.systemMb ? Math.min(100, (m.totalMb / m.systemMb) * 100) : 0;
  const usedPct = Math.max(robloxPct, m.usedPct || 0);
  const rv = app.robloxVersion || {};
  const short = (v?: string) => (v || '').split('.').slice(0, 2).join('.');
  return <div className={`side-card ${usedPct >= 90 ? 'hot' : ''}`} id="sys-card">
    <h4>{t('sys.title')}</h4>
    <div className="sys-row"><span title={pids.join('\n')}><b id="sys-count">{m.robloxCount}</b> <span id="sys-count-label">{t(m.robloxCount === 1 ? 'sys.window1' : 'sys.windows')}</span></span><span id="sys-ram">{fmtMb(m.totalMb)}</span></div>
    <div className="meter" id="sys-meter"><i className="roblox" style={{ transform: `scaleX(${(robloxPct / 100).toFixed(3)})` }} />
      <i className="other" style={{ transform: `translateX(${robloxPct.toFixed(1)}%) scaleX(${((usedPct - robloxPct) / 100).toFixed(3)})` }} /></div>
    <div className="sys-foot" id="sys-foot">{m.systemMb ? t('sys.used', { pct: Math.round(usedPct), free: fmtMb(m.availableMb || 0) }) : ''}</div>
    <div className="sys-ver" id="sys-ver">{rv.installed
      ? rv.upToDate === false
        // Out of date: Settings › Roblox version says how to update (and has Update now when a launcher here can).
        ? <button type="button" className="link" data-act="ru-open" title={`Roblox ${rv.installed}`}><i className="dot warn" />{`Roblox ${short(rv.installed)} · ${
          short(rv.latest) === short(rv.installed) ? t('rbx.newBuild') : t('rbx.available', { v: short(rv.latest) })}`}</button>
        : <span title={`Roblox ${rv.installed}`}><i className={`dot ${rv.upToDate ? 'ok' : ''}`} />{`Roblox ${short(rv.installed)}${rv.upToDate ? ` · ${t('rbx.latestShort')}` : ''}`}</span>
      : null}</div>
    <button className="btn sm w sys-mini" id="sys-mini" data-act="mini" aria-pressed={!!app.miniOpen}><Icon name="monitor" /><span>{mini}</span></button>
    <button className="btn sm w sys-close" id="sys-close" data-act="close-all" hidden={!m.robloxCount}><Icon name="power" /><span>{t('sys.closeAll')}</span></button>
  </div>;
}

// Multi-Roblox: on or off, and whether it works. When Roblox closed windows or the watch found the lock changed in the
// next build, the card says so and leads to Settings › Roblox updates. The light and words open the details; the switch
// turns it on (after the risk warning) or off.
function MultiCard() {
  const { t, setMulti } = useHost();
  const app = useAppState();
  const m = app.multi || {};
  const [on, setOn] = useLocal(!!m.enabled);
  const closed = m.enabled && !!app.robloxUpdates?.closed;
  const watched = m.enabled && m.armed && !closed && watchWarns(app);
  const cls = !m.enabled ? 'off' : m.armed && !closed && !watched ? 'ok' : 'warn';
  const text = t(!m.enabled ? 'multi.off' : closed ? 'multi.closed' : watched ? 'multi.watchWarn' : m.armed ? 'multi.ready' : 'multi.notReady');
  const tip = closed ? t('multi.closedTip', { page: t('set.p.roblox') }) : watched ? t('multi.watchTip', { page: t('set.p.roblox') }) : text;
  return <div className={`multi ${cls}`}>
    <button className="multi-open" data-act={closed || watched ? 'ru-open' : 'multi-help'} title={`${t('multi.title')}: ${tip}`}><span className="led" /><div><b>{t('multi.title')}</b><small>{text}</small></div></button>
    <label className="switch" title={t(m.enabled ? 'multi.turnOff' : 'multi.turnOn')}><input type="checkbox" id="side-multi" checked={on} aria-label={t('multi.title')}
      onChange={async e => {
        const want = e.target.checked;
        setOn(want);
        if (!await setMulti(want)) setOn(!!m.enabled);
      }} /><span /></label>
  </div>;
}

/** The game monitor's heading line: how many pictures it shows, and the hint once there are some. */
export function MonitorHead() {
  const { t, fmtNum } = useHost();
  const m = useMetrics();
  // The monitor counts its pictures (windows it can show), not every Roblox process.
  const tiles = m.monitorCount ?? m.robloxCount;
  return <p><b id="mon-count">{tiles === 1 ? t('mon.count1') : t('mon.countN', { n: fmtNum(tiles) })}</b><span id="mon-hint" hidden={!tiles}><span className="sep">·</span>{t('mon.hint')}</span></p>;
}
