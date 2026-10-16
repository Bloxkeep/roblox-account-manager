// Send feedback: the body of the dialog (an idea or a bug report for Bloxkeep's developer). Its Cancel and Send stay in
// the dialog's footer, which app.js draws: it reads what was written and shows a refusal through FeedbackApi. "See
// exactly what will be sent" asks the app for the message as it would go, error log included (account names already
// taken out by the app).
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react';
import { Icon, useHost } from './host';

export interface FeedbackValues { kind: 'bug' | 'idea'; text: string; contact: string; log: boolean }

/** What the dialog's footer (app.js) does with the form. */
export interface FeedbackApi {
  values(): FeedbackValues;
  /** A message under the form (null: none). */
  showError(message: string | null): void;
  focusText(): void;
}

export function Feedback({ locked, api }: { locked: boolean; api: Ref<FeedbackApi> }) {
  const { t, call } = useHost();
  const [kind, setKind] = useState<'bug' | 'idea'>('bug');
  const [text, setText] = useState('');
  const [contact, setContact] = useState('');
  const [log, setLog] = useState(!locked);
  const [preview, setPreview] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const textBox = useRef<HTMLTextAreaElement>(null);
  const values = { kind, text, contact, log: !locked && log };
  const current = useRef(values);
  current.current = values;

  useImperativeHandle(api, () => ({
    values: () => current.current,
    showError: setError,
    focusText: () => textBox.current?.focus(),
  }), []);
  useEffect(() => textBox.current?.focus(), []);

  // What goes changes with what's typed: an open preview is shut again rather than left out of date.
  const edited = <T,>(set: (v: T) => void) => (v: T) => { set(v); setPreview(null); };
  const look = async () => {
    if (preview !== null) { setPreview(null); return; }
    const r = await call('feedbackPreview', { ...current.current }) as { preview: string } | undefined;
    if (r) setPreview(r.preview);
  };

  return <>
    <div className="form">
      <label>{t('fb.kind')}<select className="select" name="kind" value={kind} onChange={e => {
        const k = e.target.value as 'bug' | 'idea';
        edited(setKind)(k);
        // An idea rarely needs the log; a bug report usually does.
        if (!locked) setLog(k === 'bug');
      }}>
        <option value="bug">{t('fb.bug')}</option><option value="idea">{t('fb.idea')}</option>
      </select></label>
      <label>{t('fb.text')}<textarea ref={textBox} className="field" name="text" maxLength={2000} rows={6} placeholder={t('fb.textPh')}
        value={text} onChange={e => edited(setText)(e.target.value)} /></label>
      <label><span>{t('fb.contact')} <span className="muted">{t('fb.optional')}</span></span><input className="field" name="contact" maxLength={100}
        autoComplete="off" spellCheck={false} placeholder={t('fb.contactPh')} value={contact} onChange={e => edited(setContact)(e.target.value)} /></label>
    </div>
    <label className="dont-ask"><input type="checkbox" name="log" disabled={locked} checked={!locked && log}
      onChange={e => edited(setLog)(e.target.checked)} /><span>{t(locked ? 'fb.logLocked' : 'fb.log')}</span></label>
    <div className="fb-look"><button type="button" className="link" aria-expanded={preview !== null} onClick={look}>{t('fb.preview')}</button></div>
    <pre className="fb-preview" tabIndex={0} aria-label={t('fb.preview')} hidden={preview === null}>{preview}</pre>
    <p className="fine"><Icon name="shield" /><span>{t('fb.fine')}</span></p>
    <div className="form-error" role="alert" hidden={!error}>{error}</div>
  </>;
}
