// The Accounts tab: its heading line (how many, how many playing, the selection), the bar under it (status chips, cards
// or list, group, sort), the accounts as cards or rows, and the dock at the bottom (the game box and its picker, the
// game found, recent games, and the buttons that open the selected accounts). What is selected, the filters and the
// game are app.js's (S, P), read through host.page(); what the buttons do is app.js's too (its click handler by
// data-act, its keys), so the elements keep the same data-* it looks for.
import { Fragment, memo, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { Icon, useAppState, useHost, useMetrics, usePage, type PageState } from './host';
import { richNodes } from './rich';
import { Avatar } from './Avatar';

type Account = Record<string, any>;

const isLocal = (a: Account) => a.state === 'running';
/** Auto-rejoin for the account's client: a preset that opened it may turn it on or off for that client. */
const rejoinOn = (a: Account) => a.session?.autoRejoin ?? a.autoRejoin;
const isStranded = (a: Account) => isLocal(a) && ['disconnected', 'menu', 'signedout'].includes(a.session?.connection);
const inGame = (a: Account) => (isLocal(a) && !isStranded(a)) || a.presence?.type === 'ingame';
const isOnline = (a: Account) => isLocal(a) || ['online', 'ingame', 'studio'].includes(a.presence?.type);
/** Roblox's disconnect codes, as the client logs them (278 seen in practice; the others are Roblox's documented ones). */
const KICK: Record<number, string> = { 278: 'kick.idle', 267: 'kick.game', 273: 'kick.elsewhere', 276: 'kick.elsewhere', 277: 'kick.network', 279: 'kick.connect', 288: 'kick.shutdown',
  286: 'kick.private', 259: 'kick.update', 280: 'kick.update', 264: 'kick.duplicate', 265: 'kick.duplicate',
  // The other codes of a connection that failed on the way (LostConnection in AppController.cs).
  260: 'kick.network', 261: 'kick.network', 262: 'kick.network', 266: 'kick.network', 269: 'kick.network', 290: 'kick.network', 296: 'kick.network', 297: 'kick.network', 298: 'kick.network' };
/** The colour of the avatar ring and dot: a client here in the Roblox menu is online, not in a game. */
const presenceType = (a: Account) => !isLocal(a) ? a.presence?.type || ''
  : a.session?.connection === 'disconnected' ? 'kicked' : a.session?.connection === 'menu' ? 'online'
  : a.session?.connection === 'signedout' ? 'offline' : 'ingame';
const ringOf = (a: Account) => { const p = presenceType(a); return a.cookieValid === false || a.banned ? 'invalid' : ['ingame', 'online', 'kicked'].includes(p) ? p : ''; };
/** A game's icon; a placeholder in its place when it has none or it doesn't load. */
function GameIcon({ url, className, holder }: { url?: string | null; className?: string; holder: () => ReactNode }) {
  const [failed, setFailed] = useState('');
  return url && failed !== url ? <img className={className} src={url} alt="" loading={className ? 'lazy' : undefined} decoding={className ? 'async' : undefined} onError={() => setFailed(url)} /> : <>{holder()}</>;
}

/* ───────────── heading line and bar ───────────── */

export function AccountsHead() {
  const { tn, t } = useHost();
  const app = useAppState();
  const selected = usePage(p => p.selected);
  // What the search, chip and group show (host.visible reads them): drawn again when they change.
  usePage(p => p.query); usePage(p => p.filter); usePage(p => p.group);
  const host = useHost();
  const list: Account[] = app.accounts || [];
  const n = selected.size;
  // Selected accounts the search or filter hides still count (Play, Delete act on them), so it says so.
  const shown = n ? new Set(host.visible().map(a => a.id)) : null;
  const hidden = n ? [...selected].filter(id => !shown!.has(id)).length : 0;
  return <p id="acc-sub">{richNodes(tn('acc.summary', list.length, { p: list.filter(inGame).length, o: list.filter(isOnline).length }))}
    {n ? <><span className="sep">·</span>{richNodes(t(hidden ? 'acc.selectedHidden' : 'acc.selected', { n, h: hidden }))}</>
      : list.length ? <><span className="sep">·</span>{richNodes(t('acc.selectAll'))}</> : null}</p>;
}

/**
 * The bar: one row of status chips (counting within the chosen group; those needing something done right after All),
 * cards or list, the group menu's button, the sort menu's. The chip row scrolls sideways with the wheel when it holds
 * more than fits, its edges fading on the side(s) with more; a chip chosen half under the fade slides into view.
 */
export function AccountsBar() {
  const { t, compareText } = useHost();
  const app = useAppState();
  const filter = usePage(p => p.filter), group = usePage(p => p.group), layout = usePage(p => p.layout), sort = usePage(p => p.sort);
  const list: Account[] = app.accounts || [];
  const groups = [...new Set(list.map(a => a.group).filter(Boolean))].sort(compareText);
  const inGroup = group ? list.filter(a => a.group === group) : list;
  const chips = [
    { f: 'all', text: t('chip.all'), n: inGroup.length },
    { f: 'invalid', text: t('chip.invalid'), n: inGroup.filter(a => a.cookieValid === false).length, dot: 'var(--bad)', hide: !list.some(a => a.cookieValid === false) },
    { f: 'banned', text: t('chip.banned'), n: inGroup.filter(a => a.banned).length, dot: 'var(--bad)', hide: !list.some(a => a.banned) },
    { f: 'ingame', text: t('chip.ingame'), n: inGroup.filter(inGame).length, dot: 'var(--ok)' },
    { f: 'online', text: t('chip.online'), n: inGroup.filter(isOnline).length, dot: 'var(--online)' },
    { f: 'offline', text: t('chip.offline'), n: inGroup.filter(a => !isOnline(a)).length, dot: 'var(--dim)' },
    { f: 'local', text: t('chip.local'), n: inGroup.filter(isLocal).length, hide: !list.some(isLocal) },
  ].filter(c => !c.hide);
  const row = useRef<HTMLDivElement>(null);
  const shownFilter = useRef('all');
  const fade = () => {
    const el = row.current;
    if (!el) return;
    const max = el.scrollWidth - el.clientWidth;
    el.classList.toggle('fade-l', max > 1 && el.scrollLeft > 1);
    el.classList.toggle('fade-r', max > 1 && el.scrollLeft < max - 1);
  };
  useEffect(() => {
    const el = row.current!;
    const wheel = (e: WheelEvent) => {
      if (el.scrollWidth <= el.clientWidth || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
      e.preventDefault();
      el.scrollLeft += e.deltaY;
    };
    el.addEventListener('wheel', wheel, { passive: false });
    el.addEventListener('scroll', fade, { passive: true });
    const watch = new ResizeObserver(fade);
    watch.observe(el);
    return () => { el.removeEventListener('wheel', wheel); el.removeEventListener('scroll', fade); watch.disconnect(); };
  }, []);
  // Measured once at the start of the next frame, after every change: one layout, not one per change.
  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      const el = row.current, on = el?.querySelector('.chip.on');
      if (el && on && filter !== shownFilter.current) {
        shownFilter.current = filter;
        const r = on.getBoundingClientRect(), box = el.getBoundingClientRect(), edge = 40;
        if (r.left < box.left + edge) el.scrollLeft -= box.left + edge - r.left;
        else if (r.right > box.right - edge) el.scrollLeft += r.right - (box.right - edge);
      }
      fade();
    });
    return () => cancelAnimationFrame(frame);
  });
  return <div className="bar">
    <div className="chips" id="chips" ref={row}>{list.length ? chips.map(c =>
      <button key={c.f} className={`chip ${filter === c.f ? 'on' : ''}`} data-act="filter" data-f={c.f} aria-pressed={filter === c.f}>
        {c.dot ? <i className="dot" style={{ background: c.dot }} /> : null}{c.text}<em>{c.n}</em></button>) : null}</div>
    <div className="seg" id="layout-seg">{list.length ? (['cards', 'list'] as const).map(l =>
      <button key={l} className={layout === l ? 'on' : ''} data-act="layout" data-l={l} title={t(`layout.${l}`)} aria-label={t(`layout.${l}`)} aria-pressed={layout === l}>
        <Icon name={l === 'cards' ? 'grid' : 'rows'} /></button>) : null}</div>
    <button className={`sort-btn ${group ? 'on' : ''}`} data-act="group-menu" id="group-btn" aria-haspopup="menu" hidden={!groups.length} title={group ? `${t('grp.label')}: ${group}` : ''}>
      <Icon name="tag" /><span>{`${t('grp.label')}: ${group || t('chip.all')}`}</span></button>
    <button className="sort-btn" data-act="sort-menu" id="sort-btn" aria-haspopup="menu" hidden={list.length < 2}><Icon name="sort" /><span>{`${t('sort.label')}: ${t(`sort.${sort}`)}`}</span></button>
  </div>;
}

