// Time and date boxes with Bloxkeep's own picker. The box is a plain text field the app formats ("20:00" or "8:00 PM",
// "16/10/2026" in Vietnamese): typing parses ("8pm", "2000", "16-10-26" too) and a bad text is said under the box. What
// the owner of the box hears is still "HH:mm" or "yyyy-mm-dd", as the browser's own time and date boxes gave it: a
// pick or a finished text goes in as typing would (input and change events), so React's onChange and plain change
// listeners both hear it the same way, and the text the box shows is put back after they have.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type InputHTMLAttributes, type ReactNode, type Ref } from 'react';
import { createPortal } from 'react-dom';
import { Icon, useHost } from './host';

type BoxProps = Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> & { ref?: Ref<HTMLInputElement>; pastDisabled?: boolean };

export function TimeInput(props: BoxProps) {
  return <Field kind="time" {...props} />;
}

export function DateInput(props: BoxProps) {
  return <Field kind="date" {...props} />;
}

type Kind = 'time' | 'date';
const pad = (n: number) => String(n).padStart(2, '0');
/** A local day as the app writes dates: 2026-11-04. */
export const iso = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

/** Whether times are shown with AM/PM here, as the system's own setting has it (English only: Vietnamese is 24-hour). */
const twelveHour = () => !!new Intl.DateTimeFormat(undefined, { hour: 'numeric' }).resolvedOptions().hour12;

/** "20:00", "8:00 PM", "8pm", "2000" → "20:00"; anything else null. */
export function parseTime(text: string): string | null {
  const m = /^(\d{1,2})(?:\s*[:.h]?\s*(\d{2}))?\s*([ap])?\.?\s*(?:m\.?)?$/i.exec(text.trim());
  if (!m) return null;
  let h = +m[1];
  const min = m[2] ? +m[2] : 0;
  if (m[3]) {
    if (h < 1 || h > 12) return null;
    h = (h % 12) + (m[3].toLowerCase() === 'p' ? 12 : 0);
  }
  return h <= 23 && min <= 59 ? `${pad(h)}:${pad(min)}` : null;
}

/** "20:00" as the box writes it. */
function showTime(v: string, twelve: boolean) {
  if (!/^\d\d:\d\d$/.test(v)) return v;
  const [h, m] = v.split(':').map(Number);
  return twelve ? `${pad(h % 12 || 12)}:${pad(m)} ${h < 12 ? 'AM' : 'PM'}` : v;
}

/** The order the language writes day, month and year in. */
function dateOrder(locale: string) {
  return new Intl.DateTimeFormat(locale, { day: '2-digit', month: '2-digit', year: 'numeric' }).formatToParts(new Date(2000, 10, 22))
    .map(p => p.type).filter(x => x === 'day' || x === 'month' || x === 'year');
}

/** "16/10/2026" or "16-10-26" (in the language's order), or "2026-10-16" → "2026-10-16"; anything else, or no such day, null. */
export function parseDate(text: string, locale: string): string | null {
  const t = text.trim();
  const nums = t.match(/\d+/g);
  if (!nums || nums.length !== 3 || /[^\d\s./-]/.test(t)) return null;
  const order = /^\d{4}\D/.test(t) ? ['year', 'month', 'day'] : dateOrder(locale);
  const at = (k: string) => +nums[order.indexOf(k)];
  const y = at('year') < 100 ? 2000 + at('year') : at('year');
  const d = new Date(y, at('month') - 1, at('day'));
  return y >= 1000 && y <= 9999 && d.getFullYear() === y && d.getMonth() === at('month') - 1 && d.getDate() === at('day') ? iso(d) : null;
}

/** "2026-10-16" as the box writes it: "16/10/2026" in Vietnamese. */
function showDate(v: string, locale: string) {
  if (!/^\d{4}-\d\d-\d\d$/.test(v)) return v;
  const [y, m, d] = v.split('-').map(Number);
  return new Intl.DateTimeFormat(locale, { day: '2-digit', month: '2-digit', year: 'numeric' }).format(new Date(y, m - 1, d));
}

