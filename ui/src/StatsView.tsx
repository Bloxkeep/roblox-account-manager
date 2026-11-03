// The Statistics tab: Statistics (play time per account, game and preset over a period, with its chart) and Activity
// (what the app saw, newest first, paged, filtered). Figures are asked for when the tab opens and every 30 s while shown;
// they aren't part of the pushed state. Today has a timeline (plain divs placed by their share of the day, each stay with its
// game's icon); 7 and 30 days a bar per day instead, an account's own timeline opening from its row.
import { Fragment, useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { clockText, playedText, hourText } from './clock';
import { Icon, useAppState, useHost } from './host';
import { Avatar } from './Avatar';
import { AccountCards, Clock, GameMap } from './StatsViz';

const STATS_PERIODS = [1, 7, 30];
const AL_GROUPS = ['play', 'drops', 'schedule', 'signin', 'updates'];
const AL_PAGE = 100;
const NO_PRESET = '00000000-0000-0000-0000-000000000000';

type Bucket = { start: string; seconds: number };
type StatAccount = { id: string; label?: string; username?: string; avatarUrl?: string; seconds: number; sessions: number; kicks: number; deleted?: boolean;
  topGame?: { key: string; name: string; iconUrl?: string | null } | null;
  /** Its play per hour (today) or day; its days in a row played nearly whole (23 hours or more), up to yesterday. */
  buckets?: number[] | null; streak?: number };
type StatGame = { key: string; name: string; iconUrl?: string; seconds: number; sessions: number };
type StatPreset = { id: string; name: string; iconUrl?: string; seconds: number; accounts: number; kicks: number };
type Segment = { from: string; to: string; game: string; name?: string | null; reason?: string | null };
type Loss = { kind: string; times: number; seconds: number; accounts: string[] };
type Stats = {
  sessions: number; recorded: boolean; totalSeconds: number; gamesSeconds?: number; kicks: number; hourly: boolean; buckets: Bucket[]; since: string; ends: string;
  /** 7 and 30 days: windows running in each hour of each day (its hours of play), a row a day from the first. */
  heat?: number[][];
  timeline: { id: string; segments: Segment[] }[]; losses: Loss[];
  accounts: StatAccount[]; games: StatGame[]; presets?: StatPreset[]; longest?: { account: string; label?: string; game?: string; seconds: number } | null;
  /** The period before, as far into it as this one has gone; null when the records don't reach back that far. */
  previous?: { totalSeconds: number; kicks: number } | null;
  /** Each game's icon, by its key in the timeline. */
  icons?: Record<string, string>;
  days?: number;
};
type Entry = { at: string; tone: string; accountId?: string | null; account?: string | null; text: string };
type Activity = { total: number; entries: Entry[]; accounts: { id: string; label: string }[] };
type Account = { id: string; label: string };

export interface StatsApi {
  /** Opens Activity (Ctrl+K's "Export activity"). */
  showActivity(): void;
  /** Saves what Activity's filters show as a CSV file (the app asks where). */
  exportActivity(): void;
}

export function StatsView({ api }: { api: Ref<StatsApi> }) {
  const host = useHost();
  const { t, tn, call, busy, toast, on, store } = host;
  const app = useAppState();

  const [tab, setTabState] = useState(store.get('rm.statsTab', 'stats') === 'activity' ? 'activity' : 'stats');
  const [days, setDays] = useState(() => { const d = +store.get('rm.statsDays', '7'); return STATS_PERIODS.includes(d) ? d : 7; });
  const [data, setData] = useState<Stats | null>(null);
  const [failed, setFailed] = useState(false);
  // One game's figures only (picked on the games' map), or all of them.
  const [game, setGameState] = useState<string | null>(null);
  const [filter, setFilter] = useState({ account: '', group: '', limit: AL_PAGE });
  const [log, setLog] = useState<Activity | null>(null);
  const [logFailed, setLogFailed] = useState(false);
  const scroll = useRef<HTMLDivElement>(null);

  // A reply to an older request (or for another period, other filters) is dropped. A failed refresh keeps figures
  // already shown; with none, it says so.
  const seq = useRef(0), logSeq = useRef(0), alive = useRef(true);
  const now = useRef({ days, filter, game });
  now.current = { days, filter, game };
  useEffect(() => () => { alive.current = false; }, []);
  const loadStats = async () => {
    const mine = ++seq.current;
    const d = await call('playStats', { days: now.current.days, game: now.current.game }, { silent: true }) as Stats | undefined;
    if (mine !== seq.current || !alive.current) return;
    setFailed(!d);
    if (d) setData(d);
  };
  const loadActivity = async () => {
    const mine = ++logSeq.current, f = now.current.filter;
    const d = await call('activity', { account: f.account || null, group: f.group || null, limit: f.limit }, { silent: true }) as Activity | undefined;
    if (mine !== logSeq.current || !alive.current) return;
    setLogFailed(!d);
    if (d) setLog(d);
  };

  // While on screen: fresh figures now and every 30 s, not while the window is hidden, and as soon as it comes back.
  const shown = useRef(false);
  useEffect(() => {
    let timer = 0;
    const show = () => {
      shown.current = true;
      clearInterval(timer);
      timer = window.setInterval(() => { if (!document.hidden) { loadStats(); loadActivity(); } }, 30_000);
      loadStats();
      loadActivity();
    };
    const hide = () => { shown.current = false; clearInterval(timer); };
    const back = () => { if (!document.hidden && shown.current) { loadStats(); loadActivity(); } };
    const stopShow = on('statsShown', show), stopHide = on('statsHidden', hide);
    document.addEventListener('visibilitychange', back);
    // Asked for once the app has settled (game icons may need Roblox), so Statistics opens with them already there.
    const stopSettled = on('settled', () => { if (!shown.current) loadStats(); });
    return () => { stopShow(); stopHide(); stopSettled(); clearInterval(timer); document.removeEventListener('visibilitychange', back); };
  }, []); // the loaders read the latest period and filters (now)

  // Activity's sentences name accounts as the app wrote them: once names are hidden or shown again, the kept list goes
  // (it isn't drawn again with the old names) and a fresh one is asked for.
  const hideNames = !!app.hideNames;
  const named = useRef(hideNames);
  useEffect(() => {
    if (named.current === hideNames) return;
    named.current = hideNames;
    logSeq.current++;
    setLog(null);
    setLogFailed(false);
    if (shown.current) loadActivity();
  }, [hideNames]);

  const setTab = (next: string) => {
    setTabState(next);
    store.set('rm.statsTab', next);
    if (scroll.current) scroll.current.scrollTop = 0;
  };
  const setPeriod = (d: number) => {
    if (d === days) return;
    setDays(d);
    now.current.days = d;
    store.set('rm.statsDays', String(d));
    // The old period's figures never show under the new period's chip: loading until the new ones are in.
    setData(null);
    setFailed(false);
    loadStats();
  };
  const setGame = (key: string | null) => {
    setGameState(key);
    now.current.game = key;
    loadStats();
  };
  const setLogFilter = (f: typeof filter) => {
    setFilter(f);
    now.current.filter = f;
    loadActivity();
  };
  const exportLog = (btn: HTMLElement | null) => busy(btn as HTMLElement, async () => {
    const n = await call('exportActivity', { account: now.current.filter.account || null, group: now.current.filter.group || null }) as number | undefined;
    if (n) toast(tn('alog.exported', n, { n: host.fmtNum(n) }), 'ok');
  });
  useImperativeHandle(api, () => ({
    showActivity: () => setTab('activity'),
    // The list may not be loaded yet: the app knows what the filters cover.
    exportActivity: () => { exportLog(null); },
  }), []);

  const onStats = tab === 'stats';
  return <>
    <header className="top">
      <div className="top-title"><h1>{t('stats.title')}</h1><p id="stats-sub">{t(onStats ? 'stats.note' : 'alog.note')}</p></div>
      <div className="top-tools">
        <div className="seg" id="stats-tabs" role="tablist" aria-label={t('stats.title')}>
          {[['stats', 'stats.title'], ['activity', 'alog.title']].map(([id, key]) =>
            <button key={id} type="button" role="tab" className={tab === id ? 'on' : ''} data-tab={id} aria-selected={tab === id} onClick={() => setTab(id)}>{t(key)}</button>)}
        </div>
        {/* Nothing ever recorded: no period to pick. (Nothing in this period: the others may have some.) */}
        <div className="chips" id="stats-period" role="group" aria-label={t('stats.title')} hidden={!onStats || !!data && !data.sessions && !data.recorded}>
          {STATS_PERIODS.map(d => <button key={d} className={`chip ${d === days ? 'on' : ''}`} data-days={d} aria-pressed={d === days} onClick={() => setPeriod(d)}>{t(`stats.period.${d}`)}</button>)}
        </div>
      </div>
    </header>
    <div className="scroll" ref={scroll}>
      <div className="stats">
        <div className="stats-in" id="stats-body" hidden={!onStats}>
          <Figures data={data} failed={failed} retry={btn => busy(btn, loadStats)} game={game} setGame={setGame} />
        </div>
        {/* Its name is the tab's already (Statistics | Activity): no heading over the filters. */}
        <section className="stat-card alog" id="alog-sec" aria-label={t('alog.title')} hidden={onStats}>
          <div className="alog-head">
            <div className="alog-tools">
              <select className="select sm" id="alog-account" aria-label={t('alog.filterAccount')} value={filter.account}
                onChange={e => setLogFilter({ ...filter, account: e.target.value, limit: AL_PAGE })}>
                <option value="">{t('alog.allAccounts')}</option>
                {(log?.accounts || []).map(a => <option key={a.id} value={a.id}>{a.label}</option>)}
              </select>
              <select className="select sm" id="alog-group" aria-label={t('alog.filterKind')} value={filter.group}
                onChange={e => setLogFilter({ ...filter, group: e.target.value, limit: AL_PAGE })}>
                <option value="">{t('alog.allKinds')}</option>
                {AL_GROUPS.map(g => <option key={g} value={g}>{t(`alog.g.${g}`)}</option>)}
              </select>
              <button className="btn sm" disabled={!log?.total} onClick={e => exportLog(e.currentTarget)}><Icon name="download" /><span>{t('alog.export')}</span></button>
            </div>
          </div>
          <div id="alog-body">
            <Log log={log} failed={logFailed} filtered={!!(filter.account || filter.group)} retry={btn => busy(btn, loadActivity)}
              more={btn => { const f = { ...filter, limit: filter.limit + AL_PAGE }; setFilter(f); now.current.filter = f; busy(btn, loadActivity); }} />
          </div>
        </section>
      </div>
    </div>
  </>;
}

/** Statistics: tiles (with the change from the period before), today's timeline or the play time per day, why windows
 * dropped, then by account, top games and by preset. */
function Figures({ data: d, failed, retry, game, setGame }: { data: Stats | null; failed: boolean; retry: (btn: HTMLElement) => void;
  game: string | null; setGame: (key: string | null) => void }) {
  const host = useHost();
  const { t, tn, fmtNum } = host;
  const app = useAppState();
  const accounts: Account[] = app.accounts || [];
  const byId =(id: string) => accounts.find(a => a.id === id);
  const hide = !!app.hideNames;
  const [open, setOpen] = useState('');
  const fmtSecs = (s: number) => playedText(host, s);

  if (!d) {
    return failed
      ? <div className="empty"><h2>{t('stats.error')}</h2><div className="row"><button className="btn" onClick={e => retry(e.currentTarget)}><Icon name="refresh" />{t('srv.retry')}</button></div></div>
      : <div className="empty"><span className="spin" /></div>;
  }
  if (!d.sessions) {
    return <div className="empty"><span className="pv-empty-icon"><Icon name="clock" /></span>
      {d.recorded ? <h2>{t('stats.emptyPeriod')}</h2> : <><h2>{t('stats.emptyTitle')}</h2><p>{t('stats.emptyText')}</p></>}
      {game ? <div className="row"><button className="btn" onClick={() => setGame(null)}>{t('stats.allGames')}</button></div> : null}</div>;
  }
  // A deleted account keeps the name it had when it played; names hidden: the list's "Account N".
  const nameOf = (a: StatAccount) => hide ? byId(a.id)?.label || t('stats.deleted') : a.label || t('stats.deleted');
  const avatarOf = (a: StatAccount) => <AccAvatar a={a} label={nameOf(a)} hide={hide} />;
  // 7 and 30 days: an account with stays in the period opens its own timeline.
  const opens = (id: string) => !d.hourly && d.timeline.some(r => r.id === id);
  const longName = d.longest ? (hide ? byId(d.longest.account)?.label : d.longest.label) || t('stats.deleted') : '';
  // Accounts still here that played (a deleted one isn't among the accounts it is counted against).
  const played = fmtNum(d.accounts.filter(a => a.seconds > 0 && !a.deleted).length), all = fmtNum(accounts.length);
  const period = d.hourly ? 1 : d.days || d.buckets.length;
  // "↑ 20 %" against the period before; more play time is good, more disconnects bad. Nothing when there's no before.
  const change = (now: number, before: number | undefined, moreIsGood: boolean) => {
    if (before === undefined || before <= 0) return null;
    const pct = Math.round((now - before) / before * 100);
    const vs = t(`stats.vs.${period}`);
    if (!pct) return <span className="stat-delta">{t('stats.noChange')} <span>{vs}</span></span>;
    return <span className={`stat-delta ${(pct > 0) === moreIsGood ? 'good' : 'bad'}`}>{pct > 0 ? '↑' : '↓'} {fmtNum(Math.abs(pct))} % <span>{vs}</span></span>;
  };
  const tiles: [string, ReactNode, ReactNode, string?][] = [
    [t('stats.total'), fmtSecs(d.totalSeconds), change(d.totalSeconds, d.previous?.totalSeconds, true)],
    [t('stats.played'), <>{played}<span className="of">/{all}</span></>, null, `${played}/${all}`],
    [t('stats.kicks'), fmtNum(d.kicks), change(d.kicks, d.previous?.kicks, false)],
    [t('stats.longest'), d.longest ? fmtSecs(d.longest.seconds) : '—', d.longest ? <span className="stat-sub">{[longName, d.longest.game].filter(Boolean).join(' · ')}</span> : null],
  ];
  // A game's or preset's share of the period's play time, on its small line.
  const share = (s: number) => `${fmtNum(Math.round(s / Math.max(1, d.totalSeconds) * 100))} %`;

  return <>
    <div className="stat-tiles">{tiles.map(([label, value, sub, plain], i) =>
      <div key={i} className="stat"><small>{label}</small><b title={plain || String(value)}>{value}</b>{sub}</div>)}
    </div>
    {d.hourly ? <Clock since={d.since} rows={d.timeline} accounts={d.accounts} icons={d.icons} totalSeconds={d.totalSeconds}
      nameOf={nameOf} avatarOf={avatarOf} fmtSecs={fmtSecs} />
      : <Days d={d} fmtSecs={fmtSecs} />}
    {!d.hourly && d.heat?.length ? <Heat d={d} heat={d.heat} /> : null}
    <Drops d={d} nameOf={nameOf} fmtSecs={fmtSecs} />
    <GameMap games={d.games} total={d.gamesSeconds ?? d.totalSeconds} picked={game} pick={setGame} fmtSecs={fmtSecs} />
    <AccountCards accounts={d.accounts} open={open} setOpen={setOpen} opens={opens} nameOf={nameOf} avatarOf={avatarOf} fmtSecs={fmtSecs}
      below={opens(open) ? <div className="sa-open"><Timeline d={d} nameOf={nameOf} fmtSecs={fmtSecs} only={open} /></div> : null} />
    {d.presets?.length ? <div className="stat-card"><h3>{t('stats.byPreset')}</h3><div className="sgames">{d.presets.map(p => {
      const own = p.id !== NO_PRESET, name = own ? p.name : t('stats.noPreset');
      const detail = [tn('stats.accountsN', p.accounts), p.kicks ? tn('stats.kicksN', p.kicks) : '', share(p.seconds)].filter(Boolean).join(' · ');
      return <div key={p.id} className="sgame">{own && p.iconUrl ? <GameImg src={p.iconUrl} /> : <span className="gph"><Icon name={own ? 'layers' : 'user'} /></span>}
        <div className="gtxt"><b title={own ? name : t('stats.noPresetTip')}>{name}</b><small>{detail}</small></div>
        <span className="sg-time">{fmtSecs(p.seconds)}</span></div>;
    })}</div>
      {d.presets.every(p => p.id === NO_PRESET) ? <p className="stat-note">{t('stats.presetNote')}</p> : null}</div> : null}
  </>;
}

/** "3h06" (never "0h00" for some play): the short form on a bar, the same in both languages. */
const barTime = (s: number) => {
  const m = s > 0 ? Math.max(1, Math.round(s / 60)) : 0;
  return `${Math.floor(m / 60)}h${String(m % 60).padStart(2, '0')}`;
};

/** 7 and 30 days: one bar per day on one baseline, today in full colour, the day's average beside the title. 7 days: every
 * bar's play time on top ("0" on a day without play). 30 days: today's and the longest day's (when they don't meet), a
 * date under every 5th day counted back from today; the bar under the pointer shows its own time and date instead. All of
 * it comes from the figures alone, so the first drawing is already the final one. */
function Days({ d, fmtSecs }: { d: Stats; fmtSecs: (s: number) => string }) {
  const { t, locale } = useHost();
  const n = d.buckets.length, last = n - 1;
  const most = Math.max(1, ...d.buckets.map(b => b.seconds));
  const many = n > 10, every = many ? 5 : 1;
  const top = d.buckets.reduce((best, b, i) => (b.seconds > d.buckets[best].seconds ? i : best), 0);
  const topShown = many && top !== last && last - top >= 3 && d.buckets[top]?.seconds > 0;
  // Records that don't reach back over the whole period (no period before to compare with): the average counts the days
  // since the first one with play, not days nothing could have been recorded on.
  const first = d.previous === null ? d.buckets.findIndex(b => b.seconds > 0) : 0;
  const avg = d.totalSeconds / Math.max(1, first < 0 ? n : n - first);
  return <div className="stat-card" id="stats-days"><h3>{t('stats.byDay')}{avg >= 60 ? <span className="days-avg">{t('stats.avgDay', { t: fmtSecs(avg) })}</span> : null}</h3>
    <div className={`days${many ? ' many' : ''}`}>{d.buckets.map((b, i) => {
      const day = new Date(b.start), today = i === last;
      const label = today ? t('alog.today')
        : many ? day.toLocaleDateString(locale, { day: 'numeric', month: 'numeric' }) : day.toLocaleDateString(locale, { weekday: 'short' });
      const cls = ['day', today && 'today', topShown && i === top && 'peak', !b.seconds && 'none', (last - i) % every === 0 && 'lab'].filter(Boolean).join(' ');
      return <div key={b.start} className={cls}>
        <span className="day-bar"><b>{b.seconds ? barTime(b.seconds) : 0}</b><i style={{ height: `calc((100% - 18px) * ${(b.seconds / most).toFixed(3)})` }} /></span>
        <small>{label}</small>
      </div>;
    })}</div>
  </div>;
}

/** 7 and 30 days by hour: a row a day (today last), a cell an hour, the darker the more windows ran in it. The nights a
 * PC was off or the hours everything dropped show as gaps; the cell under the pointer says its day, hour and windows. */
function Heat({ d, heat }: { d: Stats; heat: number[][] }) {
  const { t, lang, locale, fmtNum } = useHost();
  const most = Math.max(.01, ...heat.flat());
  const first = new Date(d.since), last = heat.length - 1, many = heat.length > 10;
  const dayOf = (i: number) => { const day = new Date(first); day.setDate(first.getDate() + i); return day; };
  const dayText = (i: number) => i === last ? t('alog.today')
    : dayOf(i).toLocaleDateString(locale, many ? { day: 'numeric', month: 'numeric' } : { weekday: 'short' });
  const hourOf = (h: number) => hourText(locale, lang, h);
  // Five steps of the accent over the card: an empty hour stays the well colour.
  const level = (v: number) => v <= 0 ? 0 : Math.min(4, Math.ceil(v / most * 4));
  return <div className="stat-card" id="stats-heat"><h3>{t('stats.byHour')}<span className="heat-key" aria-hidden="true">{t('stats.heatLess')}
    {[0, 1, 2, 3, 4].map(l => <i key={l} className={`h${l}`} />)}{t('stats.heatMore')}</span></h3>
    <div className={`heat${many ? ' many' : ''}`} role="img" aria-label={t('stats.byHour')}>
      <span style={{ gridRow: 1 }} />{Array.from({ length: 8 }, (_, k) => <small key={k} className="heat-h" style={{ gridRow: 1, gridColumn: `${k * 3 + 2} / span 3` }}>{hourOf(k * 3)}</small>)}
      {heat.map((row, i) => { const day = dayText(i); return <Fragment key={i}>
        <small className={`heat-d${i === last ? ' today' : ''}`}>{many && (last - i) % 5 && i !== last ? '' : day}</small>
        {row.map((v, h) => <i key={h} className={`h${level(v)}`} title={t('stats.heatTip', { d: day, h: hourOf(h), n: fmtNum(Math.round(v * 10) / 10) })} />)}
      </Fragment>; })}
    </div>
  </div>;
}

/** Each account's stays from the period's start up to now, each with its game's icon (its name too where it fits), a
 * red end where one dropped. Today every account's; for 7 and 30 days one account's (only), opened from its row. */
function Timeline({ d, nameOf, fmtSecs, only }: { d: Stats; nameOf: (a: StatAccount) => string; fmtSecs: (s: number) => string; only?: string }) {
  const { t, lang, locale } = useHost();
  // Up to now, not the whole day: at 2 am a day's width would squeeze everything into its first tenth. Two hours at least.
  const since = new Date(d.since).getTime(), span = Math.max(Date.now() - since, 2 * 3600e3);
  const at = (ms: number) => `${Math.min(100, Math.max(0, (ms - since) / span * 100)).toFixed(2)}%`;
  const clock = (ms: number) => d.hourly
    ? clockText(locale, lang, ms)
    : new Date(ms).toLocaleString(locale, { weekday: 'short', hour: '2-digit', minute: '2-digit', ...(lang === 'vi' ? { hourCycle: 'h23' as const } : {}) });
  // Labels: today by how much of it has gone, every day of the week, every 5th day of the month counted back from today.
  const hours = span / 3600e3;
  const every = d.hourly ? (hours <= 6 ? 1 : hours <= 12 ? 2 : hours <= 18 ? 3 : 6) : d.buckets.length > 10 ? 5 : 1;
  const rows = only ? d.timeline.filter(r => r.id === only) : d.timeline;
  const inner = <>
    <div className={`tl${only ? ' one' : ''}`}>{rows.map(row => {
      const a = d.accounts.find(x => x.id === row.id);
      if (!a) return null;
      const label = nameOf(a);
      return <div key={row.id} className="tl-row">
        {only ? null : <div className="sacc"><AccAvatar a={a} label={label} hide={label !== a.label} /><b title={label}>{label}</b></div>}
        <div className="tl-track">
          {row.segments.map((s, i) => {
            const from = new Date(s.from).getTime(), to = new Date(s.to).getTime();
            const share = (to - from) / span * 100, icon = d.icons?.[s.game];
            const title = [s.name, `${clock(from)} – ${clock(to)}`, fmtSecs((to - from) / 1000), s.reason ? t(`stats.why.${s.reason}`) : ''].filter(Boolean).join(' · ');
            return <i key={i} className={`tl-seg ${s.reason ? 'drop' : ''}`} title={title}
              style={{ left: at(from), width: `${Math.max(.3, share).toFixed(2)}%` }}>
              {icon && share >= 2 ? <img src={icon} alt="" onError={e => { e.currentTarget.hidden = true; }} /> : null}{share >= 12 ? <span>{s.name}</span> : null}</i>;
          })}
        </div>
      </div>;
    })}</div>
    <div className={`tl-axis${only ? ' one' : ''}`} aria-hidden="true">{d.buckets.map((b, i) => {
      const start = new Date(b.start);
      if ((d.hourly ? i : d.buckets.length - 1 - i) % every || (start.getTime() - since) / span > .95) return null;
      const label = d.hourly ? hourText(locale, lang, start.getHours())
        : every > 1 ? start.toLocaleDateString(locale, { day: 'numeric', month: 'numeric' }) : start.toLocaleDateString(locale, { weekday: 'short' });
      return <span key={i} style={{ left: at(start.getTime()) }}>{label}</span>;
    })}</div>
    {rows.some(r => r.segments.some(s => s.reason))
      ? <div className="tl-keys"><span><i className="tl-key-drop" /><em>{t('stats.kicks')}</em></span></div> : null}
  </>;
  return only ? inner : <div className="stat-card" id="stats-timeline"><h3>{t('stats.timeline')}</h3>{inner}</div>;
}

/** The time disconnects cost, by why they happened, with what would have kept the account in where Bloxkeep has it. */
function Drops({ d, nameOf, fmtSecs }: { d: Stats; nameOf: (a: StatAccount) => string; fmtSecs: (s: number) => string }) {
  const { t, tn, go, fmtNum } = useHost();
  const app = useAppState();
  if (!d.losses.length) return null;
  const accounts: (Account & { autoRejoin?: boolean })[] = app.accounts || [];
  const total = d.losses.reduce((sum, l) => sum + l.seconds, 0);
  const fix = (l: Loss) => l.kind === 'idle' ? !app.settings?.antiAfk && 'stats.do.antiAfk'
    : l.kind === 'other' ? l.accounts.some(id => accounts.find(a => a.id === id)?.autoRejoin === false) && 'stats.do.rejoin' : false;
  return <div className="stat-card" id="stats-drops"><h3>{t('stats.drops')}<span className="sd-lost" title={t('stats.lostTip')}>{t('stats.lost', { t: fmtSecs(total) })}</span></h3>
    {d.losses.map(l => {
      const names = l.accounts.map(id => d.accounts.find(a => a.id === id)).filter((a): a is StatAccount => !!a).map(nameOf).join(', ');
      const action = fix(l);
      return <div key={l.kind} className="sd-row" data-kind={l.kind}>
        <div className="sd-text"><b>{t(`stats.why.${l.kind}`)}</b><small title={names}>{tn('stats.timesN', l.times, { n: fmtNum(l.times) })} · {names}</small></div>
        <span className="sd-time" title={t('stats.lostTip')}>{t('stats.lost', { t: fmtSecs(l.seconds) })}</span>
        {action ? <button className="btn sm" onClick={() => go('settings:perf')}>{t(action)}</button> : null}
      </div>;
    })}
  </div>;
}

/** Activity's list: grouped by day, each event at its clock time (today's also as "5 min ago" on hover). */
function Log({ log: d, failed, filtered, retry, more }: {
  log: Activity | null; failed: boolean; filtered: boolean; retry: (btn: HTMLElement) => void; more: (btn: HTMLElement) => void;
}) {
  const host = useHost();
  const { t, lang, locale, fmtNum, ago } = host;
  const app = useAppState();
  const accounts: Account[] = app.accounts || [];
  if (!d) {
    return failed
      ? <p className="alog-empty">{t('alog.error')} <button type="button" className="link" onClick={e => retry(e.currentTarget)}>{t('srv.retry')}</button></p>
      : <div className="alog-empty"><span className="spin" /></div>;
  }
  if (!d.entries.length) return <p className="alog-empty">{t(filtered ? 'alog.emptyFilter' : 'alog.empty')}</p>;
  const now = new Date(), today = now.toDateString();
  const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1).toDateString();
  const clockOf = (at: Date) => clockText(locale, lang, at);
  const days: { key: string; at: Date; entries: Entry[] }[] = [];
  for (const e of d.entries) {
    const at = new Date(e.at), key = at.toDateString();
    if (days.at(-1)?.key !== key) days.push({ key, at, entries: [] });
    days.at(-1)!.entries.push(e);
  }
  return <>
    {days.map(({ key, at: dayAt, entries }) => <Fragment key={key}>
      <h4 className="alog-day">{key === today ? t('alog.today') : key === yesterday ? t('alog.yesterday')
        : dayAt.toLocaleDateString(locale, { weekday: 'long', day: 'numeric', month: 'long', ...(dayAt.getFullYear() !== now.getFullYear() ? { year: 'numeric' } : {}) })}</h4>
      <ol className="alog-list">{entries.map((e, i) => {
        const at = new Date(e.at);
        const full = at.toLocaleString(locale, { dateStyle: 'full', timeStyle: 'medium' });
        const a = e.accountId ? accounts.find(x => x.id === e.accountId) : null;
        return <li key={i} className={`alog-row ${e.tone}`}>
          <i className="dot" aria-hidden="true" />
          <time dateTime={e.at} title={key === today ? `${ago(e.at)} · ${full}` : full}>{clockOf(at)}</time>
          {e.account
            ? <span className="alog-acc" title={e.account}>{a ? <span className="avatar sm"><Avatar a={a} /></span> : null}<b>{e.account}</b></span>
            : <span className="alog-acc none" aria-hidden="true">—</span>}
          <span className="alog-text">{e.text}</span>
        </li>;
      })}</ol>
    </Fragment>)}
    {d.total > d.entries.length ? <div className="alog-more"><button className="btn sm" onClick={e => more(e.currentTarget)}>{t('alog.more')}</button>
      <small>{t('alog.shown', { n: fmtNum(d.entries.length), total: fmtNum(d.total) })}</small></div> : null}
  </>;
}

/** An account's picture; a deleted account's is a plain figure (its old picture may be someone else's by now). */
function AccAvatar({ a, label, hide }: { a: StatAccount; label: string; hide: boolean }) {
  if (a.deleted) return <span className="avatar sm gone" aria-hidden="true"><Icon name="user" /></span>;
  return <span className="avatar sm"><Avatar a={{ avatarUrl: a.avatarUrl, username: hide ? a.id : a.username || a.id, label }} /></span>;
}

/** A game's picture (gamepad placeholder when broken), loaded eagerly while the tab is hidden so the icons show at once. */
function GameImg({ src }: { src: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? <span className="gph"><Icon name="gamepad" /></span>
    : <img src={src} alt="" onLoad={e => { e.currentTarget.decode?.().catch(() => {}); }} onError={() => setFailed(true)} />;
}
