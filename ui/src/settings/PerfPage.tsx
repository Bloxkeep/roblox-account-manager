// Settings › Staying in game: the Roblox settings; auto-rejoin for every account, Anti-AFK, leaving crowded servers, the daily sign-in check; then RAM and CPU (the low-RAM
// warning, saving RAM on background windows, the RAM limit for resting ones, fast cores for the played window). Tray and GPU drawing: Settings › Bloxkeep.
import { useAppState, useHost } from '../host';
import { Group, Page, Row, Switch, useLocal } from './parts';
import { RobloxConfig } from './RobloxPage';

export function PerfPage({ title }: { title: string }) {
  const { t, tn, locale, call, toast } = useHost();
  const app = useAppState();
  const st = app.settings || {};
  const accounts: { id: string; autoRejoin?: boolean }[] = app.accounts || [];

  // Auto-rejoin: on for every account, off for every one, or (shown only) some of them.
  const on = accounts.filter(a => a.autoRejoin).length;
  const rejoinNow = !accounts.length ? 'off' : on === accounts.length ? 'on' : on === 0 ? 'off' : 'some';
  const [rejoin, setRejoin] = useLocal(rejoinNow);
  const [antiAfk, setAntiAfk] = useLocal(!!st.antiAfk);
  const [hop, setHop] = useLocal(String(st.hopAbovePlayers || 0));
  const [daily, setDaily] = useLocal(app.dailySignInCheck !== false);
  const [memWarn, setMemWarn] = useLocal(!!st.memoryWarnings);
  const [trim, setTrim] = useLocal(!!st.trimBackground);
  const [ramLimit, setRamLimit] = useLocal(String(st.ramLimitMb || 0));
  const [cores, setCores] = useLocal(!!st.splitCores);
  // Two kinds of cores (Intel 12th gen and later…): background windows go to the efficiency ones; otherwise only the
  // power-saving mode is left to give.
  const cpu: { hybrid?: boolean; efficient?: number } = st.cores || {};

  const checkedAt = app.lastSignInCheck ? new Date(app.lastSignInCheck) : null;
  const dailyWhen = checkedAt && !isNaN(+checkedAt) ? t('set.dailyLast', {
    t: checkedAt.toDateString() === new Date().toDateString()
      ? checkedAt.toLocaleTimeString(locale, { hour: 'numeric', minute: '2-digit' })
      : checkedAt.toLocaleString(locale, { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' }),
  }) : '';
  const setting = (key: string, set: (v: boolean) => void) => (v: boolean) => { set(v); call('setSettings', { [key]: v }); };

  return <Page id="perf" title={title}>
    <RobloxConfig />
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
      <Row title={t('set.hop')} desc={t('set.hopDesc')}>
        <select className="select" id="s-hop" aria-label={t('set.hop')} value={hop} onChange={e => {
          setHop(e.target.value);
          call('setSettings', { hopAbovePlayers: Number(e.target.value) });
        }}>
          <option value="0">{t('set.hopOff')}</option>
          {[1, 2, 3, 5, 8, 10, 15, 20].map(n => <option key={n} value={n}>{n === 1 ? t('set.hopN1') : t('set.hopN', { n })}</option>)}
        </select>
      </Row>
      <Row title={t('set.daily')} desc={<>{t('set.dailyDesc')} <span id="s-daily-when">{dailyWhen}</span></>}>
        <Switch id="s-daily" label={t('set.daily')} checked={daily} onChange={v => { setDaily(v); call('setDailySignInCheck', { enabled: v }); }} />
      </Row>
    </Group>
    <Group id="memory" title={t('set.g.memory')}>
      <Row title={t('set.memWarn')}><Switch id="s-memwarn" label={t('set.memWarn')} checked={memWarn} onChange={setting('memoryWarnings', setMemWarn)} /></Row>
      <Row title={t('set.trim')} desc={t('set.trimDesc')}><Switch id="s-trim" label={t('set.trim')} checked={trim} onChange={setting('trimBackground', setTrim)} /></Row>
      <Row title={t('set.ramLimit')} desc={t('set.ramLimitDesc')}>
        <select className="select" id="s-ramlimit" aria-label={t('set.ramLimit')} value={ramLimit} onChange={e => {
          setRamLimit(e.target.value);
          call('setSettings', { ramLimitMb: Number(e.target.value) });
        }}>
          <option value="0">{t('set.ramLimitOff')}</option>
          <option value="-1">{t('set.ramLimitAuto')}</option>
          <option value="512">512 MB</option>
          <option value="768">768 MB</option>
          <option value="1024">1 GB</option>
          <option value="1536">1.5 GB</option>
          <option value="2048">2 GB</option>
        </select>
      </Row>
      <Row title={t('set.cores')} desc={cpu.hybrid ? t('set.coresDesc', { e: cpu.efficient || 0 }) : t('set.coresDescOne')}>
        <Switch id="s-cores" label={t('set.cores')} checked={cores} onChange={setting('splitCores', setCores)} /></Row>
    </Group>
  </Page>;
}
