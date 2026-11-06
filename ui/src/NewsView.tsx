// A game's News part in the Games tab (GamesView.tsx): news other games' bots post (a stock, a boss spawning, a full
// moon), brought by Bloxkeep's Discord bot (AppController.Feeds.cs). When to be told (never, every piece, or one with a
// word), its sources to pick from, and its news newest first. Which games there are is the bot's: nothing here names one.
import { useEffect, useMemo, useState } from 'react';
import { Blocks, Inline, NewsCtx, timeText, type Block, type Field } from './DiscordBlocks';
import { Icon, useHost } from './host';
import { TextBox } from './settings/parts';

export type FeedCard = {
  id: string; src: string; game: string; at: number; title: string; text: string; fields: Field[]; picture: string | null; image?: string | null;
  author?: { name: string; icon: string | null; url?: string | null } | null; link: string | null; color?: number | null; until: number | null;
  blocks?: Block[] | null; spoiler?: boolean;
};
export type FeedGame = { id: string; name: string; place: number | null; iconUrl?: string | null; sources: { id: string; title: string }[]; followed: boolean; unread: number; read: number; isNew: boolean; alert: { mode: 'off' | 'all' | 'words'; words: string[] } };
export type Feeds = { on: boolean; connected: boolean; loaded: boolean; games: FeedGame[]; unread: number; fresh: number };
type T = (k: string, v?: Record<string, string | number>) => string;

const NONE: FeedCard[] = [];
/** The cards as the app last sent them, kept between visits to the tab (asked for once, then heard of as they change). */
let cardsNow: FeedCard[] | null = null;

const hex = (n?: number | null) => (n == null ? undefined : `#${n.toString(16).padStart(6, '0')}`);
/** "2:05" or "1:02:05" left. */
function left(ms: number) {
  const s = Math.max(0, Math.ceil(ms / 1000)), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, sec = s % 60;
  return `${h ? `${h}:${String(m).padStart(2, '0')}` : m}:${String(sec).padStart(2, '0')}`;
}

/** Every game's news, asked for once the tab is first shown, then heard of as it changes. */
export function useFeedCards(shown: boolean) {
  const { call, on } = useHost();
  const [cards, setCards] = useState<FeedCard[]>(cardsNow || NONE);
  useEffect(() => on('feedCards', (d?: FeedCard[]) => { cardsNow = d || []; setCards(cardsNow); }), [on]);
  useEffect(() => {
    if (!shown || cardsNow) return;
    (call('feedCards', {}, { silent: true }) as Promise<FeedCard[] | undefined>).then(d => { if (d) { cardsNow = d; setCards(d); } });
  }, [shown]);
  return { cards, loaded: cards !== NONE };
}

/** Follow a game: its news count on the tab's badge. */
export function FollowButton({ game, t }: { game: FeedGame; t: T }) {
  const { call } = useHost();
  return <button type="button" className={`btn sm ${game.followed ? 'ghost' : 'primary'}`} id="news-follow" aria-pressed={game.followed}
    onClick={() => call('feedFollow', { game: game.id, on: !game.followed })}>
    <Icon name={game.followed ? 'check' : 'plus'} /><span>{t(game.followed ? 'news.following' : 'news.follow')}</span></button>;
}

/** A game's news, shown while `shown`: read as it is shown (what was new keeps its mark while it stays open). */
export function NewsPanel({ game, cards, loaded, shown }: { game: FeedGame; cards: FeedCard[]; loaded: boolean; shown: boolean }) {
  const { t, call, locale } = useHost();
  const [source, setSource] = useState('');
  const list = useMemo(() => cards.filter(c => c.game === game.id && (!source || c.src === source)), [cards, game.id, source]);
  // How far the game was read when it was first shown this time: what came after keeps its New mark while it is open.
  const [readBefore, setReadBefore] = useState<number | null>(null);
  useEffect(() => {
    if (!shown || !loaded) return;
    setReadBefore(r => r ?? (game.followed ? game.read : Infinity));
    if (game.followed && game.unread) call('feedRead', { game: game.id }, { silent: true });
  }, [shown, loaded, game.unread, game.followed]);
  useEffect(() => { if (!shown) setReadBefore(null); }, [shown]);

  // The clock for "in 5 minutes" and countdowns: every second while something counts down, else every half minute.
  const [now, setNow] = useState(Date.now());
  const counting = list.some(c => c.until && c.until > now);
  useEffect(() => {
    if (!shown) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), counting ? 1000 : 30000);
    return () => clearInterval(timer);
  }, [shown, counting]);

  const ctx = useMemo(() => ({ t, locale, now, open: (url: string) => { call('feedOpen', { url }); } }), [t, locale, now, call]);
  return <>
    <Alerts game={game} t={t} />
    {game.sources.length > 1 ? <div className="chips news-sources" role="group" aria-label={t('news.sources')}>
      <button type="button" className={`chip ${source ? '' : 'on'}`} onClick={() => setSource('')}>{t('news.all')}</button>
      {game.sources.map(s => <button key={s.id} type="button" className={`chip ${source === s.id ? 'on' : ''}`} onClick={() => setSource(s.id)}>{s.title}</button>)}
    </div> : null}
    {list.length ? <NewsCtx.Provider value={ctx}><div className="news-list">
      {list.map(c => <NewsCard key={c.id} card={c} t={t} locale={locale} now={now} isNew={c.at > (readBefore ?? Infinity)}
        source={game.sources.length > 1 ? game.sources.find(s => s.id === c.src)?.title : undefined} />)}
    </div></NewsCtx.Provider> : loaded && shown ? <div className="empty news-empty"><h2>{t('news.nothingYet')}</h2><p>{t('news.nothingYetText')}</p></div> : null}
  </>;
}

