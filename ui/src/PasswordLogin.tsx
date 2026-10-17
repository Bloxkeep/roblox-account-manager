// The sign-in screen: username and password, with the other ways (phone, browser, cookie, a vault file) around it.
// Works on a PC with no phone and no roblox.com: the values go only to Roblox's (or Roblox VNG's) official sign-in
// host, over HTTPS. It handles a 2-step code (authenticator, email or SMS; a code filled in from a key saved here), a
// captcha (shown in a window of its own, for the person to solve), and "Try again" when Roblox signed in but the account
// couldn't be looked up. Several accounts can be added in a row. app.js's dialog keeps its frame and what closing does
// (the app drops the half-done sign-in; what was added gets selected); a way picked closes it and opens that way.
import { Fragment, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type FormEvent, type ReactNode, type Ref } from 'react';
import { Icon, useHost } from './host';

const METHOD_LABEL: Record<string, string> = { authenticator: 'pw.auth', email: 'pw.email', sms: 'pw.sms' };
const RESEND_WAIT = 30_000;
type Added = { id: string; label: string; merged?: boolean };
type Reply = { status: string; error?: string; seconds?: number; methods?: string[]; media?: string; autofill?: boolean; added?: Added; closed?: boolean };
type Step = 'login' | 'twostep' | 'captcha' | 'finish';
export interface PasswordLoginApi {
  added(): Added[];
  /** Esc or the X asks first: something typed, a request on its way, or a sign-in half way (a code already sent). */
  asksToClose(): boolean;
  /** The dialog is closing: no more timers (the app drops the half-done sign-in). */
  stop(): void;
}

/** A sentence with a name in bold in its {name} ("Enter the 6-digit code for **farm1**."). */
const boldIn = (text: string, name: string) => text.split('\u0000').map((part, n) => <Fragment key={n}>{n ? <b>{name}</b> : null}{part}</Fragment>);

