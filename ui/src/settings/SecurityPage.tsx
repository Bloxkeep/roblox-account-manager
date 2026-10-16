// Settings › Security: changing the master password. The boxes keep what is typed themselves (not React's state):
// app.js empties them when Settings is left (the "clearPassword" signal) and, rebuilding the page in another language,
// puts back what was typed and says so with an input event.
import { useEffect, useRef, useState } from 'react';
import { useHost } from '../host';
import { Group, Page } from './parts';

export function SecurityPage({ title }: { title: string }) {
  const { t, ask, busy, toast, strength, on } = useHost();
  // Problems show under the form, next to the boxes they are about; while the new one is typed, its strength.
  const [msg, setMsg] = useState({ text: '', kind: 'err' });
  const pwMsg = (text = '', kind = 'err') => setMsg({ text, kind });
  const cur = useRef<HTMLInputElement>(null), next = useRef<HTMLInputElement>(null), next2 = useRef<HTMLInputElement>(null);
  const btn = useRef<HTMLButtonElement>(null);

  // Typed passwords don't stay in the page once Settings is left.
  useEffect(() => on('clearPassword', () => {
    for (const box of [cur, next, next2]) if (box.current) box.current.value = '';
    setMsg({ text: '', kind: '' });
  }), [on]);

  const box = (id: string, key: string, auto: string, ref: typeof cur, onInput: (value: string) => void) =>
    <input className="field" type="password" id={id} placeholder={t(key)} aria-label={t(key)} maxLength={256} autoComplete={auto} ref={ref}
      onInput={e => onInput(e.currentTarget.value)} />;
  const dropError = () => { if (msg.text && msg.kind === 'err') pwMsg(''); };

  return <Page id="security" title={title}>
    <Group id="master" title={t('set.master')}>
      <form className="pw-form" id="pw-form" autoComplete="off" onSubmit={async e => {
        e.preventDefault();
        const c = cur.current!, n = next.current!, n2 = next2.current!;
        // Checked in the order the boxes are filled.
        const fail = (key: string, el: HTMLInputElement) => { pwMsg(t(key)); el.focus(); };
        if (!c.value) return fail('set.pwNeedCur', c);
        if (n.value.length < 6) return fail('set.pwShort', n);
        if (!n.value.trim()) return fail('lock.blank', n);
        if (n.value === c.value) return fail('set.pwSame', n);
        if (n.value !== n2.value) return fail('set.pwMismatch', n2);
        pwMsg('');
        let ok = false;
        await busy(btn.current!, async () => {
          try {
            await ask('changePassword', { current: c.value, next: n.value });
            ok = true;
          } catch (ex) {
            pwMsg((ex as Error).message);
            c.select();
          }
        });
        if (ok) {
          for (const el of [c, n, n2]) el.value = '';
          pwMsg('', '');
          toast(t('set.pwDone'), 'ok');
        }
      }}>
        {box('pw-cur', 'set.pwCur', 'current-password', cur, dropError)}
        {box('pw-new', 'set.pwNew', 'new-password', next, v => pwMsg(v ? t('set.pwStrength', { s: t(`lock.strength.${strength(v)}`) }) : '', 'hint'))}
        {box('pw-new2', 'set.pwNew2', 'new-password', next2, dropError)}
        <button className="btn primary" type="submit" id="pw-btn" ref={btn}>{t('set.pwBtn')}</button>
        <p className={`pw-msg ${msg.text ? msg.kind : ''}`} id="pw-msg" role="alert">{msg.text}</p>
      </form>
    </Group>
  </Page>;
}
