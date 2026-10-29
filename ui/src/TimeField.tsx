// Time and date boxes with Bloxkeep's own picker. The box is still the browser's (typing a time or a date works as it
// always did, and its value is "HH:mm" or "yyyy-mm-dd"), but its own drop-down is hidden: that one is drawn by the
// browser in its own colours, and a second click on its clock didn't close it. Here the button opens and closes a
// list in the page's colours; a pick goes into the box as typing would (its input and change events), so whatever
// listens to the box (a form, TextBox's save on change) hears it the same way.
import { useEffect, useLayoutEffect, useRef, useState, type InputHTMLAttributes, type ReactNode, type Ref } from 'react';
import { createPortal } from 'react-dom';
import { Icon, useHost } from './host';

type BoxProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> & { ref?: Ref<HTMLInputElement> };

export function TimeInput(props: BoxProps) {
  return <Field kind="time" {...props} />;
}

export function DateInput(props: BoxProps) {
  return <Field kind="date" {...props} />;
}

/** A value put into the box as typing would, so React's onChange and plain change listeners both hear it. */
function putValue(box: HTMLInputElement, value: string) {
  Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(box, value);
  box.dispatchEvent(new Event('input', { bubbles: true }));
  box.dispatchEvent(new Event('change', { bubbles: true }));
}

function Field({ kind, className, ref, ...attrs }: BoxProps & { kind: 'time' | 'date' }) {
  const { t } = useHost();
  const box = useRef<HTMLInputElement | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const label = t(kind === 'time' ? 'tf.pickTime' : 'tf.pickDate');
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  return <span className={`tf ${className || ''}`}>
    <input {...attrs} type={kind} className="tf-box" ref={el => {
      box.current = el;
      if (typeof ref === 'function') ref(el); else if (ref) ref.current = el;
    }} onKeyDown={e => {
      // The browser's own keys for its drop-down (Alt+Down, F4) open this one instead.
      if ((e.altKey && e.key === 'ArrowDown') || e.key === 'F4') {
        e.preventDefault();
        setOpen(true);
      }
      attrs.onKeyDown?.(e);
    }} />
    <button type="button" className="tf-btn" ref={button} aria-label={label} title={label} aria-haspopup="dialog" aria-expanded={open}
      disabled={attrs.disabled} onClick={() => setOpen(o => !o)}><Icon name={kind === 'time' ? 'clock' : 'calendar'} /></button>
    {open && box.current ? <Popup anchor={button.current!} close={close} label={label}>
      {kind === 'time'
        ? <TimeList value={box.current.value} pick={v => putValue(box.current!, v)} />
        : <Calendar value={box.current.value} pick={v => { putValue(box.current!, v); close(true); }} />}
    </Popup> : null}
  </span>;
}

/**
 * The list under (or over, near the bottom) its button, in the dialog it belongs to so the dialog's keys and focus
 * still hold. Closes on the button again, a click outside, Escape (before the dialog hears it), Enter, or the window
 * changing size.
 */
function Popup({ anchor, close, label, children }: { anchor: HTMLElement; close: (refocus: boolean) => void; label: string; children: ReactNode }) {
  const panel = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number } | null>(null);
  useLayoutEffect(() => {
    const a = anchor.getBoundingClientRect(), p = panel.current!.getBoundingClientRect();
    const below = a.bottom + 6, above = a.top - 6 - p.height;
    const top = below + p.height <= innerHeight - 8 || above < 8 ? Math.min(below, innerHeight - 8 - p.height) : above;
    setPlace({ left: Math.max(8, Math.min(a.right - p.width, innerWidth - 8 - p.width)), top: Math.max(8, top) });
  }, [anchor]);
  useEffect(() => {
    // Window capture runs before the dialog's own key handler (document, capture): Escape closes this, not the dialog.
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' && !(e.key === 'Enter' && panel.current?.contains(document.activeElement))) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      close(true);
    };
    // The button's own click toggles it; any other press outside closes it.
    const press = (e: MouseEvent) => {
      const at = e.target as Node;
      if (!panel.current?.contains(at) && !anchor.contains(at)) close(false);
    };
    const resized = () => close(false);
    addEventListener('keydown', key, true);
    addEventListener('mousedown', press, true);
    addEventListener('resize', resized);
    return () => {
      removeEventListener('keydown', key, true);
      removeEventListener('mousedown', press, true);
      removeEventListener('resize', resized);
    };
  }, [anchor, close]);
  // The open dialog it was opened from, else the page.
  const host = anchor.closest('.overlay') || document.body;
  return createPortal(<div className="tf-pop" ref={panel} role="dialog" aria-label={label}
    style={place ? { left: place.left, top: place.top } : { visibility: 'hidden', left: 0, top: 0 }}>{children}</div>, host);
}

