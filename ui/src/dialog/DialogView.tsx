// The app's message box (Forms/Dialog.cs draws dialog.html in a small window of its own): the page's .modal with the
// question, its buttons side by side (the main one right) or stacked as choices with a line under each, "Don't ask
// again" when offered, and a footnote. Enter presses the default button, Esc and ✕ cancel; the app keeps the answer.
// Until 2026-11 it was hand-drawn WinForms.
import { useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { Icon, useHost } from '../host';

/** A button: its words, a line under it (then all are stacked as choices), and whether it destroys something. */
export interface DialogChoice { text: string; detail: string | null; danger: boolean }

/** The app's "dialog" event. */
export interface DialogData {
  text: string;
  heading: string | null;
  kind: 'info' | 'warning' | 'error';
  choices: DialogChoice[];
  /** Enter presses this one; Esc, ✕ and the timeout pick cancel. */
  def: number;
  cancel: number;
  footnote: string | null;
  /** "Don't ask again", when the question offers it. */
  remember: string | null;
  theme: string;
  customTheme: unknown;
  language: string;
}

/** What the page sends the app. */
export interface DialogOn {
  /** pick {index, remember}, cancel. */
  act(what: string, args?: Record<string, unknown>): void;
  /** The box's natural size in CSS pixels (its text not cut, nothing scrolled). */
  size(width: number, height: number): void;
  /** The head is being dragged: the window moves with the pointer. */
  drag(): void;
}

export interface DialogViewApi { show(data: DialogData): void }

const WIDTH = 460;
const MARK: Record<DialogData['kind'], string> = { info: 'info', warning: 'alert', error: 'x' };

export function DialogRoot({ on, handle }: { on: DialogOn; handle: Ref<DialogViewApi> }) {
  const [data, setData] = useState<DialogData | null>(null);
  useImperativeHandle(handle, () => ({ show: d => setData(d) }), []);
  return data ? <DialogView d={data} on={on} /> : null;
}

export function DialogView({ d, on }: { d: DialogData; on: DialogOn }) {
  const { t, logo } = useHost();
  const box = useRef<HTMLDivElement>(null);
  const remember = useRef<HTMLInputElement>(null);
  const listed = d.choices.some(c => c.detail);
  // Filled: a destructive button (red), else the first one; which one Enter presses is def's to say.
  const anyDanger = d.choices.some(c => c.danger);
  const pick = (index: number) => on.act('pick', { index, remember: !!remember.current?.checked });
  const def = Math.min(Math.max(d.def, 0), d.choices.length - 1);

  // The natural height: the head, all of the body (even while the window is too short and it scrolls) and the foot.
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const measure = () => {
      const parts = [...el.children] as HTMLElement[];
      const h = parts.reduce((n, p) => n + (p.classList.contains('modal-body') ? p.scrollHeight : p.offsetHeight), 0);
      on.size(WIDTH, h);
    };
    measure();
    const watch = new ResizeObserver(measure);
    [...el.children].forEach(p => watch.observe(p));
    return () => watch.disconnect();
  }, [d]);

  useEffect(() => {
    (box.current?.querySelector(`[data-i="${def}"]`) as HTMLElement | null)?.focus();
  }, [d]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key === 'Escape') { e.preventDefault(); on.act('cancel'); }
      // Enter on a button presses that button (the browser does); anywhere else, the default one.
      else if (e.key === 'Enter' && !(e.target instanceof HTMLButtonElement)) { e.preventDefault(); pick(def); }
    };
    document.addEventListener('keydown', key);
    return () => document.removeEventListener('keydown', key);
  }, [d]);

  const buttons = d.choices.map((c, i) => ({ c, i }));
  return <div ref={box} className={`modal dlg${listed ? ' listed' : ''}`} role="alertdialog" aria-modal="true"
    aria-labelledby="dlg-title" aria-describedby="dlg-text">
    <div className="modal-head" onMouseDown={e => { if (e.button === 0 && !(e.target as Element).closest('button')) on.drag(); }}>
      {d.heading ? <h3 id="dlg-title">{d.heading}</h3>
        : <h3 id="dlg-title" className="dlg-app"><img src={logo} alt="" />Bloxkeep</h3>}
      <button type="button" className="icon-btn" title={t('common.close')} aria-label={t('common.close')} onClick={() => on.act('cancel')}><Icon name="x" /></button>
    </div>
    <div className="modal-body">
      <div className="dlg-msg">
        <span className={`dlg-mark k-${d.kind}`}><Icon name={MARK[d.kind]} /></span>
        <p id="dlg-text" className={`pre dlg-text${d.heading ? '' : ' strong'}`}>{d.text}</p>
      </div>
      {d.remember ? <label className="dont-ask dlg-remember"><input ref={remember} type="checkbox" /><span>{d.remember}</span></label> : null}
      {listed ? <div className="dlg-choices">
        {buttons.map(({ c, i }) => <button key={i} type="button" data-i={i} className={`dlg-choice${c.danger ? ' danger' : ''}`} onClick={() => pick(i)}>
          <b>{c.text}</b>{c.detail ? <span>{c.detail}</span> : null}
        </button>)}
      </div> : null}
      {listed && d.footnote ? <p className="dlg-note">{d.footnote}</p> : null}
    </div>
    {!listed ? <div className="modal-foot">
      {d.footnote ? <p className="dlg-note push">{d.footnote}</p> : null}
      {/* The main button right, as in the app's own dialogs: the first choice last. */}
      {[...buttons].reverse().map(({ c, i }) => {
        const filled = anyDanger ? c.danger : i === 0;
        return <button key={i} type="button" data-i={i} className={`btn${filled ? (c.danger ? ' danger' : ' primary') : ''}`} onClick={() => pick(i)}>{c.text}</button>;
      })}
    </div> : null}
  </div>;
}
