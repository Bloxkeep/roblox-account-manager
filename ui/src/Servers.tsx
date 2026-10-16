// A game's servers, to pick one to join: the selected accounts' friends in it first, then its public servers (like
// the Servers tab on its Roblox page). Roblox sorts by player count itself; ping and FPS are sorted here over the
// servers loaded so far ("ping" is the average ping of the players already in that server, what Roblox reports, not
// the ping from this PC). Roblox gives only a few lists a minute: when it says to wait, the seconds count down in
// place and the list is asked for again by itself.
import { useEffect, useRef, useState } from 'react';
import { Icon, useHost } from './host';

type Server = { jobId: string; playing: number; maxPlayers: number; ping?: number; fps?: number };
type Friend = { userId: number; name?: string; displayName?: string; avatarUrl?: string; via: string; jobId: string; placeId?: number };
type Reply = { servers: Server[]; next: string | null; busy?: boolean; retryIn?: number; left?: number };
const SORTS = ['most', 'fewest', 'ping', 'fps'];

export function Servers({ placeId, ids, chosen }: { placeId: number; ids: string[]; chosen: (placeId: number, jobId: string) => void }) {
  const { t, call } = useHost();
  const party = ids.length;
  const [sort, setSort] = useState('most');
  const [room, setRoom] = useState(party > 1);
  const [items, setItems] = useState<Server[]>([]);
  const [next, setNext] = useState<string | null>(null);
  const [loading, setLoading] = useState(true); // the first list is asked for as the dialog opens: no "none" before it
  // busy: Roblox said to wait for the list (until waitUntil); moreUntil: for the next page.
  const [busy, setBusy] = useState(false);
  const [waitUntil, setWaitUntil] = useState(0);
  const [moreUntil, setMoreUntil] = useState(0);
  const [friends, setFriends] = useState<Friend[] | null>(null);
  const [friendsHow, setFriendsHow] = useState({ asked: false, failed: false });
  const [, tick] = useState(0);
  // How many more lists Roblox gives this minute (-1 unknown), and the latest of everything for the requests' replies.
  const st = useRef({ left: -1, seq: 0, items, next, sort, alive: true });
  st.current.items = items;
  st.current.next = next;
  st.current.sort = sort;
  useEffect(() => () => { st.current.alive = false; }, []);

  const isFull = (s: Server) => s.maxPlayers > 0 && s.playing >= s.maxPlayers;
  const local = (k: string) => k === 'ping' || k === 'fps';
  // Full servers can't be joined: they go last (the sort keeps the rest in order).
  const view = (room ? items.filter(s => s.maxPlayers - s.playing >= party) : items.slice());
  if (sort === 'ping') view.sort((a, b) => (a.ping || 1e9) - (b.ping || 1e9));
  if (sort === 'fps') view.sort((a, b) => (b.fps || 0) - (a.fps || 0));
  view.sort((a, b) => +isFull(a) - +isFull(b));
  const friendsIn = (jobId: string) => friends ? friends.filter(f => f.jobId === jobId).length : 0;
  const secondsTo = (at: number) => Math.max(1, Math.ceil((at - Date.now()) / 1000));
  // A second page for ping/FPS only while that leaves one for the next click.
  const spare = () => st.current.left < 0 || st.current.left >= 2;

  const load = async (more: boolean) => {
    const seq = ++st.current.seq;
    setLoading(true);
    if (!more) setBusy(false);
    // Ping and FPS are sorted here, so start from the fullest servers (the ones people actually play on).
    const r = await call('servers', { placeId, fewest: st.current.sort === 'fewest', cursor: more ? st.current.next : null }) as Reply | undefined;
    if (seq !== st.current.seq || !st.current.alive) return;
    setLoading(false);
    st.current.left = r?.left ?? -1;
    if (r?.busy) {
      // Roblox said how long: ask again by itself then (the rows loaded so far stay for a next page).
      const until = Date.now() + (r.retryIn || 20) * 1000;
      if (more) setMoreUntil(until); else { setBusy(true); setWaitUntil(until); }
      setTimeout(() => { if (st.current.alive && seq === st.current.seq) load(more); }, until - Date.now() + 400);
    } else if (r) {
      // A server can move between pages while players come and go; keep one row per server.
      const seen = new Set(more ? st.current.items.map(s => s.jobId) : []);
      const all = (more ? st.current.items : []).concat(r.servers.filter(s => !seen.has(s.jobId) && !!seen.add(s.jobId)));
      setItems(all);
      st.current.items = all;
      setNext(r.next);
      st.current.next = r.next;
      setMoreUntil(0);
    }
    // A second page gives sorting by ping or FPS more to choose from.
    if (!more && local(st.current.sort) && r?.next && !r.busy && spare()) load(true);
  };
  // The wait counts down (once a second) while Roblox has said to wait.
  const waiting = busy || moreUntil > Date.now();
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(() => tick(n => n + 1), 1000);
    return () => clearInterval(timer);
  }, [waiting]);

  useEffect(() => {
    (async () => {
      const r = await call('friendServers', { placeId, ids }, { silent: true }) as { friends: Friend[]; asked: number } | undefined;
      if (!st.current.alive) return;
      setFriends(r?.friends || []);
      setFriendsHow({ asked: !!r?.asked, failed: !r });
    })();
    load(false);
  }, []); // once, as the dialog opens

  const pickSort = (k: string) => {
    const was = sort;
    setSort(k);
    st.current.sort = k;
    // Most/fewest come from Roblox in that order; ping/FPS just re-sort what is loaded (plus a page if needed).
    if (k === 'fewest' || was === 'fewest' || !items.length) load(false);
    else if (local(k) && items.length < 150 && next && spare() && !loading) load(true);
  };
  const moreLabel = moreUntil > Date.now() ? t('srv.moreIn', { n: secondsTo(moreUntil) }) : t('srv.more');
  const more = next ? <button className="btn ghost w" data-more disabled={loading || moreUntil > Date.now()} onClick={() => load(true)}>{moreLabel}</button> : null;
  // The list keeps its height whatever it shows (app.css), and while another order loads the rows so far stay,
  // dimmed: the window doesn't shrink to a spinner and grow back.
  const rowsShown = !busy && view.length > 0;

  return <>
    <div className="srv-head">{t('srv.friendsTitle')}</div>
    <div className={`srv-friends${friends?.length ? ' list' : ''}`} id="srv-friends">
      {friends === null ? <div className="srv-friends-note"><span className="spin" />{t('srv.friendsLooking')}</div>
        : !friends.length ? <div className="srv-friends-note">{t(friendsHow.failed ? 'srv.friendsError' : friendsHow.asked ? 'srv.friendsNone' : 'srv.friendsNoAccount')}</div>
        : friends.map((f, i) => {
          const name = f.displayName || f.name || `#${f.userId}`;
          return <div key={i} className="srv-friend">
            {f.avatarUrl ? <img src={f.avatarUrl} alt="" /> : <span className="ph" />}
            <div className="gtxt"><b title={name}>{name}</b><small>{f.name ? `@${f.name} · ` : ''}{t('srv.friendOf', { acc: f.via })}</small></div>
            <button className="btn sm" data-friend={i} title={t('srv.pickTip')} onClick={() => chosen(f.placeId || placeId, f.jobId)}>{t('srv.joinFriend')}</button>
          </div>;
        })}
    </div>
    <div className="srv-head">{t('srv.publicTitle')}</div>
    <div className="srv-sort">{SORTS.map(k => <button key={k} className={`chip ${k === sort ? 'on' : ''}`} data-sort={k} aria-pressed={k === sort} onClick={() => pickSort(k)}>{t(`srv.sort.${k}`)}</button>)}</div>
    <div className="srv-tools">
      {party > 1 ? <button className={`chip ${room ? 'on' : ''}`} data-room aria-pressed={room} onClick={() => setRoom(!room)}><Icon name="users" />{t('srv.room', { n: party })}</button> : null}
      <button className="btn sm" data-random disabled={!view.some(s => !isFull(s))} onClick={() => {
        const open = view.filter(s => !isFull(s));
        if (open.length) { const s = open[Math.floor(Math.random() * open.length)]; chosen(placeId, s.jobId); }
      }}><Icon name="repeat" />{t('srv.random')}</button>
    </div>
    <div className={`srv-list${rowsShown && loading ? ' stale' : ''}${!rowsShown ? ' note' : ''}`} id="srv-list">
      {busy ? <div className="picker-empty">{t('srv.wait', { n: secondsTo(waitUntil) })}</div>
        : rowsShown ? <>{view.map(s => <div key={s.jobId} className={`srv-row ${isFull(s) ? 'full' : ''}`}>
          <div className="srv-fill" title={t('srv.players', { n: s.playing, max: s.maxPlayers })}><i style={{ transform: `scaleX(${s.maxPlayers ? (s.playing / s.maxPlayers).toFixed(3) : 0})` }} /></div>
          <b>{t('srv.players', { n: s.playing, max: s.maxPlayers })}{friendsIn(s.jobId) ? <>{' '}<span className="srv-friend-tag" title={t('srv.friendsHere')}><Icon name="users" />{friendsIn(s.jobId)}</span></> : null}</b>
          <span className="srv-ping" title={t('srv.pingTip')}>{s.ping ? t('srv.ping', { ms: s.ping }) : ''}</span>
          <span className="srv-ping">{s.fps ? t('srv.fpsVal', { fps: Math.round(s.fps) }) : ''}</span>
          <button className="btn sm" disabled={isFull(s)} title={isFull(s) ? t('srv.full') : t('srv.pickTip')} onClick={() => chosen(placeId, s.jobId)}>{t('srv.pick')}</button>
        </div>)}{more}</>
        : loading ? <div className="picker-empty"><span className="spin" /></div>
        : items.length ? <><div className="picker-empty">{t('srv.noneRoom', { n: party })}</div>{more}</>
        : <div className="picker-empty">{t('srv.none')}</div>}
    </div>
  </>;
}
