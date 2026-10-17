// The Statistics tab, two things on it one at a time: Statistics (play time per account, game and preset, for a
// period, with its chart) and Activity (what the app saw happen, newest first, a page at a time, filtered by account
// or kind). The figures are asked for when the tab opens and every 30 s while it is on screen; they aren't part of the
// state the app pushes. The chart is plain divs, sized from the busiest bucket.
import { Fragment, useEffect, useImperativeHandle, useRef, useState, type CSSProperties, type ReactNode, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';
import { Avatar } from './Avatar';

const STATS_PERIODS = [1, 7, 30];
const BAR_HEIGHT = 140;
const AL_GROUPS = ['play', 'drops', 'schedule', 'signin', 'updates'];
const AL_PAGE = 100;
const NO_PRESET = '00000000-0000-0000-0000-000000000000';

type Bucket = { start: string; seconds: number };
type StatAccount = { id: string; label?: string; username?: string; avatarUrl?: string; seconds: number; sessions: number; kicks: number; deleted?: boolean };
type StatGame = { name: string; iconUrl?: string; seconds: number; sessions: number };
type StatPreset = { id: string; name: string; iconUrl?: string; seconds: number; accounts: number; kicks: number };
type Stats = {
  sessions: number; recorded: boolean; totalSeconds: number; kicks: number; hourly: boolean; buckets: Bucket[];
  accounts: StatAccount[]; games: StatGame[]; presets?: StatPreset[]; longest?: { account: string; label?: string; game?: string; seconds: number } | null;
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
  const [filter, setFilter] = useState({ account: '', group: '', limit: AL_PAGE });
  const [log, setLog] = useState<Activity | null>(null);
  const [logFailed, setLogFailed] = useState(false);
  const scroll = useRef<HTMLDivElement>(null);

  // A reply to an older request (or for another period, other filters) is dropped. A failed refresh keeps figures
  // already shown; with none, it says so.
  const seq = useRef(0), logSeq = useRef(0), alive = useRef(true);
  const now = useRef({ days, filter });
  now.current = { days, filter };
  useEffect(() => () => { alive.current = false; }, []);
  const loadStats = async () => {
    const mine = ++seq.current;
    const d = await call('playStats', { days: now.current.days }, { silent: true }) as Stats | undefined;
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

  // While the tab is on screen: fresh figures now and every 30 s (a stay still going grows), not while the window is
  // hidden, and as soon as it comes back.
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
    return () => { stopShow(); stopHide(); clearInterval(timer); document.removeEventListener('visibilitychange', back); };
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
        <div className="chips" id="stats-period" role="group" aria-label={t('stats.title')} hidden={!onStats}>
          {STATS_PERIODS.map(d => <button key={d} className={`chip ${d === days ? 'on' : ''}`} data-days={d} aria-pressed={d === days} onClick={() => setPeriod(d)}>{t(`stats.period.${d}`)}</button>)}
        </div>
      </div>
    </header>
    <div className="scroll" ref={scroll}>
      <div className="stats">
        <div className="stats-in" id="stats-body" hidden={!onStats}>
          <Figures data={data} failed={failed} retry={btn => busy(btn, loadStats)} />
        </div>
        <section className="stat-card alog" id="alog-sec" aria-labelledby="alog-h" hidden={onStats}>
          <div className="alog-head"><h3 id="alog-h">{t('alog.title')}</h3>
            <div className="alog-tools">
              {/* The accounts as the log names them; the filters as chosen. */}
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

/** Statistics: tiles, the chart by hour or day, then by account, top games and by preset. */
function Figures({ data: d, failed, retry }: { data: Stats | null; failed: boolean; retry: (btn: HTMLElement) => void }) {
  const host = useHost();
  const { t, tn, lang, locale, fmtNum } = host;
  const app = useAppState();
  const accounts: Account[] = app.accounts || [];
  const byId = (id: string) => accounts.find(a => a.id === id);
  const hide = !!app.hideNames;
  /** "2 h", "2 h 5 min", "1,234 h": hours grouped like other numbers, no "0 min". */
  const fmtSecs = (s: number) => {
    const m = Math.floor((s || 0) / 60), h = Math.floor(m / 60);
    return m < 60 ? t('dur.min', { m }) : m % 60 ? t('dur.hour', { h: fmtNum(h), m: m % 60 }) : t('dur.hours', { h: fmtNum(h) });
  };

  if (!d) {
    return failed
      ? <div className="empty"><h2>{t('stats.error')}</h2><div className="row"><button className="btn" onClick={e => retry(e.currentTarget)}><Icon name="refresh" />{t('srv.retry')}</button></div></div>
      : <div className="empty"><span className="spin" /></div>;
  }
  if (!d.sessions) {
    // The same empty state as the other tabs: the tab's icon, a title, one line.
    return <div className="empty"><span className="pv-empty-icon"><Icon name="clock" /></span>
      {d.recorded ? <h2>{t('stats.emptyPeriod')}</h2> : <><h2>{t('stats.emptyTitle')}</h2><p>{t('stats.emptyText')}</p></>}</div>;
  }
  const top = d.games[0];
  // Two more on a wide window (see .stat.wide): how many accounts played, and the longest stay (whose, in what).
  const longName = d.longest ? (hide ? byId(d.longest.account)?.label : d.longest.label) || t('stats.deleted') : '';
  const played = fmtNum(d.accounts.filter(a => a.seconds > 0).length), all = fmtNum(accounts.length);
  const tiles: [string, ReactNode, string, string, string?][] = [
    [t('stats.total'), fmtSecs(d.totalSeconds), '', ''], [t('stats.sessions'), fmtNum(d.sessions), '', ''], [t('stats.kicks'), fmtNum(d.kicks), '', ''],
    [t('stats.topGame'), top ? top.name : '—', '', ''],
    [t('stats.played'), <>{played}<span className="of">/{all}</span></>, 'wide', '', `${played}/${all}`],
    [t('stats.longest'), d.longest ? fmtSecs(d.longest.seconds) : '—', 'wide', d.longest ? [longName, d.longest.game].filter(Boolean).join(' · ') : ''],
  ];
  // By preset: what each preset's windows played (laid out like Top games: a list fits a narrow column better).
  const presetTop = Math.max(1, ...(d.presets || []).map(p => p.seconds));
  // The chart's labels: every hour of 6 today, every day of the week, every 5th day of the month counted back from
  // today (so today always has one). The bar holding this moment is marked as now; on Today the hours still to come
  // are drawn fainter.
  const max = Math.max(1, ...d.buckets.map(b => b.seconds));
  const every = d.hourly ? 6 : d.buckets.length > 10 ? 5 : 1;
  const nowMs = Date.now();
  const nowIndex = d.buckets.findLastIndex(b => new Date(b.start).getTime() <= nowMs);
  const sessionsText = (n: number) => (n === 1 ? t('stats.session1') : t('stats.sessionsN', { n: fmtNum(n) }));

  return <>
    <div className="stat-tiles">{tiles.map(([label, value, cls, sub, plain], i) =>
      <div key={i} className={`stat ${cls}`}><small>{label}</small><b title={sub || plain || String(value)}>{value}</b>{sub ? <span className="stat-sub">{sub}</span> : null}</div>)}
    </div>
    <div className="stat-card"><h3>{t(d.hourly ? 'stats.byHour' : 'stats.byDay')}</h3>
      <div className="sbars" style={{ '--bar-h': `${BAR_HEIGHT}px` } as CSSProperties}>{d.buckets.map((b, i) => {
        const at = new Date(b.start);
        // Hours as people say them: "6 AM" in English, "06:00" (24-hour) in Vietnamese.
        const label = d.hourly ? at.toLocaleTimeString(locale, lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric' })
          : every > 1 ? at.toLocaleDateString(locale, { day: 'numeric', month: 'numeric' }) : at.toLocaleDateString(locale, { weekday: 'short' });
        const when = d.hourly ? label : at.toLocaleDateString(locale, { weekday: 'short', day: 'numeric', month: 'short' });
        const h = b.seconds ? Math.max(3, Math.round(b.seconds / max * BAR_HEIGHT)) : 2;
        const state = i === nowIndex ? 'now' : i > nowIndex ? 'future' : '';
        return <div key={i} className={`sbar ${state} ${b.seconds ? '' : 'zero'}`} title={`${when} · ${fmtSecs(b.seconds)}`}>
          <i style={{ height: `${h}px` }} /><span>{(d.hourly ? i : d.buckets.length - 1 - i) % every === 0 ? label : ''}</span></div>;
      })}</div>
    </div>
    <div className="stat-cols">
      <div className="stat-card"><h3>{t('stats.byAccount')}</h3>
        <table className="stat-table">
          <thead><tr><th>{t('stats.account')}</th><th className="n">{t('stats.total')}</th><th className="n">{t('stats.sessions')}</th><th className="n">{t('stats.kicks')}</th></tr></thead>
          <tbody>{d.accounts.map(a => {
            // A deleted account keeps the name it had when it played (older records have none). Names hidden: the
            // list's "Account N", and a deleted one is just that.
            const label = hide ? byId(a.id)?.label || t('stats.deleted') : a.label || t('stats.deleted');
            return <tr key={a.id}>
              <td><div className="sacc"><span className="avatar sm"><Avatar a={{ avatarUrl: a.avatarUrl, username: hide ? a.id : a.username || a.id, label }} /></span>
                <b title={label}>{label}{a.deleted && a.label && !hide ? <>{' '}<small className="muted">· {t('stats.deletedTag')}</small></> : null}</b></div></td>
              <td className="n">{fmtSecs(a.seconds)}</td><td className="n">{fmtNum(a.sessions)}</td><td className="n">{fmtNum(a.kicks)}</td>
            </tr>;
          })}</tbody>
        </table>
      </div>
      <div className="stat-side">
        <div className="stat-card"><h3>{t('stats.topGames')}</h3><div className="sgames">{d.games.map((g, i) =>
          <div key={i} className="sgame"><em>{i + 1}</em>{g.iconUrl ? <GameImg src={g.iconUrl} /> : <span className="gph"><Icon name="gamepad" /></span>}
            <div className="gtxt"><b title={g.name}>{g.name}</b><small>{sessionsText(g.sessions)}</small>
              <div className="sg-meter"><i style={{ transform: `scaleX(${(g.seconds / Math.max(1, top.seconds)).toFixed(3)})` }} /></div></div>
            <span className="sg-time">{fmtSecs(g.seconds)}</span></div>)}</div></div>
        {d.presets?.length ? <div className="stat-card"><h3>{t('stats.byPreset')}</h3><div className="sgames">{d.presets.map(p => {
          const own = p.id !== NO_PRESET, name = own ? p.name : t('stats.noPreset');
          const detail = [tn('stats.accountsN', p.accounts), p.kicks ? tn('stats.kicksN', p.kicks) : ''].filter(Boolean).join(' · ');
          return <div key={p.id} className="sgame">{own && p.iconUrl ? <GameImg src={p.iconUrl} /> : <span className="gph"><Icon name={own ? 'layers' : 'user'} /></span>}
            <div className="gtxt"><b title={own ? name : t('stats.noPresetTip')}>{name}</b><small>{detail}</small>
              <div className="sg-meter"><i style={{ transform: `scaleX(${(p.seconds / presetTop).toFixed(3)})` }} /></div></div>
            <span className="sg-time">{fmtSecs(p.seconds)}</span></div>;
        })}</div>
          {d.presets.every(p => p.id === NO_PRESET) ? <p className="stat-note">{t('stats.presetNote')}</p> : null}</div> : null}
      </div>
    </div>
  </>;
}

/** Activity's list: grouped by day; today's times as "5 min ago" (the exact time on hover), older ones as clock times. */
function Log({ log: d, failed, filtered, retry, more }: {
  log: Activity | null; failed: boolean; filtered: boolean; retry: (btn: HTMLElement) => void; more: (btn: HTMLElement) => void;
}) {
  const host = useHost();
  const { t, lang, locale, fmtNum } = host;
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
  const clockOf = (at: Date) => at.toLocaleTimeString(locale, lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric', minute: '2-digit' });
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
          {key === today ? <AgoTime iso={e.at} title={full} /> : <time dateTime={e.at} title={full}>{clockOf(at)}</time>}
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

/** "5 min ago" in a <time>, kept fresh while shown. */
function AgoTime({ iso, title }: { iso: string; title: string }) {
  const { ago } = useHost();
  const [, tick] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => tick(n => n + 1), 30000);
    return () => clearInterval(timer);
  }, []);
  return <time dateTime={iso} title={title}>{ago(iso)}</time>;
}

/** A game's picture; a broken one becomes the gamepad placeholder. */
function GameImg({ src }: { src: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? <span className="gph"><Icon name="gamepad" /></span> : <img src={src} alt="" loading="lazy" onError={() => setFailed(true)} />;
}
