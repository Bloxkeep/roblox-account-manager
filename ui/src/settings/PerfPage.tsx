// Settings › Staying in game: auto-rejoin for every account at once, Anti-AFK, the daily sign-in check (with when it
// last ran); then memory (the low-RAM warning, saving RAM on background windows); then this app (keep running in the
// tray, hardware acceleration).
import { useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';

export function PerfPage({ title }: { title: string }) {
  const { t, tn, locale, call, toast, saveView } = useHost();
  const app = useAppState();
  const st = app.settings || {};
  const accounts: { id: string; autoRejoin?: boolean }[] = app.accounts || [];

  // Auto-rejoin: on for every account, off for every one, or (shown only) some of them.
  const on = accounts.filter(a => a.autoRejoin).length;
  const rejoinNow = !accounts.length ? 'off' : on === accounts.length ? 'on' : on === 0 ? 'off' : 'some';
  const [rejoin, setRejoin] = useLocal(rejoinNow);
  const [antiAfk, setAntiAfk] = useLocal(!!st.antiAfk);
  const [daily, setDaily] = useLocal(app.dailySignInCheck !== false);
  const [memWarn, setMemWarn] = useLocal(!!st.memoryWarnings);
  const [trim, setTrim] = useLocal(!!st.trimBackground);
  const [tray, setTray] = useLocal(app.closeToTray !== false);
  const [gpu, setGpu] = useLocal(!app.safeRendering);

  const checkedAt = app.lastSignInCheck ? new Date(app.lastSignInCheck) : null;
  const dailyWhen = checkedAt && !isNaN(+checkedAt) ? t('set.dailyLast', {
    t: checkedAt.toDateString() === new Date().toDateString()
      ? checkedAt.toLocaleTimeString(locale, { hour: 'numeric', minute: '2-digit' })
      : checkedAt.toLocaleString(locale, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' }),
  }) : '';
  const setting = (key: string, set: (v: boolean) => void) => (v: boolean) => { set(v); call('setSettings', { [key]: v }); };

  return <Page id="perf" title={title}>
    <Group id="perf" title={t('set.g.keep')}>
      <Row title={t('set.rejoin')} desc={t('set.rejoinDesc')}>
        <select className="select" id="s-rejoin" aria-label={t('set.rejoin')} value={rejoin} disabled={!accounts.length} onChange={async e => {
          const enable = e.target.value === 'on';
          setRejoin(e.target.value);
          const n = await call('setAutoRejoin', { ids: accounts.map(a => a.id), enabled: enable }) as number | undefined;
          // Refused: back to what the accounts say (no new state will come to do it).
          if (n === undefined) setRejoin(rejoinNow);
          else toast(tn(enable ? 't.rejoinOn' : 't.rejoinOff', n), 'ok');
        }}>
          <option value="on">{t('set.rejoinAll')}</option>
          <option value="off">{t('set.rejoinNone')}</option>
          <option value="some" disabled hidden>{t('set.rejoinSome', { n: on, total: accounts.length })}</option>
        </select>
      </Row>
      <Row title={t('set.antiAfk')} desc={t('set.antiAfkDesc')}><Switch id="s-antiafk" label={t('set.antiAfk')} checked={antiAfk} onChange={setting('antiAfk', setAntiAfk)} /></Row>
      <Row title={t('set.daily')} desc={<>{t('set.dailyDesc')} <span id="s-daily-when">{dailyWhen}</span></>}>
        <Switch id="s-daily" label={t('set.daily')} checked={daily} onChange={v => { setDaily(v); call('setDailySignInCheck', { enabled: v }); }} />
      </Row>
    </Group>
    <Group id="memory" title={t('set.g.memory')}>
      <Row title={t('set.memWarn')}><Switch id="s-memwarn" label={t('set.memWarn')} checked={memWarn} onChange={setting('memoryWarnings', setMemWarn)} /></Row>
      <Row title={t('set.trim')} desc={t('set.trimDesc')}><Switch id="s-trim" label={t('set.trim')} checked={trim} onChange={setting('trimBackground', setTrim)} /></Row>
    </Group>
    <Group id="app" title={t('set.g.app')}>
      <Row title={t('set.tray')} desc={t('set.trayDesc')}>
        <Switch id="s-tray" label={t('set.tray')} checked={tray} onChange={v => { setTray(v); call('setCloseToTray', { enabled: v }); }} />
      </Row>
      <Row title={t('set.gpu')} desc={t('set.gpuDesc')}>
        {/* Switching reloads the interface, so the reply may never come back; the reloaded page shows the new state. */}
        <Switch id="s-gpu" label={t('set.gpu')} checked={gpu} onChange={v => { setGpu(v); saveView(true); call('setRendering', { gpu: v }, { silent: true }); }} />
      </Row>
    </Group>
  </Page>;
}
