// Settings › Bloxkeep: Updates (version, update check, What's new, automatic and beta switches), This app (tray, hardware
// acceleration), Help & feedback (guide, feedback, Discord server, GitHub star), Privacy (being counted among people online).
import { useEffect, useState } from 'react';
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

export function UpdatesPage({ title }: { title: string }) {
  const { t, lang, locale, call, saveView } = useHost();
  const app = useAppState();
  const u = app.update || {};
  // The switches answer at once; the app's next state confirms them (or puts them back).
  const [auto, setAuto] = useState(!!u.auto);
  useEffect(() => setAuto(!!u.auto), [u.auto]);
  const [beta, setBeta] = useState(!!u.beta);
  useEffect(() => setBeta(!!u.beta), [u.beta]);
  const [tray, setTray] = useLocal(app.closeToTray !== false);
  const [gpu, setGpu] = useLocal(!app.safeRendering);
  const keys: { on?: boolean; taken?: string[] } = app.hotkeys || {};
  const [hotkeys, setHotkeys] = useLocal(keys.on !== false);
  const counted = app.countOnline !== false;
  const [online, setOnline] = useState(counted);
  useEffect(() => setOnline(counted), [counted]);

  // "Up to date" as of the last check, which can be up to an hour old: it says when.
  const at = u.checkedAt ? new Date(u.checkedAt) : null;
  const hm: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit', ...(lang === 'vi' ? { hourCycle: 'h23' } : {}) };
  const when = at && !isNaN(+at) ? (at.toDateString() === new Date().toDateString()
    ? at.toLocaleTimeString(locale, hm) : at.toLocaleString(locale, { day: 'numeric', month: 'short', ...hm })) : '';
  const status = !u.supported ? t('upd.unsupported')
    : u.status === 'checking' ? t('upd.checking')
    : u.status === 'downloading' ? t('upd.downloading', { v: u.version, p: u.progress })
    : u.status === 'ready' ? (u.quiet ? t('upd.readyQuiet') : t(u.auto ? 'upd.readyAuto' : 'upd.ready', { v: u.version }))
    : u.status === 'upToDate' ? t('upd.latest') + (when ? ' · ' + t('upd.checkedAt', { t: when }) : '')
    : u.status === 'error' ? (u.error ? `${t('upd.error')}: ${u.error}` : t('upd.error')) // the reason, when the app gives one
    : u.auto ? t('upd.idle') : t('upd.off');

  return <Page id="updates" title={title}>
    <Group id="updates" title={t('upd.title')}>
      <div id="s-update"><div className="set-row">
        {/* A quiet fix shows as the version it fixes; its own number on hover. */}
        <div><div className="t" title={u.build && u.build !== u.current ? u.build : undefined}>Bloxkeep {u.current || ''}</div><div className="d">{status}</div></div>
        <div className="ctl">
          <button className="btn sm ghost" data-act="whats-new">{t('wn.link')}</button>
          {!u.supported ? null : u.status === 'ready' && !u.quiet
            ? <button className="btn sm primary" data-act="apply-update"><Icon name="refresh" />{t('upd.restart')}</button>
            : <button className="btn sm" data-act="check-updates" disabled={u.status === 'checking' || u.status === 'downloading' || u.status === 'ready'}><Icon name="refresh" />{t('upd.check')}</button>}
        </div>
      </div></div>
      <Row title={t('upd.auto')}>
        <Switch id="s-autoupd" checked={auto} disabled={!u.supported} onChange={on => { setAuto(on); call('setAutoUpdate', { enabled: on }); }} />
      </Row>
      <Row title={t('upd.beta')} desc={t('upd.betaDesc')}>
        <Switch id="s-betaupd" checked={beta} disabled={!u.supported} onChange={on => { setBeta(on); call('setBetaUpdates', { enabled: on }); }} />
      </Row>
    </Group>
    <Group id="app" title={t('set.g.app')}>
      <Row title={t('set.tray')} desc={t('set.trayDesc')}>
        <Switch id="s-tray" label={t('set.tray')} checked={tray} onChange={v => { setTray(v); call('setCloseToTray', { enabled: v }); }} />
      </Row>
      {/* Hotkeys from any app; one another app already holds is said, not taken from it. */}
      <Row title={t('set.hotkeys')} desc={<>{t('set.hotkeysDesc')}{hotkeys && keys.taken?.length ? <><br /><span className="warn-text">{t('set.hotkeysTaken', { keys: keys.taken.join(', ') })}</span></> : null}</>}>
        <Switch id="s-hotkeys" label={t('set.hotkeys')} checked={hotkeys} onChange={v => { setHotkeys(v); call('setHotkeys', { enabled: v }); }} />
      </Row>
      <Row title={t('set.gpu')} desc={t('set.gpuDesc')}>
        {/* Switching reloads the interface, so the reply may never come back; the reloaded page shows the new state. */}
        <Switch id="s-gpu" label={t('set.gpu')} checked={gpu} onChange={v => { setGpu(v); saveView(true); call('setRendering', { gpu: v }, { silent: true }); }} />
      </Row>
    </Group>
    <Group id="feedback" title={t('fb.group')}>
      <Row title={t('help.title')} desc={t('help.desc')}><button className="btn sm" data-act="open-guide"><Icon name="book" />{t('help.open')}</button></Row>
      <Row title={t('fb.title')} desc={t('fb.rowDesc')}><button className="btn sm" data-act="feedback"><Icon name="message" />{t('fb.open')}</button></Row>
      <Row title={t('dc.title')} desc={t('dc.desc')}><button className="btn sm" data-act="open-discord"><Icon name="discord" />{t('dc.open')}</button></Row>
      <Row title={t('star.title')} desc={t('star.desc')}><button className="btn sm" data-act="open-github"><Icon name="star" />{t('star.open')}</button></Row>
    </Group>
    <Group id="privacy" title={t('onl.group')}>
      <Row title={t('onl.title')} desc={t('onl.desc')}>
        <Switch id="s-online" checked={online} onChange={on => { setOnline(on); call('setCountOnline', { enabled: on }); }} />
      </Row>
    </Group>
  </Page>;
}
