// Today on an account's card: a thin strip from midnight to midnight, filled where the account played (green), a red
// tick where a stay ended cut off, and a mark at now. At a glance: who idled all night, who dropped at 3 am. The figures
// come from the app's "todayStrips" (every stay since local midnight), asked for once a minute while a card shows.
import { useEffect, useState } from 'react';
import { playedText } from './clock';
import { useHost } from './host';

/** Seconds from midnight: [from, to, 1 when it ended cut off]. */
type Stay = [number, number, number];
type Strips = { since: string; now: string; rows: Record<string, Stay[]> };

const DAY = 86400;
let last: Strips | null = null, users = 0, timer = 0, back: (() => void) | null = null;
const subscribers = new Set<() => void>();

/** Today's stays of every account; one request a minute however many cards show. */
function useStrips(): Strips | null {
  const { call } = useHost();
  const [, redraw] = useState(0);
  useEffect(() => {
    const changed = () => redraw(n => n + 1);
    const ask = () => {
      void (call('todayStrips', {}, { silent: true }) as Promise<Strips | undefined>).then(d => {
        if (!d) return;
        last = d;
        subscribers.forEach(f => f());
      });
    };
    // Only while the cards can be seen: the Accounts tab is hidden, not taken down, when another tab shows.
    const tick = () => { if (!document.hidden && !document.getElementById('view-accounts')?.hidden) ask(); };
    subscribers.add(changed);
    if (users++ === 0) {
      ask();
      timer = window.setInterval(tick, 60_000);
      document.addEventListener('visibilitychange', back = tick);
    }
    return () => {
      subscribers.delete(changed);
      if (--users === 0) {
        clearInterval(timer);
        if (back) document.removeEventListener('visibilitychange', back);
      }
    };
  }, [call]);
  return last;
}

export function DayStrip({ id }: { id: string }) {
  const host = useHost(), { t, tn } = host;
  const strips = useStrips();
  if (!strips) return null;
  const stays = strips.rows[id] || [];
  const now = Math.min(DAY, Math.max(0, (Date.parse(strips.now) - Date.parse(strips.since)) / 1000));
  const played = stays.reduce((sum, [from, to]) => sum + Math.max(0, to - from), 0);
  const drops = stays.filter(s => s[2]).length;
  const tip = played ? [t('card.dayPlayed', { t: playedText(host, played) }), drops ? tn('card.dayDrops', drops) : ''].filter(Boolean).join(' · ') : t('card.dayNone');
  const pct = (s: number) => `${(s / DAY * 100).toFixed(2)}%`;
  return <div className="daystrip" title={tip} aria-label={tip} role="img">
    {stays.map(([from, to, cut], i) => <i key={i} className="ds-play" style={{ left: pct(from), width: `max(2px, ${pct(to - from)})` }}>
      {cut ? <b className="ds-cut" /> : null}</i>)}
    <span className="ds-now" style={{ left: pct(now) }} />
  </div>;
}