/* ───────────── the accounts ───────────── */

/** The ways to add an account (no accounts yet), the phone first: Roblox asks it for no password and no puzzle. */
export function AddOptions() {
  const { t } = useHost();
  return <>
    <div className="start-opts">
      <button type="button" className="start-opt rec" data-act="add-phone"><Icon name="qr" /><span><b>{t('start.phone')}<em className="rec-tag">{t('start.rec')}</em></b><small>{t('start.phoneSub')}</small></span><Icon name="chevronRight" /></button>
      <button className="start-opt" data-act="add-password"><Icon name="key" /><span><b>{t('start.pw')}</b><small>{t('start.pwSub')}</small></span><Icon name="chevronRight" /></button>
      <button className="start-opt" data-act="add-browser"><Icon name="globe" /><span><b>{t('start.browser')}</b><small>{t('start.browserSub')}</small></span><Icon name="chevronRight" /></button>
    </div>
    <div className="start-more">
      <div className="ways"><span>{t('start.more')}</span><button className="btn sm ghost" data-act="add-cookie">{t('start.cookie')}</button></div>
      <div className="ways"><span>{t('start.importFrom')}</span><button className="btn sm ghost" data-act="import-vault" title={t('start.vaultTip')}>{t('start.vault')}</button></div>
      <div className="ways"><span>{t('start.webQ')}</span><button className="btn sm ghost" data-act="open-web" title={t('start.webTip')}>{t('start.web')}</button></div>
    </div>
  </>;
}

/**
 * Every account shown, as cards or rows. One Tab stop for the whole list (roving tabindex): the card with the focus,
 * else the first selected one, else the first; the arrow keys walk the cards (app.js moveCardFocus). A card redrawn
 * or moved (Play → Close, a sort) keeps the focus: on the same button if it is still there, else on the card.
 */
