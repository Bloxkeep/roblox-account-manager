// A game's page (opened from its name under the game box): Roblox's own figures for it, when it was last updated
// (a fresh update says so: old servers may be shut down), your accounts in it over the last 7 days, the hours it is
// quiet (from the server lists Bloxkeep has seen), and its description. Buttons: play it, its wiki, its roblox.com page.
import { useState } from 'react';
import { Icon, useHost } from './host';

export interface GameAccount {
  id: string; name: string; minutes: number; kicks: number; kickWhy?: string | null; drops: number;
  /** "ingame", "banned" or "" (not in it now); lastPlayed when not in it. */
  now: string; lastPlayed?: string | null;
}
export interface GameDetails {
  placeId: number; name: string; creator: string; genre?: string | null; iconUrl?: string | null; description?: string | null;
  playing: number; visits: number; favorites: number; likes?: number | null; created?: string | null; updated?: string | null;
  accounts: GameAccount[];
  /** Players per hour of the day (0-23) as Bloxkeep saw them, null until it has seen enough. */
  hours?: (number | null)[] | null;
}

/** An update this recent is shown as news: Roblox may be shutting old servers down. */
const FRESH_UPDATE_HOURS = 24;

export function GamePage({ game }: { game: GameDetails }) {
  const { t, locale, fmtNum, ago, call } = useHost();
  const [more, setMore] = useState(false);
  const updated = game.updated ? new Date(game.updated) : null;
  const fresh = updated && Date.now() - updated.getTime() < FRESH_UPDATE_HOURS * 3600e3;
  const big = (n: number) => n >= 1e9 ? `${(n / 1e9).toLocaleString(locale, { maximumFractionDigits: 1 })} ${t('gp.billion')}`
    : n >= 1e6 ? `${(n / 1e6).toLocaleString(locale, { maximumFractionDigits: 1 })} ${t('gp.million')}` : fmtNum(n);
  const time = (iso: string) => new Date(iso).toLocaleTimeString(locale, { hour: '2-digit', minute: '2-digit' });
  const hours = game.hours && game.hours.filter(h => h != null).length >= 12 ? game.hours : null;
  const peak = hours ? Math.max(...hours.map(h => h ?? 0)) : 0;
  const quiet = hours ? quietestSpan(hours) : null;

  return <div className="gp">
    <div className="gp-head">
      {game.iconUrl ? <img className="gp-icon" src={game.iconUrl} alt="" /> : <span className="gp-icon"><Icon name="gamepad" /></span>}
      <div className="gp-title">
        <b>{game.name}</b>
        <small>{[game.creator, game.genre].filter(Boolean).join(' · ')}</small>
      </div>
      <button className="btn sm" data-gp="wiki" onClick={() => call('openGameWiki', { name: game.name })}><Icon name="book" />{t('gp.wiki')}</button>
      <button className="btn sm ghost" data-gp="roblox" title={t('gp.roblox')} aria-label={t('gp.roblox')}
        onClick={() => call('openGamePage', { placeId: game.placeId })}><Icon name="external" /></button>
    </div>

    {fresh && updated ? <div className="gp-news"><Icon name="refresh" />
      <span>{t('gp.fresh', { time: time(game.updated!), ago: ago(game.updated!) })}</span></div> : null}

    <div className="gp-stats">
      <Stat label={t('gp.playing')} value={fmtNum(game.playing)} />
      <Stat label={t('gp.visits')} value={big(game.visits)} />
      <Stat label={t('gp.likes')} value={game.likes != null ? `${game.likes}%` : '–'} />
      <Stat label={t('gp.updated')} value={game.updated ? ago(game.updated) : '–'} />
    </div>

    <h4 className="gp-h">{t('gp.yours')}<small>{t('gp.week')}</small></h4>
    {game.accounts.length ? <div className="gp-table" role="table">
      <div className="gp-row gp-th" role="row">
        <span role="columnheader">{t('gp.account')}</span><span role="columnheader">{t('gp.played')}</span>
        <span role="columnheader">{t('gp.kicks')}</span><span role="columnheader">{t('gp.drops')}</span><span role="columnheader">{t('gp.now')}</span>
      </div>
      {game.accounts.map(a => <div className="gp-row" role="row" key={a.id}>
        <span role="cell" className="gp-name">{a.name}</span>
        <span role="cell">{a.minutes >= 60 ? t('gp.hours', { n: Math.round(a.minutes / 60) }) : t('gp.minutes', { n: a.minutes })}</span>
        <span role="cell">{a.kicks ? `${a.kicks}${a.kickWhy ? ` · ${a.kickWhy}` : ''}` : '0'}</span>
        <span role="cell">{a.drops}</span>
        <span role="cell" className={`gp-now ${a.now}`}>{a.now === 'ingame' ? t('gp.inGame') : a.now === 'banned' ? t('gp.banned')
          : a.lastPlayed ? ago(a.lastPlayed) : '–'}</span>
      </div>)}
    </div> : <p className="gp-empty">{t('gp.noneYet')}</p>}

    <h4 className="gp-h">{t('gp.quiet')}<small>{t('gp.quietFrom')}</small></h4>
    {hours ? <>
      <div className="gp-hours" role="img" aria-label={quiet ? t('gp.quietSpan', { from: quiet[0], to: quiet[1] }) : t('gp.quiet')}>
        {hours.map((h, i) => <span key={i} title={`${i}h: ${h == null ? '–' : fmtNum(h)}`}
          style={{ opacity: h == null ? .08 : .18 + .82 * (h / (peak || 1)) }} />)}
      </div>
      <div className="gp-axis"><span>0h</span><span>6h</span><span>12h</span><span>18h</span><span>23h</span></div>
      {quiet ? <p className="gp-note">{t('gp.quietSpan', { from: quiet[0], to: quiet[1] })}</p> : null}
    </> : <p className="gp-empty">{t('gp.quietNotYet')}</p>}

    {game.description ? <div className="gp-desc">
      <button className="link" aria-expanded={more} onClick={() => setMore(!more)}>{t(more ? 'gp.lessAbout' : 'gp.about')}</button>
      {more ? <p>{game.description}</p> : null}
    </div> : null}
  </div>;
}

function Stat({ label, value }: { label: string; value: string }) {
  return <div className="gp-stat"><small>{label}</small><b>{value}</b></div>;
}

/** The three hours in a row with the fewest players (wrapping past midnight): [from, to] as hours of the day. */
function quietestSpan(hours: (number | null)[]): [number, number] | null {
  let best = Infinity, at = -1;
  for (let i = 0; i < 24; i++) {
    const span = [0, 1, 2].map(k => hours[(i + k) % 24]);
    if (span.some(h => h == null)) continue;
    const sum = (span as number[]).reduce((a, h) => a + h, 0);
    if (sum < best) { best = sum; at = i; }
  }
  return at < 0 ? null : [at, (at + 3) % 24];
}
