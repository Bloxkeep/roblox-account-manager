// Settings › Bloxkeep updates: the version and where the update check stands (with What's new, and Check now or
// Restart to update), the automatic updates switch, then Help & feedback (the guide, feedback, a GitHub star).
import { useEffect, useState } from 'react';
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch } from './parts';

export function UpdatesPage({ title }: { title: string }) {
  const { t, lang, locale, call } = useHost();
  const u = useAppState().update || {};
  // The switch answers at once; the app's next state confirms it (or puts it back).
  const [auto, setAuto] = useState(!!u.auto);
  useEffect(() => setAuto(!!u.auto), [u.auto]);

  // "Up to date" as of the last check, which can be up to an hour old: it says when.
  const at = u.checkedAt ? new Date(u.checkedAt) : null;
  const hm: Intl.DateTimeFormatOptions = { hour: '2-digit', minute: '2-digit', ...(lang === 'vi' ? { hourCycle: 'h23' } : {}) };
  const when = at && !isNaN(+at) ? (at.toDateString() === new Date().toDateString()
    ? at.toLocaleTimeString(locale, hm) : at.toLocaleString(locale, { day: 'numeric', month: 'short', ...hm })) : '';
  const status = !u.supported ? t('upd.unsupported')
    : u.status === 'checking' ? t('upd.checking')
    : u.status === 'downloading' ? t('upd.downloading', { v: u.version, p: u.progress })
    : u.status === 'ready' ? t(u.auto ? 'upd.readyAuto' : 'upd.ready', { v: u.version })
    : u.status === 'upToDate' ? t('upd.latest') + (when ? ' · ' + t('upd.checkedAt', { t: when }) : '')
    : u.status === 'error' ? (u.error ? `${t('upd.error')}: ${u.error}` : t('upd.error')) // the reason, when the app gives one
    : u.auto ? t('upd.idle') : t('upd.off');

  return <Page id="updates" title={title}>
    <Group id="updates">
      <div id="s-update"><div className="set-row">
        <div><div className="t">Bloxkeep {u.current || ''}</div><div className="d">{status}</div></div>
        <div className="ctl">
          <button className="btn sm ghost" data-act="whats-new">{t('wn.link')}</button>
          {!u.supported ? null : u.status === 'ready'
            ? <button className="btn sm primary" data-act="apply-update"><Icon name="refresh" />{t('upd.restart')}</button>
            : <button className="btn sm" data-act="check-updates" disabled={u.status === 'checking' || u.status === 'downloading'}><Icon name="refresh" />{t('upd.check')}</button>}
        </div>
      </div></div>
      <Row title={t('upd.auto')}>
        <Switch id="s-autoupd" checked={auto} disabled={!u.supported} onChange={on => { setAuto(on); call('setAutoUpdate', { enabled: on }); }} />
      </Row>
    </Group>
    <Group id="feedback" title={t('fb.group')}>
      <Row title={t('help.title')} desc={t('help.desc')}><button className="btn sm" data-act="open-guide"><Icon name="book" />{t('help.open')}</button></Row>
      <Row title={t('fb.title')} desc={t('fb.rowDesc')}><button className="btn sm" data-act="feedback"><Icon name="message" />{t('fb.open')}</button></Row>
      <Row title={t('star.title')} desc={t('star.desc')}><button className="btn sm" data-act="open-github"><Icon name="star" />{t('star.open')}</button></Row>
    </Group>
  </Page>;
}
