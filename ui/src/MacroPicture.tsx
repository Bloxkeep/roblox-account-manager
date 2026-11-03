// The macro form's game, without the game: a picture of the Roblox window (macroPicture), or a plain frame of its shape,
// with the same numbered dots (each step's point) and coloured boxes (its areas) Edit on game draws on the window
// itself. They are dragged here too: a dot to move its point, a box to move it, its corner to size it; from the keyboard
// the arrows move the one with the focus (Shift: further; Alt on a box: its size). Cutting a picture for a step: drag
// around it on the picture (a PNG, 256 × 256 px at most). Under the form's steps, Try it's results as plain lines.
import { useEffect, useRef, useState, type KeyboardEvent, type PointerEvent } from 'react';
import { Icon, useHost } from './host';
import { MAX_CUT, stepText, type Area, type Macro, type Rect, type Step } from './macros';

export type Picture = { image: string; w: number; h: number };
/** What the app sees on the window (macroSeen): its picture, boxes where things were found, a note. Try it's lines use
 * the same shape: step (its index) and outcome (found, notFound, missingValue, done; with no step, how the run ended). */
export type Seen = {
  accountId?: string; image?: string; w: number; h: number; boxes?: (Rect & { label?: string; kind?: string })[];
  step?: number; outcome?: string; what?: string; note?: string;
};
type Point = { x: number; y: number };
type Dot = { i: number; n: number; x: number; y: number };
type Drag = { kind: 'dot' | 'move' | 'size' | 'cut'; key: string | number; from: Point; to: Point; orig?: Rect | Point };

const clamp = (v: number, lo = 0, hi = 1) => Math.max(lo, Math.min(hi, v));
const pct = (v: number) => `${(v * 100).toFixed(2)}%`;
const MIN = 0.02;

