// The lock screen: the master password to open the vault, or, the first time (or for a vault being added), a new one
// (typed twice, its strength shown) with "Restore a backup" under it; with several vaults, which one it is and Switch
// vault, or the picker itself (Vaults.tsx). app.js decides which and when (renderLock) and what the opened vault does
// (applyState); this draws it, checks what was typed and asks the app. The boxes keep what is typed themselves: app.js
// reads them back when the screen is drawn again in another language.
import { useEffect, useImperativeHandle, useRef, useState, type Ref } from 'react';
import { Icon, useAppState, useHost, type AppState } from './host';
import { Rich } from './rich';
import { VaultFace, VaultPicker, type VaultInfo } from './Vaults';

export type LockMode = 'create' | 'unlock' | 'pick';

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
  const app = useAppState();
  const multiOnly = !!app.multiAtStart;
  // Which vault this is, once there are vaults (or one is being added); its name is typed while it is being added.
  const vaults = app.vaults || {};
  const adding = !!vaults.adding;
  // Shown once there is a choice (two vaults or more, or one being added); with one vault the screen is as it always was.
  const vault: VaultInfo | null = vaults.current && ((vaults.list || []).length > 1 || adding)
    ? { id: vaults.current, name: vaults.name || '', picture: vaults.picture || null } : null;
  const form = useRef<HTMLFormElement>(null), pw1 = useRef<HTMLInputElement>(null), pw2 = useRef<HTMLInputElement>(null);
  const nameBox = useRef<HTMLInputElement>(null);
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
    const f = form.current;
    if (!f) return; // drawn again meanwhile (another language): this screen is gone
    setError(msg);
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
        state = await ask(create ? 'createVault' : 'unlock', create ? { password: a.value, name: nameBox.current?.value || '' } : { password: a.value }) as AppState;
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

  if (mode === 'pick') return <div className="lock vault-pick">
    <div className="dots" />
    <div className="lock-lang"><LangSwitch /></div>
    <VaultPicker />
  </div>;

  return <div className="lock">
    <div className="dots" />
    <div className="lock-lang"><LangSwitch /></div>
    <form className="lock-card" id="lock-form" autoComplete="off" data-mode={mode} ref={form} onSubmit={submit}>
      <img className="logo-img" src={logo} alt="" />
      {vault ? <div className="lock-vault"><VaultFace vault={vault} className="sm" /><b title={vault.name}>{vault.name}</b>
        {adding ? <button type="button" className="link" data-act="vault-cancel-add">{t('common.cancel')}</button>
          : <button type="button" className="link" data-act="vault-pick">{t('vault.switch')}</button>}</div> : null}
      <h2>{t(adding ? 'vault.newTitle' : create ? 'lock.createTitle' : 'lock.unlockTitle')}</h2>
      <p><Rich k={adding ? 'vault.newText' : create ? 'lock.createText' : 'lock.unlockText'} /></p>
      {notice ? <p className="lock-paused">{notice}</p> : null}
      <div className="stack">
        {adding ? <input className="field" id="vault-name" ref={nameBox} defaultValue={vault?.name || ''} maxLength={40} placeholder={t('vault.name')}
          aria-label={t('vault.name')} spellCheck={false} /> : null}
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
      {/* Set up: a backup instead. Unlock: for a forgotten master password (it can't be recovered), a backup whose
          password is remembered, or another vault (this one stays on the list until it is removed in the picker). */}
      <div className="lock-alt"><button type="button" className="link" data-act="restore-backup">{t('lock.restore')}</button>
        {create ? null : <><span className="sep">·</span><button type="button" className="link" data-act="vault-add">{t('vault.new')}</button></>}</div>
      {/* For someone who signs in to Roblox only in their own browser: Multi-Roblox without a vault or accounts. */}
      {create && !adding ? <div className="lock-alt lock-multi">{multiOnly
        ? <><span id="lock-multi-on">{t('lock.multiOnlyOn')}</span> <button type="button" className="link" data-act="multi-only-off">{t('lock.multiOnlyOff')}</button></>
        : <button type="button" className="link" data-act="multi-only">{t('lock.multiOnly')}</button>}</div> : null}
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