/** A value put into the box without React's value tracking noticing, so its onChange hears it as typing. */
const setNative = (box: HTMLInputElement, value: string) => Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, 'value')!.set!.call(box, value);

// What a box holds right now, read from its text (its owner hears a text only once it is finished): for a form saved
// while a box still has the focus (Enter).
const readers = new WeakMap<HTMLInputElement, () => { value: string; bad: boolean; text: string }>();
export function readBox(box: HTMLInputElement | null | undefined) {
  return box ? readers.get(box)?.() ?? null : null;
}

/** What a box of that kind wants written ("20:00", today's date as the language writes it), for the words under a bad text. */
export const exampleOf = (kind: Kind, locale: string) => kind === 'time' ? '20:00' : showDate(iso(new Date()), locale);

function Field({ kind, className, ref, value: given = '', onChange, onKeyDown, disabled, pastDisabled, ...attrs }: BoxProps & { kind: Kind }) {
  const { t, lang, locale } = useHost();
  const value = String(given);
  const twelve = kind === 'time' && lang === 'en' && twelveHour();
  const box = useRef<HTMLInputElement | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [bad, setBad] = useState(false);
  const byKeys = useRef(false);
  const emitting = useRef(false);
  const label = t(kind === 'time' ? 'tf.pickTime' : 'tf.pickDate');
  const parse = (text: string) => kind === 'time' ? parseTime(text) : parseDate(text, locale);
  const show = (v: string) => kind === 'time' ? showTime(v, twelve) : showDate(v, locale);
  const latest = useRef({ parse, show, value });
  latest.current = { parse, show, value };
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) button.current?.focus();
  };
  const mark = (el: HTMLInputElement, isBad: boolean) => {
    if (isBad) el.dataset.bad = '1'; else delete el.dataset.bad;
  };

  /** What the owner hears: the box's own events with the canonical value in it, the text it shows put back afterwards. */
  const emit = (v: string) => {
    const el = box.current!;
    const shown = el.value;
    emitting.current = true;
    try {
      setNative(el, v);
      el.dispatchEvent(new Event('input', { bubbles: true }));
      el.dispatchEvent(new Event('change', { bubbles: true }));
    } finally {
      emitting.current = false;
    }
    el.value = shown;
  };
  /** A text finished (the box left, Enter): parsed, shown the way the app writes it, heard by the owner. */
  const finish = () => {
    const el = box.current!;
    const text = el.value.trim();
    const v = text ? latest.current.parse(text) : '';
    mark(el, v === null);
    setBad(v === null);
    if (v !== null) el.value = v ? latest.current.show(v) : '';
    if ((v ?? '') !== latest.current.value) emit(v ?? '');
  };
  const pick = (v: string) => {
    const el = box.current!;
    mark(el, false);
    setBad(false);
    emit(v);
    el.value = show(v);
  };
  const finishRef = useRef(finish);
  finishRef.current = finish;

  // One function for the box's whole life: a new one each time would be called again at every draw, and put the text back.
  const outer = useRef(ref);
  outer.current = ref;
  const mount = useCallback((el: HTMLInputElement | null) => {
    if (box.current && box.current !== el) readers.delete(box.current);
    box.current = el;
    const to = outer.current;
    if (typeof to === 'function') to(el); else if (to) to.current = el;
    if (!el) return;
    el.value = latest.current.show(latest.current.value);
    readers.set(el, () => {
      const text = el.value.trim();
      const v = text ? latest.current.parse(text) : '';
      return { value: v ?? '', bad: v === null, text };
    });
  }, []);
  useLayoutEffect(() => {
    // The browser's own change (the box left, Enter) is this field's business: the owner hears the finished value instead.
    const el = box.current!;
    const own = (e: Event) => {
      if (emitting.current) return;
      e.stopImmediatePropagation();
      finishRef.current();
    };
    el.addEventListener('change', own);
    return () => el.removeEventListener('change', own);
  }, []);
  // The owner changed it (the app answered): the box shows it; a text still being typed stays. Not while the owner is
  // being told something (its own change handler draws again before the change event follows the input one).
  useEffect(() => {
    const el = box.current!;
    if (emitting.current) return;
    const now = el.value.trim() ? latest.current.parse(el.value) ?? '' : '';
    if (now !== value) {
      el.value = show(value);
      mark(el, false);
      setBad(false);
    }
  }, [value]);
  // The language or the clock changed: the same time or date written the new way.
  useEffect(() => {
    const el = box.current!;
    if (latest.current.value && !el.dataset.bad) el.value = show(latest.current.value);
  }, [twelve, locale]);

  const errId = attrs.id ? `${attrs.id}-err` : undefined;
  const example = exampleOf(kind, locale);
  return <>
    <span className={`tf ${className || ''}`}>
      <input {...attrs} type="text" className="tf-box" ref={mount} disabled={disabled} autoComplete="off" spellCheck={false}
        placeholder={attrs.placeholder ?? (kind === 'time' ? show('20:00') : undefined)}
        aria-invalid={bad || undefined} aria-describedby={bad ? errId : undefined}
        onChange={e => {
          // Typing: nothing is said to the owner before the text is finished; a mistake already shown goes at the first key.
          if (emitting.current) return onChange?.(e);
          mark(e.currentTarget, !!e.currentTarget.value.trim() && parse(e.currentTarget.value) === null);
          if (bad) setBad(false);
        }}
        onKeyDown={e => {
          // The browser's own keys for its drop-down (Alt+Down, F4) open this one instead.
          if ((e.altKey && e.key === 'ArrowDown') || e.key === 'F4') {
            e.preventDefault();
            byKeys.current = true;
            setOpen(true);
          }
          onKeyDown?.(e);
        }} />
      <button type="button" className="tf-btn" ref={button} aria-label={label} title={label} aria-haspopup="dialog" aria-expanded={open}
        disabled={disabled} onClick={e => { byKeys.current = e.detail === 0; setOpen(o => !o); }}><Icon name={kind === 'time' ? 'clock' : 'calendar'} /></button>
      {open && box.current ? <Popup anchor={button.current!} close={close} label={label} takeFocus={byKeys.current}>
        {kind === 'time'
          ? <TimeList value={readBox(box.current)?.value || ''} twelve={twelve} pick={pick} />
          : <Calendar value={readBox(box.current)?.value || ''} pastDisabled={!!pastDisabled} pick={v => { pick(v); close(true); }} />}
      </Popup> : null}
    </span>
    {bad ? <small className="field-err tf-err" id={errId} role="alert">{t(kind === 'time' ? 'tf.badTime' : 'tf.badDate', { example })}</small> : null}
  </>;
}