export function AccountGrid() {
  const host = useHost();
  const { t, logo } = host;
  const app = useAppState();
  // Only what the list shows depends on: the dock's game box, the picker and the live figures don't draw it again.
  const page = {
    selected: usePage(p => p.selected), layout: usePage(p => p.layout), query: usePage(p => p.query), filter: usePage(p => p.filter),
    group: usePage(p => p.group), sort: usePage(p => p.sort), introShown: usePage(p => p.introShown),
  };
  const all: Account[] = app.accounts || [];
  const shown = all.length ? host.visible() : [];
  const asList = page.layout === 'list';
  // Relative times ("5 min ago") and countdowns drift: the cards are drawn again once a minute while the window is
  // visible, whether or not the app pushed anything meanwhile.
  const [minute, tick] = useState(0);
  useEffect(() => {
    const again = () => { if (!document.hidden) tick(n => n + 1); };
    const timer = setInterval(again, 60_000);
    document.addEventListener('visibilitychange', again);
    return () => { clearInterval(timer); document.removeEventListener('visibilitychange', again); };
  }, []);
  const grid = useRef<HTMLDivElement>(null);
  // Where the focus was as this drawing started: a card drawn anew or moved gets it back (see the layout effect).
  const before = useRef<{ id: string; act: string } | null>(null);
  const active = document.activeElement;
  const card = active?.closest?.('#grid .card') as HTMLElement | null;
  before.current = card ? { id: card.dataset.id!, act: active === card ? 'card' : (active!.closest('[data-act]') as HTMLElement | null)?.dataset.act || 'card' } : null;
  useLayoutEffect(() => {
    // No accounts yet: nothing to play, and in a small window the dock would cover the sign-in choices.
    document.getElementById('view-accounts')?.classList.toggle('no-accounts', !all.length);
    const box = grid.current;
    if (!box) return;
    const was = before.current;
    if (was && (!document.activeElement || document.activeElement === document.body)) {
      const el = box.querySelector<HTMLElement>(`.card[data-id="${CSS.escape(was.id)}"]`);
      if (el) ((was.act !== 'card' && el.querySelector<HTMLElement>(`[data-act="${CSS.escape(was.act)}"]`)) || el).focus();
    }
  });
  // The Tab stop is set again when the list itself changes (a state push, a search, a chip, the layout, the sort), not
  // when only the selection does: a card clicked keeps being the stop.
  useLayoutEffect(() => { if (grid.current) host.setTabStop(grid.current); }, [app, page.query, page.filter, page.group, page.layout, page.sort, shown.length]);
  useEffect(() => { if (shown.length || all.length) host.introShown(); });

  if (!shown.length) {
    return <div id="grid">{all.length
      ? <div className="empty"><h2>{t('nores.title')}</h2><p>{t('nores.text')}</p>
        <div className="row"><button className="btn" data-act="clear-search">{t(page.query.trim() ? 'nores.clear' : 'nores.all')}</button></div></div>
      : <div className="empty"><img className="art" src={logo} alt="" /><h2>{t('empty.title')}</h2><p>{t('empty.text')}</p><AddOptions /></div>}</div>;
  }
  const cols = asList ? listColumns(shown, host.fmtNum) : '';
  return <div id="grid">
    <GridBox key={asList ? 'list' : 'cards'} asList={asList} intro={!page.introShown} label={t('acc.title')} box={grid}>
      {shown.map(a => <AccountItem key={a.id} a={a} asList={asList} sel={page.selected.has(a.id)} lang={host.lang} minute={minute} cols={cols} />)}
    </GridBox>
  </div>;
}

/**
 * The list's columns after the status, and the longest thing each shows among the rows shown: VNG, Robux, the group,
 * auto-rejoin, PID and memory ("" or false: no row has it, no column). As text, so the rows drawn again only when it changes.
 */
type Cols = { vng: boolean; robux: string; group: string; rejoin: boolean; pid: string; live: boolean };
function listColumns(shown: Account[], fmtNum: (n: number) => string): string {
  const longest = (list: string[]) => list.reduce((a, b) => (b.length > a.length ? b : a), '');
  const cols: Cols = {
    vng: shown.some(a => a.realm === 'vng'),
    robux: longest(shown.map(a => typeof a.robux === 'number' ? fmtNum(a.robux) : a.robuxWait ? '…' : '')),
    group: longest(shown.map(a => a.group || '')),
    rejoin: shown.some(rejoinOn),
    pid: longest(shown.filter(isLocal).map(a => a.session?.pid > 0 ? `PID ${a.session.pid}` : '')),
    live: shown.some(isLocal),
  };
  return JSON.stringify(cols);
}

/**
 * An unseen copy of the longest thing in a list column, under what the row shows: every row's cell is then as wide,
 * whatever the font, so the columns stand straight.
 */
const Sizer = ({ children }: { children: ReactNode }) => <span className="sizer" aria-hidden="true">{children}</span>;

/** The list's box: cards fade in only the first time it is drawn, not on every update. */
function GridBox({ asList, intro, label, box, children }: { asList: boolean; intro: boolean; label: string; box: React.RefObject<HTMLDivElement | null>; children: ReactNode }) {
  const [fading, setFading] = useState(intro);
  useEffect(() => {
    if (!fading) return;
    const timer = setTimeout(() => setFading(false), 800);
    return () => clearTimeout(timer);
  }, []);
  // A card, or a button on it, that gets the focus (a click, Shift+Tab) becomes the list's one Tab stop.
  const { setTabStop } = useHost();
  return <div ref={box} className={`grid ${asList ? 'list' : ''} ${fading ? 'intro' : ''}`} role="listbox" aria-multiselectable="true" aria-label={label}
    onFocus={e => {
      const card = (e.target as HTMLElement).closest<HTMLElement>('.card');
      if (card && card.tabIndex !== 0 && card.parentElement) setTabStop(card.parentElement, card);
    }}>{children}</div>;
}

const AccountItem = memo(function AccountItem({ a, asList, sel, cols }: { a: Account; asList: boolean; sel: boolean; lang: string; minute: number; cols: string }) {
  return asList ? <AccountRow a={a} sel={sel} cols={cols} /> : <AccountCard a={a} sel={sel} />;
});

/** "5 min ago" in a sentence ("Opened {t}"), as of now. */
function useAgo() {
  const { t, ago } = useHost();
  return (iso: string, tpl: string) => t(tpl, { t: ago(iso) });
}
/** A countdown in whole minutes, rounded up, then "in a moment" for the last one (a rejoin waiting its turn). */
function useUntil() {
  const { t } = useHost();
  return (iso: string, tpl: string, soon: string) => {
    const s = (new Date(iso).getTime() - Date.now()) / 1000;
    return s < 60 ? t(soon) : t(tpl, { n: Math.ceil(s / 60) });
  };
}

