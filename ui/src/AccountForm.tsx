// Adding an account by its cookie, or editing one: alias, group, password, authenticator key (accounts already here), note.
// The key itself never comes to the page: a saved one shows only its current code; a typed one is checked by the app as it
// is typed (it shows the code it makes, to compare). app.js's dialog asks this form what to save (collect) and if dirty.
import { useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';
import { Rich } from './rich';
import { Avatar } from './Avatar';
import { SuggestInput } from './Suggest';

type Account = { id: string; label: string; username?: string; userId?: number; alias?: string; group?: string; note?: string; hasPassword?: boolean; hasAuthenticator?: boolean };
export interface AccountFormApi {
  collect(): { args: Record<string, unknown> } | { error: string };
  /** A refusal (the app's words) under the form. */
  fail(message: string): void;
  dirty(): boolean;
}

/** "123 456": a code in two halves, as authenticator apps show it. */
const split = (c: string) => c.length > 4 ? `${c.slice(0, Math.ceil(c.length / 2))} ${c.slice(Math.ceil(c.length / 2))}` : c;

export function AccountForm({ account: a, api, browser }: { account: Account | null; api: Ref<AccountFormApi>; browser: () => void }) {
  const host = useHost();
  const { t, call } = host;
  const app = useAppState();
  const isNew = !a;
  const hide = !!app.hideNames;
  const groups = [...new Set<string>((app.accounts || []).map((x: Account) => x.group).filter(Boolean))];
  // While names are hidden the alias can't be shown, so it can't be edited either: it isn't sent, and stays as it is.
  const aliasHidden = !isNew && hide;
  const hasKey = !isNew && !!a.hasAuthenticator;

  const [cookie, setCookie] = useState('');
  const [alias, setAlias] = useState(aliasHidden ? '' : a?.alias || '');
  const [group, setGroup] = useState(a?.group || '');
  const [password, setPassword] = useState('');
  const [note, setNote] = useState(a?.note || '');
  const [key, setKey] = useState('');
  const [removeKey, setRemoveKey] = useState(false);
  // Secret boxes shown as text; one counts as changed once typed in (a saved password only shown isn't an edit).
  const [shown, setShown] = useState<Record<string, boolean>>({});
  const typed = useRef(new Set<string>());
  const [error, setError] = useState('');
  const errorLine = useRef<HTMLDivElement>(null);
  useEffect(() => { if (error) errorLine.current?.scrollIntoView({ block: 'nearest' }); }, [error]);

  // The code on show (the saved key's, or the typed key's preview), counted down and asked for again when it changes.
  const [code, setCode] = useState('');
  const [left, setLeft] = useState('');
  const [msg, setMsg] = useState<{ node: ReactNode; kind: string }>({ node: '', kind: '' });
  const seq = useRef(0), timer = useRef(0), alive = useRef(true);
  const keyNow = useRef('');
  keyNow.current = key.trim();
  const removing = useRef(false);
  removing.current = removeKey;
  useEffect(() => () => { alive.current = false; seq.current++; clearTimeout(timer.current); }, []);
  const showCode = async () => {
    clearTimeout(timer.current);
    const mine = ++seq.current;
    const k = keyNow.current, saved = hasKey && !removing.current && !k;
    if (!saved && !k) {
      setMsg(removing.current
        ? { node: <>{t('am.totpRemoved')} <button type="button" className="link" data-totp="undo" onClick={() => undo()}>{t('am.totpUndo')}</button></>, kind: '' }
        : { node: '', kind: '' });
      return;
    }
    const r = await call(saved ? 'totpCode' : 'totpCheck', saved ? { id: a!.id } : { key: k }, { silent: true }) as { ok: boolean; error?: string; code: string; seconds: number } | undefined;
    if (mine !== seq.current || !alive.current || !r) return;
    if (!r.ok) return setMsg({ node: r.error, kind: 'err' });
    const until = Date.now() + r.seconds * 1000;
    if (saved) { setCode(split(r.code)); setMsg({ node: '', kind: '' }); }
    else setMsg({ node: t('am.totpNow', { code: split(r.code) }), kind: 'ok' });
    const count = () => {
      if (mine !== seq.current || !alive.current) return;
      const s = Math.ceil((until - Date.now()) / 1000);
      if (s <= 0) return showCode();
      if (saved) setLeft(t('am.totpLeft', { s }));
      timer.current = window.setTimeout(count, 1000);
    };
    count();
  };
  useEffect(() => { if (hasKey) showCode(); }, []); // the saved key's code, as the form opens
  // A key being typed is checked once the typing rests (a reply for what was typed before doesn't count any more).
  const typeWait = useRef(0);
  useEffect(() => () => clearTimeout(typeWait.current), []);

  const live = useRef<HTMLDivElement>(null), box = useRef<HTMLInputElement>(null);
  // Remove (or its Undo) only changes the form: the key goes once the form is saved.
  const remove = () => {
    setRemoveKey(true);
    removing.current = true;
    showCode();
    setTimeout(() => box.current?.focus());
  };
  const undo = () => {
    setRemoveKey(false);
    removing.current = false;
    setKey('');
    keyNow.current = '';
    showCode();
    setTimeout(() => live.current?.querySelector<HTMLElement>('[data-totp=copy]')?.focus());
  };

  const reveal = async (name: string, value: string, set: (v: string) => void) => {
    if (shown[name]) return setShown({ ...shown, [name]: false });
    // Only the saved password can be shown from the app: an authenticator key never comes back to the page.
    if (!isNew && name === 'password' && !typed.current.has(name) && !value) {
      const v = await call('reveal', { id: a!.id, field: name });
      if (typeof v === 'string') set(v);
    }
    setShown({ ...shown, [name]: true });
  };
  const eye = (name: string, value: string, set: (v: string) => void) =>
    <button type="button" className="icon-btn" data-reveal={name} title={t('common.show')} onClick={() => reveal(name, value, set)}><Icon name={shown[name] ? 'eyeOff' : 'eye'} /></button>;

  const values = () => JSON.stringify([isNew ? (typed.current.has('cookie') ? cookie : '') : null, alias, group,
    typed.current.has('password') ? password : '', key, note, removeKey]);
  const initial = useRef('');
  useEffect(() => { initial.current = values(); }, []); // as it opened
  const latest = useRef({ values, collect: () => ({}) as ReturnType<AccountFormApi['collect']> });
  latest.current.values = values;
  latest.current.collect = () => {
    setError('');
    if (isNew && !cookie.trim()) {
      setError(t('am.needCookie'));
      return { error: t('am.needCookie') };
    }
    const args: Record<string, unknown> = { group, note };
    if (!aliasHidden) args.alias = alias;
    if (isNew) Object.assign(args, { cookie, password });
    else {
      args.id = a!.id;
      if (typed.current.has('password')) args.password = password;
      // A key typed replaces the saved one (the app checks it); Remove sends none; otherwise it stays.
      if (key.trim()) args.authenticator = key.trim();
      else if (removeKey) args.authenticator = '';
    }
    return { args };
  };
  useImperativeHandle(api, () => ({
    collect: () => latest.current.collect(),
    fail: message => setError(message),
    dirty: () => latest.current.values() !== initial.current,
  }), []);

  const typedIn = (name: string, set: (v: string) => void) => (e: { target: { value: string } }) => { typed.current.add(name); set(e.target.value); };
  const liveShown = hasKey && !removeKey;

  return <>
    {isNew
      ? <div className="callout" onClick={e => { if ((e.target as HTMLElement).closest('[data-local=browser]')) browser(); }}><Icon name="info" /><div><Rich k="am.callout" /></div></div>
      // Names hidden: no @username, and no user ID either (anyone can look an account up by it).
      : <div className="acc-head"><div className="avatar"><Avatar a={a} /></div>
        <div><b>{a.label}</b><div className="muted">{[a.username ? `@${a.username}` : '', hide ? '' : `ID ${a.userId}`].filter(Boolean).join(' · ')}</div></div></div>}
    <div className="form">
      {isNew ? <label>{t('am.cookie')}
        <div className="pw"><input className="field mono" type={shown.cookie ? 'text' : 'password'} name="cookie" maxLength={4096} placeholder="_|WARNING:-DO-NOT-SHARE-THIS…"
          spellCheck={false} value={cookie} onChange={typedIn('cookie', setCookie)} />{eye('cookie', cookie, setCookie)}</div></label> : null}
      <label>{t('am.alias')}{aliasHidden
        ? <input className="field" name="alias" maxLength={60} disabled placeholder={t('am.aliasHidden')} />
        : <input className="field" name="alias" maxLength={60} placeholder={t('am.aliasPh')} value={alias} onChange={e => setAlias(e.target.value)} />}</label>
      <label>{t('am.group')}<SuggestInput className="field" name="group" maxLength={40} placeholder={t('am.groupPh')} value={group} setValue={setGroup}
        options={groups} label={t('am.group')} /></label>
      <label><span>{t('am.pw')} <span className="muted">{t('am.pwHint')}</span></span>
        <div className="pw"><input className="field" type={shown.password ? 'text' : 'password'} name="password" maxLength={200}
          placeholder={!isNew && a.hasPassword ? t('am.pwSaved') : t('am.pwNone')} value={password} onChange={typedIn('password', setPassword)} />{eye('password', password, setPassword)}</div></label>
      {isNew ? null : <div className="totp">
        <span className="totp-label">{t('am.totp')} <span className="muted">{t('am.totpHint')}</span></span>
        <div className="totp-live" id="am-totp-live" ref={live} hidden={!liveShown}>
          <b className="totp-code" id="am-totp-code">{code || '··· ···'}</b><span className="totp-left" id="am-totp-left">{left}</span>
          <button type="button" className="btn sm" data-totp="copy" onClick={() => host.copyField(a.id, 'totp')}><Icon name="copy" />{t('am.totpCopy')}</button>
          <button type="button" className="btn sm ghost" data-totp="remove" onClick={remove}>{t('am.totpRemove')}</button>
        </div>
        <div className="pw" id="am-totp-box" hidden={liveShown}><input className="field mono" type={shown.authenticator ? 'text' : 'password'} name="authenticator" ref={box} maxLength={512}
          placeholder={t('am.totpPh')} spellCheck={false} autoComplete="off" aria-label={t('am.totp')} aria-describedby="am-totp-msg" value={key} onChange={e => {
            setKey(e.target.value);
            keyNow.current = e.target.value.trim();
            clearTimeout(typeWait.current);
            seq.current++;
            typeWait.current = window.setTimeout(showCode, 300);
          }} />{eye('authenticator', key, setKey)}</div>
        <p className={`totp-msg ${msg.kind}`} id="am-totp-msg" aria-live="polite">{msg.node}</p>
        <p className="totp-note">{t('am.totpNote')}</p>
      </div>}
      <label>{t('am.note')}<textarea className="field" name="note" maxLength={500} rows={2} value={note} onChange={e => setNote(e.target.value)} /></label>
    </div>
    <p className="fine"><Icon name="shield" /><span>{t('am.fine')}</span></p>
    <div className="form-error" role="alert" hidden={!error} ref={errorLine}>{error}</div>
  </>;
}
