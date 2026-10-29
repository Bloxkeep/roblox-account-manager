// A colour box with Bloxkeep's own picker (Settings › Appearance › Custom). The browser's colour box opens a window of
// its own in its own colours; this one opens a panel in the page's: a few ready colours, a square for how light and
// how strong, a strip for the hue, and the #rrggbb to type. Each change is told as it is made (onChange), as the
// browser's box tells it while its colour is dragged.
import { useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import { useHost } from './host';
import { Popup } from './TimeField';

const READY = ['#141a26', '#0b0d12', '#1b2a3a', '#2a1b3d', '#3a1d2b', '#123524', '#f1e9da', '#ffffff',
  '#22c55e', '#3b82f6', '#a855f7', '#ec4899', '#ef4444', '#f59e0b', '#14b8a6', '#eab308'];

const hexToRgb = (h: string) => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
const rgbToHex = (c: number[]) => '#' + c.map(v => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0')).join('');

function toHsv([r, g, b]: number[]) {
  r /= 255; g /= 255; b /= 255;
  const max = Math.max(r, g, b), d = max - Math.min(r, g, b);
  const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
  return { h: h * 60, s: max === 0 ? 0 : d / max, v: max };
}

function fromHsv({ h, s, v }: { h: number; s: number; v: number }) {
  const f = (n: number) => {
    const k = (n + h / 60) % 6;
    return (v - v * s * Math.max(0, Math.min(k, 4 - k, 1))) * 255;
  };
  return rgbToHex([f(5), f(3), f(1)]);
}

export function ColorInput({ id, value, label, onChange }: { id: string; value: string; label: string; onChange: (hex: string) => void }) {
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  return <>
    <button type="button" className="ct-color" id={id} ref={button} aria-label={label} title={value} aria-haspopup="dialog" aria-expanded={open}
      data-value={value} onClick={() => setOpen(o => !o)}><span style={{ background: value }} /></button>
    {open ? <Popup anchor={button.current!} close={close} label={label}><Picker value={value} onChange={onChange} /></Popup> : null}
  </>;
}

function Picker({ value, onChange }: { value: string; onChange: (hex: string) => void }) {
  const { t } = useHost();
  // The hue is kept here: grey has none of its own, and dragging through it mustn't lose the one chosen.
  const [hsv, setHsv] = useState(() => toHsv(hexToRgb(value)));
  const [typed, setTyped] = useState(value);
  const set = (next: { h: number; s: number; v: number }) => {
    setHsv(next);
    const hex = fromHsv(next);
    setTyped(hex);
    onChange(hex);
  };
  const pick = (hex: string) => {
    setHsv(toHsv(hexToRgb(hex)));
    setTyped(hex);
    onChange(hex);
  };
  const area = useRef<HTMLDivElement>(null);
  const drag = (e: ReactPointerEvent<HTMLDivElement>) => {
    const r = area.current!.getBoundingClientRect();
    set({ ...hsv, s: Math.max(0, Math.min(1, (e.clientX - r.left) / r.width)), v: Math.max(0, Math.min(1, 1 - (e.clientY - r.top) / r.height)) });
  };
  return <div className="cf">
    <div className="cf-ready">
      {READY.map(c => <button type="button" key={c} className={c === value ? 'on' : ''} style={{ background: c }} aria-label={c} title={c} onClick={() => pick(c)} />)}
    </div>
    <div className="cf-area" ref={area} style={{ backgroundColor: `hsl(${hsv.h} 100% 50%)` }}
      onPointerDown={e => { e.currentTarget.setPointerCapture(e.pointerId); drag(e); }}
      onPointerMove={e => { if (e.currentTarget.hasPointerCapture(e.pointerId)) drag(e); }}>
      <span className="cf-dot" style={{ left: `${hsv.s * 100}%`, top: `${(1 - hsv.v) * 100}%`, background: value }} />
    </div>
    <input type="range" className="cf-hue" min={0} max={359} value={Math.round(hsv.h)} aria-label={t('cf.hue')}
      onChange={e => set({ ...hsv, h: +e.target.value })} />
    <input className="field sm cf-hex" value={typed} maxLength={7} spellCheck={false} aria-label={t('cf.hex')}
      onChange={e => {
        const v = e.target.value.trim();
        setTyped(v);
        const hex = (v.startsWith('#') ? v : '#' + v).toLowerCase();
        if (/^#[0-9a-f]{6}$/.test(hex)) pick(hex);
      }} />
  </div>;
}
