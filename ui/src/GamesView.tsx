// The Games tab: the games Bloxkeep's bot has news or a price list for and the popular ones it names (wiki.js
// POPULAR_GAMES), each with its wiki named by the bot, as cards with the game's own picture and colour
// (AppController.Wiki.cs, from its Roblox page): the ones this PC's accounts played (Statistics' stays) or follow first, then
// the popular ones. A game is here only with a wiki or news ready: the user never looks for a wiki or pastes one (the
// owner's call on 2026-11-06), and a game played here that the bot doesn't name isn't shown. Opened, a game has its News,
// its whole Wiki (WikiSite.tsx) and its Prices, and a switch to any other game without going back.
import { RadarStrip, playingOf, useCommunity, type RadarLine } from './Community';
import { useEffect, useMemo, useRef, useState } from 'react';
import { Icon, useAppState, useHost, usePage } from './host';
import { Pop } from './MacroPop';
import { FollowButton, NewsPanel, useFeedCards, type FeedGame, type Feeds } from './NewsView';
import { PricePanel, useWikiGames, type WikiInfo } from './WikiView';
import { WikiSite, type WikiPick } from './WikiSite';
import { timeText } from './DiscordBlocks';

type Look = { icon: string | null; cover: string | null; tint: string | null; ink: string | null };
type Game = { id: string; name: string; place: number; news?: FeedGame; prices?: WikiInfo; pick?: WikiPick; added?: boolean; played?: Played;
  /** How many PCs with Bloxkeep play it now (Community.tsx), and its radar lines. */
  playing?: number; radar?: RadarLine[] };
/** A game this PC's accounts played in the last 90 days (AppController.PlayedGames). */
type Played = { name: string; place: number; hours: number; last: number };
type Part = 'news' | 'wiki' | 'prices';
type T = (k: string, v?: Record<string, string | number>) => string;

/** A Roblox game's name without what its makers dress it in ("[🎃] Adopt Me!" → "Adopt Me!", "🔥 UPD 24 Blox Fruits" → "Blox Fruits"),
 * the name its wiki goes by. */