/** When to be told of the game's news. */
function Alerts({ game, t }: { game: FeedGame; t: T }) {
  const { call } = useHost();
  const mode = game.alert.mode;
  return <div className="news-alert">
    <span className="news-alert-label"><Icon name="bell" />{t('news.tellMe')}</span>
    <div className="seg news-seg" role="radiogroup" aria-label={t('news.tellMe')}>
      {(['off', 'all', 'words'] as const).map(m => <button key={m} type="button" role="radio" aria-checked={mode === m} className={mode === m ? 'on' : ''}
        onClick={() => call('feedAlert', { game: game.id, mode: m })}>{t(`news.tell.${m}`)}</button>)}
    </div>
    {mode === 'words' ? <TextBox className="field sm news-words" saved={game.alert.words.join(', ')} maxLength={400} spellCheck={false}
      placeholder={t('news.wordsHint')} aria-label={t('news.words')} commit={v => { call('feedAlert', { game: game.id, words: v }); }} /> : null}
  </div>;
}

/** Blocks for a card from before the bot sent them: its words, its items, its big picture. */
function oldBlocks(c: FeedCard): Block[] {
  return [
    ...(c.text ? [{ t: 'md', md: c.text } as Block] : []),
    ...(c.fields.length ? [{ t: 'fields', items: c.fields.map(f => ({ ...f, inline: true })) } as Block] : []),
    ...(c.image ? [{ t: 'gallery', items: [{ url: c.image }] } as Block] : []),
  ];
}

function NewsCard({ card: c, t, locale, now, isNew, source }: { card: FeedCard; t: T; locale: string; now: number; isNew: boolean; source?: string }) {
  const { call } = useHost();
  const until = c.until && c.until > now ? c.until : null;
  const from = c.author?.name || source;
  const open = (url?: string | null) => url ? () => { call('feedOpen', { url }); } : undefined;
  return <article className={`news-card ${isNew ? 'is-new' : ''}`} style={{ '--nc': hex(c.color) } as React.CSSProperties}>
    <div className="nc-meta">
      {c.author?.icon ? <img className="nc-by" src={c.author.icon} alt="" loading="lazy" /> : null}
      {from ? c.author?.url ? <button type="button" className="nc-src nc-src-link" onClick={open(c.author.url)}>{from}</button> : <span className="nc-src">{from}</span> : null}
      {c.author?.name && source ? <span className="nc-tag">{source}</span> : null}
      <time dateTime={new Date(c.at).toISOString()} title={new Date(c.at).toLocaleString(locale)}>{timeText(c.at, 'R', locale, now)}</time>
      {isNew ? <b className="nc-new">{t('news.new')}</b> : null}
    </div>
    <div className="nc-main">
      {c.picture ? <img className="nc-pic" src={c.picture} alt="" loading="lazy" /> : null}
      {c.title ? <h3>{c.link ? <button type="button" className="nc-title-link" title={c.link} onClick={open(c.link)}><Inline text={c.title} /></button> : <Inline text={c.title} />}</h3> : null}
      {until ? <span className="nc-until" title={timeText(until, 'f', locale, now)}><Icon name="clock" />{t('news.left', { t: left(until - now) })}</span> : null}
      <Blocks blocks={c.blocks || oldBlocks(c)} spoiler={c.spoiler} />
    </div>
  </article>;
}
