// Accounts from another Bloxkeep vault file (another PC's, or a backup): pick the file, type that file's own master
// password, tick what to bring over. The app keeps the opened file; this page only ever sees names and states, never
// cookies or passwords. app.js's dialog keeps its Cancel and the main button, whose words and state this sets for each
// step (Choose file… / Open, or Wait N s after three wrong passwords / Import); the button asks this to go on (next).
// The password box keeps what is typed itself (not React's state), as the lock screen's does.
import { Fragment, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type ReactNode, type Ref } from 'react';
import { Icon, useAppState, useHost } from './host';
import { Rich } from './rich';
import { Avatar } from './Avatar';

type FileAccount = { key: number; label: string; username?: string; group?: string; realm?: string; cookieValid?: boolean | null; hasPassword?: boolean; exists?: boolean; avatarUrl?: string };
type Preview = { token: string; accounts: FileAccount[]; presets: number };
export type ImportResult = { added: number; updated: number; presets: number; presetsSkipped: number; ids?: string[] };
export interface ImportVaultApi { next(btn: HTMLButtonElement): void }

export function ImportVault({ api, primary, finished }: {
  api: Ref<ImportVaultApi>; primary: () => HTMLButtonElement | null; finished: (r: ImportResult) => void;
}) {
  const host = useHost();
  const { t, ask, busy } = host;
  const app = useAppState();
  const [step, setStep] = useState<'pick' | 'password' | 'preview'>('pick');
  const [file, setFile] = useState<{ name: string } | null>(null);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [error, setError] = useState('');
  const [ticked, setTicked] = useState<Set<number>>(new Set());
  const [presets, setPresets] = useState(false);
  const [shown, setShown] = useState(false);
  // After three wrong passwords the app makes each try wait: Open counts down meanwhile.
  const [waitUntil, setWaitUntil] = useState(0);
  const [, tick] = useState(0);
  const pw = useRef<HTMLInputElement>(null), root = useRef<HTMLDivElement>(null);
  const waiting = step === 'password' && waitUntil > Date.now();
  useEffect(() => {
    if (!waiting) return;
    const timer = setInterval(() => tick(n => n + 1), 500);
    return () => clearInterval(timer);
  }, [waiting]);

  // The main button follows the step: its words, and off with nothing ticked or while a wrong password waits.
  const n = preview ? preview.accounts.filter(a => ticked.has(a.key)).length : 0;
  useLayoutEffect(() => {
    const btn = primary();
    if (!btn || btn.classList.contains('busy')) return;
    const left = Math.ceil((waitUntil - Date.now()) / 1000);
    const text = step === 'pick' ? t('iv.choose') : step === 'password' ? (left > 0 ? t('iv.wait', { n: left }) : t('iv.open')) : t('iv.importBtn');
    if (btn.textContent !== text) btn.textContent = text;
    btn.disabled = step === 'password' ? left > 0 : step === 'preview' ? !n && !presets : false;
  });
  // Into the password step: its box takes the focus. Into the list: the password box that had it is gone, so Import
  // takes it (the first account when nothing is ticked).
  useEffect(() => {
    if (step !== 'password') return;
    const timer = setTimeout(() => pw.current?.focus(), 30);
    return () => clearTimeout(timer);
  }, [step, file]);
  useLayoutEffect(() => {
    if (step !== 'preview') return;
    const dialog = root.current?.closest('.overlay');
    if (dialog?.contains(document.activeElement)) return;
    const btn = primary();
    (btn?.disabled ? root.current?.querySelector<HTMLElement>('.iv-item input, #iv-presets') : btn)?.focus();
  }, [step, preview]);

  const pick = async (btn: HTMLElement) => {
    setError('');
    let r: { name: string; wait?: number } | undefined;
    try { r = await busy(btn, () => ask('vaultImportPick')) as typeof r; } catch (e) { return setError((e as Error).message); }
    if (!r) return; // the file dialog was cancelled: things stay as they were
    setFile(r);
    setPreview(null);
    setShown(false);
    setStep('password');
    setWaitUntil(r.wait ? Date.now() + r.wait * 1000 : 0);
  };
  const open = async (btn: HTMLElement) => {
    const password = pw.current!.value;
    if (!password) return setError(t('iv.needPw'));
    setError('');
    let r: (Preview & { status: string; error?: string; wait?: number }) | undefined;
    try { r = await busy(btn, () => ask('vaultImportOpen', { password })) as typeof r; } catch (e) { return setError((e as Error).message); }
    if (!r) return;
    if (r.status !== 'ok') {
      setError(r.error || '');
      pw.current!.select();
      pw.current!.focus();
      if (r.wait) setWaitUntil(Date.now() + r.wait * 1000);
      return;
    }
    setPreview(r);
    setTicked(new Set(r.accounts.filter(a => !a.exists).map(a => a.key)));
    setPresets(false);
    setStep('preview');
  };
  const apply = async (btn: HTMLElement) => {
    setError('');
    let r: ImportResult;
    try {
      r = await busy(btn, () => ask('vaultImportApply', { token: preview!.token, keys: preview!.accounts.filter(a => ticked.has(a.key)).map(a => a.key), presets })) as ImportResult;
    } catch (e) { return setError((e as Error).message); }
    finished(r);
  };
  const latest = useRef({ step, pick, open, apply });
  latest.current = { step, pick, open, apply };
  useImperativeHandle(api, () => ({
    next: btn => { const l = latest.current; (l.step === 'pick' ? l.pick : l.step === 'password' ? l.open : l.apply)(btn); },
  }), []);

  const errorLine = <div className="form-error" id="iv-err" role="alert" hidden={!error}>{error}</div>;
  const safe = <p className="fine"><Icon name="shield" /><span>{t('iv.safe')}</span></p>;
  const fileLine = file ? <div className="iv-file"><Icon name="folder" /><b title={file.name}>{file.name}</b>
    <button type="button" className="link" id="iv-other" onClick={e => pick(e.currentTarget)}>{t('iv.otherFile')}</button></div> : null;

  if (step === 'pick') return <div id="iv-body" ref={root}>
    <p className="modal-text"><Rich k="iv.intro" /></p>{safe}{errorLine}
  </div>;
  if (step === 'password') return <div id="iv-body" ref={root}>
    {fileLine}
    <div className="form"><label>{t('iv.pw')}
      <span className="pw"><input className="field" type={shown ? 'text' : 'password'} id="iv-pw" ref={pw} maxLength={256} autoComplete="off" spellCheck={false} />
        <button type="button" className="icon-btn" id="iv-eye" title={t(shown ? 'lock.hidePw' : 'lock.showPw')} aria-label={t(shown ? 'lock.hidePw' : 'lock.showPw')}
          aria-pressed={shown} onClick={() => setShown(!shown)}><Icon name={shown ? 'eyeOff' : 'eye'} /></button></span></label></div>
    {safe}{errorLine}
  </div>;

  // The list: names hidden, the file's accounts too, numbered by their place in the file.
  const list = preview!.accounts;
  const hide = !!app.hideNames;
  const signIn = (a: FileAccount) => a.cookieValid === true ? <span className="iv-ck ok">{t('iv.ckOk')}</span>
    : a.cookieValid === false ? <span className="iv-ck bad">{t('iv.ckBad')}</span> : <span className="iv-ck">{t('iv.ckUnknown')}</span>;
  return <div id="iv-body" ref={root}>
    {fileLine}
    {list.length ? <>
      <div className="iv-bar"><span id="iv-count">{t('iv.count', { n, total: list.length })}</span>
        <button type="button" className="link" id="iv-all" onClick={() => setTicked(new Set(list.map(a => a.key)))}>{t('iv.all')}</button>
        <button type="button" className="link" id="iv-none" onClick={() => setTicked(new Set())}>{t('iv.none')}</button></div>
      <div className="rc-list iv-list">{list.map(x => {
        const a = hide ? { ...x, label: t('hide.account', { n: x.key + 1 }), username: '' } : x;
        const parts: ReactNode[] = [a.username ? `@${a.username}` : '', a.group || '', signIn(a), a.hasPassword ? t('iv.pwSaved') : ''].filter(Boolean);
        return <label key={a.key} className="rc-item iv-item">
          <input type="checkbox" value={a.key} checked={ticked.has(a.key)} onChange={e => {
            const next = new Set(ticked);
            if (e.target.checked) next.add(a.key); else next.delete(a.key);
            setTicked(next);
          }} />
          <div className="avatar sm"><Avatar a={a} /></div>
          <span className="iv-name"><b title={a.label}>{a.label}</b>
            <small>{parts.map((p, i) => <Fragment key={i}>{i ? ' · ' : null}{p}</Fragment>)}</small></span>
          {a.realm === 'vng' ? <span className="tag vng" title={t('card.vngTip')}>VNG</span> : null}
          <span className={`state-pill ${a.exists ? 'off' : 'ok'}`}>{t(a.exists ? 'iv.exists' : 'iv.new')}</span>
        </label>;
      })}</div>
    </> : <p className="modal-text">{t('iv.empty')}</p>}
    {preview!.presets ? <label className="iv-presets"><input type="checkbox" id="iv-presets" checked={presets} onChange={e => setPresets(e.target.checked)} />
      <span><b>{t('iv.presets', { n: preview!.presets })}</b><small>{t('iv.presetsSub')}</small></span></label> : null}
    {list.some(a => a.exists) ? <p className="fine"><Icon name="info" /><span>{t('iv.existsNote')}</span></p> : null}
    {errorLine}
  </div>;
}

