// How Arrange windows lays the windows out, picked from small screen pictures like Windows' snap layouts: "fill", grids
// whose cells are the windows' size whatever their number, and a size of one's own. Pointing at one lights the cells the
// open windows would take. In the dock's Windows menu (picking one arranges at once) and Settings › Launching.
import { useState } from 'react';
import { useHost, useMetrics } from './host';

export const LAYOUTS = ['fill', '2x1', '3x1', '2x2', '3x2', '3x3', '4x3', '4x4'] as const;

/** Sizes of one's own to start from, widescreen and 4:3, large to tiny (a small window is less work for the graphics card). */
export const SIZES: Record<string, [number, number][]> = {
  '16:9': [[1920, 1080], [1600, 900], [1280, 720], [1024, 576], [960, 540], [854, 480], [640, 360], [480, 270], [320, 180]],
  '4:3': [[1024, 768], [800, 600], [640, 480], [480, 360], [400, 300], [320, 240]],
};

/** A grid's columns and rows ("3x2"); "fill" as many as the windows need (one at least); null for the others. */
function gridOf(layout: string, count: number): [number, number] | null {
  if (layout === 'fill') {
    const cols = Math.max(1, Math.ceil(Math.sqrt(count)));
    return [cols, Math.max(1, Math.ceil(count / cols))];
  }
  const m = /^([1-8])x([1-8])$/.exec(layout);
  return m ? [+m[1], +m[2]] : null;
}

export function SnapPicker({ value, width, height, onPick, label }: {
  value: string; width: number; height: number; onPick: (layout: string) => void; label?: string;
}) {
  const { t } = useHost();
  const count = useMetrics().robloxCount || 0;
  const [lit, setLit] = useState<string | null>(null);
  // A size of one's own reads "Custom" (its numbers alone said nothing, 2026-10-26); the size is in its tooltip.
  const name = (l: string) => l === 'fill' ? t('snap.fill') : l === 'custom' ? t('snap.customName') : l.replace('x', ' × ');
  const tip = (l: string) => l === 'custom' ? `${t('snap.customName')} · ${t('snap.custom', { w: width, h: height })}` : name(l);
  const tile = (l: string) => {
    const grid = gridOf(l, count);
    const cells = grid ? grid[0] * grid[1] : 1;
    const on = lit === l ? Math.min(count, cells) : 0;
    return <button key={l} type="button" role="menuitemradio" aria-checked={value === l} data-layout={l}
      className={`snap-opt${value === l ? ' current' : ''}`} title={tip(l)}
      onMouseEnter={() => setLit(l)} onMouseLeave={() => setLit(null)} onFocus={() => setLit(l)} onBlur={() => setLit(null)}
      onClick={() => onPick(l)}>
      <span className="snap-screen">
        {grid
          ? <span className="snap-grid" style={{ gridTemplateColumns: `repeat(${grid[0]}, 1fr)`, gridTemplateRows: `repeat(${grid[1]}, 1fr)` }}>
            {Array.from({ length: cells }, (_, i) => <span key={i} className={i < on ? 'on' : ''} />)}
          </span>
          // A size of one's own: one window of that size in the corner (as wide a share of a 1920 × 1080 screen).
          : <span className={`snap-own${lit === l && count ? ' on' : ''}`}
            style={{ width: `${Math.min(100, width / 19.2)}%`, height: `${Math.min(100, height / 10.8)}%` }} />}
      </span>
      <span className="snap-name">{name(l)}</span>
    </button>;
  };
  return <div className="snap" role="group" aria-label={label || t('m.arrange')}>
    {label ? <div className="snap-label">{label}</div> : null}
    <div className="snap-opts">{[...LAYOUTS, 'custom'].map(tile)}</div>
  </div>;
}
