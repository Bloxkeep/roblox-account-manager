// Statistics drawn rather than listed (StatsView.tsx has the figures): today as a 24-hour clock with a ring per account,
// the games as a map of boxes sized by their play time (a click counts only that game), and each account as a card with
// its play line, drops, main game and its run of full days.
import { useLayoutEffect, useRef, useState, type CSSProperties, type ReactNode } from 'react';
import { Icon, useHost, useMetrics } from './host';

export type VizSegment = { from: string; to: string; game: string; name?: string | null; reason?: string | null };
export type VizAccount = { id: string; label?: string; seconds: number; kicks: number; deleted?: boolean;
  topGame?: { key: string; name: string; iconUrl?: string | null } | null; buckets?: number[] | null; streak?: number };
export type VizGame = { key: string; name: string; iconUrl?: string; seconds: number; sessions: number };

/** A game's own hue, the same everywhere it shows (clock, map, cards); kept off red, which says "dropped". */
export function hueOf(key: string): number {
  let h = 0;
  for (let i = 0; i < key.length; i++) h = (h * 31 + key.charCodeAt(i)) >>> 0;
  return 40 + h % 280;
}
const hued = (key: string) => ({ '--h': hueOf(key) } as CSSProperties);

/* ───────────── today as a clock ───────────── */

