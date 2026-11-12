// The home page: the game the accounts play now (or played last) on a stage of its own picture, the shelf of games running
// and played lately along the stage's foot, then each account as a seat, the presets, and what Bloxkeep's users play now.
// Nothing here is new to the app: the games come from the accounts' sessions and the play history (playedGames), the
// pictures from Roblox (gameLooks, as the Games tab has them), and every button does what the same button elsewhere does
// (app.js put-game, reveal, view; host.runPreset).
import { useEffect, useMemo, useRef, useState } from 'react';
import { GameIcon, isKicked, isLocal, ringOf, statusLine } from './Accounts';
import { Avatar } from './Avatar';
import { useCommunity } from './Community';
import { Icon, useAppState, useHost, useMetrics, usePage } from './host';
import { Button, IconButton } from './kit';

type Account = Record<string, any>;
type Look = { icon: string | null; cover: string | null };
type Game = { place: number; name: string; icon?: string | null; accounts: Account[]; hours?: number; last?: number };
type Played = { name: string; place: number; hours: number; last: number };

// Kept between visits to the page, like the Games tab's (a picture asked for once is not asked for again).
const looksNow: Record<string, Look> = {};
let playedNow: Played[] | null = null;
const sameName = (a: string, b: string) => a.replace(/\[[^\]]*\]\s*/g, '').trim().toLowerCase() === b.replace(/\[[^\]]*\]\s*/g, '').trim().toLowerCase();

/** Needs you first (kicked, signed out, banned, held at a bot check), then playing, opening, then the rest by name. */
function seatOf(a: Account): 'warn' | 'play' | 'open' | 'off' {
  if (isKicked(a) || ringOf(a) === 'invalid' || (isLocal(a) && ['signedout'].includes(a.session?.connection)) || a.session?.phase === 'challenged') return 'warn';
  if (isLocal(a)) return 'play';
  if (a.state === 'launching' || a.rejoin) return 'open';
  return 'off';
}
const SEAT_ORDER = { warn: 0, play: 1, open: 2, off: 3 };
const SEATS_SHOWN = 12;

export function HomeView() {
  const { call } = useHost();
  const app = useAppState();
  const shown = usePage(p => p.view === 'home');
  const accounts: Account[] = app.accounts || [];

  // The games this PC played in the last weeks (the Games tab asks for the same), asked for once the page is first shown.
  const [played, setPlayed] = useState<Played[] | null>(playedNow);
  useEffect(() => {
    if (!shown || played) return;
    (call('playedGames', {}, { silent: true }) as Promise<Played[] | undefined>).then(r => { playedNow = r || []; setPlayed(playedNow); }).catch(() => {});
  }, [shown]);

  // Running: every account with a window here in a game, by game, most accounts first. Then the games played lately.
  const games = useMemo(() => {
    const running = new Map<number, Game>();
    for (const a of accounts) {
      const g = a.session?.game;
      if (!(isLocal(a) || a.state === 'launching') || !g?.placeId) continue;
      const had: Game = running.get(g.placeId) || { place: g.placeId, name: g.name || '', icon: g.iconUrl, accounts: [] };
      had.accounts.push(a);
      running.set(g.placeId, had);
    }
    const now = [...running.values()].sort((x, y) => y.accounts.length - x.accounts.length || x.name.localeCompare(y.name));
    const isRunning = (place: number, name: string) => now.some(g => g.place === place || (name && sameName(g.name, name)));
    const recent: Game[] = (played?.length ? [...played].sort((x, y) => y.last - x.last).map(p => ({ place: p.place, name: p.name, accounts: [], hours: p.hours, last: p.last }))
      : (app.settings?.recentGames || []).map((r: any) => ({ place: r.placeId, name: r.name, icon: r.iconUrl, accounts: [] })))
      .filter((g: Game) => g.place > 0 && g.name && !isRunning(g.place, g.name)).slice(0, 8);
    return { now, recent, all: [...now, ...recent] };
  }, [accounts, played, app.settings?.recentGames]);

  // The pictures, asked for once per game; asked once more a little later for those Roblox hadn't made ready.
  const [looks, setLooks] = useState<Record<string, Look>>(looksNow);
  const places = games.all.map(g => g.place).filter(p => !looksNow[p]?.cover).join(',');
  useEffect(() => {
    if (!shown || !places) return;
    const ask = () => (call('gameLooks', { places: places.split(',').map(Number) }, { silent: true }) as Promise<Record<string, Look> | undefined>)
      .then(d => { if (d) { Object.assign(looksNow, d); setLooks({ ...looksNow }); } }).catch(() => {});
    ask();
    const again = setTimeout(ask, 6000);
    return () => clearTimeout(again);
  }, [shown, places]);

  // The game on the stage: the one picked on the shelf while it is still there, else the first.
  const [picked, setPicked] = useState<number | null>(null);
  // No account yet: the stage welcomes and says how to add one, whatever this PC played before.
  const focus = accounts.length ? games.all.find(g => g.place === picked) || games.all[0] || null : null;

  return <div className="hm" id="home">
    <Stage game={focus} running={!!focus && games.now.includes(focus)} look={focus ? looks[focus.place] : undefined} none={!accounts.length} />
    {focus ? <Shelf games={games} focus={focus} looks={looks} pick={setPicked} /> : null}
    <div className="hm-lower">
      <Seats accounts={accounts} />
      <div className="hm-side">
        <Presets />
        <Community shown={shown} />
      </div>
    </div>
  </div>;
}