export function GameFrame({ picture, size, dots, boxes, found, cutting, cancelCut, onDot, onBox, onCut, goAccounts }: {
  picture: Picture | null; size?: { w: number; h: number }; dots: Dot[]; boxes: Area[]; found: (Rect & { label?: string })[]; cutting: boolean;
  cancelCut: () => void; onDot: (i: number, x: number, y: number) => void; onBox: (name: string, r: Rect) => void;
  /** A drag around something while cutting: the picture cut from it (base64 PNG), and whether it was cut down to the largest size. */
  onCut: (r: Rect, image: string, cutDown: boolean) => void;
  /** With no Roblox window: the blank picture's way to Accounts, where one is opened. */
  goAccounts?: () => void;
}) {
  const { t } = useHost();
  const frame = useRef<HTMLDivElement>(null);
  const img = useRef<HTMLImageElement>(null);
  const canvas = useRef<HTMLCanvasElement | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  useEffect(() => { canvas.current = null; }, [picture?.image]);

  // The picture's own pixels, drawn once into a canvas the first time a part of it is cut.
  const cut = (r: Rect): [string, boolean] => {
    const el = img.current;
    if (!el || !el.naturalWidth) return ['', false];
    if (!canvas.current) {
      const c = document.createElement('canvas');
      c.width = el.naturalWidth;
      c.height = el.naturalHeight;
      c.getContext('2d', { willReadFrequently: true })!.drawImage(el, 0, 0);
      canvas.current = c;
    }
    const c = canvas.current;
    const sx = Math.round(r.x * c.width), sy = Math.round(r.y * c.height);
    const w0 = Math.max(1, Math.round(r.w * c.width)), h0 = Math.max(1, Math.round(r.h * c.height));
    const w = Math.min(MAX_CUT, w0), h = Math.min(MAX_CUT, h0);
    const out = document.createElement('canvas');
    out.width = w;
    out.height = h;
    out.getContext('2d')!.drawImage(c, sx, sy, w, h, 0, 0, w, h);
    return [out.toDataURL('image/png').replace(/^data:image\/png;base64,/, ''), w < w0 || h < h0];
  };

  const at = (e: PointerEvent): Point => {
    const r = frame.current!.getBoundingClientRect();
    return { x: clamp((e.clientX - r.left) / r.width), y: clamp((e.clientY - r.top) / r.height) };
  };
  // Where each dot and box is while one is dragged (the rest as the form has them).
  const moved = (d: Drag): Rect | Point => {
    const dx = d.to.x - d.from.x, dy = d.to.y - d.from.y;
    if (d.kind === 'dot') { const o = d.orig as Point; return { x: clamp(o.x + dx), y: clamp(o.y + dy) }; }
    const o = d.orig as Rect;
    if (d.kind === 'move') return { ...o, x: clamp(o.x + dx, 0, 1 - o.w), y: clamp(o.y + dy, 0, 1 - o.h) };
    return { ...o, w: clamp(o.w + dx, MIN, 1 - o.x), h: clamp(o.h + dy, MIN, 1 - o.y) };
  };
  const begin = (e: PointerEvent, d: Omit<Drag, 'from' | 'to'>) => {
    // Dots and boxes are moved on a picture of the game; the blank frame has nothing to place them against.
    if (e.button !== 0 || !picture) return;
    e.stopPropagation();
    e.preventDefault();
    frame.current!.setPointerCapture(e.pointerId);
    const p = at(e);
    setDrag({ ...d, from: p, to: p });
  };
  const end = (e: PointerEvent) => {
    if (!drag) return;
    const d = { ...drag, to: at(e) };
    setDrag(null);
    if (d.kind === 'cut') {
      const box = { x: Math.min(d.from.x, d.to.x), y: Math.min(d.from.y, d.to.y), w: Math.abs(d.from.x - d.to.x), h: Math.abs(d.from.y - d.to.y) };
      if (box.w < 0.01 || box.h < 0.01) return;
      const [image, cutDown] = cut(box);
      return onCut(box, image, cutDown);
    }
    const r = moved(d);
    if (d.kind === 'dot') onDot(d.key as number, r.x, r.y); else onBox(d.key as string, r as Rect);
  };
  // The keyboard: arrows move what has the focus by 1 % (Shift: 10 %); Alt with the arrows sizes a box.
  const keys = (e: KeyboardEvent, what: { dot?: Dot; box?: Area }) => {
    const step = e.shiftKey ? 0.1 : 0.01;
    const d = ({ ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] } as Record<string, number[]>)[e.key];
    if (!d) return;
    e.preventDefault();
    e.stopPropagation();
    if (what.dot) return onDot(what.dot.i, clamp(what.dot.x + d[0]), clamp(what.dot.y + d[1]));
    const r = what.box!.rect;
    onBox(what.box!.name, e.altKey ? { ...r, w: clamp(r.w + d[0], MIN, 1 - r.x), h: clamp(r.h + d[1], MIN, 1 - r.y) }
      : { ...r, x: clamp(r.x + d[0], 0, 1 - r.w), y: clamp(r.y + d[1], 0, 1 - r.h) });
  };

  const shape = picture ? picture.w / picture.h : size && size.w > 0 && size.h > 0 ? size.w / size.h : 16 / 9;
  const live = drag && drag.kind !== 'cut' ? moved(drag) : null;
  const cutBox = drag?.kind === 'cut' ? { x: Math.min(drag.from.x, drag.to.x), y: Math.min(drag.from.y, drag.to.y), w: Math.abs(drag.from.x - drag.to.x), h: Math.abs(drag.from.y - drag.to.y) } : null;
  // A window of another shape than the one the macro was made on: its points land elsewhere (they are shares of the window).
  const off = picture && size && size.w > 0 && size.h > 0 && Math.abs(shape / (size.w / size.h) - 1) > 0.05;

  return <>
    <div className={`mc-pic-frame${picture ? '' : ' blank'}${cutting ? ' cutting' : ''}`} ref={frame} tabIndex={cutting ? 0 : -1} role="group"
      aria-label={t('macro.pic.label')} style={{ aspectRatio: String(shape) }}
      onPointerDown={e => { if (cutting && picture) begin(e, { kind: 'cut', key: 'cut' }); }}
      onPointerMove={e => { if (drag) setDrag({ ...drag, to: at(e) }); }} onPointerUp={end} onPointerCancel={() => setDrag(null)}
      onKeyDown={e => { if (e.key === 'Escape' && cutting) { e.stopPropagation(); cancelCut(); } }}>
      {picture ? <img ref={img} src={picture.image} alt="" draggable={false} onLoad={() => { canvas.current = null; }} />
        : <span className="mc-pic-blank"><span>{t('macro.pic.blank')}</span>{goAccounts ? <button type="button" className="btn sm" onClick={goAccounts}>{t('macro.goAccounts')}</button> : null}</span>}
      {boxes.map(a => {
        const r = live && drag?.key === a.name ? live as Rect : a.rect;
        return <span key={a.name} className="mc-pic-area" style={{ left: pct(r.x), top: pct(r.y), width: pct(r.w), height: pct(r.h), borderColor: a.colour }}
          tabIndex={picture ? 0 : -1} role="button" aria-label={t('macro.pic.boxTip', { area: a.label })} title={t('macro.pic.boxTip', { area: a.label })}
          onPointerDown={e => begin(e, { kind: 'move', key: a.name, orig: a.rect })} onKeyDown={e => keys(e, { box: a })}>
          <i style={{ background: a.colour }}>{a.label}</i>
          <b className="mc-pic-handle" style={{ background: a.colour }} onPointerDown={e => begin(e, { kind: 'size', key: a.name, orig: a.rect })} />
        </span>;
      })}
      {found.map((f, k) => <span key={`f${k}`} className="mc-pic-found" style={{ left: pct(f.x), top: pct(f.y), width: pct(f.w), height: pct(f.h) }}>{f.label ? <i>{f.label}</i> : null}</span>)}
      {dots.map(d => {
        const p = live && drag?.key === d.i ? live as Point : d;
        return <span key={d.i} className="mc-pic-dot" style={{ left: pct(p.x), top: pct(p.y) }} tabIndex={picture ? 0 : -1} role="button"
          aria-label={t('macro.pic.dotTip', { n: d.n })} title={t('macro.pic.dotTip', { n: d.n })}
          onPointerDown={e => begin(e, { kind: 'dot', key: d.i, orig: { x: d.x, y: d.y } })} onKeyDown={e => keys(e, { dot: d })}>{d.n}</span>;
      })}
      {cutBox ? <span className="mc-pic-drag" style={{ left: pct(cutBox.x), top: pct(cutBox.y), width: pct(cutBox.w), height: pct(cutBox.h) }} /> : null}
    </div>
    {cutting ? <p className="mc-pic-hint" role="status"><span>{t(picture ? 'macro.pic.cutHow' : 'macro.pic.cutNeedsPic')}</span>
      <button type="button" className="link" onClick={cancelCut}>{t('macro.pic.cancel')}</button></p> : null}
    {off ? <p className="mc-warn" role="note"><Icon name="alert" /><span>{t('macro.pic.aspect', { a: `${picture!.w} × ${picture!.h}`, b: `${size!.w} × ${size!.h}` })}</span></p> : null}
  </>;
}

