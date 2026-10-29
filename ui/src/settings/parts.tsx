// The pieces Settings pages are made of, as app.js's settingsHtml() draws them: page, group, row, switch. Buttons keep
// their data-act: app.js's one click handler still does what they do.
import { useEffect, useLayoutEffect, useRef, useState, type InputHTMLAttributes, type ReactNode, type Ref } from 'react';
import { Icon, useHost } from '../host';
import { TimeInput } from '../TimeField';

/** A value that answers a click at once and follows the app's state when it comes. A new again (a count) puts the saved
 * value back even when it didn't change: after a value the app refused. */
export function useLocal<T>(value: T, again = 0): [T, (v: T) => void] {
  const [local, setLocal] = useState(value);
  useEffect(() => setLocal(value), [value, again]);
  return [local, setLocal];
}

export function Page({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  // While the Settings search box has words in it, a page drawn again (a row added, a class changed) is searched again.
  const { settingsDrawn } = useHost();
  useLayoutEffect(() => settingsDrawn());
  return <><h2 className="set-page-title" id={`set-page-${id}-h`}>{title}</h2>{children}</>;
}

export function Group({ id, title, children }: { id: string; title?: string; children: ReactNode }) {
  return <div className="set-grp" data-grp={id} id={`sec-${id}`}>
    {title ? <h3 className="set-grp-title">{title}</h3> : null}
    <div className="set-card">{children}</div>
  </div>;
}

export function Row({ title, desc, extra, children }: { title: ReactNode; desc?: ReactNode; extra?: ReactNode; children?: ReactNode }) {
  return <div className="set-row">
    <div><div className="t">{title}{extra}</div>{desc ? <div className="d">{desc}</div> : null}</div>
    <div className="ctl">{children}</div>
  </div>;
}

export function Switch({ id, label, title, checked, disabled, onChange }:
  { id: string; label?: string; title?: string; checked: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return <label className="switch" title={title}><input type="checkbox" id={id} aria-label={label || undefined} checked={checked} disabled={disabled}
    onChange={e => onChange(e.target.checked)} /><span /></label>;
}

/** A text box saved when left or on Enter (its change event), not on every key. commit answers false when refused here:
 * the box is marked (.bad) and keeps it; the saved value comes back, unmarked, when the app's value or again changes. */
export function TextBox({ saved, again = 0, commit, className, ref, ...attrs }: {
  saved: string; again?: number; commit: (value: string) => boolean | void; className: string; ref?: Ref<HTMLInputElement>;
} & Omit<InputHTMLAttributes<HTMLInputElement>, 'value' | 'onChange' | 'className'>) {
  const [value, setValue] = useLocal(saved, again);
  const [bad, setBad] = useState(false);
  useEffect(() => setBad(false), [saved, again]);
  const box = useRef<HTMLInputElement | null>(null);
  const latest = useRef(commit);
  latest.current = commit;
  useEffect(() => {
    const el = box.current!;
    const changed = () => setBad(latest.current(el.value) === false);
    el.addEventListener('change', changed);
    return () => el.removeEventListener('change', changed);
  }, []);
  const own = {
    ref: (el: HTMLInputElement | null) => {
      box.current = el;
      if (typeof ref === 'function') ref(el); else if (ref) ref.current = el;
    },
    className: `${className}${bad ? ' bad' : ''}`, value, onChange: (e: { target: HTMLInputElement }) => setValue(e.target.value),
  };
  // A time box gets Bloxkeep's own picker (TimeField.tsx) rather than the browser's.
  if (attrs.type === 'time') {
    const { type: _, ...rest } = attrs;
    return <TimeInput {...rest} {...own} />;
  }
  return <input {...attrs} {...own} />;
}

/** A part of a settings card that stays closed until wanted: its line reads like a row title, with a chevron. */
export function More({ id, title, children }: { id: string; title: string; children: ReactNode }) {
  return <details className="set-more" id={id}><summary>{title}<Icon name="chevronDown" /></summary>{children}</details>;
}