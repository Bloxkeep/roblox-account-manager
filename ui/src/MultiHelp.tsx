// Multi-Roblox's dialog body: off, what it does and Turn on; on and working, a tick; blocked (Roblox was open before), the
// steps to clear it, giving way to the tick as soon as it works. Then, when on, how windows open. The footer is app.js's.
import { useEffect, useState } from 'react';
import { Icon, useAppState, useHost } from './host';
import { Rich } from './rich';

import { Button, Switch } from './kit';
export function MultiHelp({ enable }: { enable: () => void }) {
  const { t, tn, call, busy, toast } = useHost();
  const m = useAppState().multi || {};
  const [trayDone, setTrayDone] = useState(false);
  // The switches answer at once; the app's next state confirms (or puts them back).
  const [windowed, setWindowed] = useState(!!m.windowed);
  const [tile, setTile] = useState(!!m.tile);
  useEffect(() => setWindowed(!!m.windowed), [m.windowed]);
  useEffect(() => setTile(!!m.tile), [m.tile]);

  const flag = (id: string, key: string, on: boolean, set: (v: boolean) => void, setting: string) =>
    <label className="mh-flag"><span><b>{t(key)}</b><small>{t(`${key}Desc`)}</small></span>
      <Switch inside id={id} checked={on} onChange={e => { set(e.target.checked); call('setSettings', { [setting]: e.target.checked }); }} /></label>;

  return <>
    {!m.enabled ? <>
      <p className="modal-text">{t('mh.off')}</p>
      <Button kind="primary" onClick={enable}><Icon name="layers" />{t('mh.enable')}</Button>
    </> : m.armed ? <>
      <div className="hero-ok"><Icon name="check" /></div>
      <p className="modal-text center">{t('mh.ok1')}</p>
      <p className="muted center">{t('mh.ok2')}</p>
    </> : <>
      <p className="modal-text">{t('mh.bad')}</p>
      <ol className="steps">
        <li><div><Rich k="mh.s1" /><br /><Button size="sm" onClick={async e => {
          const n = await busy(e.currentTarget, () => call('closeAll')) as number | undefined;
          if (n !== undefined) toast(n ? tn('t.closingAll', n) : t('t.noRoblox'), 'ok');
        }}><Icon name="power" />{t('mh.s1btn')}</Button></div></li>
        {m.trayAutostart ? <li><div><Rich k="mh.s2" /><br /><Button size="sm" disabled={trayDone} onClick={async e => {
          if ((await busy(e.currentTarget, () => call('disableTrayAutostart'))) !== undefined) setTrayDone(true);
        }}>{t(trayDone ? 'mh.s2done' : 'mh.s2btn')}</Button></div></li> : null}
        <li><div><Rich k="mh.s3" /></div></li>
      </ol>
    </>}
    {m.enabled ? <>
      {flag('mh-windowed', 'mh.windowed', windowed, setWindowed, 'multiWindowed')}
      {flag('mh-tile', 'mh.tile', tile, setTile, 'multiTile')}
    </> : null}
  </>;
}