/** What the account is doing: playing here or elsewhere, opening, kicked, waiting to rejoin, signed out, offline… */
function Activity({ a }: { a: Account }) {
  const { t } = useHost();
  const ago = useAgo(), until = useUntil();
  const p = a.presence;
  const gIcon = (url?: string) => <GameIcon url={url} className="gicon" holder={() => <span className="gicon"><Icon name="gamepad" /></span>} />;
  const box = (cls: string, ic: ReactNode, k: string, v: ReactNode, title?: string | null) =>
    <div className={`activity ${cls}`}>{ic}<div className="txt"><div className="k" title={k}>{k}</div><div className="v" title={title === null ? undefined : title ?? String(v)}>{v}</div></div></div>;
  const iconBox = (name: string) => <span className="gicon"><Icon name={name} /></span>;
  if (a.state === 'launching') return box('launching', <span className="gicon"><span className="spin" /></span>, t('act.opening'), a.session?.game?.name || t('act.waitWindow'));
  // Auto-rejoin waiting its turn (longer after several drops in a row), or a preset's restart about to open it again.
  if (a.rejoin && !isLocal(a)) return box('launching', iconBox(a.rejoin.restart ? 'refresh' : 'repeat'), t(a.rejoin.restart ? 'rj.kRestart' : 'rj.k'),
    <><span>{until(a.rejoin.at, 'rj.inMin', 'rj.soon')}</span>{a.rejoin.game?.name ? ` · ${a.rejoin.game.name}` : ''}</>, null);
  if (isLocal(a)) {
    const game = p?.type === 'ingame' && p.game ? { name: p.game, iconUrl: p.iconUrl } : a.session?.game;
    const c = a.session?.connection;
    // Banned by the game: put out of it (Roblox gives the same code 267 as a kick), or stuck on Join Error 600 on the
    // way in; with its words when it gave any.
    if (a.session?.bannedByGame)
      return box('kicked', iconBox('alert'), t('act.bannedByGame'), [a.session.kickMessage, game?.name].filter(Boolean).join(' · ') || t('act.stillOpen'));
    if (c === 'disconnected') {
      // Why is the status itself (kicked by the game, server shut down…); without a reason, it says the reason isn't known.
      const r = a.session.reason;
      const why = r ? (KICK[r] ? t(KICK[r]) : t('kick.code', { code: r })) : t('act.kicked');
      return box('kicked', iconBox('alert'), why, [r ? '' : t('kick.unknown'), a.session.kickMessage, game?.name].filter(Boolean).join(' · ') || t('act.stillOpen'));
    }
    // Left before any server took it (a join error's Leave, or a wait given up): Roblox's log doesn't say which.
    if (c === 'menu' && a.session.phase === 'joinFailed')
      return box('online', iconBox('monitor'), t('act.joinFailed'), [a.session.game?.name, t('act.stillOpen')].filter(Boolean).join(' · '));
    if (c === 'menu') return box('online', iconBox('monitor'), t('act.menuHere'), t('act.menuWhere'));
    if (c === 'signedout') return box('signedout', iconBox('logIn'), t('act.signedOutHere'), t('act.signedOutWhere'));
    // Roblox's join answer says the server is full: its place in line when Roblox gave one.
    if (c === 'joining' && a.session.phase === 'full')
      return box('launching', gIcon(game?.iconUrl), t('act.serverFull'),
        [a.session.queuePosition ? t('act.inLine', { n: a.session.queuePosition }) : '', game?.name].filter(Boolean).join(' · ') || t('act.gameLoading'));
    return box('ingame', gIcon(game?.iconUrl), t(onTheWay(a, c)), game?.name || t('act.gameLoading'));
  }
  switch (p?.type) {
    case 'ingame': return box('ingame', gIcon(p.iconUrl), t('act.playingElse'), p.game || t('act.hidden'));
    case 'online': return box('online', iconBox('globe'), t('act.online'), t('act.onlineWhere'));
    case 'studio': return box('studio', iconBox('code'), t('act.studio'), p.game || t('act.studioWhat'));
    case 'offline':
    case 'invisible':
      // Signed out: that is what to act on, not "Offline" under a "Needs sign-in" badge.
      if (a.cookieValid === false) return box('signedout', iconBox('logIn'), t('act.signedOut'), t('act.signInAgain'));
      if (p.lastOnline) { const v = ago(p.lastOnline, 'act.lastOnline'); return box('offline', iconBox('moon'), t('act.offline'), <span>{v}</span>, v); }
      return box('offline', iconBox('moon'), t('act.offline'), t('act.inactive'));
    default:
      return a.cookieValid === false
        ? box('signedout', iconBox('logIn'), t('act.signedOut'), t('act.signInAgain'))
        : box('unknown', iconBox('info'), t('act.unknown'), t('act.updating'));
  }
}

