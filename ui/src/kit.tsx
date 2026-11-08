// Bloxkeep's controls, one of each: every screen uses these instead of writing the markup itself, so a control looks and
// behaves the same everywhere and changes in one place. They keep app.css's class names (.btn, .icon-btn, .switch…):
// the themes style those (app.css has ~1,600 theme rules on them) and app.js finds buttons by them (.modal-foot .btn.primary,
// busy() adds .busy). Anything a control doesn't name itself (id, title, aria-*, data-*, handlers, ref) goes straight onto
// the element.
import type { ButtonHTMLAttributes, HTMLAttributes, InputHTMLAttributes, ReactNode, Ref, TextareaHTMLAttributes } from 'react';
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

/** A one-line text box (.field). size sm: 30px tall, for rows and toolbars. The class adds the place's look (num, mono…). */
export function Field({ size, className, ...attrs }: Omit<InputHTMLAttributes<HTMLInputElement>, 'size'> & { ref?: Ref<HTMLInputElement>; size?: 'sm' }) {
  return <input {...attrs} className={cx('field', size, className)} />;
}

/** A text box of several lines (.field). */
export function TextArea({ className, ...attrs }: TextareaHTMLAttributes<HTMLTextAreaElement> & { ref?: Ref<HTMLTextAreaElement> }) {
  return <textarea {...attrs} className={cx('field', className)} />;
}

/** A short word of state in front of other words (Banned, Expired). tone bad: red. */
export function Badge({ tone, className, ...attrs }: HTMLAttributes<HTMLSpanElement> & { tone?: 'bad' }) {
  return <span {...attrs} className={cx('badge', tone, className)} />;
}

/** Choices side by side in one box (.seg), one of them on. wide: the box fills the row, the choices share it. The
 * container says what the choices are (role tablist or radiogroup, aria-label); each choice is a Segment. */
export function Segmented({ wide, className, ...attrs }: HTMLAttributes<HTMLDivElement> & { ref?: Ref<HTMLDivElement>; wide?: boolean }) {
  return <div {...attrs} className={cx('seg', wide && 'wide', className)} />;
}

/** One choice of a Segmented. as: tab (aria-selected, in a tablist), radio (aria-checked, in a radiogroup) or toggle
 * (aria-pressed); on: it is the one chosen. */
export function Segment({ as, on, className, ...attrs }: ButtonAttrs & { as: 'tab' | 'radio' | 'toggle'; on: boolean }) {
  const state = as === 'tab' ? { role: 'tab', 'aria-selected': on } : as === 'radio' ? { role: 'radio', 'aria-checked': on } : { 'aria-pressed': on };
  return <button {...state} {...attrs} className={cx(on && 'on', className)} />;
}

/** A tick box with its words beside it, all one label. The class names the place's look (dont-ask, mc-check…); title
 * goes on the label. */
export function Check({ className, title, label, children, ...input }: InputAttrs & { label?: ReactNode; children?: ReactNode }) {
  return <label className={className} title={title}><input type="checkbox" {...input} />{label != null ? <span>{label}</span> : null}{children}</label>;
}
