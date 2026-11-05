// Settings › Notifications: Windows notifications on or off, then each kind (they only matter while the first is on),
// then the phone (the same events through ntfy), then the questions answered "Don't ask again" and the button that asks
// them all again.
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, TextBox, useLocal } from './parts';

/** The kinds of Windows notification that can be turned off one by one (the app maps each event to one of them). */
export const NOTIFY_KINDS = ['drop', 'schedule', 'signin', 'ram', 'launch'];

export function NotifyPage({ title }: { title: string }) {
  const { t, tn, call, busy, toast } = useHost();
  const app = useAppState();
  const [notify, setNotify] = useLocal(app.notifications !== false);
  const [off, setOff] = useLocal<string[]>(app.notifyOff || []);
  const skipped = (app.skipConfirm || []).length;
  const phone: { enabled?: boolean; topic?: string; error?: string | null } = app.phone || {};
  const [phoneOn, setPhoneOn] = useLocal(!!phone.enabled);

  return <Page id="notify" title={title}>
    <Group id="notify">
      <Row title={t('set.notify')} desc={t('set.notifyDesc')}>
        <Switch id="s-notify" checked={notify} onChange={on => { setNotify(on); call('setNotifications', { enabled: on }); }} />
      </Row>
    </Group>
    <Group id="notify-kinds" title={t('set.notifyKinds')}>
      <div className={`set-sub${notify ? '' : ' off'}`} id="s-notify-kinds">
        {NOTIFY_KINDS.map(kind => <Row key={kind} title={t(`set.notify.${kind}`)}>
          <Switch id={`s-notify-${kind}`} label={t(`set.notify.${kind}`)} checked={!off.includes(kind)} disabled={!notify}
            onChange={on => { setOff(on ? off.filter(k => k !== kind) : [...off, kind]); call('setNotifications', { kind, enabled: on }); }} />
        </Row>)}
      </div>
    </Group>
    {/* The phone: ntfy's free app subscribed to a topic only this person knows. */}
    <Group id="phone" title={t('set.phone')}>
      <Row title={t('set.phoneOn')} desc={t('set.phoneDesc')}>
        <Switch id="s-phone" label={t('set.phoneOn')} checked={phoneOn} onChange={on => { setPhoneOn(on); call('setPhone', { enabled: on }); }} />
      </Row>
      {phoneOn && phone.topic ? <Row title={t('set.phoneTopic')}
        desc={<>{t('set.phoneTopicDesc')}{phone.error ? <><br /><span className="warn-text">{phone.error}</span></> : null}</>}>
        <TextBox className="field sm phone-topic" saved={phone.topic} spellCheck={false} autoComplete="off" maxLength={200}
          aria-label={t('set.phoneTopic')} commit={v => { call('setPhone', { topic: v }); }} />
        <button className="btn sm ghost" id="s-phone-copy" onClick={() => navigator.clipboard.writeText(phone.topic || '').then(() => toast(t('set.phoneCopied'), 'ok'), () => {})}>
          <Icon name="copy" />{t('set.phoneCopy')}</button>
        <button className="btn sm" id="s-phone-test" onClick={e => busy(e.currentTarget, async () => {
          if (await call('phoneTest') !== undefined) toast(t('set.phoneSent'), 'ok');
        })}>{t('set.phoneTest')}</button>
      </Row> : null}
    </Group>
    {/* Questions answered "Don't ask again" (a launch warning, closing all Roblox, locking…): all asked again at once. */}
    <Group id="asks" title={t('set.g.asks')}>
      <Row title={t('set.asks')} desc={<span id="s-asks-n">{skipped ? tn('set.asksN', skipped) : t('set.asksNone')}</span>}>
        <button className="btn sm" data-act="reset-asks" id="s-asks-btn" disabled={!skipped}>{t('set.asksBtn')}</button>
      </Row>
    </Group>
  </Page>;
}
