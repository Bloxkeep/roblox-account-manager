// The overlay over a game (OverlayWindow.cs draws overlay.html in a window lying over the game's picture area, so every
// click lands here, never on the game) and the colour picker over a monitor. Until 2026-11 it was hand-drawn WinForms
// (GameOverlay). Edit: a macro's points (numbered dots) and areas (labelled boxes with corner handles) over the game
// dimmed by the window's own opacity; a dot is dragged, a box dragged by its inside and sized by its corners. Areas: the
// boxes only. Pick and Screen: the picture frozen, a magnifier at the pointer, a click takes the place (the app reads its
// colour from the same picture). A bar at the top says what it is for; Esc or Enter ends it.
// Places go to the app as parts of the picture area (0–1, as the macro keeps them), when a drag is let go.
import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { useHost } from '../host';

export interface OverlayDot { step: number; x: number; y: number }
export interface OverlayRect { x: number; y: number; w: number; h: number }
export interface OverlayArea { name: string; label: string; colour: string; rect: OverlayRect }
/** The app's "overlay" event. */
export interface OverlayData {
  mode: 'edit' | 'areas' | 'pick' | 'screen';
  /** Edit and Areas: the macro's name (empty for a new one). */
  name: string;
  dots: OverlayDot[];
  areas: OverlayArea[];
  /** Pick and Screen: the picture as it was a moment ago (a PNG data address). */
  picture: string | null;
}

export interface OverlayOn {
  /** dot {step, x, y}, area {name, x, y, w, h}, pick {x, y}, done, cancel. */
  act(what: string, args?: Record<string, unknown>): void;
}
export interface OverlayViewApi { show(data: OverlayData): void }

/** As the WinForms overlay had them, in CSS pixels: a corner's reach, a box's smallest side, a dot's radius. */
const GRIP = 8, MIN = 12, DOT = 11;
/** The magnifier: this many pixels each way around the pointer, each this big. */
const SPAN = 9, ZOOM = 12;

type Grip = 'move' | 'tl' | 'tr' | 'bl' | 'br';
/** What is held, and where it is now (what the app is told when it is let go). */
type Drag = { kind: 'dot'; index: number; at?: { x: number; y: number } }
  | { kind: 'area'; index: number; grip: Grip; start: { l: number; t: number; r: number; b: number }; fromX: number; fromY: number; rect?: OverlayRect };

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const round4 = (v: number) => Math.round(v * 10000) / 10000;
/** Black or white words on a colour, whichever reads (as the WinForms overlay chose them). */
function ink(hex: string): string {
  const n = parseInt(hex.slice(1), 16);
  const r = (n >> 16) & 255, g = (n >> 8) & 255, b = n & 255;
  const max = Math.max(r, g, b), min = Math.min(r, g, b);
  return (max + min) / 510 > 0.55 ? '#000' : '#fff';
}
const hex2 = (n: number) => n.toString(16).padStart(2, '0');

/** The box after dragging a corner or its inside by dx, dy: inside the window, at least MIN each way (AreaGeometry.Drag). */
function dragBox(s: { l: number; t: number; r: number; b: number }, grip: Grip, dx: number, dy: number, W: number, H: number) {
  let { l, t, r, b } = s;
  if (grip === 'move') {
    dx = clamp(dx, -l, W - r);
    dy = clamp(dy, -t, H - b);
    return { l: l + dx, t: t + dy, r: r + dx, b: b + dy };
  }
  if (grip === 'tl' || grip === 'bl') l = clamp(l + dx, 0, r - MIN);
  if (grip === 'tr' || grip === 'br') r = clamp(r + dx, l + MIN, W);
  if (grip === 'tl' || grip === 'tr') t = clamp(t + dy, 0, b - MIN);
  if (grip === 'bl' || grip === 'br') b = clamp(b + dy, t + MIN, H);
  return { l, t, r, b };
}

export function OverlayRoot({ on, handle }: { on: OverlayOn; handle: Ref<OverlayViewApi> }) {
  const [data, setData] = useState<OverlayData | null>(null);
  useImperativeHandle(handle, () => ({ show: d => setData(d) }), []);
  if (!data) return null;
  return <OverlayView data={data} on={on} />;
}