/** Whether times are shown with AM/PM here, as the box itself shows them (the browser's own setting). */
const twelveHour = () => !!new Intl.DateTimeFormat(undefined, { hour: 'numeric' }).resolvedOptions().hour12;
const pad = (n: number) => String(n).padStart(2, '0');

/** Hours, minutes (and AM/PM): a column each, the chosen ones marked; a click changes that part at once. */
function TimeList({ value, pick }: { value: string; pick: (v: string) => void }) {
  const [h0, m0] = /^\d\d:\d\d/.test(value) ? value.split(':').map(Number) : [new Date().getHours(), 0];
  const [h, setH] = useState(h0), [m, setM] = useState(m0);
  const twelve = twelveHour();
  const set = (hour: number, minute: number) => {
    setH(hour);
    setM(minute);
    pick(`${pad(hour)}:${pad(minute)}`);
  };
  const hours = twelve ? [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] : [...Array(24).keys()];
  const pm = h >= 12;
  return <div className="tf-time">
    <Column label="h" items={hours.map(x => ({ key: x, text: twelve ? String(x) : pad(x), on: twelve ? h % 12 === x % 12 : h === x }))}
      pick={x => set(twelve ? (x % 12) + (pm ? 12 : 0) : x, m)} />
    <Column label="m" items={[...Array(60).keys()].map(x => ({ key: x, text: pad(x), on: m === x }))} pick={x => set(h, x)} />
    {twelve ? <Column label="ampm" items={[{ key: 0, text: 'AM', on: !pm }, { key: 1, text: 'PM', on: pm }]}
      pick={x => set((h % 12) + (x ? 12 : 0), m)} /> : null}
  </div>;
}

function Column({ label, items, pick }: { label: string; items: { key: number; text: string; on: boolean }[]; pick: (key: number) => void }) {
  const list = useRef<HTMLDivElement>(null);
  // The chosen one in sight, near the top, when the list opens.
  useLayoutEffect(() => {
    const on = list.current!.querySelector<HTMLElement>('.on');
    if (on) list.current!.scrollTop = on.offsetTop - list.current!.offsetTop - 4;
  }, []);
  return <div className="tf-col" ref={list} data-col={label}>
    {items.map(i => <button type="button" key={i.key} className={i.on ? 'on' : ''} aria-pressed={i.on} onClick={() => pick(i.key)}>{i.text}</button>)}
  </div>;
}

/** A month at a time, the week starting as the interface's language has it; today ringed, the chosen day filled. */
function Calendar({ value, pick }: { value: string; pick: (v: string) => void }) {
  const { t, lang, locale, weekdayName } = useHost();
  const chosen = /^\d{4}-\d\d-\d\d$/.test(value) ? value : '';
  const start = chosen ? new Date(chosen + 'T00:00') : new Date();
  const [month, setMonth] = useState(new Date(start.getFullYear(), start.getMonth(), 1));
  const first = lang === 'vi' ? 1 : 0; // Monday in Vietnamese, Sunday in English
  const lead = (month.getDay() - first + 7) % 7;
  const days = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
  const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  const today = iso(new Date());
  const shift = (n: number) => setMonth(new Date(month.getFullYear(), month.getMonth() + n, 1));
  return <div className="tf-cal">
    <div className="tf-cal-head">
      <button type="button" className="icon-btn" aria-label={t('tf.prevMonth')} title={t('tf.prevMonth')} onClick={() => shift(-1)}><Icon name="chevronLeft" /></button>
      <b>{month.toLocaleDateString(locale, { month: 'long', year: 'numeric' })}</b>
      <button type="button" className="icon-btn" aria-label={t('tf.nextMonth')} title={t('tf.nextMonth')} onClick={() => shift(1)}><Icon name="chevronRight" /></button>
    </div>
    <div className="tf-cal-grid">
      {[...Array(7).keys()].map(i => <span key={'w' + i} className="tf-wd">{weekdayName((i + first) % 7)}</span>)}
      {[...Array(lead).keys()].map(i => <span key={'e' + i} />)}
      {[...Array(days).keys()].map(i => {
        const day = iso(new Date(month.getFullYear(), month.getMonth(), i + 1));
        return <button type="button" key={day} data-day={day} className={`${day === chosen ? 'on' : ''}${day === today ? ' today' : ''}`}
          aria-pressed={day === chosen} onClick={() => pick(day)}>{i + 1}</button>;
      })}
    </div>
  </div>;
}