/** A running account's live figures: its window's memory and how long it has been open. */
function Live({ a, row }: { a: Account; row?: boolean }) {
  const { t, fmtMb, duration } = useHost();
  const m = useMetrics();
  const mb = m.accounts.find(x => x.id === a.id)?.ramMb;
  const ram = mb ? fmtMb(mb) : '…';
  const up = a.session ? duration(a.session.since) : '';
  return row
    ? <span className="row-ram" title={t('card.ramTip')}><b className="ram">{ram}</b></span>
    : <div className="live"><span title={t('card.ramTip')}><Icon name="chip" /><b className="ram">{ram}</b></span><span title={t('card.upTip')}><Icon name="clock" /><b className="up">{up}</b></span><Pid a={a} /></div>;
}
/** The client's process id, for tools that pick a Roblox window by it; a click copies it. Nothing until the window is there. */
function Pid({ a }: { a: Account }) {
  const { t } = useHost();
  return a.session?.pid > 0 ? <button className="pid" data-act="copy-pid" title={t('card.pidTip')}>{`PID ${a.session.pid}`}</button> : null;
}
/** A list row's columns after the status: each column the list has, in every row, as wide as its longest (Sizer). */
function RowColumns({ a, c, local }: { a: Account; c: Cols; local: boolean }) {
  const { t, fmtNum, fmtMb } = useHost();
  const cell = (on: unknown, shown: ReactNode, longest: ReactNode, end = false) =>
    on ? <span className={`cell${end ? ' end' : ''}`}>{shown}<Sizer>{longest}</Sizer></span> : null;
  return <>
    <div className="row-meta">
      {cell(c.vng, a.realm === 'vng' ? <span className="tag vng" title={t('card.vngTip')}>VNG</span> : null, <span className="tag vng">VNG</span>)}
      {cell(c.robux, typeof a.robux === 'number' ? <span className="robux" title={t('card.robuxTip')}><Icon name="robux" />{fmtNum(a.robux)}</span>
        : a.robuxWait ? <span className="robux wait" title={t('card.robuxTip')}><Icon name="robux" />…</span> : null,
        <span className="robux"><Icon name="robux" />{c.robux}</span>, true)}
      {cell(c.group, a.group ? <span className="tag" title={a.group}>{a.group}</span> : null, <span className="tag">{c.group}</span>)}
      {cell(c.rejoin, rejoinOn(a) ? <span className="rejoin" title={t('card.rejoinTip')}><Icon name="repeat" /></span> : null, <span className="rejoin"><Icon name="repeat" /></span>)}
    </div>
    <div className="row-live">
      {cell(c.live && c.pid, local ? <Pid a={a} /> : null, <span className="pid">{c.pid}</span>, true)}
      {cell(c.live, local ? <Live a={a} row /> : null, <b className="ram">{fmtMb(1023)}</b>, true)}
    </div>
  </>;
}
function Meta({ a }: { a: Account }) {
  const { t, fmtNum } = useHost();
  return <>
    {a.realm === 'vng' ? <span className="tag vng" title={t('card.vngTip')}>VNG</span> : null}
    {typeof a.robux === 'number' ? <span className="robux" title={t('card.robuxTip')}><Icon name="robux" />{fmtNum(a.robux)}</span>
      : a.robuxWait ? <span className="robux wait" title={t('card.robuxTip')}><Icon name="robux" />…</span> : null}
    {a.group ? <span className="tag" title={a.group}>{a.group}</span> : null}
    {rejoinOn(a) ? <span className="rejoin" title={t('card.rejoinTip')}><Icon name="repeat" /></span> : null}
  </>;
}

function AccountCard({ a, sel }: { a: Account; sel: boolean }) {
  const { t } = useHost();
  const ago = useAgo();
  const local = isLocal(a), pType = presenceType(a);
  // A ban says more than the sign-in state, so it takes the badge's place.
  const badge = local ? <span className="badge run" title={a.session?.pid > 0 ? t('card.runningPid', { pid: a.session.pid }) : ''}><i className="pulse" />{t('card.local')}</span>
    : a.state === 'launching' ? <span className="badge wait"><span className="spin" />{t('card.launching')}</span>
    : a.banned ? <span className="badge bad" title={t('card.bannedTip')}>{t('card.banned')}</span>
    : a.cookieValid === false ? <span className="badge bad">{t('card.expired')}</span> : null;
  const sub = [a.username ? `@${a.username}` : '', a.displayName && a.displayName !== a.username && a.displayName !== a.label ? a.displayName : ''].filter(Boolean).join(' · ');
  // The main button, by what it does (a new one, not the old one changed: the focus then goes as it did, to the card).
  const action = local || a.state === 'launching'
    ? <button key="close-one" className="pill-btn stop" data-act="close-one"><Icon name="power" />{t('card.close')}</button>
    : a.rejoin
      ? <button key="cancel-rejoin" className="pill-btn stop" data-act="cancel-rejoin" title={t('rj.cancelTip')}><Icon name="x" />{t('rj.cancel')}</button>
    : a.cookieValid === false
      ? <button key="relogin" className="pill-btn relogin" data-act="relogin" title={t('card.reloginTip')}><Icon name="logIn" />{t('card.relogin')}</button>
      : <button key="play-one" className="pill-btn play" data-act="play-one" disabled={a.state === 'launching'}><Icon name="play" />{t('card.play')}</button>;
  return <article className={`card ${local ? 'local' : ''}${sel ? ' sel' : ''}`} data-id={a.id} data-act="card" role="option" tabIndex={-1} aria-selected={sel} aria-label={a.label}>
    <div className="card-top"><span className="check"><Icon name="check" /></span>{badge}</div>
    <div className={`avatar ${ringOf(a)}`}><Avatar a={a} /><i className={`pdot ${pType}`} /></div>
    <div className="name" title={a.label}>{a.label}</div>
    <div className="sub" title={sub}>{sub}</div>
    <Activity a={a} />
    {local ? <Live a={a} /> : null}
    {/* "Opened 8 h ago" only while the account isn't running here (a running card shows its play time instead). */}
    <div className="meta"><Meta a={a} />{local ? null : <span className="ago" title={t('card.lastTip')}><Icon name="history" />{a.lastUsed ? <span>{ago(a.lastUsed, 'card.opened')}</span> : t('card.never')}</span>}</div>
    {a.note ? <div className="note" title={a.note}>{a.note}</div> : null}
    <div className="card-actions">
      {action}
      <button className="icon-btn" data-act="edit" title={t('card.editTip')}><Icon name="edit" /></button>
      <button className="icon-btn" data-act="card-menu" title={t('card.moreTip')} aria-haspopup="menu"><Icon name="more" /></button>
    </div>
  </article>;
}

/** In a game or on the way: teleporting, waiting for a server (started, none yet), joining it, or playing. */
function onTheWay(a: Account, c: string | undefined, at: 'act' | 'row' = 'act'): string {
  const phase = a.session?.phase;
  if (phase === 'teleporting') return 'act.teleporting';
  if (phase === 'waiting' && c === 'joining') return 'act.waitServer';
  return c === 'ingame' ? (at === 'row' ? 'row.playing' : 'act.playingHere') : (at === 'row' ? 'row.joining' : 'act.joiningHere');
}