export function PasswordLogin({ forRealm, api, way }: {
  forRealm: string | null; api: Ref<PasswordLoginApi>; way: (way: string, realm: string) => void;
}) {
  const host = useHost();
  const { t, ask, busy, toast, store, call } = host;
  const [realm, setRealm] = useState(forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : ''));
  const [step, setStep] = useState<Step>('login');
  const [err, setErr] = useState('');
  const [user, setUser] = useState('');
  const [pass, setPass] = useState('');
  const [shown, setShown] = useState(false);
  const [code, setCode] = useState('');
  const [filledNote, setFilledNote] = useState(false);
  // The 2-step methods this account has that take a typed code (the app asks Roblox), the preferred one first.
  const [methods, setMethods] = useState(['authenticator', 'email']);
  const [method, setMethod] = useState('authenticator');
  // A sign-in request on its way: every button that would send another stays off until the answer, so one click never
  // sends the password twice.
  const [pending, setPending] = useState(false);
  const [sending, setSending] = useState(false);
  const [added, setAdded] = useState<Added[]>([]);
  const [, tick] = useState(0);
  // Each time the screen is drawn anew (a step, a realm, an account added) its first box takes the focus.
  const [drawn, setDrawn] = useState(0);
  // Email/SMS ways a code has gone out by (Roblox sends one by the preferred way itself) and when "Resend" works again
  // for each; when Verify (and Send) work again after "too many tries"; the authenticator code filled in from a key
  // saved here (never sent by itself, nor another after Roblox refused one). One code screen's, all of it.
  const s = useRef({ sent: new Set<string>(), resendAt: {} as Record<string, number>, verifyAt: 0, autofill: false, filled: '', fillTimer: 0, fillSeq: 0,
    pending: false, alive: true, added: [] as Added[], method: 'authenticator', code: '' });
  s.current.method = method;
  s.current.code = code;
  const box = useRef<Record<string, HTMLElement | null>>({});
  const ref = (k: string) => (el: HTMLElement | null) => { box.current[k] = el; };

  useImperativeHandle(api, () => ({
    added: () => s.current.added,
    asksToClose: () => s.current.pending || stepNow.current !== 'login' || !!(userNow.current.trim() || passNow.current),
    stop: () => { s.current.alive = false; clearTimeout(s.current.fillTimer); s.current.fillSeq++; },
  }), []);
  const stepNow = useRef(step), userNow = useRef(user), passNow = useRef(pass);
  stepNow.current = step; userNow.current = user; passNow.current = pass;

  // The countdowns ("Resend in 25 s", "Verify in 40 s") are drawn again each second while one runs.
  const now = Date.now();
  const verifyWait = Math.ceil((s.current.verifyAt - now) / 1000);
  const resendWait = Math.ceil(((s.current.resendAt[method] || 0) - now) / 1000);
  const counting = step === 'twostep' && (verifyWait > 0 || (s.current.sent.has(method) && resendWait > 0));
  useEffect(() => {
    if (!counting) return;
    let timer = 0;
    const next = () => { tick(n => n + 1); timer = window.setTimeout(next, 1000); };
    timer = window.setTimeout(next, 1000);
    return () => clearTimeout(timer);
  }, [counting]);
  // The puzzle and "Try again" screens have no box: their button takes the focus.
  useEffect(() => {
    const timer = setTimeout(() => {
      const first = step === 'login' && user ? box.current.pass : step === 'login' ? box.current.user
        : step === 'twostep' ? box.current.otp : step === 'captcha' ? box.current.solve : box.current.finish;
      first?.focus();
    }, 30);
    return () => clearTimeout(timer);
  }, [drawn]);
  // Back on the first screen the password goes with the attempt that used it (the username stays).
  const draw = (next: Step, error = '') => { setStep(next); setErr(error); if (next === 'login') setPass(''); setDrawn(n => n + 1); };

  // The buttons busy() also turns off and back on (its spinner): their state is put on them after every drawing. A
  // disabled prop alone isn't enough: React skips it when it didn't change, after busy() had turned the button back on
  // ("Resend in 30 s" or "Verify in 60 s" clickable).
  const wasSent = s.current.sent.has(method), held = s.current.verifyAt > now;
  useLayoutEffect(() => {
    const set = (k: string, off: boolean) => { const b = box.current[k] as HTMLButtonElement | null; if (b && !b.classList.contains('busy')) b.disabled = off; };
    set('go', pending);
    set('solve', pending);
    set('finish', pending);
    set('verify', pending || verifyWait > 0);
    // Off while a send is on its way too, so flipping tabs fast can't send a second one.
    set('send', sending || held || (wasSent && resendWait > 0));
    document.querySelectorAll<HTMLButtonElement>('#pw-body [data-realm]').forEach(b => { b.disabled = pending; });
  });

  /** Leaving a code screen: what was sent, and the waits, belong to that sign-in, not to the next one. */
  const resetTwoStep = () => {
    s.current.sent.clear();
    s.current.resendAt = {};
    s.current.verifyAt = 0;
    s.current.autofill = false;
    s.current.filled = '';
    clearTimeout(s.current.fillTimer);
    s.current.fillSeq++;
    setCode('');
    setFilledNote(false);
  };
  useEffect(() => () => { s.current.alive = false; clearTimeout(s.current.fillTimer); }, []);

  /** The saved key's code in the box, following it as it changes while the box is untouched. */
  const fillCode = async () => {
    clearTimeout(s.current.fillTimer);
    const seq = ++s.current.fillSeq;
    const untouched = () => !s.current.code || s.current.code === s.current.filled;
    if (!s.current.autofill || s.current.method !== 'authenticator' || !untouched()) return;
    let r: { code: string; seconds: number } | undefined;
    try { r = await ask('loginTwoStepCode') as typeof r; } catch { return; }
    if (seq !== s.current.fillSeq || !s.current.alive || !r?.code || s.current.method !== 'authenticator' || !untouched()) return;
    s.current.filled = r.code;
    s.current.code = r.code;
    setCode(r.code);
    setFilledNote(true);
    s.current.fillTimer = window.setTimeout(fillCode, Math.max(1, r.seconds) * 1000 + 300);
  };

  /** One sign-in request at a time. */
  const request = async (btn: HTMLElement | null, cmd: string, args: Record<string, unknown>) => {
    if (s.current.pending) return undefined;
    try {
      return await busy(btn!, () => { s.current.pending = true; setPending(true); return ask(cmd, args); }) as Reply | undefined;
    } finally {
      s.current.pending = false;
      setPending(false);
    }
  };

  const handle = (r: Reply | undefined) => {
    if (!r || !s.current.alive) return;
    // The attempt is over (wrong password, account locked, too many tries), or what it asked for was answered and then
    // failed: back to the first step with the reason, the username kept.
    if (r.status === 'login') { resetTwoStep(); return draw('login', r.error || ''); }
    // Signed in, but the account couldn't be looked up: "Try again" only does that.
    if (r.status === 'finish') { resetTwoStep(); return draw('finish', r.error || ''); }
    // Too many codes tried: the code screen stays, Verify waits a minute.
    if (r.status === 'wait') {
      s.current.verifyAt = Date.now() + (r.seconds || 60) * 1000;
      const hadFocus = box.current.verify === document.activeElement;
      setErr(r.error || '');
      tick(n => n + 1);
      // Verify turns itself off while it has the focus: the code box takes it, ready for the next code.
      const dialog = box.current.verify?.closest('.overlay');
      if (hadFocus || !dialog?.contains(document.activeElement)) box.current.otp?.focus();
      return;
    }
    if (r.status === 'captcha') {
      // Solved, and Roblox wants another one (it can chain them): say so, or it looks like nothing happened.
      return draw('captcha', stepNow.current === 'captcha' ? t('pw.captchaAgain') : '');
    }
    if (r.status === 'twostep') {
      resetTwoStep();
      const known = (r.methods || []).filter(x => METHOD_LABEL[x]);
      const list = known.length ? known : ['authenticator', 'email'];
      // Roblox may already have sent the code a given way (e.g. by email): start on that tab, and it sends the code by
      // the preferred way (or the one it named) as soon as it asks for it.
      const first = list.includes(r.media || '') ? r.media! : list[0];
      if (first === 'email' || first === 'sms') { s.current.sent.add(first); s.current.resendAt[first] = Date.now() + RESEND_WAIT; }
      s.current.autofill = !!r.autofill;
      s.current.method = first;
      setMethods(list);
      setMethod(first);
      draw('twostep');
      fillCode(); // on the authenticator tab (now or once picked), with the key saved here
      return;
    }
    if (r.status === 'added') {
      const a = r.added!;
      s.current.added = [...s.current.added, a];
      setAdded(s.current.added);
      toast(t(a.merged ? 't.merged' : 't.added', { name: a.label }), 'ok');
      resetTwoStep();
      setUser('');
      setPass('');
      draw('login');
    }
  };

  const submitLogin = async (e: FormEvent) => {
    e.preventDefault();
    const name = user.trim();
    setUser(name);
    if (!name || !pass) return setErr(t('pw.need'));
    setErr('');
    let r: Reply | undefined;
    try { r = await request(box.current.go, 'loginPassword', { username: name, password: pass, realm }); } catch (ex) {
      setErr((ex as Error).message);
      // Typing again replaces the refused password instead of adding to it.
      const p = box.current.pass as HTMLInputElement | null;
      p?.focus();
      p?.select();
      return;
    }
    handle(r);
  };
  const submitTwoStep = async (e: FormEvent) => {
    e.preventDefault();
    const typed = code.trim();
    if (!typed) return setErr(t('pw.needCode'));
    setErr('');
    let r: Reply | undefined;
    try { r = await request(box.current.verify, 'loginTwoStep', { method, code: typed }); } catch (ex) { return setErr((ex as Error).message); }
    handle(r);
  };
  const sendCode = async (via = method) => {
    const btn = box.current.send as HTMLButtonElement | null;
    if (!btn || btn.disabled || sending) return;
    setErr('');
    let r: Reply | undefined;
    setSending(true);
    try { r = await busy(btn, () => ask('loginTwoStepSend', { method: via })) as Reply | undefined; } catch (ex) { return setErr((ex as Error).message); } finally { setSending(false); }
    if (r?.status !== 'sent') return handle(r);
    s.current.sent.add(via);
    s.current.resendAt[via] = Date.now() + RESEND_WAIT;
    tick(n => n + 1);
  };
  const solveOrFinish = async (btn: HTMLElement, cmd: string) => {
    setErr('');
    let r: Reply | undefined;
    try { r = await request(btn, cmd, {}); } catch (ex) { return setErr((ex as Error).message); }
    // The puzzle window was closed: "Solve" asks for a fresh one. Any other answer moves on (or is a new puzzle).
    if (r?.status === 'captcha' && r.closed) return;
    handle(r);
  };

  const errorLine = <div className="form-error" id="pw-err" hidden={!err}>{err}</div>;
  // Back ends this attempt in the app too: the password goes, and a reply still on its way is dropped.
  const back = <button type="button" className="auth-back" data-back onClick={() => { call('loginCancel', {}, { silent: true }); resetTwoStep(); draw('login'); }}>{t('pw.back')}</button>;
  const wayButton = (name: string, words: string, extra: { className?: string; title?: string } = {}) =>
    <button type="button" className={extra.className ?? 'btn sm ghost'} data-x2={name} title={extra.title} onClick={() => way(name, realm)}>{words}</button>;
  let screen: ReactNode;

  if (step === 'captcha' || step === 'finish') {
    const captcha = step === 'captcha';
    screen = <>
      <div className="auth-step">
        <span className="auth-badge"><Icon name={captcha ? 'shield' : 'shieldCheck'} /></span>
        <h4>{t(captcha ? 'pw.captchaTitle' : 'pw.finishTitle')}</h4>
        <p>{t(captcha ? 'pw.captcha' : 'pw.finish')}</p>
      </div>
      <div className="auth-acts">
        {errorLine}
        <button type="button" className="btn go w" id={captcha ? 'pw-solve' : 'pw-finish'} ref={ref(captcha ? 'solve' : 'finish')}
          onClick={e => solveOrFinish(e.currentTarget, captcha ? 'loginCaptcha' : 'loginFinish')}>{t(captcha ? 'pw.solve' : 'pw.retry')}</button>
        {back}
      </div>
    </>;
  } else if (step === 'twostep') {
    screen = <>
      <div className="auth-step">
        <span className="auth-badge"><Icon name="shieldCheck" /></span>
        <h4>{t('pw.twoTitle')}</h4>
        <p>{boldIn(t('pw.twoLead', { name: '\u0000' }), user)}</p>
      </div>
      {methods.length > 1 ? <div className="seg wide" role="tablist">
        {methods.map(x => <button key={x} type="button" role="tab" data-m={x} className={method === x ? 'on' : ''} aria-selected={method === x} onClick={() => {
          setMethod(x);
          s.current.method = x;
          // Email and SMS codes go out on their own, as on roblox.com: another way gets one sent when it's picked.
          if (x !== 'authenticator' && !s.current.sent.has(x)) setTimeout(() => sendCode(x));
          // A filled-in authenticator code isn't an email or SMS code: it goes with another way, and comes back with its own.
          if (x !== 'authenticator' && s.current.filled && s.current.code === s.current.filled) { setCode(''); s.current.code = ''; setFilledNote(false); }
          fillCode();
          box.current.otp?.focus();
        }}>{t(METHOD_LABEL[x])}</button>)}
      </div> : null}
      <form id="pw-two" className="auth-form" autoComplete="off" onSubmit={submitTwoStep}>
        <input className="field auth-code" id="pw-otp" ref={ref('otp')} inputMode="numeric" maxLength={40} autoComplete="one-time-code" placeholder="000000" aria-label={t('pw.code')}
          value={code} onChange={e => {
            // A pasted "123 456" or "123-456" keeps its six digits; letters can't be typed.
            const digits = e.target.value.replace(/\D/g, '').slice(0, 6);
            setCode(digits);
            s.current.code = digits;
            // Typed over: the person's own code now, left alone.
            if (digits !== s.current.filled) setFilledNote(false);
          }} />
        <p className="auth-hint" id="pw-filled" hidden={!filledNote}>{t('pw.filled')}</p>
        <div className="auth-sendrow" id="pw-sendrow" hidden={method === 'authenticator'}>
          <span className="auth-sentnote" id="pw-sentnote">{wasSent ? t(method === 'sms' ? 'pw.sentSms' : 'pw.sentEmail') : ''}</span>
          <button type="button" className="link auth-send" id="pw-send" ref={ref('send')} onClick={() => sendCode()}>
            {!wasSent ? t('pw.send') : resendWait > 0 ? t('pw.resendIn', { s: resendWait }) : t('pw.resend')}</button>
        </div>
        {errorLine}
        <button className="btn go w" type="submit" id="pw-verify" ref={ref('verify')}>
          {verifyWait > 0 ? t('pw.verifyIn', { s: verifyWait }) : t('pw.verify')}</button>
        {back}
      </form>
    </>;
  } else {
    // The phone comes first (no password, no puzzle); the browser sign-in is roblox.com only, so it isn't offered for
    // Roblox VNG.
    screen = <>
      <div className="auth-head"><img src={host.logo} alt="" /><h3>{t('pw.title')}</h3></div>
      <div className="seg wide" role="tablist">
        {[['', 'Roblox'], ['vng', 'Roblox VNG']].map(([r, words]) => <button key={r} type="button" role="tab" data-realm={r} className={realm === r ? 'on' : ''}
          aria-selected={realm === r} onClick={() => {
            if (pending || r === realm) return; // the answer on its way is for the realm it was sent to
            setRealm(r);
            store.set('rm.qlRealm', r);
            setShown(false); // what was typed stays, hidden again
            setDrawn(n => n + 1);
          }}>{words}</button>)}
      </div>
      {added.length ? <div className="auth-added"><Icon name="check" /><span>{t('pw.addedList', { names: added.map(x => x.label).join(', ') })}</span></div> : null}
      <div className="auth-phone">
        <button type="button" className="start-opt rec" data-x2="phone" onClick={() => way('phone', realm)}><Icon name="qr" />
          <span><b>{t('start.phone')}<em className="rec-tag">{t('start.rec')}</em></b><small>{t('start.phoneSub')}</small></span><Icon name="chevronRight" /></button>
      </div>
      <div className="auth-or"><span>{t('pw.orPw')}</span></div>
      <form id="pw-login" className="auth-form tight" autoComplete="off" onSubmit={submitLogin}>
        <label className="auth-label">{t('pw.user')}
          <span className="lead-ic"><Icon name="user" /><input className="field" id="pw-user" ref={ref('user')} maxLength={60} autoComplete="off" spellCheck={false}
            value={user} onChange={e => setUser(e.target.value)} /></span></label>
        <label className="auth-label">{t('pw.pass')}
          <span className="pw lead-ic"><Icon name="lock" /><input className="field" type={shown ? 'text' : 'password'} id="pw-pass" ref={ref('pass')} maxLength={200} autoComplete="off"
            value={pass} onChange={e => setPass(e.target.value)} />
            <button type="button" className="icon-btn" id="pw-eye" title={t(shown ? 'lock.hidePw' : 'lock.showPw')} aria-label={t(shown ? 'lock.hidePw' : 'lock.showPw')}
              aria-pressed={shown} onClick={() => setShown(!shown)}><Icon name={shown ? 'eyeOff' : 'eye'} /></button></span></label>
        {errorLine}
        <button className="btn w" type="submit" id="pw-go" ref={ref('go')}>{t('pw.go')}</button>
        <p className="auth-hint">{t('pw.extra')}</p>
      </form>
      <div className="auth-more">
        <div className="ways"><span>{t('start.more')}</span>{realm === 'vng' ? null : wayButton('browser', t('pw.withBrowser'))}{wayButton('cookie', t('pw.cookie'))}</div>
        <div className="ways"><span>{t('start.importFrom')}</span>{wayButton('vault', t('pw.vault'), { title: t('start.vaultTip') })}</div>
      </div>
      <p className="auth-note"><Icon name="lock" /><span>{t('pw.note')}</span></p>
    </>;
  }
  return <div id="pw-body">{screen}</div>;
}
