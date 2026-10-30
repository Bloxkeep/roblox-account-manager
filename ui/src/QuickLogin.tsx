// Quick Login: Roblox's own way of adding an account from a signed-in phone. Bloxkeep shows Roblox's code (with its QR),
// the phone confirms, the account comes in, then a new code. VNG accounts confirm in the Roblox VNG app. app.js's dialog
// keeps its buttons (browser sign-in, roblox.com only; Done) and what closing does (forget the code, select what was added).
import { Fragment, useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { useAppState, useHost } from './host';

type Added = { id: string; label: string; merged?: boolean };
export interface QuickLoginApi {
  added(): Added[];
  /** The dialog is closing: no more polls or codes (the app forgets the code). */
  stop(): void;
}
type Phase = 'getting' | 'waiting' | 'linked' | 'adding' | 'cancelled' | 'expired' | 'error';
const mmss = (n: number) => `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`;

export function QuickLogin({ forName, forRealm, api, browserButton }: {
  forName: string; forRealm: string | null; api: Ref<QuickLoginApi>; browserButton: () => HTMLElement | null;
}) {
  const host = useHost();
  const { t, ask, toast, store } = host;
  const hide = !!useAppState().hideNames; // names hidden (Settings): the phone's account isn't named either
  const [realm, setRealm] = useState(forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : ''));
  // The tab picked now: a renewal (code expired, account added) runs from the poll of an older render, whose own realm
  // was the one at that code's start; reading it here keeps the VNG tab on VNG codes (review, 2026-10-30).
  const realmNow = useRef(realm);
  realmNow.current = realm;
  const [code, setCode] = useState('');
  const [qr, setQr] = useState('');
  const [phase, setPhase] = useState<Phase>('getting');
  const [error, setError] = useState('');
  const [linked, setLinked] = useState<{ name: string; picture: string | null }>({ name: '', picture: null });
  // When the code runs out: the seconds left are worked out from it (a ticker counting down by itself fell behind while
  // the window was hidden, its timers slowed); the ticker only draws them again.
  const [until, setUntil] = useState(0);
  const [, tick] = useState(0);
  const [added, setAdded] = useState<Added[]>([]);
  const st = useRef({ gen: 0, timer: 0, ticker: 0, expiredInRow: 0, alive: true, added: [] as Added[] });
  const stop = () => { clearTimeout(st.current.timer); clearInterval(st.current.ticker); };
  useImperativeHandle(api, () => ({ added: () => st.current.added, stop: () => { st.current.alive = false; stop(); } }), []);
  useEffect(() => () => { st.current.alive = false; stop(); }, []);
  // A declined or expired code can't be used any more: it goes, so nobody types it in.
  const clearCode = () => { setCode(''); setQr(''); };

  const start = async (forRealmNow = realmNow.current) => {
    const my = ++st.current.gen;
    stop();
    clearCode();
    setPhase('getting');
    let r: { code: string; qr: string | null; seconds: number };
    try {
      r = await ask('quickLoginStart', { realm: forRealmNow }) as typeof r;
    } catch (e) {
      if (my === st.current.gen && st.current.alive) { setError((e as Error).message); setPhase('error'); }
      return;
    }
    if (my !== st.current.gen || !st.current.alive) return;
    setCode(r.code);
    setQr(r.qr || '');
    setUntil(Date.now() + r.seconds * 1000);
    setPhase('waiting');
    st.current.ticker = window.setInterval(() => tick(n => n + 1), 1000);
    st.current.timer = window.setTimeout(() => poll(my), 2500);
  };

  const poll = async (my: number) => {
    let r: { status: string; added?: Added; name?: string; picture?: string | null; seconds?: number };
    try {
      r = await ask('quickLoginPoll', {}) as typeof r;
    } catch (e) {
      if (my !== st.current.gen || !st.current.alive) return;
      // That code is spent: it goes with the error, as it does when it is declined or runs out.
      clearInterval(st.current.ticker);
      clearCode();
      setError((e as Error).message);
      setPhase('error');
      return;
    }
    if (my !== st.current.gen || !st.current.alive) return;
    switch (r.status) {
      case 'added': {
        const a = r.added!;
        st.current.added = [...st.current.added, a];
        setAdded(st.current.added);
        toast(t(a.merged ? 't.merged' : 't.added', { name: a.label }), 'ok');
        st.current.expiredInRow = 0;
        return start();
      }
      case 'linked':
        st.current.expiredInRow = 0;
        setLinked({ name: r.name || '', picture: r.picture || null });
        setPhase('linked');
        break;
      case 'adding':
        setPhase('adding');
        break;
      case 'cancelled':
        // Nothing counts down any more: the ticker stops with the code.
        clearInterval(st.current.ticker);
        clearCode();
        return setPhase('cancelled');
      case 'expired':
      case 'none':
        // Renew quietly a few times; after that stop asking Roblox until the person comes back.
        if (++st.current.expiredInRow <= 3) return start();
        clearInterval(st.current.ticker);
        clearCode();
        return setPhase('expired');
      default:
        if (typeof r.seconds === 'number') setUntil(Date.now() + r.seconds * 1000);
        setPhase('waiting');
    }
    st.current.timer = window.setTimeout(() => poll(my), 2500);
  };
  useEffect(() => { start(); }, []); // the first code as the dialog opens
  useEffect(() => { const b = browserButton(); if (b) b.hidden = realm === 'vng'; }, [realm]);

  const again = () => { st.current.expiredInRow = 0; start(); };
  const newCode = <button className="btn sm" data-new onClick={again}>{t('ql.newCode')}</button>;
  const status: [ReactNode, string] = phase === 'getting' ? [<><span className="spin" /><span>{t('ql.getting')}</span></>, '']
    : phase === 'waiting' ? [<><span className="spin" /><span>{t('ql.waiting', { time: mmss(Math.max(0, Math.ceil((until - Date.now()) / 1000))) })}</span></>, '']
    : phase === 'linked' ? [<>{linked.picture ? <img className="ql-pic" src={linked.picture} alt="" /> : <span className="spin" />}
      <span>{hide ? t('ql.linkedHidden') : t('ql.linked', { name: linked.name })}</span></>, '']
    : phase === 'adding' ? [<><span className="spin" /><span>{t('ql.adding')}</span></>, '']
    : phase === 'cancelled' ? [<><span>{t('ql.cancelled')}</span>{newCode}</>, 'bad']
    : phase === 'expired' ? [<><span>{t('ql.expired')}</span>{newCode}</>, '']
    : [<><span>{error}</span>{newCode}</>, 'bad'];

  return <>
    {/* Signing a signed-out account in again: its name at the top, in bold. */}
    {forName ? <p className="ql-for">{t('ql.for', { name: '\u0000' }).split('\u0000').map((part, n) => <Fragment key={n}>{n ? <b>{forName}</b> : null}{part}</Fragment>)}</p> : null}
    <div className="ql-realm" role="tablist">
      {[['', 'Roblox'], ['vng', 'Roblox VNG']].map(([r, words]) =>
        <button key={r} role="tab" data-realm={r} className={realm === r ? 'on' : ''} aria-selected={realm === r} onClick={() => {
          if (r === realm) return;
          setRealm(r);
          realmNow.current = r;
          store.set('rm.qlRealm', r);
          st.current.expiredInRow = 0;
          start(r);
        }}>{words}</button>)}
    </div>
    <div className="ql">
      <div className="ql-qr" id="ql-qr">{qr ? <img src={qr} alt="QR" /> : null}</div>
      <div className="ql-side">
        <div className="ql-code" id="ql-code">{code || '······'}</div>
        <ol className="ql-steps"><li id="ql-step1">{t(realm === 'vng' ? 'ql.step1Vng' : 'ql.step1')}</li><li>{t('ql.step2')}</li><li>{t('ql.step3')}</li></ol>
      </div>
    </div>
    <div className={`ql-status ${status[1]}`} id="ql-status">{status[0]}</div>
    <div className="ql-added" id="ql-added">{added.length ? t('ql.addedList', { names: added.map(x => x.label).join(', ') }) : ''}</div>
  </>;
}

