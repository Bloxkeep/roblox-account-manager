// The pieces Settings pages are made of, drawn as app.js's settingsHtml() draws them: a page (its title, then groups),
// a group (a small heading over a card of rows), a row (title and description on the left, its control on the right),
// a switch. Buttons keep their data-act: app.js's one click handler for the whole page still does what they do.
import { useEffect, useState, type ReactNode } from 'react';

/** A control's value that answers a click at once, and follows the app's state when that comes (confirming it or not). */
export function useLocal<T>(value: T): [T, (v: T) => void] {
  const [local, setLocal] = useState(value);
  useEffect(() => setLocal(value), [value]);
  return [local, setLocal];
}

export function Page({ id, title, children }: { id: string; title: string; children: ReactNode }) {
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

export function Switch({ id, label, checked, disabled, onChange }:
  { id: string; label?: string; checked: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return <label className="switch"><input type="checkbox" id={id} aria-label={label || undefined} checked={checked} disabled={disabled}
    onChange={e => onChange(e.target.checked)} /><span /></label>;
}
