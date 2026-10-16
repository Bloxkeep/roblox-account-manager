// Multi-Roblox: the body of its dialog. Off: what it does, and Turn on. On and working: a tick. On but blocked (Roblox
// was open before): the steps to clear it. Then, when on, how the windows open (windowed, tiled). It follows the app's
// state, so the steps give way to the tick as soon as Multi-Roblox works. The footer (Turn off, Done) is app.js's.
import { useEffect, useState } from 'react';
import { Icon, useAppState, useHost } from './host';
import { Rich } from './rich';

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
      <span className="switch"><input type="checkbox" id={id} checked={on} onChange={e => { set(e.target.checked); call('setSettings', { [setting]: e.target.checked }); }} /><span /></span></label>;

  return <>
    {!m.enabled ? <>
      <p className="modal-text">{t('mh.off')}</p>
      <button className="btn primary" onClick={enable}><Icon name="layers" />{t('mh.enable')}</button>
    </> : m.armed ? <>
      <div className="hero-ok"><Icon name="check" /></div>
      <p className="modal-text center">{t('mh.ok1')}</p>
      <p className="muted center">{t('mh.ok2')}</p>
    </> : <>
      <p className="modal-text">{t('mh.bad')}</p>
      <ol className="steps">
        <li><div><Rich k="mh.s1" /><br /><button className="btn sm" onClick={async e => {
          const n = await busy(e.currentTarget, () => call('closeAll')) as number | undefined;
          if (n !== undefined) toast(n ? tn('t.closingAll', n) : t('t.noRoblox'), 'ok');
        }}><Icon name="power" />{t('mh.s1btn')}</button></div></li>
        {m.trayAutostart ? <li><div><Rich k="mh.s2" /><br /><button className="btn sm" disabled={trayDone} onClick={async e => {
          if ((await busy(e.currentTarget, () => call('disableTrayAutostart'))) !== undefined) setTrayDone(true);
        }}>{t(trayDone ? 'mh.s2done' : 'mh.s2btn')}</button></div></li> : null}
        <li><div><Rich k="mh.s3" /></div></li>
      </ol>
    </>}
    {m.enabled ? <>
      {flag('mh-windowed', 'mh.windowed', windowed, setWindowed, 'multiWindowed')}
      {flag('mh-tile', 'mh.tile', tile, setTile, 'multiTile')}
    </> : null}
  </>;
}
