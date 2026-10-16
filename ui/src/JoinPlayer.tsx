// Join a player: the body of the dialog (a Roblox name or user ID, the names looked up before, and what the lookup
// said). The lookup and the launch stay in app.js, behind the dialog's "Find and join": it reads the name and writes the
// answer here through JoinPlayerApi. The recent names follow the app's state, so a name forgotten or newly remembered
// shows as soon as the app says.
import { useImperativeHandle, useRef, useState, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';

export type Tone = 'info' | 'warn' | 'error';
export interface JoinPlayerApi {
  name(): string;
  /** What the lookup said, as HTML from app.js (it escapes the names in it); '' hides it. */
  say(html: string, tone?: Tone): void;
  /** Back to the name box, its text selected when asked (to type another name over it). */
  focusName(select?: boolean): void;
}

export function JoinPlayer({ count, submit, api }: { count: number; submit: () => void; api: Ref<JoinPlayerApi> }) {
  const { t, tn, call } = useHost();
  const recent: string[] = useAppState().settings?.recentPlayers || [];
  const [name, setName] = useState('');
  const [msg, setMsg] = useState<{ html: string; tone: Tone }>({ html: '', tone: 'info' });
  const current = useRef('');
  const box = useRef<HTMLInputElement>(null);
  useImperativeHandle(api, () => ({
    name: () => current.current,
    say: (html, tone = 'info') => setMsg({ html, tone }),
    focusName: select => { box.current?.focus(); if (select) box.current?.select(); },
  }), []);
  const write = (v: string) => { current.current = v; setName(v); };

  return <>
    <div className="form jp">
      <p className="modal-text">{tn('jp.note', count)}</p>
      <label>{t('jp.label')}<input ref={box} className="field" id="jp-name" maxLength={100} autoComplete="off" spellCheck={false}
        placeholder={t('jp.placeholder')} value={name} autoFocus onChange={e => { write(e.target.value); setMsg({ html: '', tone: 'info' }); }} /></label>
      <div className="jp-recent" id="jp-recent" role="group" aria-label={t('jp.recent')} hidden={!recent.length}>
        {recent.length ? <small>{t('jp.recent')}</small> : null}
        {recent.map(n => <span key={n} className="jp-chip">
          {/* A name from the list is looked up at once. */}
          <button type="button" className="jp-pick" onClick={() => { write(n); submit(); }}>{n}</button>
          <button type="button" className="jp-x" title={t('jp.forget')} aria-label={t('jp.forgetN', { name: n })}
            onClick={async () => { await call('forgetPlayer', { name: n }); box.current?.focus(); }}><Icon name="x" /></button>
        </span>)}
      </div>
      <div className={`jp-msg ${msg.tone}`} id="jp-msg" role="status" aria-live="polite" hidden={!msg.html} dangerouslySetInnerHTML={{ __html: msg.html }} />
    </div>
    <p className="fine"><Icon name="shield" /><span>{t('jp.privacy')}</span></p>
  </>;
}