/** The game on the stage, on its own picture: what it is doing, and the two things to do with it. */
function Stage({ game, running, look, none }: { game: Game | null; running: boolean; look?: Look; none: boolean }) {
  const { t, tn, duration, ago, lang } = useHost();
  // The picture fades in once it has loaded (never a half-drawn one), and the last one stays under it while it does.
  // A picture that doesn't load leaves the stage plain (the theme's surface), as before it loaded.
  const [loaded, setLoaded] = useState('');
  const [bad, setBad] = useState('');
  const cover = look?.cover && look.cover !== bad ? look.cover : null;
  if (!game) {
    return <section className="hm-stage empty" aria-labelledby="hm-name">
      <div className="hm-stage-text">
        <h2 className="hm-name" id="hm-name">{t(none ? 'home.welcome' : 'home.noGame')}</h2>
        <p className="hm-facts">{t(none ? 'home.welcomeSub' : 'home.noGameSub')}</p>
        <div className="hm-acts">{none
          ? <Button kind="primary" data-act="add-choose"><Icon name="userPlus" /><span>{t('acc.add')}</span></Button>
          : <Button kind="primary" data-act="view" data-view="accounts"><Icon name="gamepad" /><span>{t('home.pickGame')}</span></Button>}</div>
      </div>
    </section>;
  }
  const n = game.accounts.length;
  const since = game.accounts.map(a => a.session?.since).filter(Boolean).sort()[0];
  const pic = !!cover && loaded === cover;
  return <section className={`hm-stage${pic ? ' pic' : ''}`} aria-labelledby="hm-name" data-place={game.place}>
    {cover ? <img key={cover} className={`hm-art${pic ? ' in' : ''}`} src={cover} alt="" onLoad={() => setLoaded(cover)} onError={() => setBad(cover)} /> : null}
    <div className="hm-stage-text">
      <h2 className="hm-name" id="hm-name" title={game.name}>{game.name}</h2>
      <p className="hm-facts">{running
        ? <><span className="hm-now"><i className="hm-live" aria-hidden="true" />{t('home.playing')}</span><span className="sep">·</span><b>{tn('home.accN', n)}</b>
          {since ? <><span className="sep">·</span>{t('home.for', { t: duration(since) })}</> : null}</>
        : <><span className="hm-now">{t('home.recent')}</span>{game.hours ? <><span className="sep">·</span><b>{t('home.hours', { n: game.hours.toLocaleString(lang === 'vi' ? 'vi-VN' : 'en-US') })}</b></> : null}
          {game.last ? <><span className="sep">·</span>{t('home.last', { t: ago(new Date(game.last).toISOString()) })}</> : null}</>}</p>
      <div className="hm-acts">{running
        ? <><Button kind="primary" data-act="view" data-view="monitor"><Icon name="screens" /><span>{tn('home.windowsN', n)}</span></Button>
          <Button data-act="put-game" data-place={game.place}><Icon name="userPlus" /><span>{t('home.addMore')}</span></Button></>
        : <Button kind="go" data-act="put-game" data-place={game.place}><Icon name="play" /><span>{t('home.pickAccounts')}</span></Button>}</div>
    </div>
  </section>;
}