/** The account's status in a few words, for the compact list. */
function statusLine(t: (k: string, v?: Record<string, string | number>) => string, a: Account): { cls: string; text: string; until?: string | null } {
  const p = a.presence;
  if (a.state === 'launching') return { cls: 'launching', text: t('act.opening') };
  if (a.rejoin && !isLocal(a)) return { cls: 'launching', text: t(a.rejoin.restart ? 'rj.rowRestart' : 'rj.k'), until: a.rejoin.restart ? null : a.rejoin.at };
  if (isLocal(a)) {
    const c = a.session?.connection;
    const game = (p?.type === 'ingame' && p.game) || a.session?.game?.name || '';
    const withGame = (text: string) => game ? `${text} · ${game}` : text;
    if (c === 'disconnected' || a.session?.bannedByGame) {
      const why = a.session.bannedByGame ? t('act.bannedByGame')
        : a.session.reason ? (KICK[a.session.reason] ? t(KICK[a.session.reason]) : t('kick.code', { code: a.session.reason })) : t('row.kickedUnknown');
      return { cls: 'kicked', text: withGame(a.session.kickMessage ? `${why}: ${a.session.kickMessage}` : why) };
    }
    if (c === 'menu' && a.session.phase === 'joinFailed') return { cls: 'online', text: withGame(t('act.joinFailed')) };
    if (c === 'joining' && a.session.phase === 'full')
      return { cls: 'launching', text: withGame(a.session.queuePosition ? `${t('row.full')} · ${t('act.inLine', { n: a.session.queuePosition })}` : t('act.serverFull')) };
    if (c === 'menu') return { cls: 'online', text: t('act.menuHere') };
    if (c === 'signedout') return { cls: 'signedout', text: t('act.signedOutHere') };
    return { cls: 'ingame', text: withGame(t(onTheWay(a, c, 'row'))) };
  }
  // A ban or a lost sign-in is the red pill in front; a lost sign-in needs no words after it.
  switch (p?.type) {
    case 'ingame': return { cls: 'ingame', text: `${t('row.elsewhere')} · ${p.game || t('act.hidden')}` };
    case 'online': return { cls: 'online', text: t('act.online') };
    case 'studio': return { cls: 'studio', text: t('act.studio') };
    case 'offline':
    case 'invisible': return a.cookieValid === false && !a.banned ? { cls: 'signedout', text: '' } : { cls: 'offline', text: t('act.offline') };
    default: return a.cookieValid === false && !a.banned ? { cls: 'signedout', text: '' } : { cls: 'unknown', text: t(a.banned ? 'act.offline' : 'act.unknown') };
  }
}

/** One line per account for the compact list; the same actions as a card (select, right-click menu, play). */
function AccountRow({ a, sel, cols }: { a: Account; sel: boolean; cols: string }) {
  const { t, duration } = useHost();
  const until = useUntil();
  const local = isLocal(a), pType = presenceType(a);
  const st = statusLine(t, a);
  const up = local && st.cls === 'ingame' && a.session ? duration(a.session.since) : '';
  const action = local || a.state === 'launching' ? <button key="close-one" className="icon-btn stop" data-act="close-one" title={t('card.close')} aria-label={t('card.close')}><Icon name="power" /></button>
    : a.rejoin ? <button key="cancel-rejoin" className="icon-btn stop" data-act="cancel-rejoin" title={t('rj.cancelTip')} aria-label={t('rj.cancelTip')}><Icon name="x" /></button>
    : a.cookieValid === false ? <button key="relogin" className="icon-btn relogin" data-act="relogin" title={t('card.relogin')} aria-label={t('card.relogin')}><Icon name="logIn" /></button>
    : <button key="play-one" className="icon-btn play" data-act="play-one" title={t('card.play')} aria-label={t('card.play')} disabled={a.state === 'launching'}><Icon name="play" /></button>;
  // The card's red pill in a list row too (banned, or signed out), in front of the status words.
  const badge = local || a.state === 'launching' ? null
    : a.banned ? <span className="badge bad" title={t('card.bannedTip')}>{t('card.banned')}</span>
    : a.cookieValid === false ? <span className="badge bad">{t('card.expired')}</span> : null;
  const at = a.username ? `@${a.username}` : '';
  return <article className={`card row ${local ? 'local' : ''}${sel ? ' sel' : ''}`} data-id={a.id} data-act="card" role="option" tabIndex={-1} aria-selected={sel} aria-label={a.label}>
    <span className="check"><Icon name="check" /></span>
    <div className={`avatar sm ${ringOf(a)}`}><Avatar a={a} /><i className={`pdot ${pType}`} /></div>
    <div className="row-name"><b title={a.label}>{a.label}</b>{a.username ? <small title={at}>{at}</small> : null}</div>
    {up
      // Playing here: how long, right after the game (as on the card), never cut off by a long game name.
      ? <div className={`row-status timed ${st.cls}`} title={st.text}><span className="row-st">{st.text}</span>
        <span className="row-up" title={t('card.upTip')}><Icon name="clock" /><b className="up">{up}</b></span></div>
      : <div className={`row-status ${st.cls}`} title={st.text}>{badge}{st.until ? <span>{until(st.until, 'rj.rowMin', 'rj.rowSoon')}</span> : st.text}</div>}
    <RowColumns a={a} c={JSON.parse(cols)} local={local} />
    <div className="row-actions">{action}<button className="icon-btn" data-act="card-menu" title={t('card.moreTip')} aria-label={t('card.moreTip')} aria-haspopup="menu"><Icon name="more" /></button></div>
  </article>;
}

/* ───────────── the dock ───────────── */

/**
 * The dock: the game's picture (it opens the picker), the game box with its picker, the Job ID box, Join a player,
 * what the game box holds (the game found, a link's kind, a mistake, what is going on) with recent games to pick
 * again, and the buttons for the selected accounts. The two boxes keep what is typed themselves: app.js reads and
 * fills them.
 */
