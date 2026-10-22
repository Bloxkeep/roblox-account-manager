// Join a player: the body of the dialog (a Roblox name or user ID, the names looked up before, and what the lookup
// said). The lookup and the launch stay in app.js, behind the dialog's "Find and join": it reads the name and writes the
// answer here through JoinPlayerApi. The recent names follow the app's state, so a name forgotten or newly remembered
// shows as soon as the app says.
import { Fragment, useImperativeHandle, useRef, useState, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';

export type Tone = 'info' | 'warn' | 'error';
export interface JoinPlayerApi {
  name(): string;
  /**
   * What the lookup said ('' hides it): with a spinner while looking, or a name in bold where the words have a NUL
   * character in its place ("{name} isn't in a game" asked for with name '\u0000').
   */
  say(text: string, tone?: Tone, opts?: { spin?: boolean; bold?: string }): void;
  /** Back to the name box, its text selected when asked (to type another name over it). */
  focusName(select?: boolean): void;
  /** The box "follow this player when the accounts rejoin": ticked, their rejoins go wherever the player is by then. */
  follow(): boolean;
}

export function JoinPlayer({ count, submit, api }: { count: number; submit: () => void; api: Ref<JoinPlayerApi> }) {
  const { t, tn, call } = useHost();
  const recent: string[] = useAppState().settings?.recentPlayers || [];
  const [name, setName] = useState('');
  type Msg = { text: string; tone: Tone; spin?: boolean; bold?: string };
  const [msg, setMsg] = useState<Msg>({ text: '', tone: 'info' });
  const current = useRef('');
  const box = useRef<HTMLInputElement>(null);
  // Remembered by the app (FollowJoinedPlayer, on until unticked) once a join goes ahead with it.
  const [follow, setFollow] = useState<boolean>(useAppState().settings?.followJoinedPlayer !== false);
  const following = useRef(follow);
  following.current = follow;
  useImperativeHandle(api, () => ({
    name: () => current.current,
    say: (text, tone = 'info', opts = {}) => setMsg({ text, tone, ...opts }),
    focusName: select => { box.current?.focus(); if (select) box.current?.select(); },
    follow: () => following.current,
  }), []);
  const write = (v: string) => { current.current = v; setName(v); };

  return <>
    <div className="form jp">
      <p className="modal-text">{tn('jp.note', count)}</p>
      <label>{t('jp.label')}<input ref={box} className="field" id="jp-name" maxLength={100} autoComplete="off" spellCheck={false}
        placeholder={t('jp.placeholder')} value={name} autoFocus onChange={e => { write(e.target.value); setMsg({ text: '', tone: 'info' }); }} /></label>
      <div className="jp-recent" id="jp-recent" role="group" aria-label={t('jp.recent')} hidden={!recent.length}>
        {recent.length ? <small>{t('jp.recent')}</small> : null}
        {recent.map(n => <span key={n} className="jp-chip">
          {/* A name from the list is looked up at once. */}
          <button type="button" className="jp-pick" onClick={() => { write(n); submit(); }}>{n}</button>
          <button type="button" className="jp-x" title={t('jp.forget')} aria-label={t('jp.forgetN', { name: n })}
            onClick={async () => { await call('forgetPlayer', { name: n }); box.current?.focus(); }}><Icon name="x" /></button>
        </span>)}
      </div>
      <label className="dont-ask jp-follow"><input type="checkbox" id="jp-follow" checked={follow} onChange={e => setFollow(e.target.checked)} />
        <span>{t('jp.follow')}</span></label>
      <div className={`jp-msg ${msg.tone}`} id="jp-msg" role="status" aria-live="polite" hidden={!msg.text}>
        {msg.spin ? <span className="spin" /> : null}
        {msg.bold !== undefined ? msg.text.split('\u0000').map((part, i) => <Fragment key={i}>{i ? <b>{msg.bold}</b> : null}{part}</Fragment>) : msg.text}</div>
    </div>
    <p className="fine"><Icon name="shield" /><span>{t('jp.privacy')}</span></p>
  </>;
}
