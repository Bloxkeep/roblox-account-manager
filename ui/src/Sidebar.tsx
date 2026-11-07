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
  const nav = (v: string, icon: string, extra?: React.ReactNode, tip = t(`nav.${v}`)) =>
    <button className={`nav ${view === v ? 'on' : ''}`} data-act="view" data-view={v} title={tip}><Icon name={icon} /><span>{t(`nav.${v}`)}</span>{extra}</button>;
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
      {/* Game news: its followed games' unread news, or New while a game is there that wasn't seen in it yet. */}
      {nav('news', 'news', app.feeds?.unread ? <em className="nav-news" id="nav-news">{app.feeds.unread > 99 ? '99+' : app.feeds.unread}</em>
        : app.feeds?.fresh ? <em className="nav-news" id="nav-news">{t('news.new')}</em> : null,
        app.feeds?.unread ? `${t('nav.news')} (${t('news.unreadN', { n: app.feeds.unread })})` : undefined)}
      {/* Macros are new and work differently on each PC (Roblox windows taken to the front in turn): marked Beta. */}
      {nav('macros', 'keyboard', <em className="nav-beta">{t('macro.beta')}</em>, `${t('nav.macros')} (${t('macro.beta')})`)}
      {nav('monitor', 'screens')}
      {nav('stats', 'clock')}
      {nav('settings', 'sliders')}
      {/* The mini window opens a window, it isn't a tab: under them, set apart by a hairline. */}
      <button className="nav nav-mini" id="sys-mini" data-act="mini" title={mini} aria-pressed={!!app.miniOpen}><Icon name="monitor" /><span>{mini}</span></button>
    </div>
    <div className="side-bottom">
      <div id="side-warn">{app.robloxInstalled ? null : <div className="warn-card" title={t('side.noRoblox')}><Icon name="alert" /><span>{t('side.noRoblox')}</span></div>}</div>
      {/* One panel (themes draw it as their card): Roblox on this PC, with Multi-Roblox's switch at its foot. */}
      <div className="side-card rig" id="sys-rig">
        <SysCard />
        <div id="multi-card"><MultiCard /></div>
      </div>
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

/** Where a window's account is, as its face shows it: in a game, in Roblox's menu, dropped from its server. */
type Mood = 'play' | 'menu' | 'drop';
const moodOf = (a: any): Mood => a.session?.connection === 'disconnected' ? 'drop' : a.session?.connection === 'menu' || a.session?.connection === 'signedout' ? 'menu' : 'play';
/** The baseplate's studs: two rows of ten, 5 % of the PC's RAM each. */
const BRICKS = 20;
/** Ten across, one on each stud column; past ten, the last place says "+N". */
const SLOTS = 10;

/** Bloxkeep's block face (its logo's), one per Roblox window: smiling in a game, flat-mouthed in the menu, X-eyed when
 * dropped. */
function Face({ mood, tip }: { mood: Mood; tip: string }) {
  return <svg className={`face mood-${mood}`} viewBox="0 0 32 32" aria-hidden="true"><title>{tip}</title>
    <rect className="fb" x="1" y="1" width="30" height="30" rx="7" />
    {mood === 'drop'
      ? <path className="fl" d="M7.5 10.5l4 4m0-4l-4 4M20.5 10.5l4 4m0-4l-4 4" />
      : <><rect className="fi" x="8.4" y="10" width="3.4" height="5.6" rx="1.7" /><rect className="fi" x="20.2" y="10" width="3.4" height="5.6" rx="1.7" /></>}
    <path className="fl" d={mood === 'play' ? 'M9.6 20.5c3.6 3.4 9.2 3.4 12.8 0' : mood === 'menu' ? 'M11 22h10' : 'M11 23.5c3-2.4 7-2.4 10 0'} />
  </svg>;
}

/**
 * Roblox on this PC, drawn as the farm: a block face for every Roblox window (the account's, or a window Bloxkeep didn't
 * open) standing on a baseplate whose studs are the PC's RAM, 5 % each (Roblox's, the rest in use, free). Above them
 * the windows and their memory with Close all; under them the RAM's number, Roblox's CPU and GPU, and its version.
 */
