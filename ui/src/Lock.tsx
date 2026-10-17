// The lock screen: the master password to open the vault, or, the first time, a new one (typed twice, its strength
// shown) with "Restore a backup" under it. app.js decides which and when (renderLock) and what the opened vault does
// (applyState); this draws it, checks what was typed and asks the app. The boxes keep what is typed themselves: app.js
// reads them back when the screen is drawn again in another language.
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react';
import { Icon, useHost, type AppState } from './host';
import { Rich } from './rich';

export type LockMode = 'create' | 'unlock';

/** The language buttons (app.js's click handler changes the language: data-act="lang"). */
export function LangSwitch() {
  const { lang, tIn } = useHost();
  return <div className="lang" role="group" aria-label="Language">{(['en', 'vi'] as const).map(l =>
    <button key={l} data-act="lang" data-lang={l} className={l === lang ? 'on' : ''} aria-pressed={l === lang} title={tIn(l, 'lang.name')}>{l.toUpperCase()}</button>)}</div>;
}

export function Lock({ mode, notice, kept, opened }: {
  mode: LockMode; notice: string; kept: { pw1: string; pw2: string } | null; opened: (state: AppState) => void;
}) {
  const { t, logo, ask, busy, strength } = useHost();
  const create = mode === 'create';
  const form = useRef<HTMLFormElement>(null), pw1 = useRef<HTMLInputElement>(null), pw2 = useRef<HTMLInputElement>(null);
  const btn = useRef<HTMLButtonElement>(null), meter = useRef<HTMLElement>(null), label = useRef<HTMLSpanElement>(null);
  const [error, setError] = useState('');
  const [shown, setShown] = useState(false);
  const [caps, setCaps] = useState(false);

  /** A master password's strength as a bar and a word (app.js showStrength): only a guide, any 6+ characters do. */
  const showStrength = (v: string) => {
    const score = [v.length >= 8, v.length >= 12, /[a-z]/.test(v) && /[A-Z]/.test(v), /\d/.test(v), /[^A-Za-z0-9]/.test(v)].filter(Boolean).length;
    const level = strength(v);
    meter.current!.style.width = `${v ? Math.max(12, score * 20) : 0}%`;
    meter.current!.style.background = `var(--${level === 'weak' ? 'bad' : level === 'ok' ? 'warn' : 'ok'})`;
    label.current!.textContent = v ? t(`lock.strength.${level}`) : '';
    label.current!.className = `pw-strength ${v ? level : ''}`;
  };
  useEffect(() => {
    // Drawn again in another language (the same screen): what was typed stays.
    if (kept) {
      pw1.current!.value = kept.pw1;
      if (pw2.current) pw2.current.value = kept.pw2;
      if (create) showStrength(kept.pw1);
    }
    const timer = setTimeout(() => pw1.current?.focus(), 50);
    return () => clearTimeout(timer);
  }, []);

  const fail = (msg: string) => {
    setError(msg);
    const f = form.current!;
    f.classList.remove('shake');
    void f.offsetWidth; // starts the shake again
    f.classList.add('shake');
  };
  const submit = async (e: React.FormEvent) => {
    e.preventDefault();
    const a = pw1.current!, b = pw2.current;
    if (create) {
      if (a.value.length < 6) return fail(t('lock.short'));
      if (!a.value.trim()) return fail(t('lock.blank'));
      if (a.value !== b!.value) return fail(t('lock.mismatch'));
    } else if (!a.value) {
      return fail(t('lock.empty'));
    }
    setError('');
    let state: AppState | undefined;
    await busy(btn.current!, async () => {
      try {
        state = await ask(create ? 'createVault' : 'unlock', { password: a.value }) as AppState;
      } catch (ex) {
        fail((ex as Error).message);
        a.select();
      }
    });
    if (state) {
      a.value = '';
      if (b) b.value = '';
      opened(state);
    }
  };
  // An old error goes once the password is being typed again; Caps Lock is pointed out while it is on.
  const typing = { onInput: () => setError(''), onKeyDown: (e: React.KeyboardEvent) => setCaps(!!e.getModifierState?.('CapsLock')), onKeyUp: (e: React.KeyboardEvent) => setCaps(!!e.getModifierState?.('CapsLock')) };
  const eye = t(shown ? 'lock.hidePw' : 'lock.showPw');

  return <div className="lock">
    <div className="dots" />
    <div className="lock-lang"><LangSwitch /></div>
    <form className="lock-card" id="lock-form" autoComplete="off" data-mode={mode} ref={form} onSubmit={submit}>
      <img className="logo-img" src={logo} alt="" />
      <h2>{t(create ? 'lock.createTitle' : 'lock.unlockTitle')}</h2>
      <p><Rich k={create ? 'lock.createText' : 'lock.unlockText'} /></p>
      {notice ? <p className="lock-paused">{notice}</p> : null}
      <div className="stack">
        <div className="pw"><input className="field" type={shown ? 'text' : 'password'} id="pw1" ref={pw1} placeholder={t('lock.pw')} maxLength={256} aria-label={t('lock.pw')}
          {...typing} onInput={() => { setError(''); if (create) showStrength(pw1.current!.value); }} />
          {/* Before it is first pressed it reads "Show password", as app.js drew it. */}
          <button type="button" className="icon-btn" id="pw-eye" title={eye} aria-label={eye} aria-pressed={shown} onClick={() => setShown(!shown)}><Icon name={shown ? 'eyeOff' : 'eye'} /></button></div>
        {create ? <>
          <input className="field" type={shown ? 'text' : 'password'} id="pw2" ref={pw2} placeholder={t('lock.pw2')} maxLength={256} aria-label={t('lock.pw2')} {...typing} />
          <div className="strength-row"><div className="strength"><i id="pw-meter" ref={meter} /></div><span id="pw-strength" ref={label} aria-live="polite" /></div>
        </> : null}
      </div>
      <div className="lock-caps" id="lock-caps" hidden={!caps}><Icon name="alert" />{t('lock.caps')}</div>
      <div className="lock-error" id="lock-error" role="alert">{error}</div>
      <button className="btn primary big w" type="submit" id="lock-btn" ref={btn}>{t(create ? 'lock.create' : 'lock.unlock')}</button>
      {create ? <div className="lock-alt"><button type="button" className="link" data-act="restore-backup">{t('lock.restore')}</button></div> : null}
    </form>
  </div>;
}

/** The loading screen right after unlocking (app.js showBoot): the logo, a bar the app's real steps move, the step. */
export interface BootApi { paint(text: string, value: number): void }
export function Boot({ api }: { api: Ref<BootApi> }) {
  const { logo } = useHost();
  const [at, setAt] = useState({ text: '', value: 0.05 });
  useImperativeHandle(api, () => ({ paint: (text, value) => setAt({ text, value }) }), []);
  return <div className="boot" id="boot" role="progressbar" aria-valuemin={0} aria-valuemax={100} aria-valuenow={Math.round(at.value * 100)}><img src={logo} alt="" /><b>Bloxkeep</b>
    <div className="boot-bar"><i id="boot-fill" style={{ transform: `scaleX(${Math.min(1, at.value).toFixed(3)})` }} /></div><span id="boot-text">{at.text}</span></div>;
}