export function Dock() {
  const { t } = useHost();
  const app = useAppState();
  const page = usePage(p => p);
  const n = page.selected.size;
  const P = page.picker;
  return <div className="dock">
    <button className="game-thumb" id="game-thumb" data-act="browse-games" title={t('dock.browseTip')}>
      <GameIcon key={page.game?.iconUrl || ''} url={page.game?.iconUrl} holder={() => page.game?.iconUrl ? <span className="gph"><Icon name="gamepad" /></span> : <Icon name="gamepad" />} /></button>
    <div className="picker" id="picker" hidden={!P.open}>{P.open ? <Picker P={P} /> : null}</div>
    <div className="dock-mid">
      <div className="dock-inputs">
        <label className="inwrap grow" title={t('dock.placeTip')}><Icon name="search" /><input id="place" autoComplete="off" placeholder={t('dock.place')} maxLength={500} spellCheck={false}
          aria-label={t('dock.place')} role="combobox" aria-haspopup="listbox" aria-autocomplete="list" aria-controls="picker-list" aria-expanded={P.open}
          aria-activedescendant={P.open && P.index >= 0 ? `pk-${P.index}` : undefined} /></label>
        <label className="inwrap job" title={t('dock.jobTip')}><Icon name="server" /><input id="job" placeholder={t('dock.job')} maxLength={60} spellCheck={false} aria-label={t('dock.job')} /></label>
        {/* Join a player needs selected accounts, like Open Roblox and Close next to it: dimmed until there are some. */}
        <button className="dock-join" data-act="join-player" title={t('jp.buttonTip')} aria-label={t('jp.button')} disabled={!n}><Icon name="userGo" /></button>
      </div>
      <DockInfo page={page} recentGames={app.settings?.recentGames || []} />
    </div>
    <div className="dock-actions" id="dock-actions">{app.launching
      ? <button className="play-btn stop" data-act="cancel-launch"><Icon name="stop" /><span>{t('dock.stop')}</span></button>
      : <DockButtons page={page} accounts={app.accounts || []} />}</div>
  </div>;
}

function DockButtons({ page, accounts }: { page: PageState; accounts: Account[] }) {
  const { t } = useHost();
  const n = page.selected.size;
  const anyLocal = accounts.some(a => page.selected.has(a.id) && isLocal(a));
  // With no game in the box, the main button opens Roblox on its home screen (one window per account).
  return <>
    <button className="btn ghost" data-act="arrange" title={t('dock.arrangeTip')} aria-label={t('dock.arrangeTip')}><Icon name="screens" /></button>
    <button className="btn ghost" data-act="open-app" disabled={!n} title={t('dock.openAppTip')} aria-label={t('dock.openAppTip')}><Icon name="monitor" /></button>
    <button className="btn ghost" data-act="close-sel" disabled={!anyLocal} title={t('dock.closeTip')} aria-label={t('dock.closeTip')}><Icon name="power" /></button>
    <button className="btn ghost" data-act="preset-menu" title={t('ps.tip')} aria-label={t('ps.tip')} aria-haspopup="menu"><Icon name="layers" /></button>
    <button className="play-btn" data-act="launch" disabled={!n}><Icon name={page.hasGame ? 'play' : 'monitor'} /><span>{t(!n ? 'dock.pick' : page.hasGame ? 'dock.play' : 'dock.openHome')}</span>{n ? <em>{n}</em> : null}</button>
  </>;
}

/**
 * What the game box holds, and recent games to pick again. The recent games are a shortcut (the picker's Recent tab has
 * them too), the game's name is not: they go from the end, then the whole row, until the name has room to be read.
 */
function DockInfo({ page, recentGames }: { page: PageState; recentGames: { placeId: number; name: string; iconUrl?: string }[] }) {
  const { t, fmtNum } = useHost();
  const box = useRef<HTMLDivElement>(null);
  const g = page.game;
  let info: ReactNode;
  if (page.progress) info = <><span className="spin" /><span className="progress" title={page.progress}>{page.progress}</span></>;
  else if (!g) info = <span className="hint">{richNodes(t('dock.hint'))}</span>;
  else if (g.loading) info = <><span className="spin" /><span>{t('dock.finding')}</span></>;
  else if (!g.ok) info = <span className="err" title={g.error}>{g.error}</span>;
  else if (g.kind === 'share') info = <><span className="kind">{t('dock.share')}</span><span>{t('dock.shareNote')}</span>
    <button className="srv-open srv-save" data-act="save-server" title={t('sv.saveTip')} aria-label={t('sv.save')}><Icon name="bookmark" /></button></>;
  else info = <>
    <span className="gname" title={g.name}>{g.name || `Place ${g.placeId}`}</span>
    {g.kind === 'private' ? <span className="kind">{t('dock.private')}</span> : g.kind === 'server' ? <span className="kind">{t('dock.server')}</span> : null}
    {g.creator ? <span className="creator">{t('dock.by', { c: g.creator })}</span> : null}
    {g.playing ? <span className="players">{t('dock.playing', { n: fmtNum(g.playing) })}</span> : null}
    {g.placeId && g.kind !== 'private' ? <button className="srv-open" data-act="servers"><Icon name="server" /><span>{t('srv.button')}</span></button> : null}
    {/* Only its icon (its name on hover): with words it took the room of the recent games beside it. */}
    {g.placeId ? <button className="srv-open srv-save" data-act="save-server" title={t('sv.saveTip')} aria-label={t('sv.save')}><Icon name="bookmark" /></button> : null}
  </>;
  // The game already in the box isn't offered again as a shortcut.
  const recents = page.progress ? [] : recentGames.filter(r => r.placeId !== g?.placeId).slice(0, 3);
  const fit = () => {
    const row = box.current?.querySelector<HTMLElement>('.recents');
    if (!row) return;
    const el = box.current!, chips = [...row.children] as HTMLElement[];
    chips.forEach(c => { c.hidden = false; });
    row.hidden = false;
    const name = el.querySelector('.gname'), err = el.querySelector('.err');
    const cramped = () => el.scrollWidth > el.clientWidth + 1
      || (name && name.scrollWidth > name.clientWidth && name.clientWidth < 240) // 1 px over already ends it in "…"
      || (err && err.scrollHeight > err.clientHeight + 1); // a reason cut short: the chips make room for it first
    for (let i = chips.length - 1; i >= 0 && cramped(); i--) chips[i].hidden = true;
    if (chips.every(c => c.hidden)) row.hidden = true;
  };
  // Measured when what it holds changes (one layout, at the start of the next frame) and when its size does.
  const sig = JSON.stringify([page.progress, g, recents.map(r => [r.placeId, r.name])]);
  useEffect(() => {
    const frame = requestAnimationFrame(fit);
    return () => cancelAnimationFrame(frame);
  }, [sig]);
  useEffect(() => {
    const watch = new ResizeObserver(() => fit());
    watch.observe(box.current!);
    return () => watch.disconnect();
  }, []);
  return <div className="dock-info" id="dock-info" aria-live="polite" ref={box}>{info}{recents.length ? <div className="recents">{recents.map(r =>
    <div key={r.placeId} className="recent" title={r.name}><button className="recent-pick" data-act="recent" data-place={r.placeId}>
      <GameIcon url={r.iconUrl} holder={() => <i className="ph" />} /><span>{r.name}</span></button>
      <button className="rm" data-act="recent-remove" data-place={r.placeId} title={t('dock.recentRemove')} aria-label={t('dock.recentRemove')}><Icon name="x" /></button></div>)}</div> : null}</div>;
}

