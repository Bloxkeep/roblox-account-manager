// Bloxkeep's controls, one of each: every screen uses these instead of writing the markup itself, so a control looks and
// behaves the same everywhere and changes in one place. They keep app.css's class names (.btn, .icon-btn, .switch…):
// the themes style those (app.css has ~1,600 theme rules on them) and app.js finds buttons by them (.modal-foot .btn.primary,
// busy() adds .busy). Anything a control doesn't name itself (id, title, aria-*, data-*, handlers, ref) goes straight onto
// the element.
import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, Ref } from 'react';
import { Icon } from './host';

const cx = (...names: (string | false | null | undefined)[]) => names.filter(Boolean).join(' ');

type ButtonAttrs = ButtonHTMLAttributes<HTMLButtonElement> & { ref?: Ref<HTMLButtonElement> };

/** A button with words (and maybe an icon before them).
 * size: sm 30px, big 48px, else 38px. kind: primary (the page's one main action, inverse), go (Play's green),
 * danger (red). ghost: no fill (with or without a kind). wide: the whole width. pressed: a toggle that is on (.on and
 * aria-pressed; leave it out for a button that isn't a toggle). */
export function Button({ size, kind, ghost, wide, pressed, className, ...attrs }: ButtonAttrs & {
  size?: 'sm' | 'big'; kind?: 'primary' | 'go' | 'danger'; ghost?: boolean; wide?: boolean; pressed?: boolean;
}) {
  return <button {...(pressed === undefined ? {} : { 'aria-pressed': pressed })} {...attrs}
    className={cx('btn', size, kind, ghost && 'ghost', wide && 'w', pressed && 'on', className)} />;
}

/** A square button with only an icon: give it a title (and an aria-label when the title isn't the whole name).
 * icon: the icon's name; children come after it (a badge, a second icon). */
export function IconButton({ icon, className, children, ...attrs }: ButtonAttrs & { icon?: string; children?: ReactNode }) {
  return <button {...attrs} className={cx('icon-btn', className)}>{icon ? <Icon name={icon} /> : null}{children}</button>;
}

type InputAttrs = Omit<InputHTMLAttributes<HTMLInputElement>, 'type'> & { ref?: Ref<HTMLInputElement> };

/** An on/off switch: a checkbox drawn as a sliding knob. title goes on the switch (what turning it does); the rest on
 * the checkbox. inside: the switch sits in a label of its own (a settings row), so it is a span, not a second label. */
export function Switch({ title, inside, className, ...input }: InputAttrs & { inside?: boolean }) {
  const box = <><input type="checkbox" {...input} /><span /></>;
  return inside
    ? <span className={cx('switch', className)} title={title}>{box}</span>
    : <label className={cx('switch', className)} title={title}>{box}</label>;
}

/** A tick box with its words beside it, all one label. The class names the place's look (dont-ask, mc-check…); title
 * goes on the label. */
export function Check({ className, title, label, children, ...input }: InputAttrs & { label?: ReactNode; children?: ReactNode }) {
  return <label className={className} title={title}><input type="checkbox" {...input} />{label != null ? <span>{label}</span> : null}{children}</label>;
}
