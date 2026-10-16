// Settings › Backup & data: backing up to a folder (which one, folders found on this PC to use instead, a warning when
// a cloud folder meets an easy password, the last backup and Back up now); then restoring and importing; then where the
// app keeps its data.
import { Ago, Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

export function BackupPage({ title }: { title: string }) {
  const { t, call } = useHost();
  const app = useAppState();
  const b = app.backup || {};
  const [on, setOn] = useLocal(b.enabled !== false);
  const suggested: { path: string; name: string }[] = b.suggested || [];

  return <Page id="backup" title={title}>
    <Group id="backup" title={t('bk.title')}>
      <Row title={t('bk.on')} desc={t('bk.onDesc')}>
        <Switch id="s-bk-on" label={t('bk.on')} checked={on} onChange={v => { setOn(v); call('setBackup', { enabled: v }); }} />
      </Row>
      <Row title={t('bk.folder')} desc={<>
        <span className="path" id="s-bk-path">{b.folder || t('bk.noFolder')}</span>
        <div className="bk-suggest" id="s-bk-suggest">
          {suggested.map(s => <button key={s.path} className="btn sm ghost" data-act="bk-use" data-path={s.path} title={s.path}>{t('bk.use', { name: s.name })}</button>)}
        </div>
      </>}>
        <button className="btn sm" data-act="bk-pick">{t('bk.choose')}</button>
        <button className="btn sm" data-act="bk-open" id="s-bk-open" hidden={!b.folder}><Icon name="folder" />{t('set.openFolder')}</button>
      </Row>
      <div id="s-bk-note">{b.cloud && b.weakPassword ? <div className="set-note"><Icon name="alert" /><span>{t('bk.weak')}</span></div> : null}</div>
      <div id="s-bk-status">
        <div className="set-row">
          <div>
            <div className="t">{b.lastBackup ? <Ago iso={b.lastBackup} tpl="bk.last" /> : t('bk.never')}</div>
            {b.error ? <div className="d err">{b.error}</div> : null}
          </div>
          <div className="ctl"><button className="btn sm" data-act="bk-now" disabled={!b.folder}>{t('bk.now')}</button></div>
        </div>
      </div>
    </Group>
    <Group id="restore" title={t('set.g.restore')}>
      <Row title={t('bk.restore')} desc={t('bk.restoreDesc')}><button className="btn sm" data-act="bk-restore">{t('bk.restoreBtn')}</button></Row>
      <Row title={t('iv.setRow')} desc={t('iv.setRowDesc')}><button className="btn sm" data-act="bk-import">{t('iv.setBtn')}</button></Row>
    </Group>
    <Group id="data" title={t('set.data')}>
      <Row title={t('set.dataPath')} desc={<span className="path" id="s-path">{app.dataPath || ''}</span>}>
        <button className="btn sm" data-act="open-data"><Icon name="folder" />{t('set.openFolder')}</button>
      </Row>
    </Group>
  </Page>;
}