const plainName = (name: string) => name.replace(/\[[^\]]*\]|\([^)]*\)|【[^】]*】/g, ' ').replace(/[^\p{L}\p{N}\s'!:&.-]/gu, ' ')
  .replace(/\b(UPD(ATE)?|NEW|EVENT|x\d+)\b\s*\d*/gi, ' ').replace(/\s+/g, ' ').trim() || name;
/** Each place's look, asked of the app (it asks Roblox, and keeps them on disk), kept between visits to the tab. */
const looksNow: Record<string, Look> = {};
const fold = (s: string) => s.toLowerCase().normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd');
/** A game's name to match the same game from two sides ("[🎃] Blox Fruits" and "Blox Fruits"). */
const sameName = (s: string) => fold(plainName(s)).replace(/[^a-z0-9]/g, '');
/** One of the user's own games: news, played here, or added. */
const mine = (g: Game) => !!(g.news || g.played || g.added);

/** The games played here, asked each time the tab is shown (they change as accounts play). */
function usePlayed(shown: boolean) {
  const { call } = useHost();
  const [played, setPlayed] = useState<Played[]>([]);
  useEffect(() => {
    if (!shown) return;
    (call('playedGames', {}, { silent: true }) as Promise<Played[] | undefined>).then(d => { if (Array.isArray(d)) setPlayed(d); }, () => {});
  }, [shown]);
  return played;
}

export function GamesView() {
  const { t, call, locale } = useHost();
  const app = useAppState();
  const feeds: Feeds = app.feeds || { on: false, connected: false, loaded: false, games: [], unread: 0, fresh: 0 };
  const picks: WikiPick[] = app.wikis?.games || [];
  const shown = usePage(p => p.view) === 'news';
  const { cards, loaded } = useFeedCards(shown);
  const { games: botGames, failed } = useWikiGames(shown);
  const played = usePlayed(shown);
  const community = useCommunity(shown);
  useEffect(() => { if (shown && feeds.fresh) call('feedSeen', {}, { silent: true }); }, [shown, feeds.fresh]);

  // One card per game, from any side; followed games first, then the ones with news unread, then the ones added here.
  const games = useMemo(() => {
    const by = new Map<string, Game>();
    for (const n of feeds.games) by.set(n.id, { id: n.id, name: n.name, place: n.place || 0, news: n });
    for (const w of botGames || []) {
      const had = by.get(w.id);
      // The wiki the bot names for it (checked by hand), over any the app once found by the game's name.
      const pick = w.wiki ? { id: w.id, name: w.name, site: w.wiki, place: w.place || 0, img: null, added: false } : picks.find(p => p.id === w.id);
      by.set(w.id, { ...had, id: w.id, name: had?.name || w.name, place: had?.place || w.place || 0, prices: w.kind === 'prices' ? w : had?.prices, pick });
    }
    // Games added here before (their wiki picked then) stay.
    for (const p of picks)
      if (p.added && p.site && !by.has(p.id)) by.set(p.id, { id: p.id, name: p.name, place: p.place || 0, pick: p, added: true });
    // A game played here joins the card it has (by its place or its name); one the bot doesn't name has no wiki here.
    for (const p of played) {
      const had = [...by.values()].find(g => (p.place && g.place === p.place) || sameName(g.name) === sameName(p.name));
      if (had) had.played = p;
    }
    // What Bloxkeep's users play now: on its card, and the popular ones played most first.
    for (const g of by.values()) {
      g.playing = playingOf(community?.games, g.place, g.name)[0]?.pcs;
      g.radar = playingOf(community?.radar, g.place, g.name);
    }
    // Only a game with its wiki (the bot names one for each of its games): one with only news or prices isn't shown.
    return [...by.values()].filter(g => g.pick).sort((a, b) => Number(mine(b)) - Number(mine(a)) || Number(!!b.news?.followed) - Number(!!a.news?.followed)
      || (b.news?.unread || 0) - (a.news?.unread || 0) || (b.played?.hours || 0) - (a.played?.hours || 0)
      || Number(!!a.added) - Number(!!b.added) || (b.playing || 0) - (a.playing || 0) || a.name.localeCompare(b.name));
  }, [feeds.games, botGames, played, picks, community]);

  const [looks, setLooks] = useState<Record<string, Look>>(looksNow);
  const wanted = games.map(g => g.place).filter(p => p && !(p in looksNow));
  useEffect(() => {
    if (!shown || !wanted.length) return;
    const ask = (places: number[], again: boolean) => (call('gameLooks', { places }, { silent: true }) as Promise<Record<string, Look> | undefined>).then(d => {
      for (const p of places) looksNow[p] = d?.[p] || { icon: null, cover: null, tint: null, ink: null };
      setLooks({ ...looksNow });
      // The pictures come first, their colours a moment later (WikiLook.cs): asked once more for the ones still without.
      const pale = places.filter(p => looksNow[p].icon && !looksNow[p].tint);
      if (again && pale.length) setTimeout(() => ask(pale, false), 4000);
    });
    ask(wanted, true);
  }, [shown, wanted.join()]);
  const lookOf = (g: Game): Look => {
    const l = looks[g.place];
    return { icon: l?.icon || g.news?.iconUrl || g.pick?.img || null, cover: l?.cover || null, tint: l?.tint || null, ink: l?.ink || null };
  };

  const [openId, setOpenId] = useState<string | null>(null);
  const [part, setPart] = useState<Part>('news');
  const game = games.find(g => g.id === openId) || null;
  const parts: Part[] = game ? [...(game.news ? ['news' as const] : []), ...(game.pick ? ['wiki' as const] : []), ...(game.prices ? ['prices' as const] : [])] : [];
  const at: Part | null = parts.includes(part) ? part : parts[0] || null;
  const scroll = useRef<HTMLDivElement>(null);
  const openGame = (id: string) => { setOpenId(id); scroll.current?.scrollTo({ top: 0 }); };
  const remove = (g: Game) => {
    call('wikiPick', { id: g.id, site: '' }, { silent: true });
    setOpenId(null);
  };

  const status = !feeds.on ? t('news.offHere') : !feeds.connected ? (feeds.loaded ? t('news.offline') : t('news.connecting')) : t('news.live');
  const look = game ? lookOf(game) : null;
  const tint = look?.tint ? { '--gt': look.tint, '--gt-ink': look.ink } as React.CSSProperties : undefined;
  return <div className={`gm ${game ? 'in-game' : ''}`} style={tint}>
    <header className="top">
      <div className="top-title">{game
        ? <div className="gm-crumbs">
          <button type="button" className="gm-crumb" id="gm-all" onClick={() => setOpenId(null)}>{t('news.title')}</button>
          <Icon name="chevronRight" />
          <Switcher games={games} game={game} lookOf={lookOf} t={t} onPick={openGame} />
        </div>
        : <h1>{t('news.title')}</h1>}
        <p id="news-sub"><i className={`dot ${feeds.connected ? 'ok' : ''}`} />{status}</p></div>
    </header>
    <div className="scroll news-scroll" id="news-scroll" ref={scroll}>
      {!game ? <>
        <RadarStrip lines={community?.radar || []} t={t} />
        <Grid games={games} lookOf={lookOf} t={t} onOpen={openGame}
          empty={!games.length ? (failed || botGames ? t('news.noneTitle') : t('news.connecting')) : null} />
      </>
        : <>
          <Hero game={game} look={look!} t={t} locale={locale} onRemove={game.added ? () => remove(game) : null} />
          <RadarStrip lines={game.radar || []} t={t} named={false} />
          {parts.length > 1 ? <div className="seg gm-parts" role="tablist" aria-label={game.name}>
            {parts.map(p => <button key={p} type="button" role="tab" aria-selected={at === p} className={at === p ? 'on' : ''} data-part={p} onClick={() => setPart(p)}>
              <Icon name={p === 'news' ? 'news' : p === 'prices' ? 'tag' : 'book'} />
              <span>{p === 'news' ? t('games.news') : p === 'prices' ? t('prices.tab') : t('games.wiki')}</span>
              {p === 'news' && game.news?.unread ? <em>{game.news.unread}</em> : null}
            </button>)}
          </div> : null}
          {game.news ? <div hidden={at !== 'news'}><NewsPanel key={game.id} game={game.news} cards={cards} loaded={loaded} shown={shown && at === 'news'} /></div> : null}
          {game.pick ? <div hidden={at !== 'wiki'}>
            <WikiSite key={`${game.id}|${game.pick.site}`} pick={game.pick} gameId={game.id} shown={shown && at === 'wiki'} /></div> : null}
          {game.prices ? <div hidden={at !== 'prices'}><PricePanel key={game.id} info={game.prices} shown={shown && at === 'prices'} /></div> : null}
        </>}
    </div>
  </div>;
}

/** The games as cards, found by name when there are many. */
function Grid({ games, lookOf, t, empty, onOpen }: {
  games: Game[]; lookOf: (g: Game) => Look; t: T; empty: string | null; onOpen: (id: string) => void;
}) {
  const [query, setQuery] = useState('');
  const words = fold(query).trim();
  const list = words ? games.filter(g => fold(g.name).includes(words)) : games;
  if (empty) return <div className="empty news-empty"><h2>{empty}</h2><p>{t('news.noneText')}</p></div>;
  return <>
    {games.length > 6 ? <div className="pl-tools gm-tools"><label className="pl-search"><Icon name="search" />
      <input className="field sm" type="search" value={query} placeholder={t('games.search', { n: games.length })} aria-label={t('games.search', { n: games.length })}
        maxLength={40} autoComplete="off" spellCheck={false} onChange={e => setQuery(e.target.value)} /></label></div> : null}
    {list.length ? (words || list.every(mine) || !list.some(mine) ? [list] : [list.filter(mine), list.filter(g => !mine(g))]).map((part, i, parts) => <section key={i} className="gm-sec">
      {parts.length > 1 ? <h2 className="gm-sec-title">{i ? t('games.popular') : t('games.mine')}</h2> : null}
      <div className="gm-grid">{part.map(g => {
      const l = lookOf(g);
      return <button key={g.id} type="button" className="gm-card" data-game={g.id} onClick={() => onOpen(g.id)}
        style={l.tint ? { '--gt': l.tint, '--gt-ink': l.ink } as React.CSSProperties : undefined}>
        <span className="gm-cover">{l.cover ? <img src={l.cover} alt="" loading="lazy" /> : null}</span>
        <span className="gm-card-body">
          <GameIcon look={l} name={g.name} />
          <span className="gm-card-text"><b>{g.name}</b>
            <span className="gm-tags">
              {g.news ? <span className={`gm-tag ${g.news.unread ? 'hot' : ''}`}><Icon name="news" />{g.news.unread ? t('news.unreadN', { n: g.news.unread }) : t('games.news')}</span> : null}
              {g.pick ? <span className="gm-tag"><Icon name="book" />{t('games.wiki')}</span> : null}
              {g.prices ? <span className="gm-tag"><Icon name="tag" />{t('prices.tab')}</span> : null}
              {g.played ? <span className="gm-tag" title={t('games.playedTip')}><Icon name="gamepad" />{t('games.playedH', { n: g.played.hours < 1 ? '<1' : Math.round(g.played.hours) })}</span> : null}
              {g.playing ? <span className="gm-tag playing" title={t('cm.playingTip')}><Icon name="users" />{t('cm.playingN', { n: g.playing })}</span> : null}
              {g.radar?.length ? <span className="gm-tag radar" title={t('cm.radarTip')}><Icon name="alert" />{t('cm.radarTag')}</span> : null}
            </span></span>
          {g.news?.followed ? <span className="gm-followed" title={t('news.following')}><Icon name="check" /></span> : g.news?.isNew ? <em className="news-new">{t('news.new')}</em> : null}
        </span>
      </button>;
    })}</div></section>) : <div className="empty news-empty"><h2>{t('wiki.nothing')}</h2></div>}
  </>;
}

function GameIcon({ look, name, big }: { look: Look; name: string; big?: boolean }) {
  return look.icon ? <img className={`gm-icon ${big ? 'big' : ''}`} src={look.icon} alt="" referrerPolicy="no-referrer" />
    : <span className={`gm-icon gm-mark ${big ? 'big' : ''}`}>{name.slice(0, 1)}</span>;
}

/** The open game's banner: its picture, name, where its news and wiki come from, Follow (or, for one added here, Remove). */
function Hero({ game, look, t, locale, onRemove }: { game: Game; look: Look; t: T; locale: string; onRemove: (() => void) | null }) {
  const from = [
    ...(game.news?.sources.length ? [game.news.sources.map(s => s.title).join(' · ')] : []),
    ...(game.pick ? [game.pick.site.replace(/^https:\/\//, '')] : []),
    ...(game.prices?.at ? [`${t('prices.tab')} · ${t('wiki.read', { t: timeText(game.prices.at, 'R', locale, Date.now()) })}`] : []),
  ];
  return <section className={`gm-hero ${look.cover ? 'has-cover' : ''}`}>
    <div className="gm-hero-text">
      <GameIcon look={look} name={game.name} big />
      <div><h2>{game.name}</h2><p>{[...from, ...(game.playing ? [t('cm.playingN', { n: game.playing })] : [])].join(' · ')}</p></div>
      {game.news ? <FollowButton game={game.news} t={t} />
        : onRemove ? <button type="button" className="btn sm ghost gm-remove" id="gm-remove" onClick={onRemove}><Icon name="x" /><span>{t('games.remove')}</span></button> : null}
    </div>
    {look.cover ? <img className="gm-hero-cover" src={look.cover} alt="" /> : null}
  </section>;
}

/** The open game's name, as a button to any other game: a list found by name. */
function Switcher({ games, game, lookOf, t, onPick }: { games: Game[]; game: Game; lookOf: (g: Game) => Look; t: T; onPick: (id: string) => void }) {
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  const [query, setQuery] = useState('');
  const words = fold(query).trim();
  const list = words ? games.filter(g => fold(g.name).includes(words)) : games;
  const close = () => { setAnchor(null); setQuery(''); };
  return <>
    <button type="button" className="gm-switch" id="gm-switch" aria-haspopup="listbox" aria-expanded={!!anchor}
      onClick={e => anchor ? close() : setAnchor(e.currentTarget)}>
      <GameIcon look={lookOf(game)} name={game.name} /><h1>{game.name}</h1><Icon name="chevronDown" /></button>
    {anchor ? <Pop anchor={anchor} close={close} label={t('games.switch')} className="mc-pop menu gm-switch-pop">
      {games.length > 6 ? <input className="field sm" type="search" value={query} placeholder={t('games.search', { n: games.length })} aria-label={t('games.search', { n: games.length })}
        maxLength={40} autoComplete="off" spellCheck={false} onChange={e => setQuery(e.target.value)}
        onKeyDown={e => { if (e.key === 'Enter' && list[0]) { onPick(list[0].id); close(); } }} /> : null}
      <div className="gm-switch-list" role="listbox" aria-label={t('games.switch')}>
        {list.map(g => <button key={g.id} type="button" role="option" aria-selected={g.id === game.id} className={g.id === game.id ? 'on' : ''}
          onClick={() => { onPick(g.id); close(); }}>
          <GameIcon look={lookOf(g)} name={g.name} /><span>{g.name}</span>{g.news?.unread ? <em>{g.news.unread}</em> : null}
        </button>)}
      </div>
    </Pop> : null}
  </>;
}
