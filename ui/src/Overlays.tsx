// What shows over the page: a dialog's frame (title, close button, body, footer buttons), the bodies of the plain
// dialogs (a question, a list of errors, the Discord link box), a menu, and the messages in the corner. app.js still
// decides when they open and close and what their buttons do (modal, confirmBox, openMenu, toast); these draw them.
import { useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { Icon, useHost } from './host';
import { richNodes } from './rich';

/* ───────────── dialogs ───────────── */

export type ModalAction = { text: string; kind?: string };
export interface ModalOpts {
  /** The title: the app's words (markup they carry drawn as such), or something drawn here (What's new's). */
  title: string | ReactNode;
  titleId: string;
  /** Drawn here, or null for a body a React part of app.js's choosing is put into (.modal-body). */
  body: ReactNode | null;
  actions: ModalAction[];
  /** A footer button pressed (app.js runs its onClick with the button). */
  act(i: number, button: HTMLButtonElement): void;
  /** The ✕ (app.js asks first when something typed would go). */
  close(): void;
  /** Extra classes on the box (auth, palette) and its body (srv-body); a name for a box with no title. */
  boxClass?: string; bodyClass?: string; label?: string;
  /** Where a refused save says why, above the buttons (the preset form). */
  footError?: boolean;
}

export function ModalFrame({ title, titleId, body, actions, act, close, boxClass, bodyClass, label, footError }: ModalOpts) {
  const { t } = useHost();
  return <div className={`modal${boxClass ? ` ${boxClass}` : ''}`} role="dialog" aria-modal="true" aria-labelledby={title ? titleId : undefined} aria-label={label}>
    <div className="modal-head"><h3 id={titleId}>{typeof title === 'string' ? richNodes(title) : title}</h3>
      <button className="icon-btn" data-x="" title={t('common.close')} onClick={close}><Icon name="x" /></button></div>
    <div className={`modal-body${bodyClass ? ` ${bodyClass}` : ''}`}>{body}</div>
    {footError ? <div className="form-error ps-err" role="alert" hidden /> : null}
    {actions.length ? <div className="modal-foot">{actions.map((a, i) =>
      <button key={i} className={`btn ${a.kind || ''}`} data-i={i} onClick={e => act(i, e.currentTarget)}>{a.text}</button>)}</div> : null}
  </div>;
}

/** A question, with "Don't ask again" when it can be skipped from then on (the app's SkippableConfirms). */
export function AskBody({ message, remember }: { message: string; remember: boolean }) {
  const { t } = useHost();
  return <><p className="pre modal-text">{message}</p>
    {remember ? <label className="dont-ask"><input type="checkbox" id="dont-ask" /><span>{t('common.dontAsk')}</span></label> : null}</>;
}

/** What went wrong, one line each (a launch's accounts that didn't open). */
export function ErrorsBody({ errors }: { errors: string[] }) {
  return <ul className="errs">{errors.map((x, i) => <li key={i}>{x}</li>)}</ul>;
}

/** Settings › Discord's "Paste a link": the webhook link's box and what is wrong with it (app.js checks and saves). */
export function DiscordLinkBody() {
  const { t } = useHost();
  return <><p className="modal-text">{richNodes(t('dc.pasteText'))}</p>
    <input className="field" id="dc-link" type="text" maxLength={300} spellCheck={false} autoComplete="off" placeholder="https://discord.com/api/webhooks/…" aria-label={t('dc.webhook')} />
    <p className="dc-err" id="dc-link-err" role="alert" /></>;
}

/** What's new's title: the logo, "What's new", and the version and its date under it. */
export function WhatsNewTitle({ sub }: { sub: string }) {
  const { t, logo } = useHost();
  return <span className="wn-title"><img src={logo} alt="" /><span>{t('wn.heading')}<small>{sub}</small></span></span>;
}

/* ───────────── menus ───────────── */

export type MenuItem = '-' | { icon?: string; text: string; title?: string; hint?: string; danger?: boolean; checked?: boolean; disabled?: boolean; run?: () => void };

/**
 * A menu's items (app.js makes the menu, places it and closes it). For the dock's presets the items before the first
 * line scroll in a list of their own, Save and Edit staying at its foot.
 */
export function MenuItems({ items, pick, split }: { items: MenuItem[]; pick: (i: number, byKeyboard: boolean) => void; split: boolean }) {
  const draw = (it: MenuItem, i: number) => it === '-' ? <hr key={i} />
    : <button key={i} role="menuitem" data-mi={i} className={`${it.danger ? 'danger' : ''} ${it.checked ? 'checked' : ''}`} disabled={it.disabled}
      onClick={e => pick(i, e.detail === 0)}><Icon name={it.checked ? 'check' : it.icon || ''} /><span title={it.title || undefined}>{it.text}</span>{it.hint ? <kbd>{it.hint}</kbd> : null}</button>;
  const foot = items.indexOf('-');
  if (!split || foot < 0) return <>{items.map(draw)}</>;
  return <><div className="pm-list">{items.slice(0, foot).map(draw)}</div>{items.slice(foot).map((it, k) => draw(it, foot + k))}</>;
}

/* ───────────── messages in the corner ───────────── */

export type ToastKind = 'info' | 'ok' | 'warn' | 'error';
type Toast = { id: number; text: string; kind: ToastKind; action: { label: string; run: () => void } | null; out: boolean; again: number };
export interface ToastsApi {
  show(text: string, kind: ToastKind, action: Toast['action']): void;
  /** Every message goes at once (the vault locked: they may name accounts). */
  clear(): void;
}

/**
 * The messages in the corner, three at most (a stack mustn't cover the page). Each stays long enough to be read (about
 * 60 ms a character, longer for an error or one with a button), held while pointed at or focused; a click on it
 * closes it, like a Windows notification. The same message again while it shows stays longer instead of stacking.
 */
export function Toasts({ api }: { api: Ref<ToastsApi> }) {
  // The list itself, changed at once (a second message in the same moment sees the first), drawn after.
  const items = useRef<Toast[]>([]);
  const [, draw] = useState(0);
  const set = (next: Toast[]) => { items.current = next; draw(n => n + 1); };
  const list = items.current;
  const seq = useRef(0);
  const timers = useRef(new Map<number, number>());
  const els = useRef(new Map<number, HTMLDivElement>());
  const dismiss = (id: number) => {
    clearTimeout(timers.current.get(id));
    set(items.current.map(x => x.id === id ? { ...x, out: true } : x));
    setTimeout(() => set(items.current.filter(x => x.id !== id)), 220);
  };
  const arm = (id: number, ms: number) => {
    clearTimeout(timers.current.get(id));
    timers.current.set(id, window.setTimeout(() => dismiss(id), ms));
  };
  useImperativeHandle(api, () => ({
    show(text, kind, action) {
      const life = Math.min(Math.max(kind === 'error' ? 6500 : action ? 9000 : 3800, text.length * 60), 15000);
      const same = !action && items.current.find(x => !x.out && x.text === text && x.kind === kind);
      if (same) {
        const el = els.current.get(same.id);
        // Held while pointed at or focused: leaving it starts the countdown, as for any toast.
        if (el && !el.matches(':hover') && !el.contains(document.activeElement)) arm(same.id, life);
        else clearTimeout(timers.current.get(same.id));
        if (el) {
          el.classList.remove('again');
          void el.offsetWidth; // starts the nudge again
          el.classList.add('again');
        }
        return;
      }
      const id = ++seq.current;
      const l = items.current;
      set([...l.slice(Math.max(0, l.length - 2)), { id, text, kind, action, out: false, again: 0 }]);
      arm(id, life);
    },
    clear() {
      timers.current.forEach(t => clearTimeout(t));
      timers.current.clear();
      set([]);
    },
  }), []);
  useEffect(() => () => timers.current.forEach(t => clearTimeout(t)), []);
  return <>{list.map(x => <div key={x.id} ref={el => { if (el) els.current.set(x.id, el); else els.current.delete(x.id); }}
    className={`toast ${x.kind}${x.out ? ' out' : ''}`} role={x.kind === 'error' ? 'alert' : 'status'}
    onClick={e => { if (!(e.target as HTMLElement).closest('.toast-act')) { clearTimeout(timers.current.get(x.id)); dismiss(x.id); } }}
    onMouseEnter={() => clearTimeout(timers.current.get(x.id))}
    onFocus={() => clearTimeout(timers.current.get(x.id))}
    onMouseLeave={e => { if (!e.currentTarget.contains(document.activeElement)) arm(x.id, 2500); }}
    onBlur={e => { if (!e.currentTarget.matches(':hover')) arm(x.id, 2500); }}>
    <Icon name={x.kind === 'ok' ? 'check' : x.kind === 'info' ? 'info' : 'alert'} /><span>{x.text}</span>
    {x.action ? <button className="toast-act" onClick={() => { x.action!.run(); dismiss(x.id); }}>{x.action.label}</button> : null}
  </div>)}</>;
}
