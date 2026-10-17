// Settings › Launching: Multi-Roblox (with whether it is ready), names on windows, how a window opens and how windows
// are arranged; then what opens Roblox (with a note while the Store edition is involved) and the wait between accounts.
import { useEffect, useRef } from 'react';
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

export function LaunchPage({ title }: { title: string }) {
  const { t, call, setMulti } = useHost();
  const app = useAppState();
  const st = app.settings || {};
  const m = app.multi || {};
  const ins = app.installs || {};

  const [multi, setMultiOn] = useLocal(!!m.enabled);
  const [rename, setRename] = useLocal(!!st.renameWindows);
  const [pictures, setPictures] = useLocal(st.windowPictures ?? !!st.renameWindows);
  const [taskbar, setTaskbar] = useLocal(st.taskbarGroups || 'roblox');
  // One of three: as Roblox sizes it, maximised, or tiled (the app keeps the other one off).
  const [openAs, setOpenAs] = useLocal(m.tile ? 'tile' : st.maximizeOnOpen ? 'maximize' : 'roblox');
  const [layout, setLayout] = useLocal(st.windowLayout || 'fill');

  // [value, words, usable]: other Bloxstrap-style launchers only appear when found on this PC (or still chosen after
  // being removed).
  const others: string[] = ins.others || [];
  const savedOther = (st.launchVia || '').startsWith('other:') ? st.launchVia.slice(6) : null;
  const options: [string, string, boolean][] = [
    ['auto', ins.handler ? t('launch.auto', { name: ins.handler }) : t('launch.autoNone'), true],
    ['bloxstrap', 'Bloxstrap', !!ins.bloxstrap],
    ['fishstrap', 'Fishstrap', !!ins.fishstrap],
    ...[...new Set([...others, ...(savedOther ? [savedOther] : [])])].map(n => [`other:${n}`, n, others.includes(n)] as [string, string, boolean]),
    ['roblox', t('launch.roblox'), !!ins.roblox],
    ['direct', t('launch.direct'), !!ins.direct],
    ...(ins.store || st.launchVia === 'store' ? [['store', t('launch.storeName'), !!ins.store] as [string, string, boolean]] : []),
    ...(st.customLauncher ? [['custom', t('launch.custom', { name: st.customLauncher }), true] as [string, string, boolean]] : []),
  ];
  const saved = st.launchVia || 'auto';
  const [via, setVia] = useLocal(options.some(o => o[0] === saved) ? saved : options[0][0]);
  // The Store edition is new here: said while it is the one chosen, or when it is the only Roblox on the PC.
  const storeNote = !ins.store ? '' : st.launchVia === 'store' ? t('launch.storeTest') : !ins.direct ? t('launch.storeOnly') : '';

  // The wait shows its number while the slider moves, and is saved once it is let go (the slider's own change event:
  // React's onChange is every move).
  const [delay, setDelay] = useLocal<number>(st.launchDelaySeconds ?? 0);
  const slider = useRef<HTMLInputElement>(null);
  useEffect(() => {
    const el = slider.current!;
    const save = () => call('setSettings', { launchDelaySeconds: +el.value });
    el.addEventListener('change', save);
    return () => el.removeEventListener('change', save);
  }, [call]);

  const pill = !m.enabled ? 'off' : m.armed ? 'ok' : 'warn';
  return <Page id="launch" title={title}>
    <Group id="launch" title={t('set.g.windows')}>
      <Row title={t('set.multi')} extra={<>{' '}<span className={`state-pill ${pill}`} id="s-multi-pill">{t(!m.enabled ? 'pill.off' : m.armed ? 'pill.ready' : 'pill.notReady')}</span></>}
        desc={<>{t('set.multiDesc')} <button type="button" className="link" data-act="multi-help">{t('set.details')}</button></>}>
        {/* Turning it on asks first (the risk warning); declined, the switch goes back. */}
        <Switch id="s-multi" label={t('set.multi')} checked={multi} onChange={async on => {
          setMultiOn(on);
          if (!await setMulti(on)) setMultiOn(!!m.enabled);
        }} />
      </Row>
      <Row title={t('set.rename')} desc={t('set.renameDesc')}>
        <Switch id="s-rename" label={t('set.rename')} checked={rename} onChange={on => { setRename(on); call('setSettings', { renameWindows: on }); }} />
      </Row>
      <Row title={t('set.pictures')} desc={t('set.picturesDesc')}>
        <Switch id="s-pictures" label={t('set.pictures')} checked={pictures} onChange={on => { setPictures(on); call('setSettings', { windowPictures: on }); }} />
      </Row>
      <Row title={t('set.taskbar')} desc={t('set.taskbarDesc')}>
        <select className="select" id="s-taskbar" aria-label={t('set.taskbar')} value={taskbar} onChange={e => {
          setTaskbar(e.target.value);
          call('setSettings', { taskbarGroups: e.target.value });
        }}>
          {['roblox', 'account', 'preset'].map(k => <option key={k} value={k}>{t(`set.taskbar.${k}`)}</option>)}
        </select>
      </Row>
      <Row title={t('set.openAs')} desc={t('set.openAsDesc')}>
        <select className="select" id="s-openas" aria-label={t('set.openAs')} value={openAs} onChange={e => {
          const v = e.target.value;
          setOpenAs(v);
          call('setSettings', v === 'tile' ? { multiTile: true } : v === 'maximize' ? { maximizeOnOpen: true } : { multiTile: false, maximizeOnOpen: false });
        }}>
          {['roblox', 'maximize', 'tile'].map(k => <option key={k} value={k}>{t(`set.openAs.${k}`)}</option>)}
        </select>
      </Row>
      <Row title={t('set.winLayout')} desc={t('set.winLayoutDesc')}>
        <select className="select" id="s-winlayout" aria-label={t('set.winLayout')} value={layout}
          onChange={e => { setLayout(e.target.value); call('setSettings', { windowLayout: e.target.value }); }}>
          {['fill', 'small', 'medium', 'large'].map(k => <option key={k} value={k}>{t(`set.winLayout.${k}`)}</option>)}
        </select>
      </Row>
    </Group>
    <Group id="client" title={t('set.g.client')}>
      <Row title={t('set.client')} desc={t('set.clientDesc')}>
        <select className="select" id="s-launch" aria-label="Roblox client" value={via}
          onChange={e => { setVia(e.target.value); call('setSettings', { launchVia: e.target.value }); }}>
          {options.map(([value, words, ok]) => <option key={value} value={value} disabled={!ok}>{ok ? words : t('launch.missing', { name: words })}</option>)}
        </select>
        {/* A double click opens one file window (app.js busy). */}
        <button className="btn sm" data-act="pick-launcher" title={t('launch.pick')}>{t('launch.pickBtn')}</button>
      </Row>
      <div id="s-store">{storeNote ? <div className="set-note"><Icon name="info" /><span>{storeNote}</span></div> : null}</div>
      <Row title={t('set.delay')} desc={t('set.delayDesc')}>
        <input type="range" id="s-delay" min="0" max="60" step="1" aria-label={t('set.delay')} ref={slider} value={delay}
          onChange={e => setDelay(+e.target.value)} />
        <b id="s-delay-v" className="range-v">{t('set.seconds', { n: delay })}</b>
      </Row>
    </Group>
  </Page>;
}