/** The games along the stage's foot: running ones (a dot for each account in them), a hairline, then those played lately.
 * Arrow keys move along it; the one on the stage is marked. */
function Shelf({ games, focus, looks, pick }: { games: { now: Game[]; recent: Game[]; all: Game[] }; focus: Game | null; looks: Record<string, Look>; pick: (place: number) => void }) {
  const { t, tn, ago } = useHost();
  const box = useRef<HTMLDivElement>(null);
  const move = (e: React.KeyboardEvent, i: number) => {
    const to = e.key === 'ArrowRight' ? i + 1 : e.key === 'ArrowLeft' ? i - 1 : e.key === 'Home' ? 0 : e.key === 'End' ? games.all.length - 1 : -1;
    if (to < 0 || to >= games.all.length) return;
    e.preventDefault();
    pick(games.all[to].place);
    box.current?.querySelectorAll<HTMLElement>('.hm-tile')[to]?.focus();
  };
  const tile = (g: Game, i: number) => {
    const on = g === focus;
    const icon = () => <GameIcon url={looks[g.place]?.icon || g.icon} className="hm-ico" holder={() => <Icon name="gamepad" />} />;
    return <button key={g.place} type="button" className="hm-tile" aria-pressed={on} tabIndex={on ? 0 : -1} title={g.name}
      onClick={() => pick(g.place)} onKeyDown={e => move(e, i)}>
      <span className="hm-pic"><GameIcon url={looks[g.place]?.cover} className="hm-cover" holder={icon} /></span>
      <b>{g.name}</b>
      {g.accounts.length
        ? <small><span className="hm-dots" aria-hidden="true">{g.accounts.slice(0, 8).map(a => <i key={a.id} className={seatOf(a)} />)}</span>{tn('home.runningN', g.accounts.length)}</small>
        : <small>{g.last ? ago(new Date(g.last).toISOString()) : t('home.recentShort')}</small>}
    </button>;
  };
  return <div className="hm-shelf" ref={box} role="group" aria-label={t('home.games')}>
    {games.now.map((g, i) => tile(g, i))}
    {games.now.length && games.recent.length ? <span className="hm-split" aria-hidden="true" /> : null}
    {games.recent.map((g, i) => tile(g, games.now.length + i))}
  </div>;
}