/** Try it's results: a line a step, said plainly ("Found the picture", "Not seen yet in Fish area: get it on screen, then
 * try again"), and how the run ended. */
export function TryLines({ lines, trying, steps, m, name, note }: { lines: Seen[]; trying: boolean; steps: Step[]; m: Macro; name: string; note?: string }) {
  const { t } = useHost();
  const line = (d: Seen) => {
    const s = d.step != null ? steps[d.step] : undefined;
    const say = s ? stepText(t, s, m, d.step! + 1) : '';
    const ok = d.outcome === 'found' || d.outcome === 'done';
    const text = d.step == null ? t(`macro.try.end.${d.outcome === 'done' ? 'done' : 'stopped'}`, { name })
      : d.outcome === 'found' ? t('macro.try.found', { n: d.step + 1, what: say })
      : d.outcome === 'done' ? t('macro.try.done', { n: d.step + 1, what: say })
      : d.outcome === 'missingValue' ? t('macro.try.empty', { n: d.step + 1 })
      : t('macro.try.notSeen', { n: d.step + 1, what: say });
    return { ok, text };
  };
  return <section className="mc-tries" aria-labelledby="mc-tries-h">
    <b id="mc-tries-h">{t('macro.try.title')}</b>
    <ul role="log" aria-live="polite">{lines.map((d, k) => { const l = line(d); return <li key={k} className={l.ok ? 'ok' : 'no'}><Icon name={l.ok ? 'check' : 'alert'} /><span>{l.text}</span></li>; })}
      {trying ? <li className="busy"><Icon name="clock" /><span>{t('macro.try.running', { name })}</span></li> : null}</ul>
    {note ? <small className="mc-tries-note">{note}</small> : null}
  </section>;
}
