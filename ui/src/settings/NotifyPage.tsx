// Settings › Notifications: Windows notifications on or off, then each kind (they only matter while the first is on),
// then the questions answered "Don't ask again" and the button that asks them all again.
import { useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

/** The kinds of Windows notification that can be turned off one by one (the app maps each event to one of them). */
export const NOTIFY_KINDS = ['drop', 'schedule', 'signin', 'ram', 'launch'];

export function NotifyPage({ title }: { title: string }) {
  const { t, tn, call } = useHost();
  const app = useAppState();
  const [notify, setNotify] = useLocal(app.notifications !== false);
  const [off, setOff] = useLocal<string[]>(app.notifyOff || []);
  const skipped = (app.skipConfirm || []).length;

  return <Page id="notify" title={title}>
    <Group id="notify">
      <Row title={t('set.notify')} desc={t('set.notifyDesc')}>
        <Switch id="s-notify" checked={notify} onChange={on => { setNotify(on); call('setNotifications', { enabled: on }); }} />
      </Row>
    </Group>
    <Group id="notify-kinds" title={t('set.notifyKinds')}>
      {/* The per-kind switches only mean something while the main one is on. */}
      <div className={`set-sub${notify ? '' : ' off'}`} id="s-notify-kinds">
        {NOTIFY_KINDS.map(kind => <Row key={kind} title={t(`set.notify.${kind}`)}>
          <Switch id={`s-notify-${kind}`} label={t(`set.notify.${kind}`)} checked={!off.includes(kind)} disabled={!notify}
            onChange={on => { setOff(on ? off.filter(k => k !== kind) : [...off, kind]); call('setNotifications', { kind, enabled: on }); }} />
        </Row>)}
      </div>
    </Group>
    {/* Questions answered "Don't ask again" (a launch warning, closing all Roblox, locking…): all asked again at once. */}
    <Group id="asks" title={t('set.g.asks')}>
      <Row title={t('set.asks')} desc={<span id="s-asks-n">{skipped ? tn('set.asksN', skipped) : t('set.asksNone')}</span>}>
        <button className="btn sm" data-act="reset-asks" id="s-asks-btn" disabled={!skipped}>{t('set.asksBtn')}</button>
      </Row>
    </Group>
  </Page>;
}
