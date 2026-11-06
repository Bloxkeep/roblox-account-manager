// What Bloxkeep's users play now, as the online count's bot tells every app back with each beat (AppController.Online.cs,
// services/DiscordBot/worker/online.js communityOf): the games played most (a game shows with 3 PCs on it or more), the
// radar (a game where 3 PCs or more dropped out the same way in the last 15 minutes) and the alts board (the PCs running
// the most accounts, under a made-up name or the one they chose). With the count off, asked for apart (communityNow).
import { useEffect } from 'react';
import { Icon, useAppState, useHost, usePage } from './host';
import { TextBox } from './settings/parts';

export type PlayingGame = { game: number; name: string | null; place: number | null; pcs: number };
export type RadarLine = PlayingGame & { why: 'kick' | 'lost' | 'shutdown' | 'join'; minutes: number };
export type Community = {
  pcs: number; alts: number; games: PlayingGame[]; radar: RadarLine[]; board: { name: string; alts: number }[];
  /** This PC on the board: its place among how many; null when it isn't on it. */
  you: { rank: number | null; of: number } | null;
};
type T = (k: string, v?: Record<string, string | number>) => string;

/** The sums; while the count is off and this is on screen, asked for once a minute (with the count on, each beat brings them). */
export function useCommunity(shown: boolean): Community | null {
  const app = useAppState();
  const { call } = useHost();
  const off = app.countOnline === false;
  useEffect(() => {
    if (!shown || !off)
      return;
    const ask = () => { call('communityNow', {}, { silent: true }).catch(() => {}); };
    ask();
    const timer = setInterval(ask, 60_000);
    return () => clearInterval(timer);
  }, [shown, off]);
  return app.community?.data || null;
}

const sameName = (s: string) => s.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, '');
/** A game card's line in it: by its start place, else by its name. */
export function playingOf<L extends PlayingGame>(list: L[] | undefined, place: number, name: string) {
  return (list || []).filter(g => (place && g.place === place) || (g.name && sameName(g.name) === sameName(name)));
}

/** "Radar": games where several Bloxkeep users dropped out the same way just now, as a strip of warnings. */
export function RadarStrip({ lines, t, named = true }: { lines: RadarLine[]; t: T; named?: boolean }) {
  if (!lines.length) return null;
  return <div className="cm-radar" role="status">
    {lines.map(r => <p key={`${r.game}|${r.why}`}><Icon name="alert" />
      <span>{named ? <b>{r.name || t('cm.aGame')}</b> : null}{named ? ': ' : ''}{t(`cm.radar.${r.why}`, { n: r.pcs, m: r.minutes })}</span></p>)}
  </div>;
}

/** Statistics' alts board: the PCs running the most accounts now, this one marked; joined here under a name. */
export function AltsBoard() {
  const { t, call, locale } = useHost();
  const app = useAppState();
  const c = useCommunity(usePage(p => p.view) === 'stats');
  const counted = app.countOnline !== false, name: string = app.boardName || '';
  const num = (n: number) => n.toLocaleString(locale);
  const most = c?.board[0]?.alts || 1;
  const isMe = (i: number) => !!name && c?.you?.rank === i + 1;
  // The first three on a podium (2nd, 1st, 3rd, each as high as its alts), the rest as bars against the first.
  const top = c?.board.slice(0, 3) || [], rest = c?.board.slice(3) || [];
  return <section className="stat-card cm-board" id="alts-board" aria-labelledby="alts-board-h">
    <div className="cm-head">
      <h3 id="alts-board-h">{t('cm.board')}</h3>
      {c ? <span className="cm-live"><i />{t('cm.live')}</span> : null}
      {c ? <div className="cm-totals">
        <span><b>{num(c.alts)}</b>{t('cm.accounts')}</span>
        <span><b>{num(c.pcs)}</b>{t('cm.pcs')}</span>
      </div> : null}
    </div>
    {!c ? <p className="cm-note">{t('news.connecting')}</p>
      : !c.board.length ? <p className="cm-note">{t('cm.empty')}</p>
      : <>
        <ol className="cm-podium">{[1, 0, 2].filter(i => top[i]).map(i => <li key={i} className={`cm-step p${i + 1} ${isMe(i) ? 'me' : ''}`}
          style={{ '--h': 0.45 + 0.55 * top[i].alts / most } as React.CSSProperties} aria-label={`${i + 1}. ${top[i].name}: ${t('cm.altsN', { n: num(top[i].alts) })}`}>
          <span className="cm-who"><span className="cm-medal">{i + 1}</span><b title={top[i].name}>{top[i].name}</b>{isMe(i) ? <em>{t('cm.you')}</em> : null}</span>
          <span className="cm-block"><strong>{num(top[i].alts)}</strong><small>{t('cm.alts')}</small></span>
        </li>)}</ol>
        {rest.length ? <ol className="cm-rows" start={4}>{rest.map((b, j) => {
          const i = j + 3;
          return <li key={i} className={`cm-row ${isMe(i) ? 'me' : ''}`}>
            <span className="cm-rank">{i + 1}</span>
            <span className="cm-name"><span title={b.name}>{b.name}</span>{isMe(i) ? <em>{t('cm.you')}</em> : null}</span>
            <span className="cm-bar"><i style={{ '--f': b.alts / most } as React.CSSProperties} /></span>
            <b>{num(b.alts)}</b>
          </li>;
        })}</ol> : null}
      </>}
    {!counted ? <div className="cm-join"><small>{t('cm.joinOff')}</small></div> : <div className="cm-join">
      {name && c?.you?.rank && c.you.rank > c.board.length ? <p>{t('cm.yourRank', { r: c.you.rank, n: c.you.of })}</p> : null}
      {name && c?.you && !c.you.rank ? <p>{t('cm.notYet')}</p> : null}
      <label><span>{t(name ? 'cm.nameOn' : 'cm.join')}</span>
        <TextBox className="field sm" saved={name} maxLength={20} spellCheck={false} placeholder={t('cm.namePh')} aria-label={t('cm.join')}
          commit={v => { call('setBoardName', { name: v }); }} /></label>
      <small>{t('cm.joinNote')}</small>
    </div>}
  </section>;
}