function SysCard() {
  const { t, fmtMb } = useHost();
  const app = useAppState();
  const m = useMetrics();
  const running = (app.accounts || []).filter((a: any) => a.state === 'running');
  const pids = running.filter((a: any) => a.session?.pid > 0).map((a: any) => `${a.label} · PID ${a.session.pid}`);
  const count = m.robloxCount || 0;
  const people: { key: string; mood: Mood; tip: string }[] = running.map((a: any) => ({ key: a.id, mood: moodOf(a), tip: `${a.label} · ${t(`sys.mood.${moodOf(a)}`)}` }));
  // Windows Bloxkeep didn't open (or whose account it can't tell) get a face too, in no particular mood.
  while (people.length < count) people.push({ key: `w${people.length}`, mood: 'play', tip: t('sys.otherWindow') });
  const over = people.length > SLOTS;
  const shown = over ? people.slice(0, SLOTS - 1) : people;
  const robloxPct = m.systemMb ? Math.min(100, (m.totalMb / m.systemMb) * 100) : 0;
  const usedPct = Math.max(robloxPct, m.usedPct || 0);
  const robloxStuds = Math.min(BRICKS, Math.round(robloxPct / (100 / BRICKS)));
  const usedStuds = Math.max(robloxStuds, Math.min(BRICKS, Math.round(usedPct / (100 / BRICKS))));
  const used = m.systemMb ? t('sys.used', { pct: Math.round(usedPct), free: fmtMb(m.availableMb || 0) }) : undefined;
  const rv = app.robloxVersion || {};
  const short = (v?: string) => (v || '').split('.').slice(0, 2).join('.');
  return <div className={`sys ${usedPct >= 90 ? 'hot' : ''}`} id="sys-card">
    {/* Roblox not installed: no windows to count and no version (the warning above says it). */}
    <div className="sys-head" hidden={!app.robloxInstalled}>
      <span className="sys-count" title={pids.join('\n') || undefined}><b id="sys-count">{count}</b> <span id="sys-count-label">{t(count === 1 ? 'sys.window1' : 'sys.windows')}</span>
        <span className="sys-ram" id="sys-ram">{fmtMb(m.totalMb)}</span></span>
      <button type="button" className="sys-close" id="sys-close" data-act="close-all" hidden={!count} title={t('sys.closeAll')} aria-label={t('sys.closeAll')}><Icon name="power" /></button>
    </div>
    <div className="lobby">
      {shown.length
        ? shown.map(p => <Face key={p.key} mood={p.mood} tip={p.tip} />)
        : <span className="sys-none">{t(app.robloxInstalled ? 'sys.none' : 'side.noRoblox')}</span>}
      {over ? <em className="sys-more" title={t('sys.moreTip', { n: people.length - shown.length })}>+{people.length - shown.length}</em> : null}
    </div>
    <div className="bricks" id="sys-meter" title={used} aria-hidden="true">
      {Array.from({ length: BRICKS }, (_, i) => <i key={i} className={i < robloxStuds ? 'r' : i < usedStuds ? 'o' : ''} />)}</div>
    <div className="sys-foot">
      <span id="sys-foot" title={used}>{m.systemMb ? t('sys.ramPct', { pct: Math.round(usedPct) }) : ''}</span>
      {count && (m.cpuPct != null || m.gpuPct != null)
        ? <span id="sys-usage" title={t('sys.usageTip')}>{[m.cpuPct != null ? `CPU ${m.cpuPct}%` : '', m.gpuPct != null ? `GPU ${m.gpuPct}%` : ''].filter(Boolean).join(' · ')}</span>
        : null}</div>
    <div className="sys-ver" id="sys-ver" hidden={!app.robloxInstalled}>{rv.installed
      ? rv.upToDate === false
        // Out of date: Settings › Roblox version says how to update (and has Update now when a launcher here can).
        ? <button type="button" className="link" data-act="ru-open" title={`Roblox ${rv.installed}`}><i className="dot warn" />{`Roblox ${short(rv.installed)} · ${
          short(rv.latest) === short(rv.installed) ? t('rbx.newBuild') : t('rbx.available', { v: short(rv.latest) })}`}</button>
        : <span title={`Roblox ${rv.installed}`}><i className={`dot ${rv.upToDate ? 'ok' : ''}`} />{`Roblox ${short(rv.installed)}${rv.upToDate ? ` · ${t('rbx.latestShort')}` : ''}`}</span>
      : null}</div>
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
  // The card's words are short (On, Off…); what they mean is on hover.
  const tip = closed ? t('multi.closedTip', { page: t('set.p.roblox') }) : watched ? t('multi.watchTip', { page: t('set.p.roblox') })
    : t(!m.enabled ? 'multi.offTip' : m.armed ? 'multi.readyTip' : 'multi.notReadyTip');
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