// The list under (or over, near the bottom) its button, inside its dialog so the dialog's keys and focus still hold.
// Closes on the button again, a click outside, Escape (before the dialog hears it), Enter, or the window resizing.
// fit: as wide as its anchor and lined up with its left edge (a box's suggestions), not ending at its right edge.
// takeFocus: opened from the keyboard, so the keys go on inside it (it sits at the end of its dialog, Tab took ages to reach it).
export function Popup({ anchor, close, label, role = 'dialog', fit = false, id, takeFocus = false, children }: {
  anchor: HTMLElement; close: (refocus: boolean) => void; label: string; role?: string; fit?: boolean; id?: string; takeFocus?: boolean; children: ReactNode;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number; width?: number } | null>(null);
  // Placed again whenever what is around it moves: it is fixed to the window, so scrolling the dialog or page it was
  // opened from left it behind, and a calendar's six-week month (or a shorter list of suggestions) changed its height
  // after it was placed, running it off the bottom (review, 2026-10-30).
  const placeIt = useCallback(() => {
    const p = panel.current;
    if (!p) return;
    const a = anchor.getBoundingClientRect();
    const width = fit ? Math.min(a.width, innerWidth - 16) : undefined;
    if (width !== undefined) p.style.width = `${width}px`;
    const size = p.getBoundingClientRect();
    const below = a.bottom + 6, above = a.top - 6 - size.height;
    const top = below + size.height <= innerHeight - 8 || above < 8 ? Math.min(below, innerHeight - 8 - size.height) : above;
    const left = fit ? a.left : a.right - size.width;
    setPlace(old => {
      const next = { left: Math.max(8, Math.min(left, innerWidth - 8 - size.width)), top: Math.max(8, top), width };
      return old && old.left === next.left && old.top === next.top && old.width === next.width ? old : next;
    });
  }, [anchor, fit]);
  useLayoutEffect(() => {
    placeIt();
    const watch = new ResizeObserver(() => placeIt());
    watch.observe(panel.current!);
    return () => watch.disconnect();
  }, [placeIt]);
  const placed = place !== null;
  useEffect(() => {
    if (placed && takeFocus)
      panel.current?.querySelector<HTMLElement>('.on, button, input')?.focus();
  }, [placed, takeFocus]);
  useEffect(() => {
    // Any scroll but its own list's: its button moved with what scrolled. Scrolled out of sight there, it closes (it
    // would otherwise hang beside a box that isn't there any more).
    const scrolled = (e: Event) => {
      const at = e.target;
      if (at instanceof Node && panel.current?.contains(at)) return;
      // Something else scrolling (a log behind a dialog) didn't move its button.
      if (at instanceof Element && !at.contains(anchor)) return;
      const a = anchor.getBoundingClientRect();
      const view = at instanceof Element ? at.getBoundingClientRect() : { top: 0, bottom: innerHeight, left: 0, right: innerWidth };
      if (a.bottom <= view.top || a.top >= view.bottom || a.right <= view.left || a.left >= view.right) return close(false);
      placeIt();
    };
    addEventListener('scroll', scrolled, { capture: true, passive: true });
    return () => removeEventListener('scroll', scrolled, { capture: true });
  }, [anchor, close, placeIt]);
  useEffect(() => {
    // Window capture runs before the dialog's own key handler (document, capture): Escape closes this, not the dialog.
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape' && !(e.key === 'Enter' && panel.current?.contains(document.activeElement))) return;
      e.stopImmediatePropagation();
      // Enter on one of its buttons is that button's click (a day, the next month): cancelled here, it picked nothing.
      if (e.key === 'Enter' && document.activeElement?.tagName === 'BUTTON') return;
      e.preventDefault();
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
  return createPortal(<div className={`tf-pop${fit ? ' fit' : ''}`} ref={panel} role={role} aria-label={label} id={id}
    style={place ? { left: place.left, top: place.top, width: place.width } : { opacity: 0, left: 0, top: 0 }}>{children}</div>, host);
}

/** Hours, minutes (and AM/PM): a column each, the chosen ones marked; a click changes that part at once. */
function TimeList({ value, twelve, pick }: { value: string; twelve: boolean; pick: (v: string) => void }) {
  const [h0, m0] = /^\d\d:\d\d/.test(value) ? value.split(':').map(Number) : [new Date().getHours(), 0];
  const [h, setH] = useState(h0), [m, setM] = useState(m0);
  const set = (hour: number, minute: number) => {
    setH(hour);
    setM(minute);
    pick(`${pad(hour)}:${pad(minute)}`);
  };
  const hours = twelve ? [12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11] : [...Array(24).keys()];
  const pm = h >= 12;
  return <div className="tf-time-box">
    <div className="tf-time">
      <Column label="h" items={hours.map(x => ({ key: x, text: twelve ? String(x) : pad(x), on: twelve ? h % 12 === x % 12 : h === x }))}
        pick={x => set(twelve ? (x % 12) + (pm ? 12 : 0) : x, m)} />
      <Column label="m" items={[...Array(60).keys()].map(x => ({ key: x, text: pad(x), on: m === x }))} pick={x => set(h, x)} />
      {twelve ? <Column label="ampm" items={[{ key: 0, text: 'AM', on: !pm }, { key: 1, text: 'PM', on: pm }]}
        pick={x => set((h % 12) + (x ? 12 : 0), m)} /> : null}
    </div>
    {/* The quarter hours in one row: most start times are on one, and the minute column is sixty long. */}
    <div className="tf-quick">{[0, 15, 30, 45].map(x =>
      <button type="button" key={x} className={m === x ? 'on' : ''} aria-pressed={m === x} onClick={() => set(h, x)}>{pad(x)}</button>)}</div>
  </div>;
}

function Column({ label, items, pick }: { label: string; items: { key: number; text: string; on: boolean }[]; pick: (key: number) => void }) {
  const list = useRef<HTMLDivElement>(null);
  useLayoutEffect(() => {
    const on = list.current!.querySelector<HTMLElement>('.on');
    if (on) list.current!.scrollTop = on.offsetTop - list.current!.offsetTop - 4;
  }, []);
  return <div className="tf-col" ref={list} data-col={label}>
    {items.map(i => <button type="button" key={i.key} className={i.on ? 'on' : ''} aria-pressed={i.on} onClick={() => pick(i.key)}>{i.text}</button>)}
  </div>;
}

/** A month at a time, the week starting as the interface's language has it; today ringed, the chosen day filled. */
function Calendar({ value, pastDisabled, pick }: { value: string; pastDisabled: boolean; pick: (v: string) => void }) {
  const { t, lang, locale, weekdayName } = useHost();
  const chosen = /^\d{4}-\d\d-\d\d$/.test(value) ? value : '';
  const start = chosen ? new Date(chosen + 'T00:00') : new Date();
  const [month, setMonth] = useState(new Date(start.getFullYear(), start.getMonth(), 1));
  const first = lang === 'vi' ? 1 : 0; // Monday in Vietnamese, Sunday in English
  const lead = (month.getDay() - first + 7) % 7;
  const days = new Date(month.getFullYear(), month.getMonth() + 1, 0).getDate();
  const today = iso(new Date());
  const shift = (n: number) => setMonth(new Date(month.getFullYear(), month.getMonth() + n, 1));
  return <div className="tf-cal">
    <div className="tf-cal-head">
      <button type="button" className="icon-btn" aria-label={t('tf.prevMonth')} title={t('tf.prevMonth')} onClick={() => shift(-1)}><Icon name="chevronLeft" /></button>
      <b>{t('tf.monthYear', { month: month.toLocaleDateString(locale, { month: 'long' }), n: month.getMonth() + 1, year: month.getFullYear() })}</b>
      <button type="button" className="icon-btn" aria-label={t('tf.nextMonth')} title={t('tf.nextMonth')} onClick={() => shift(1)}><Icon name="chevronRight" /></button>
    </div>
    <div className="tf-cal-grid">
      {[...Array(7).keys()].map(i => <span key={'w' + i} className="tf-wd">{weekdayName((i + first) % 7)}</span>)}
      {[...Array(lead).keys()].map(i => <span key={'e' + i} />)}
      {[...Array(days).keys()].map(i => {
        const day = iso(new Date(month.getFullYear(), month.getMonth(), i + 1));
        const past = pastDisabled && day < today;
        return <button type="button" key={day} data-day={day} className={`${day === chosen ? 'on' : ''}${day === today ? ' today' : ''}`}
          aria-pressed={day === chosen} disabled={past} title={past ? t('tf.past') : undefined} onClick={() => pick(day)}>{i + 1}</button>;
      })}
    </div>
  </div>;
}