/** Each account as a seat: filled while it plays here, amber when it needs you, dashed when it isn't open. */
function Seats({ accounts }: { accounts: Account[] }) {
  const { t, tn, fmtMb } = useHost();
  const m = useMetrics();
  if (!accounts.length) return <section className="hm-sec" />;
  const list = [...accounts].sort((x, y) => SEAT_ORDER[seatOf(x)] - SEAT_ORDER[seatOf(y)] || String(x.label).localeCompare(String(y.label)));
  const playing = list.filter(a => seatOf(a) === 'play').length, warn = list.filter(a => seatOf(a) === 'warn').length;
  const more = list.length > SEATS_SHOWN ? list.length - (SEATS_SHOWN - 1) : 0;
  return <section className="hm-sec" aria-labelledby="hm-acc">
    <div className="hm-sec-h">
      <h3 id="hm-acc">{t('nav.accounts')}</h3>
      <span>{t('home.playingOf', { n: playing, of: list.length })}{warn ? <><span className="sep">·</span><em>{tn('home.needN', warn)}</em></> : null}</span>
      <button type="button" className="link" data-act="view" data-view="accounts">{t('home.all')}</button>
    </div>
    <div className="hm-seats">
      {(more ? list.slice(0, SEATS_SHOWN - 1) : list).map(a => {
        const seat = seatOf(a);
        // What needs you, in the Accounts tab's words; else the status line it shows.
        const said = a.banned ? t('card.banned') : a.cookieValid === false ? t('act.signedOut') : a.session?.phase === 'challenged' ? t('act.botCheck')
          : statusLine(t, a).text || t('act.offline');
        const mb = seat === 'play' ? m.accounts.find(x => x.id === a.id)?.ramMb : 0;
        return <button key={a.id} type="button" className={`hm-seat ${seat}`} data-act="reveal" data-id={a.id} title={`${a.label} · ${said}`}>
          <span className="avatar sm"><Avatar a={a} /></span>
          <span className="hm-who"><b>{a.label}</b><small>{said}</small></span>
          {mb ? <span className="hm-ram">{fmtMb(mb)}</span> : null}
        </button>;
      })}
      {more ? <button type="button" className="hm-seat more" data-act="view" data-view="accounts">{t('home.moreN', { n: more })}</button> : null}
    </div>
  </section>;
}

/** The presets, with Run on each (as in the Presets tab) and when the next one runs by itself. */
function Presets() {
  const { t, runPreset, nextRunText } = useHost();
  const app = useAppState();
  const presets: any[] = (app.presets || []).slice(0, 5);
  const byId = new Map<string, Account>((app.accounts || []).map((a: Account) => [a.id, a]));
  if (!presets.length) return null;
  return <section className="hm-sec" aria-labelledby="hm-ps">
    <div className="hm-sec-h"><h3 id="hm-ps">{t('nav.presets')}</h3><button type="button" className="link" data-act="view" data-view="presets">{t('home.all')}</button></div>
    <div className="hm-list">{presets.map(p => {
      const n = (p.accountIds || []).filter((id: string) => byId.has(id)).length;
      const sub = [p.gameName || t('ps.noGame'), t(n === 1 ? 'ps.acc1' : 'ps.accN', { n }), nextRunText(p)].filter(Boolean).join(' · ');
      return <div key={p.id} className="hm-row">
        <span className="hm-row-text"><b title={p.name}>{p.name}</b><small title={sub}>{sub}</small></span>
        <IconButton icon="play" className="hm-run" disabled={!n} title={t('home.runPreset', { name: p.name })} aria-label={t('home.runPreset', { name: p.name })}
          onClick={() => { runPreset(p.id); }} />
      </div>;
    })}</div>
  </section>;
}

/** What Bloxkeep's users play now (the online count's figures): a game picked here goes in the game box. */
function Community({ shown }: { shown: boolean }) {
  const { t, tn } = useHost();
  const c = useCommunity(shown);
  const games = (c?.games || []).filter(g => g.name).slice(0, 5);
  if (!games.length) return null;
  return <section className="hm-sec" aria-labelledby="hm-cm">
    <div className="hm-sec-h"><h3 id="hm-cm">{t('home.community')}</h3></div>
    <div className="hm-list">{games.map(g => g.place
      ? <button key={g.game} type="button" className="hm-row" data-act="put-game" data-place={g.place} title={t('home.putGame', { name: g.name! })}>
        <span className="hm-row-text"><b>{g.name}</b></span><span className="hm-pcs">{tn('home.pcsN', g.pcs)}</span></button>
      : <div key={g.game} className="hm-row"><span className="hm-row-text"><b>{g.name}</b></span><span className="hm-pcs">{tn('home.pcsN', g.pcs)}</span></div>)}</div>
  </section>;
}
