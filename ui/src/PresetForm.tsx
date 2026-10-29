// The preset form (new or edit): name, game (the box works like the dock's: recent, pinned, favourite and popular
// games, a name searched, or a link or Place ID pasted, the chosen game shown under it), kind of server, wait between
// accounts, Job ID; the accounts, ticked, in their launch order; options for the windows it opens; its own Discord
// channel; the schedule. app.js's dialog keeps the title, the buttons and where a refused save says why; it asks this
// form for what to save (collect), which box a refusal is about (refusedBox) and whether anything changed (dirty).
import { useEffect, useImperativeHandle, useRef, useState, type KeyboardEvent, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';
import { OrderList, type OrderAccount, type OrderListApi } from './OrderList';
import { DateInput, TimeInput } from './TimeField';

const PRESET_SERVERS = ['any', 'most', 'fewest', 'ping', 'random', 'job', 'follow'];
const RESTART_HOURS = [0, 2, 4, 6, 12]; // the app's own choices (RestartChoices)
// The wait between accounts, in seconds (the app keeps 0 to 120); a preset saved with another number keeps it too.
const DELAYS = [0, 5, 10, 15, 30, 45, 60, 90, 120];
const TABS = ['recent', 'pinned', 'favorites', 'popular'];
const WEEK = [1, 2, 3, 4, 5, 6, 0]; // Monday first

type Schedule = { enabled?: boolean; time?: string; days?: number[]; date?: string; stop?: string; stopHours?: number; stopTime?: string };
export type PresetBase = {
  id?: string; name: string; placeInput: string; server: string; jobId: string; followPlayer?: string; delaySeconds: number | null;
  schedule: Schedule | null; antiAfk: boolean | null; autoRejoin: boolean | null; rejoinWith?: string; profile: string; restartHours: number;
  arrange: boolean | null; discord: string | null;
};
type Game = { placeId: number; name: string; iconUrl?: string; playing?: number };
export interface PresetFormApi {
  /** What Save sends (savePreset's args), or why it can't and the box to show. */
  collect(): { args: Record<string, unknown> } | { error: string; box: HTMLElement | null };
  /** The box the app's refusal is most likely about, from what the form holds. */
  refusedBox(): HTMLElement | null;
  /** Anything changed since it opened (Esc or the X ask first then). */
  dirty(): boolean;
}

const tri = (v: boolean | null | undefined) => v === true ? 'on' : v === false ? 'off' : '';
const fromTri = (v: string) => v === 'on' ? true : v === 'off' ? false : null;
/** Today as the date box writes it (the picker opens on the right month). */
const today = () => new Date(Date.now() - new Date().getTimezoneOffset() * 60000).toISOString().slice(0, 10);

export function PresetForm({ base, isNew, accounts, ticked, gone, api }: {
  base: PresetBase; isNew: boolean; accounts: OrderAccount[]; ticked: string[]; gone: number; api: Ref<PresetFormApi>;
}) {
  const host = useHost();
  const { t, tn, call, busy, toast } = host;
  const app = useAppState();
  const sc = base.schedule || {};

  const [name, setName] = useState(base.name);
  // A name that is empty or still the one filled in automatically follows the game picked; a typed one stays.
  const autoName = useRef(isNew ? base.name : '');
  const [place, setPlace] = useState(base.placeInput);
  const [server, setServer] = useState(base.server);
  const [delay, setDelay] = useState(base.delaySeconds === null || base.delaySeconds === undefined ? '' : String(base.delaySeconds));
  const [job, setJob] = useState(base.jobId || '');
  const [follow, setFollow] = useState(base.followPlayer || '');
  const [count, setCount] = useState(ticked.length);
  const order = useRef<OrderListApi>(null);
  const [afk, setAfk] = useState(tri(base.antiAfk));
  const [rejoin, setRejoin] = useState(tri(base.autoRejoin));
  const [rejoinWith, setRejoinWith] = useState(base.rejoinWith || '');
  const [profile, setProfile] = useState(base.profile || '');
  const [restart, setRestart] = useState(String(base.restartHours || 0));
  const [arrange, setArrange] = useState(tri(base.arrange));
  const [dcMode, setDcMode] = useState(base.discord ? 'own' : '');
  const [dcLink, setDcLink] = useState('');
  const [schOn, setSchOn] = useState(!!sc.enabled);
  const [time, setTime] = useState(sc.time || '');
  const [mode, setMode] = useState(sc.date ? 'once' : 'days');
  const [days, setDays] = useState<number[]>(sc.days || []);
  const [date, setDate] = useState(sc.date || today());
  const [stop, setStop] = useState(sc.stop || 'never');
  const [hours, setHours] = useState(String(sc.stopHours && sc.stopHours > 0 ? sc.stopHours : 2));
  const [stopTime, setStopTime] = useState(sc.stopTime || '');

  const boxes = useRef<Record<string, HTMLElement | null>>({});
  const box = (key: string) => (el: HTMLElement | null) => { boxes.current[key] = el; };
  const form = useRef<HTMLDivElement>(null);

  const snapshot = () => JSON.stringify([name, place, server, delay, job, follow, order.current?.ids(), order.current?.ticked(), afk, rejoin, rejoinWith, profile,
    restart, arrange, dcMode, dcLink, schOn, time, mode, [...days].sort(), date, stop, hours, stopTime]);
  const initial = useRef('');
  useEffect(() => { initial.current = snapshot(); }, []); // once, as it opened

  const latest = useRef({ snapshot, collect: () => ({}) as ReturnType<PresetFormApi['collect']>, refusedBox: () => null as HTMLElement | null });
  latest.current.snapshot = snapshot;
  latest.current.collect = () => {
    // Checked here rather than clamped quietly by the app: 1000 hours or a quarter hour would save as something else.
    const h = +hours.trim();
    if (schOn && stop === 'after' && !(h >= 0.5 && h <= 168 && Number.isInteger(h * 2))) return { error: t('ps.schHoursBad'), box: boxes.current.hours };
    // Its own Discord channel: undefined keeps the saved link, '' goes back to Settings' channel, a link replaces it;
    // chosen but no link pasted is said here, next to the box.
    let discordWebhook: string | undefined;
    if (dcMode !== 'own') discordWebhook = base.discord ? '' : undefined;
    else if (dcLink.trim()) discordWebhook = dcLink.trim();
    else if (!base.discord) return { error: t('ps.dc.needLink'), box: boxes.current.dcLink };
    const d = delay.trim();
    return {
      args: {
        id: base.id || '', name: name.trim(), placeInput: place.trim(), server, jobId: job.trim(), followPlayer: follow.trim(),
        ids: order.current!.ticked(), // ticked order = launch order
        delaySeconds: d === '' ? null : Math.min(120, Math.max(0, Math.round(+d) || 0)),
        antiAfk: fromTri(afk), autoRejoin: fromTri(rejoin), rejoinWith: withOn ? withShown : '', profile, restartHours: +restart || 0, arrange: fromTri(arrange),
        discordWebhook,
        // A schedule never set up (off, no start time) is saved as none: no "Schedule off" row, no switch that couldn't
        // be turned on.
        schedule: !schOn && !time.trim() ? null : {
          enabled: schOn, time: time.trim(), days: mode === 'once' ? [] : WEEK.filter(x => days.includes(x)),
          date: mode === 'once' ? date.trim() : '', stop, stopHours: +hours.trim() || 0, stopTime: stopTime.trim(),
        },
      },
    };
  };
  latest.current.refusedBox = () => {
    const firstTick = form.current?.parentElement?.querySelector<HTMLElement>('.ord-item input') || null;
    const firstDay = boxes.current.days?.querySelector<HTMLElement>('.chip') || null;
    return !name.trim() ? boxes.current.name
      : !order.current!.ticked().length ? firstTick
      : server === 'job' && !job.trim() ? boxes.current.job
      : server === 'follow' && !follow.trim() ? boxes.current.follow
      : schOn && !time.trim() ? boxes.current.time
      : schOn && mode !== 'once' && !days.length ? firstDay
      : schOn && mode === 'once' && !date.trim() ? boxes.current.date : null;
  };
  useImperativeHandle(api, () => ({
    collect: () => latest.current.collect(),
    refusedBox: () => latest.current.refusedBox(),
    dirty: () => latest.current.snapshot() !== initial.current,
  }), []);

  // Which server a dropped window rejoins: any, another account's, or a ticked account's. Only with a game, not when
  // following a player (its rejoins follow them) nor with auto-rejoin off; an account unticked since falls back to any.
  const tickedIds = order.current?.ticked() ?? ticked;
  const withOn = !!place.trim() && server !== 'follow' && rejoin !== 'off';
  const withShown = rejoinWith === '' || rejoinWith === 'any' || tickedIds.includes(rejoinWith) ? rejoinWith : '';
  // One short note under the options, only for a choice that needs explaining (a line each).
  const optNote = [withOn && withShown ? t('ps.opt.withNote') : '', profile === 'afk' ? t('ps.opt.lightNote') : '', +restart ? t(place.trim() || server === 'follow' ? 'ps.opt.restartNote' : 'ps.opt.restartNoGame') : '']
    .filter(Boolean).join('\n');
  const followSet = (on: boolean | number) => t('ps.opt.followSet',
    { state: typeof on === 'number' ? t('set.seconds', { n: on }) : t(on ? 'ps.opt.stOn' : 'ps.opt.stOff') });
  const triSelect = (id: string, value: string, set: (v: string) => void, follow: string) =>
    <select className="select" id={id} value={value} onChange={e => set(e.target.value)}>
      {[['', follow], ['on', t('ps.opt.on')], ['off', t('ps.opt.off')]].map(([k, text]) => <option key={k} value={k}>{text}</option>)}
    </select>;
  const dcOn = !!app.discord?.enabled;

  return <>
    <div className="form preset-form" ref={form}>
      <label>{t('ps.name')}<input className="field" id="ps-name" ref={box('name')} maxLength={60} placeholder={t('ps.namePh')} value={name} onChange={e => setName(e.target.value)} /></label>
      <GameBox place={place} setPlace={setPlace} chose={g => {
        if (!name.trim() || name === autoName.current) {
          setName(g.name);
          autoName.current = g.name;
        }
      }} />
      <div className="preset-row">
        <label>{t('ps.server')}<select className="select" id="ps-server" value={server} onChange={e => setServer(e.target.value)}>
          {PRESET_SERVERS.map(k => <option key={k} value={k}>{t(`ps.srv.${k}`)}</option>)}</select></label>
        <label>{t('ps.delay')}<select className="select" id="ps-delay" value={delay} onChange={e => setDelay(e.target.value)}>
          <option value="">{followSet(app.settings?.launchDelaySeconds ?? 8)}</option>
          {[...new Set([...DELAYS, ...(delay === '' ? [] : [+delay])])].sort((a, b) => a - b)
            .map(n => <option key={n} value={String(n)}>{t('set.seconds', { n })}</option>)}</select></label>
      </div>
      <label id="ps-job-row" hidden={server !== 'job'}>{t('ps.job')}<input className="field" id="ps-job" ref={box('job')} maxLength={60} spellCheck={false}
        placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value={job} onChange={e => setJob(e.target.value)} /></label>
      <label id="ps-follow-row" hidden={server !== 'follow'}>{t('ps.follow')}<input className="field" id="ps-follow" ref={box('follow')} maxLength={100}
        spellCheck={false} autoComplete="off" list="ps-follow-recent" placeholder={t('jp.placeholder')} value={follow} onChange={e => setFollow(e.target.value)} />
        <datalist id="ps-follow-recent">{(app.settings?.recentPlayers || []).map((n: string) => <option key={n} value={n} />)}</datalist>
        <small className="muted">{t('ps.followNote')}</small></label>
      <div className="preset-accs">{t('ps.accounts')} <span className="muted" id="ps-count">({count})</span><small>{t('ps.order')}</small></div>
      {/* Accounts deleted since the preset was saved aren't in the list: said so, rather than dropped without a word. */}
      {gone ? <p className="ps-gone"><Icon name="info" /><span>{tn('ps.gone', gone)}</span></p> : null}
    </div>
    <OrderList accounts={accounts} ticked={ticked} api={order} onChange={() => setCount(order.current!.ticked().length)} />
    <div className="form preset-form">
      <div className="ps-opts" role="group" aria-labelledby="ps-opts-title">
        <div className="ps-opts-head"><b id="ps-opts-title">{t('ps.opts')}</b></div>
        <div className="ps-opts-grid">
          <label>{t('ps.opt.afk')}{triSelect('ps-afk', afk, setAfk, followSet(!!app.settings?.antiAfk))}</label>
          <label>{t('ps.opt.rejoin')}{triSelect('ps-rejoin', rejoin, setRejoin, t('ps.opt.followAcc'))}</label>
          <label hidden={!withOn}>{t('ps.opt.rejoinWith')}<select className="select" id="ps-rejoin-with" value={withShown} onChange={e => setRejoinWith(e.target.value)}>
            <option value="">{t('ps.opt.anyServer')}</option>
            <option value="any">{t('ps.opt.withAny')}</option>
            {tickedIds.map(id => <option key={id} value={id}>{t('ps.opt.withAcc', { name: accounts.find(a => a.id === id)?.label || id })}</option>)}
          </select></label>
          <label>{t('ps.opt.profile')}<select className="select" id="ps-profile" value={profile} onChange={e => setProfile(e.target.value)}>
            <option value="">{t('ps.opt.normal')}</option><option value="afk">{t('ps.opt.light')}</option></select></label>
          <label>{t('ps.opt.restart')}<select className="select" id="ps-restart" value={restart} onChange={e => setRestart(e.target.value)}>
            {RESTART_HOURS.map(h => <option key={h} value={h}>{h ? t('ps.opt.every', { n: h }) : t('ps.opt.never')}</option>)}</select></label>
          <label>{t('ps.opt.arrange')}{triSelect('ps-arrange', arrange, setArrange, followSet(!!(app.multi?.enabled && app.multi?.tile)))}</label>
        </div>
        <p className="ps-opts-note" id="ps-opts-note" hidden={!optNote}>{optNote}</p>
      </div>
      {/* Its own Discord channel: its schedule messages, its summary after a scheduled stop and the alerts of the windows
          it opened go there. The page never holds a saved link, only its masked form. */}
      <div className="ps-opts" role="group" aria-labelledby="ps-dc-title">
        <div className="ps-opts-head"><b id="ps-dc-title">Discord</b></div>
        <label className="ps-dc-mode">{t('ps.dc.where')}<select className="select" id="ps-dc-mode" value={dcMode} onChange={e => {
          setDcMode(e.target.value);
          if (e.target.value === 'own' && !base.discord) setTimeout(() => boxes.current.dcLink?.focus());
        }}><option value="">{t('ps.dc.main')}</option><option value="own">{t('ps.dc.own')}</option></select></label>
        <div className="ps-dc-own" id="ps-dc-own" hidden={dcMode !== 'own'}>
          <input className="field" id="ps-dc-link" ref={box('dcLink')} type="text" maxLength={300} spellCheck={false} autoComplete="off"
            placeholder={base.discord || 'https://discord.com/api/webhooks/…'} aria-label={t('ps.dc.link')} value={dcLink} onChange={e => setDcLink(e.target.value)} />
          <button type="button" className="btn" id="ps-dc-test" onClick={e => busy(e.currentTarget, async () => {
            if (await call('presetDiscordTest', { id: base.id || '', url: dcLink.trim() }) !== undefined) toast(t('dc.testOk'), 'ok');
          })}>{t('dc.test')}</button>
        </div>
        <p className="ps-opts-note">{base.discord ? `${t('ps.dc.saved')}\n` : ''}{t('ps.dc.note')}{dcOn ? '' : `\n${t('ps.dc.off')}`}</p>
      </div>
      <div className="ps-sched">
        <div className="ps-sched-head">
          <label className="switch"><input type="checkbox" id="ps-sch-on" checked={schOn} aria-label={t('ps.sch')} onChange={e => setSchOn(e.target.checked)} /><span /></label>
          <div><b>{t('ps.sch')}</b><small>{t('ps.schDesc')}</small></div>
        </div>
        <div className="ps-sched-body" id="ps-sch-body" hidden={!schOn}>
          <div className="preset-row">
            <label>{t('ps.schTime')}<TimeInput className="field" id="ps-sch-time" ref={box('time')} value={time} onChange={e => setTime(e.target.value)} /></label>
            <label>{t('ps.schRepeat')}<select className="select" id="ps-sch-mode" value={mode} onChange={e => setMode(e.target.value)}>
              <option value="days">{t('ps.schDays')}</option><option value="once">{t('ps.schOnce')}</option></select></label>
          </div>
          <div className="day-chips" id="ps-sch-days" ref={box('days')} role="group" aria-label={t('ps.schDays')} hidden={mode === 'once'}>
            {WEEK.map(d => <button key={d} type="button" className={`chip ${days.includes(d) ? 'on' : ''}`} data-day={d} aria-pressed={days.includes(d)}
              onClick={() => setDays(days.includes(d) ? days.filter(x => x !== d) : [...days, d])}>{host.weekdayName(d)}</button>)}
          </div>
          <label id="ps-sch-date-row" hidden={mode !== 'once'}>{t('ps.schDate')}<DateInput className="field" id="ps-sch-date" ref={box('date')} value={date} onChange={e => setDate(e.target.value)} /></label>
          <div className="preset-row">
            <label>{t('ps.schStop')}<select className="select" id="ps-sch-stop" value={stop} onChange={e => setStop(e.target.value)}>
              {['never', 'after', 'at'].map(k => <option key={k} value={k}>{t(`ps.schStop.${k}`)}</option>)}</select></label>
            <label id="ps-sch-hours-row" hidden={stop !== 'after'}>{t('ps.schHours')}<input className="field" id="ps-sch-hours" ref={box('hours')} type="number" min="0.5" max="168" step="0.5"
              value={hours} onChange={e => setHours(e.target.value)} /></label>
            <label id="ps-sch-stoptime-row" hidden={stop !== 'at'}>{t('ps.schStopTime')}<TimeInput className="field" id="ps-sch-stoptime" value={stopTime} onChange={e => setStopTime(e.target.value)} /></label>
          </div>
          <details className="ps-note"><summary><Icon name="info" />{t('ps.schNoteTitle')}</summary><p className="ps-sched-note">{t('ps.schNote')}</p></details>
        </div>
      </div>
    </div>
  </>;
}

/**
 * The game box with its list: focusing it empty offers recent, pinned, favourite and popular games, typing a name
 * searches Roblox, and a link or Place ID can be pasted (the game it is shows under the box). The same keys as the
 * dock's box: arrows walk the list, Enter picks, Esc closes the list (and only the list: the dialog's own Esc and Enter
 * stand back while aria-expanded says a list is open).
 */
function GameBox({ place, setPlace, chose }: { place: string; setPlace: (v: string) => void; chose: (g: Game) => void }) {
  const host = useHost();
  const { t, call } = host;
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<'browse' | 'search'>('browse');
  const [tab, setTab] = useState('recent');
  const [results, setResults] = useState<Game[]>([]);
  const [loading, setLoading] = useState(false);
  const [index, setIndex] = useState(-1);
  const [lists, setLists] = useState<Record<string, Game[]> | null>(() => host.gameListsCached() as Record<string, Game[]> | null);
  const [card, setCard] = useState<Game | null>(null);
  const seq = useRef(0), timer = useRef(0), alive = useRef(true);
  useEffect(() => () => { alive.current = false; clearTimeout(timer.current); }, []);
  const rows = (mode === 'search' ? results : lists?.[tab]) || [];

  const show = async (m: 'browse' | 'search') => {
    setMode(m);
    setIndex(-1);
    setOpen(true);
    if (m === 'browse' && !host.gameListsCached()) {
      setLoading(true);
      const r = await host.loadGameLists() as Record<string, Game[]> | undefined; // shared with the dock's picker
      setLoading(false);
      if (r) {
        setLists(r);
        if (!r[tab]?.length) setTab(TABS.find(k => r[k]?.length) || 'recent');
      }
    }
  };
  const close = () => { setOpen(false); setIndex(-1); };
  const choose = (g: Game) => {
    setPlace(`https://www.roblox.com/games/${g.placeId}`);
    close();
    setCard(g);
    chose(g);
  };
  // A link or Place ID typed or pasted: show which game it is.
  const resolve = async (value: string) => {
    if (!value || !host.looksLikeTarget(value)) return setCard(null);
    const r = await call('resolveGame', { input: value }, { silent: true }) as (Game & { ok?: boolean }) | undefined;
    if (alive.current && latestPlace.current.trim() === value) setCard(r?.ok ? r : null);
  };
  const latestPlace = useRef(place);
  latestPlace.current = place;
  useEffect(() => { resolve(place.trim()); }, []); // the game it opened with

  const typed = (raw: string) => {
    setPlace(raw);
    latestPlace.current = raw;
    const value = raw.trim();
    clearTimeout(timer.current);
    const mine = ++seq.current; // a search still on its way answers an older box
    setLoading(false); // a search that was still pending is over; its placeholders must not stay on
    if (!value) {
      setCard(null);
      return show('browse');
    }
    if (host.looksLikeTarget(value)) {
      close();
      timer.current = window.setTimeout(() => resolve(value), 400);
      return;
    }
    setCard(null);
    const cached = host.searchCached(value.toLowerCase()) as Game[] | null;
    if (cached) {
      setResults(cached);
      return show('search');
    }
    setLoading(true);
    show('search');
    timer.current = window.setTimeout(async () => {
      const r = await call('searchGames', { query: value }, { silent: true }) as Game[] | undefined;
      if (mine !== seq.current || !alive.current) return;
      setResults(r || []);
      setLoading(false);
      if (r) host.rememberSearch(value.toLowerCase(), r);
    }, 200);
  };
  const keys = (e: KeyboardEvent<HTMLInputElement>) => {
    if (!open) return;
    if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && rows.length) {
      e.preventDefault();
      setIndex((index + (e.key === 'ArrowDown' ? 1 : rows.length - 1) + (index < 0 && e.key === 'ArrowUp' ? 1 : 0)) % rows.length);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (loading) return; // results for what was just typed aren't in yet
      if (rows.length && (index >= 0 || mode === 'search')) choose(rows[Math.max(0, index)]);
    } else if (e.key === 'Escape') {
      close();
    }
  };
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => { list.current?.querySelector('.game-row.on')?.scrollIntoView({ block: 'nearest' }); }, [index]);

  return <>
    <label>{t('ps.game')}<input className="field" id="ps-place" maxLength={500} spellCheck={false} autoComplete="off" placeholder={t('dock.place')}
      role="combobox" aria-haspopup="listbox" aria-autocomplete="list" aria-expanded={open} aria-activedescendant={open && index >= 0 ? `psg-${index}` : undefined}
      value={place} onChange={e => typed(e.target.value)} onKeyDown={keys}
      onFocus={() => { if (!place.trim()) show('browse'); }}
      onBlur={() => setTimeout(() => { if (alive.current) close(); }, 120)} /></label>
    {/* mousedown kept from the box: it stays focused while a game is chosen. */}
    <div className="ps-pick" id="ps-pick" hidden={!open} ref={list} onMouseDown={e => e.preventDefault()}>
      {open ? <>
        {mode === 'browse' ? <div className="picker-tabs">{TABS.filter(k => k !== 'popular' || lists?.popular?.length).map(k =>
          // Out of the Tab order: Tab leaves the box for the next field (the list closes when the box loses the focus).
          <button key={k} type="button" className={`ptab ${tab === k ? 'on' : ''}`} data-gtab={k} tabIndex={-1}
            onClick={() => { setTab(k); setIndex(-1); }}>{t(`picker.${k}`)}</button>)}</div> : null}
        <div className="ps-pick-list" role="listbox">
          {loading ? [0, 1, 2].map(i => <div key={i} className="game-row skel"><span className="gph" /><div className="gtxt"><b /><small /></div></div>)
            : !rows.length ? <div className="picker-empty">{t(mode === 'search' ? 'picker.noResults' : `picker.empty.${tab}`)}</div>
            : rows.map((g, i) => <div key={`${g.placeId}-${i}`} className={`game-row ${i === index ? 'on' : ''}`} role="option" id={`psg-${i}`}
              aria-selected={i === index} data-g={i} onClick={() => choose(g)}>
              {g.iconUrl ? <GameIcon src={g.iconUrl} /> : <span className="gph"><Icon name="gamepad" /></span>}
              <div className="gtxt"><b title={g.name}>{g.name}</b><small>{g.playing ? t('dock.playing', { n: host.fmtNum(g.playing) }) : `Place ${g.placeId}`}</small></div>
            </div>)}
        </div>
      </> : null}
    </div>
    <div className="ps-game" id="ps-game">{card?.name ? <>{card.iconUrl ? <GameIcon src={card.iconUrl} /> : <span className="gph"><Icon name="gamepad" /></span>}<b>{card.name}</b></> : null}</div>
  </>;
}

/** A game's picture; a broken one becomes the gamepad placeholder (React's own, not the page-wide swap). */
function GameIcon({ src }: { src: string }) {
  const [failed, setFailed] = useState(false);
  return failed ? <span className="gph"><Icon name="gamepad" /></span> : <img src={src} alt="" onError={() => setFailed(true)} />;
}