const TABS = ['recent', 'pinned', 'saved', 'favorites', 'popular'];

/**
 * The game box's list: recent, pinned, saved servers, favourite and popular games while it is empty, a search's results
 * once typed in. Saved servers come with the app's state (they change as they are saved), the rest with gameLists.
 */
function Picker({ P }: { P: PageState['picker'] }) {
  // The game box drives the list (arrow keys, Enter, Esc), as a combobox's list is: its buttons take no Tab stop, so Tab
  // and Shift+Tab leave the box for what is next to it.
  const { t, fmtNum } = useHost();
  const app = useAppState();
  const saved = P.mode === 'browse' && P.tab === 'saved';
  const rows: any[] = (P.mode === 'search' ? P.results : saved ? app.settings?.savedServers : P.lists?.[P.tab]) || [];
  const list = useRef<HTMLDivElement>(null);
  // The row chosen with the keys comes into view when the list changes, not with every state the app pushes.
  useLayoutEffect(() => { list.current?.querySelector('.game-row.on')?.scrollIntoView({ block: 'nearest' }); },
    [P.index, P.mode, P.tab, P.query, P.loading, P.refreshing, rows]);
  const skeleton = <div className="game-row skel"><span className="gph" /><div className="gtxt"><b /><small /></div></div>;
  return <>
    {P.mode === 'browse'
      ? <div className="picker-tabs" role="tablist">{TABS.filter(k => k !== 'popular' || P.lists?.popular?.length).map(k =>
        <button key={k} className={`ptab ${P.tab === k ? 'on' : ''}`} role="tab" tabIndex={-1} data-act="picker-tab" data-tab={k}>{t(`picker.${k}`)}</button>)}</div>
      : <div className="picker-head">{t('picker.results', { q: P.query })}</div>}
    <div className={`picker-list ${P.refreshing ? 'refreshing' : ''}`} role="listbox" id="picker-list" ref={list}>{P.loading
      ? [0, 1, 2, 3, 4].map(i => <Fragment key={i}>{skeleton}</Fragment>)
      : !rows.length ? <div className="picker-empty">{t(P.mode === 'search' ? 'picker.noResults' : `picker.empty.${P.tab}`)}</div>
      : saved ? rows.map((s, i) => <div key={s.id} className={`game-row saved ${i === P.index ? 'on' : ''}`} role="option" id={`pk-${i}`} aria-selected={i === P.index} data-act="pick-game" data-i={i}>
        <GameIcon key={s.iconUrl || ''} url={s.iconUrl} holder={() => <span className="gph"><Icon name="bookmark" /></span>} />
        <div className="gtxt"><b title={s.name}>{s.name}</b><small>{[s.gameName || (s.placeId ? `Place ${s.placeId}` : ''), s.link ? t('sv.kindLink') : '', s.code ? t('sv.kindCode', { code: s.code }) : ''].filter(Boolean).join(' · ')}</small></div>
        {s.code ? <button className="icon-btn" tabIndex={-1} data-act="copy-server-code" data-i={i} title={t('sv.copyCode')} aria-label={t('sv.copyCode')}><Icon name="copy" /></button> : null}
        <button className="icon-btn" tabIndex={-1} data-act="delete-server" data-i={i} title={t('sv.delete')} aria-label={t('sv.delete')}><Icon name="x" /></button>
      </div>)
      : rows.map((g, i) => <div key={`${g.placeId}-${i}`} className={`game-row ${i === P.index ? 'on' : ''}`} role="option" id={`pk-${i}`} aria-selected={i === P.index} data-act="pick-game" data-i={i}>
        <GameIcon key={g.iconUrl || ''} url={g.iconUrl} holder={() => <span className="gph"><Icon name="gamepad" /></span>} />
        <div className="gtxt"><b title={g.name}>{g.name}</b><small>{g.playing ? t('dock.playing', { n: fmtNum(g.playing) }) : `Place ${g.placeId}`}</small></div>
        <button className={`icon-btn pin ${g.pinned ? 'on' : ''}`} tabIndex={-1} data-act="pin-game" data-i={i} title={t(g.pinned ? 'picker.unpin' : 'picker.pin')}><Icon name={g.pinned ? 'starFill' : 'star'} /></button>
      </div>)}</div>
  </>;
}