export function OverlayView({ data, on }: { data: OverlayData; on: OverlayOn }) {
  const { t } = useHost();
  const picking = data.mode === 'pick' || data.mode === 'screen';
  // The app's dots and areas, kept here while one is dragged (the app's next draft waits until it is let go).
  const [dots, setDots] = useState(data.dots);
  const [areas, setAreas] = useState(data.areas);
  const drag = useRef<Drag | null>(null);
  const [dragging, setDragging] = useState<Drag | null>(null);
  const [view, setView] = useState({ w: innerWidth, h: innerHeight });
  const [at, setAt] = useState<{ x: number; y: number } | null>(null);
  const [cursor, setCursor] = useState('crosshair');
  const root = useRef<HTMLDivElement>(null);
  useEffect(() => { if (!drag.current) { setDots(data.dots); setAreas(data.areas); } }, [data]);
  useEffect(() => {
    const resize = () => setView({ w: innerWidth, h: innerHeight });
    addEventListener('resize', resize);
    return () => removeEventListener('resize', resize);
  }, []);
  // Esc or Enter ends it (Done; Cancel while picking), wherever the focus is.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' && e.key !== 'Enter') return;
      e.preventDefault();
      on.act(picking ? 'cancel' : 'done');
    };
    addEventListener('keydown', key);
    return () => removeEventListener('keydown', key);
  }, [on, picking]);
  useLayoutEffect(() => { root.current?.focus(); }, []);

  // ── the picture to pick on, read pixel by pixel for the magnifier ──
  const pixels = useRef<CanvasRenderingContext2D | null>(null);
  const lens = useRef<HTMLCanvasElement>(null);
  const [colour, setColour] = useState<string | null>(null);
  useEffect(() => {
    pixels.current = null;
    if (!picking || !data.picture) return;
    const img = new Image();
    img.onload = () => {
      const c = document.createElement('canvas');
      c.width = img.naturalWidth;
      c.height = img.naturalHeight;
      const g = c.getContext('2d', { willReadFrequently: true });
      if (!g) return;
      g.drawImage(img, 0, 0);
      pixels.current = g;
      if (at) paintLens(at.x, at.y);
    };
    img.src = data.picture;
  }, [picking, data.picture]);
  /** The picture's pixel under a point of the window (the picture is stretched over it, as the app reads it). */
  const pixelAt = (x: number, y: number) => {
    const g = pixels.current!;
    return { px: clamp(Math.floor(x / innerWidth * g.canvas.width), 0, g.canvas.width - 1), py: clamp(Math.floor(y / innerHeight * g.canvas.height), 0, g.canvas.height - 1) };
  };
  function paintLens(x: number, y: number) {
    const g = pixels.current, out = lens.current?.getContext('2d');
    if (!g || !out) return;
    const { px, py } = pixelAt(x, y);
    const half = (SPAN - 1) / 2;
    out.imageSmoothingEnabled = false;
    out.fillStyle = '#000';
    out.fillRect(0, 0, SPAN * ZOOM, SPAN * ZOOM);
    out.drawImage(g.canvas, px - half, py - half, SPAN, SPAN, 0, 0, SPAN * ZOOM, SPAN * ZOOM);
    const [r, gg, b] = g.getImageData(px, py, 1, 1).data;
    setColour(`#${hex2(r)}${hex2(gg)}${hex2(b)}`);
  }

  // ── pointer ──
  const W = view.w, H = view.h;
  const fromBar = (e: React.PointerEvent) => !!(e.target as Element).closest('.ov-bar');
  const down = (e: React.PointerEvent) => {
    if (fromBar(e)) return;
    if (picking) {
      if (e.button === 2) on.act('cancel');
      return;
    }
    if (e.button !== 0) return;
    const target = e.target as HTMLElement;
    const dot = target.closest<HTMLElement>('[data-dot]');
    const box = target.closest<HTMLElement>('[data-area]');
    let d: Drag | null = null;
    if (dot) d = { kind: 'dot', index: Number(dot.dataset.dot) };
    else if (box) {
      const index = Number(box.dataset.area), r = areas[index].rect;
      const grip = (target.dataset.grip as Grip | undefined) ?? 'move';
      d = { kind: 'area', index, grip, start: { l: r.x * W, t: r.y * H, r: (r.x + r.w) * W, b: (r.y + r.h) * H }, fromX: e.clientX, fromY: e.clientY };
    }
    if (!d) return;
    e.preventDefault();
    try { root.current?.setPointerCapture(e.pointerId); } catch { /* a pointer Windows no longer has */ }
    drag.current = d;
    setDragging(d);
  };
  const move = (e: React.PointerEvent) => {
    if (picking) {
      setAt({ x: e.clientX, y: e.clientY });
      paintLens(e.clientX, e.clientY);
      return;
    }
    const d = drag.current;
    if (!d) {
      const target = e.target as HTMLElement;
      setCursor(fromBar(e) ? 'default' : target.closest('[data-dot]') ? 'move'
        : target.dataset.grip === 'tl' || target.dataset.grip === 'br' ? 'nwse-resize'
        : target.dataset.grip === 'tr' || target.dataset.grip === 'bl' ? 'nesw-resize'
        : target.closest('[data-area]') ? 'move' : 'crosshair');
      return;
    }
    if (d.kind === 'dot') {
      const x = round4(clamp(e.clientX, 0, W - 1) / W), y = round4(clamp(e.clientY, 0, H - 1) / H);
      d.at = { x, y };
      setDots(list => list.map((p, i) => i === d.index ? { ...p, x, y } : p));
    } else {
      const b = dragBox(d.start, d.grip, e.clientX - d.fromX, e.clientY - d.fromY, W, H);
      const rect = { x: round4(b.l / W), y: round4(b.t / H), w: round4((b.r - b.l) / W), h: round4((b.b - b.t) / H) };
      d.rect = rect;
      setAreas(list => list.map((a, i) => i === d.index ? { ...a, rect } : a));
    }
  };
  const up = (e: React.PointerEvent) => {
    if (picking) {
      if (e.button === 0 && !fromBar(e)) on.act('pick', { x: round4(clamp(e.clientX / W, 0, 1)), y: round4(clamp(e.clientY / H, 0, 1)) });
      return;
    }
    const d = drag.current;
    if (!d) return;
    drag.current = null;
    setDragging(null);
    try { root.current?.releasePointerCapture(e.pointerId); } catch { /* already let go */ }
    if (d.kind === 'dot') {
      const p = dots[d.index];
      if (p && d.at) on.act('dot', { step: p.step, ...d.at });
    } else {
      const a = areas[d.index];
      if (a && d.rect) on.act('area', { name: a.name, ...d.rect });
    }
  };

  const title = data.mode === 'edit' ? t('ov.editing', { name: data.name || t('ov.newMacro') })
    : data.mode === 'areas' ? t('ov.areas', { name: data.name })
    : data.mode === 'pick' ? t('ov.pick') : t('ov.screen');
  const pct = (v: number) => `${v * 100}%`;
  // The magnifier beside the pointer, on the other side when it would run off the window.
  const lensW = SPAN * ZOOM, lensH = lensW + 28;
  const lensAt = at && { left: at.x + 24 + lensW > W ? at.x - 24 - lensW : at.x + 24, top: at.y + 24 + lensH > H ? at.y - 24 - lensH : at.y + 24 };

  return <div ref={root} className={`ov ov-${data.mode}`} tabIndex={-1} style={{ cursor: picking ? 'crosshair' : dragging ? (cursor === 'crosshair' ? 'move' : cursor) : cursor }}
    onPointerDown={down} onPointerMove={move} onPointerUp={up} onContextMenu={e => e.preventDefault()}>
    {picking && data.picture ? <img className="ov-picture" src={data.picture} alt="" draggable={false} /> : null}
    {areas.map((a, i) => {
      const r = a.rect;
      const held = dragging?.kind === 'area' && dragging.index === i;
      return <div key={a.name} className={`ov-area ${held ? 'held' : ''}`} data-area={i}
        style={{ left: pct(r.x), top: pct(r.y), width: pct(r.w), height: pct(r.h), ['--c' as string]: a.colour }}>
        <span className="ov-tag" style={{ color: ink(a.colour) }} title={a.label}>{a.label}</span>
        {(['tl', 'tr', 'bl', 'br'] as const).map(g => <i key={g} className={`ov-grip ${g}`} data-grip={g} style={{ width: GRIP * 2, height: GRIP * 2, margin: -GRIP }} />)}
      </div>;
    })}
    {dots.map((d, i) => <div key={`${d.step}`} className={`ov-dot ${dragging?.kind === 'dot' && dragging.index === i ? 'held' : ''}`} data-dot={i}
      style={{ left: pct(d.x), top: pct(d.y), width: DOT * 2, height: DOT * 2 }}>{d.step + 1}</div>)}
    <div className="ov-bar" role="toolbar" aria-label={title}>
      <b className="ov-title">{title}</b>
      {picking ? null : <span className="ov-hint">{t(data.mode === 'edit' ? 'ov.dragHint' : 'ov.areasHint')}</span>}
      {picking
        ? <button type="button" className="btn sm ghost" onClick={() => on.act('cancel')}>{t('ov.cancel')}</button>
        : <button type="button" className="btn sm primary" onClick={() => on.act('done')}>{t('ov.done')}</button>}
    </div>
    {picking ? <div className="ov-lens" hidden={!lensAt || !colour} style={lensAt ? { left: lensAt.left, top: lensAt.top } : undefined}>
      <canvas ref={lens} width={lensW} height={lensW} />
      <i className="ov-lens-mark" style={{ left: (SPAN - 1) / 2 * ZOOM, top: (SPAN - 1) / 2 * ZOOM, width: ZOOM, height: ZOOM }} />
      <span className="ov-lens-hex" style={colour ? { background: colour, color: ink(colour) } : undefined}>{colour}</span>
    </div> : null}
  </div>;
}