const C = 200, R_OUT = 158, R_IN = 66, MAX_RINGS = 24;
/** Where on the dial a moment of the day is: midnight at the top, clockwise. */
const point = (r: number, turn: number) => [C + r * Math.sin(turn * 2 * Math.PI), C - r * Math.cos(turn * 2 * Math.PI)];
function arc(r: number, from: number, to: number): string {
  const [x0, y0] = point(r, from), [x1, y1] = point(r, Math.max(to, from + .0015));
  return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${r} ${r} 0 ${to - from > .5 ? 1 : 0} 1 ${x1.toFixed(2)} ${y1.toFixed(2)}`;
}

/** Today: a ring per account (the most played outside), its stays as arcs in their game's colour, a red notch where one
 * dropped, the hand at now. The middle says today's play time and the windows open now; an account pointed at (on the
 * dial or in the list beside it) says its own. */
export function Clock({ since, rows, accounts, icons, totalSeconds, nameOf, avatarOf, fmtSecs }: {
  since: string; rows: { id: string; segments: VizSegment[] }[]; accounts: VizAccount[]; icons?: Record<string, string>; totalSeconds: number;
  nameOf: (a: VizAccount) => string; avatarOf: (a: VizAccount) => ReactNode; fmtSecs: (s: number) => string;
}) {
  const { t, tn, lang, locale } = useHost();
  const m = useMetrics();
  const [active, setActive] = useState('');
  const start = new Date(since).getTime(), day = 86400e3;
  const turn = (ms: number) => Math.min(1, Math.max(0, (ms - start) / day));
  const shown = rows.filter(r => accounts.some(a => a.id === r.id)).slice(0, MAX_RINGS);
  const n = Math.max(1, shown.length), band = Math.min(16, (R_OUT - R_IN) / n), stroke = Math.max(2, band - (n > 12 ? 1 : 2));
  const now = turn(Date.now());
  const hour = (h: number) => new Date(2000, 0, 1, h).toLocaleTimeString(locale, lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric' });
  const clock = (ms: number) => new Date(ms).toLocaleTimeString(locale, lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric', minute: '2-digit' });
  const focus = accounts.find(a => a.id === active);
  // The games played today, in the order they first show, for the key under the list.
  const games = new Map<string, string>();
  for (const r of shown) for (const s of r.segments) if (!games.has(s.game)) games.set(s.game, s.name || '');
  const label = `${t('stats.timeline')}: ${fmtSecs(totalSeconds)}`;
  return <div className="stat-card" id="stats-clock"><h3>{t('stats.timeline')}</h3>
    <div className="clk">
      <svg className="clk-dial" viewBox="0 0 400 400" role="img" aria-label={label} onMouseLeave={() => setActive('')}>
        {Array.from({ length: 24 }, (_, h) => {
          const [x0, y0] = point(R_OUT + 8, h / 24), [x1, y1] = point(R_OUT + (h % 6 ? 13 : 17), h / 24);
          return <line key={h} className={`clk-tick${h % 6 ? '' : ' big'}`} x1={x0} y1={y0} x2={x1} y2={y1} />;
        })}
        {/* The side hours sit further out: they clear their tick by their width. */}
        {[0, 6, 12, 18].map(h => { const [x, y] = point(R_OUT + (h % 12 ? 40 : 30), h / 24); return <text key={h} className="clk-hour" x={x} y={y}>{hour(h)}</text>; })}
        {shown.map((row, k) => {
          const r = R_OUT - band * k - band / 2;
          return <g key={row.id} className={`clk-ring${active && active !== row.id ? ' dim' : ''}`} onMouseEnter={() => setActive(row.id)}>
            <circle className="clk-track" cx={C} cy={C} r={r} strokeWidth={stroke} />
            {row.segments.map((s, i) => {
              const a0 = turn(new Date(s.from).getTime()), a1 = turn(new Date(s.to).getTime());
              const icon = icons?.[s.game];
              const [ix, iy] = point(r, a0 + Math.min(.025, (a1 - a0) / 2));
              const size = Math.min(stroke - 2, 13);
              return <g key={i}>
                <path className="clk-arc" d={arc(r, a0, a1)} strokeWidth={stroke} style={hued(s.game)}>
                  <title>{[s.name, `${clock(new Date(s.from).getTime())} – ${clock(new Date(s.to).getTime())}`, s.reason ? t(`stats.why.${s.reason}`) : ''].filter(Boolean).join(' · ')}</title>
                </path>
                {icon && size >= 9 && a1 - a0 > .05 ? <image href={icon} x={ix - size / 2} y={iy - size / 2} width={size} height={size} className="clk-icon" /> : null}
                {s.reason ? (() => { const [x0, y0] = point(r - stroke / 2 - 2, a1), [x1, y1] = point(r + stroke / 2 + 2, a1);
                  return <line className="clk-drop" x1={x0} y1={y0} x2={x1} y2={y1} />; })() : null}
              </g>;
            })}
          </g>;
        })}
        {/* The hand crosses the rings only, never the words in the middle; its dot sits at the hour ring. */}
        {(() => { const [x0, y0] = point(R_OUT - band * n - 6, now), [x, y] = point(R_OUT + 10, now);
          return <><line className="clk-hand" x1={x0} y1={y0} x2={x} y2={y} /><circle className="clk-pin" cx={x} cy={y} r={3.5} /></>; })()}
        <text className="clk-big" x={C} y={C - 8}>{fmtSecs(focus ? focus.seconds : totalSeconds)}</text>
        <text className="clk-small" x={C} y={C + 16}>{focus ? nameOf(focus) : m.robloxCount ? tn('stats.runningNow', m.robloxCount) : t('alog.today')}</text>
      </svg>
      <div className="clk-side">
        <div className="clk-list" onMouseLeave={() => setActive('')}>{shown.map(row => {
          const a = accounts.find(x => x.id === row.id)!;
          return <div key={row.id} className={`clk-acc${active === row.id ? ' on' : ''}`} onMouseEnter={() => setActive(row.id)}>
            {avatarOf(a)}<b title={nameOf(a)}>{nameOf(a)}</b>
            {a.kicks ? <span className="clk-kicks" title={tn('stats.kicksN', a.kicks)}>{a.kicks}</span> : null}
            <span className="clk-time">{fmtSecs(a.seconds)}</span>
          </div>;
        })}</div>
        {rows.length > shown.length ? <p className="stat-note">{tn('stats.moreRings', rows.length - shown.length)}</p> : null}
        <div className="clk-games">{[...games].map(([key, name]) => <span key={key} style={hued(key)}><i />{name || '…'}</span>)}
          {shown.some(r => r.segments.some(s => s.reason)) ? <span className="drop"><i />{t('stats.kicks')}</span> : null}</div>
      </div>
    </div>
  </div>;
}

/* ───────────── the games as a map ───────────── */

type Box = { x: number; y: number; w: number; h: number };

/** Boxes for the values, as square as they can be (squarified treemap), filling w×h. */
function squarify(values: number[], w: number, h: number): Box[] {
  const total = values.reduce((s, v) => s + v, 0) || 1;
  const areas = values.map(v => v / total * w * h);
  const out: Box[] = new Array(values.length);
  let x = 0, y = 0, W = w, H = h, i = 0;
  const worst = (row: number[], side: number) => {
    const sum = row.reduce((s, a) => s + a, 0), max = Math.max(...row), min = Math.min(...row);
    return Math.max(side * side * max / (sum * sum), sum * sum / (side * side * min));
  };
  while (i < areas.length) {
    const side = Math.min(W, H);
    const row = [areas[i]];
    let j = i + 1;
    while (j < areas.length && worst([...row, areas[j]], side) <= worst(row, side)) row.push(areas[j++]);
    const sum = row.reduce((s, a) => s + a, 0);
    if (W >= H) {
      const cw = sum / H;
      let cy = y;
      row.forEach((a, k) => { out[i + k] = { x, y: cy, w: cw, h: a / cw }; cy += a / cw; });
      x += cw; W -= cw;
    } else {
      const ch = sum / W;
      let cx = x;
      row.forEach((a, k) => { out[i + k] = { x: cx, y, w: a / ch, h: ch }; cx += a / ch; });
      y += ch; H -= ch;
    }
    i = j;
  }
  return out;
}

const MAP_H = 236, MAP_GAP = 4;

/** The games as boxes sized by their play time, in their colour, icon, name and time where they fit. A click counts only
 * that game in every figure on the page (picked: the box again, or the chip, counts all of them again). */
export function GameMap({ games, total, picked, pick, fmtSecs }: {
  games: VizGame[]; total: number; picked: string | null; pick: (key: string | null) => void; fmtSecs: (s: number) => string;
}) {
  const { t, fmtNum } = useHost();
  const host = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(0);
  useLayoutEffect(() => {
    const el = host.current;
    if (!el) return;
    const measure = () => setWidth(el.clientWidth);
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const shown = games.filter(g => g.seconds > 0);
  const boxes = width ? squarify(shown.map(g => g.seconds), width, MAP_H) : [];
  return <div className="stat-card" id="stats-games"><h3>{t('stats.topGames')}
    {picked ? <button type="button" className="chip on gm-chip" onClick={() => pick(null)}>{t('stats.onlyGame', { game: games.find(g => g.key === picked)?.name || '…' })}<Icon name="x" /></button> : null}</h3>
    <div className="gm" ref={host} style={{ height: MAP_H }}>{boxes.map((b, i) => {
      const g = shown[i], share = Math.round(g.seconds / Math.max(1, total) * 100);
      const big = b.w >= 120 && b.h >= 76, mid = b.w >= 64 && b.h >= 40;
      return <button key={g.key} type="button" className={`gm-box${picked === g.key ? ' on' : ''}${big ? ' big' : mid ? '' : ' tiny'}`} style={{ ...hued(g.key),
        left: b.x + MAP_GAP / 2, top: b.y + MAP_GAP / 2, width: Math.max(0, b.w - MAP_GAP), height: Math.max(0, b.h - MAP_GAP) }}
        title={`${g.name} · ${fmtSecs(g.seconds)} · ${fmtNum(share)} %`} aria-pressed={picked === g.key} onClick={() => pick(picked === g.key ? null : g.key)}>
        {g.iconUrl && mid ? <img src={g.iconUrl} alt="" /> : null}
        {mid ? <span className="gm-txt"><b>{g.name}</b><small>{fmtSecs(g.seconds)} · {fmtNum(share)} %</small></span> : null}
      </button>;
    })}</div>
  </div>;
}

/* ───────────── each account as a card ───────────── */

/** An account's card: its play time, a bar per hour (today) or day, its drops, main game and its run of full days.
 * 7 and 30 days: a click opens its own timeline under the cards. */
export function AccountCards({ accounts, hourly, open, setOpen, opens, nameOf, avatarOf, fmtSecs, below }: {
  accounts: VizAccount[]; hourly: boolean; open: string; setOpen: (id: string) => void; opens: (id: string) => boolean;
  nameOf: (a: VizAccount) => string; avatarOf: (a: VizAccount) => ReactNode; fmtSecs: (s: number) => string; below: ReactNode;
}) {
  const { t, tn } = useHost();
  const most = Math.max(1, ...accounts.flatMap(a => a.buckets || []));
  return <div className="stat-card" id="stats-accounts"><h3>{t('stats.byAccount')}</h3>
    <div className="sa-grid">{accounts.map(a => {
      const can = opens(a.id), on = can && open === a.id, label = nameOf(a);
      const body = <>
        <span className="sa-head">{avatarOf(a)}<b title={label}>{label}</b>{a.deleted && label !== t('stats.deleted') ? <small className="muted">{t('stats.deletedTag')}</small> : null}</span>
        <span className="sa-time">{fmtSecs(a.seconds)}</span>
        <span className="sa-bars" aria-hidden="true">{(a.buckets || []).map((v, i) => <i key={i} style={{ height: `${Math.max(v ? 8 : 0, v / most * 100).toFixed(1)}%` }} />)}</span>
        <span className="sa-foot">
          {a.topGame ? <span className="sa-game" title={a.topGame.name} style={hued(a.topGame.key)}><i />{a.topGame.name}</span> : null}
          {a.kicks ? <span className="sa-kicks">{tn('stats.kicksN', a.kicks)}</span> : null}
        </span>
        {(a.streak || 0) >= 2 ? <span className="sa-streak" title={t('stats.streakTip')}><Icon name="calendar" />{tn('stats.streak', a.streak!)}</span> : null}
      </>;
      return can ? <button key={a.id} type="button" className={`sa-card opens${on ? ' on' : ''}`} aria-expanded={on} title={t('stats.showPlayed')}
        onClick={() => setOpen(on ? '' : a.id)}>{body}</button>
        : <div key={a.id} className={`sa-card${hourly ? ' today' : ''}`}>{body}</div>;
    })}</div>
    {below}
  </div>;
}
