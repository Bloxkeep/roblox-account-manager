// Settings › Discord: sending and the webhook link, the play-time summary, alerts, the status message, quiet hours, how
// messages look. The link itself never comes to the page unless the person asks to see it (Show); masked otherwise.
import { useEffect, useLayoutEffect, useRef, useState, type Ref } from 'react';
import { Ago, Icon, useAppState, useHost } from '../host';
import { NOTIFY_KINDS } from './NotifyPage';
import { Group, More, Page, Row, Switch, TextBox, useLocal } from './parts';

const DC_ID = /^\d{17,20}$/;
const PING_MODES = ['none', 'here', 'everyone', 'user', 'role'];
const NONE: string[] = [];

export function DiscordPage({ title }: { title: string }) {
  const { t, tn, locale, call, toast, busy, discordLink, on: signal } = useHost();
  const app = useAppState();
  const d = app.discord || {};
  const s = d.summary || {};
  const events: string[] = d.events || NONE;

  // Refused by the app (the reason is in a toast): every control shows the saved value again, so nothing looks saved
  // that wasn't. Accepted, the box shows what was kept too (an empty name becomes "Bloxkeep").
  const [again, setAgain] = useState(0);
  const putBack = () => setAgain(n => n + 1);
  const set = async (args: Record<string, unknown>) => {
    const r = await call('setDiscord', args);
    putBack();
    return r;
  };

  const [on, setOn] = useLocal(!!d.enabled, again);
  const [presence, setPresence] = useLocal(!!d.presence, again);
  const [hideNames, setHideNames] = useLocal(!!d.hideNames, again);
  const whenNow = s.daily && s.weekly ? 'both' : s.daily ? 'daily' : s.weekly ? 'weekly' : 'none';
  const [when, setWhen] = useLocal(whenNow, again);
  const [weekday, setWeekday] = useLocal(String(s.weekday ?? ''), again);
  const [after, setAfter] = useLocal(!!s.afterSchedule, again);
  const [days, setDays] = useLocal(String(s.days ?? ''), again);
  const [board, setBoard] = useLocal(!!d.board?.enabled, again);
  const [minutes, setMinutes] = useLocal(String(d.board?.minutes ?? ''), again);
  const [quiet, setQuiet] = useLocal(!!d.quiet?.enabled, again);
  const [lang, setLang] = useLocal(String(d.language ?? ''), again);
  const [style, setStyle] = useLocal(String(d.style ?? ''), again);
  const [kinds, setKinds] = useLocal(events, again);
  // Alerts on or off as a whole: every kind follows, one at a time; the switch waits (and shows the choice) meanwhile.
  const [alertsWant, setAlertsWant] = useState<boolean | null>(null);

  // The whole link, shown until hidden again: after half a minute, when Settings or the window is left, or another link replaced it.
  const [shown, setShown] = useState<{ url: string; hint: string } | null>(null);
  const hideTimer = useRef(0);
  const hide = () => { clearTimeout(hideTimer.current); setShown(null); };
  useEffect(() => signal('hideDiscordLink', hide), [signal]);
  useEffect(() => () => clearTimeout(hideTimer.current), []);
  const link = shown && d.hasWebhook && shown.hint === d.webhookHint ? shown.url : null;

  // A time box emptied isn't a time: the saved one comes back.
  const time = (key: string) => (v: string) => { if (v) set({ [key]: v }); else putBack(); };
  const quietFrom = useRef<HTMLInputElement>(null), quietTo = useRef<HTMLInputElement>(null);
  // From and To the same never applies: said so when it happens.
  const quietTime = (key: string) => (v: string) => {
    time(key)(v);
    if (quietFrom.current!.value === quietTo.current!.value) toast(t('dc.quietSame'), 'warn');
  };

  // A ping for "alerts" is the same ping for every kind of alert; it shows "Varies by alert" while they differ.
  const setPing = async (kind: string, value: string) => {
    if (kind !== 'alerts') return set({ ping: { kind, value } });
    for (const k of NOTIFY_KINDS) if (await call('setDiscord', { ping: { kind: k, value } }) === undefined) break;
    putBack();
  };
  const alertPings = NOTIFY_KINDS.map(k => d.pings?.[k] || 'none');
  const ping = (kind: string) => <Ping kind={kind} again={again} setPing={setPing} putBack={putBack}
    saved={kind !== 'alerts' ? d.pings?.[kind] || 'none' : alertPings.every(p => p === alertPings[0]) ? alertPings[0] : 'mixed'} />;

  const timeBox = (id: string, label: string, saved: string, commit: (v: string) => void, ref?: Ref<HTMLInputElement>) =>
    <TextBox className="field sm time" type="time" id={id} aria-label={label} saved={saved ?? ''} again={again} commit={commit} ref={ref} />;
  const weekdays = [1, 2, 3, 4, 5, 6, 0];
  const picked = (s.accounts || []).length;

  return <Page id="discord" title={title}>
    {/* "Playing Bloxkeep" on the person's profile (the Discord app on this PC), once Bloxkeep's Discord application exists. */}
    {d.presenceReady ? <Group id="dc-profile">
      <Row title={t('dc.presence')} desc={t('dc.presenceDesc')}>
        <Switch id="s-dc-presence" label={t('dc.presence')} checked={presence} onChange={async v => {
          setPresence(v);
          if (await call('setDiscordPresence', { enabled: v }) === undefined) putBack();
        }} />
      </Row>
    </Group> : null}
    <RemoteGroup />
    <Group id="discord">
      <Row title={t('dc.on')} desc={t('dc.onDesc')}>
        {/* No link yet: the switch asks for one, and turns on once it is saved. */}
        <Switch id="s-dc-on" label={t('dc.on')} title={d.hasWebhook ? '' : t('dc.needLink')} checked={on} onChange={v => {
          if (v && !d.hasWebhook) return discordLink(true);
          setOn(v);
          set({ enabled: v });
        }} />
      </Row>
      <div id="s-dc-note">{d.stopped ? <div className="set-note"><Icon name="alert" /><span>{t('dc.gone')}</span></div> : null}</div>
      <Row title={t('dc.webhook')} desc={<><span className="path" id="s-dc-hook">{link ?? (d.hasWebhook ? d.webhookHint : t('dc.noLink'))}</span><div>{t('dc.webhookHelp')}</div></>}>
        <button className="btn sm" data-dc="paste" id="s-dc-paste">{t(d.hasWebhook ? 'dc.replace' : 'dc.paste')}</button>
        <button className="btn sm ghost" id="s-dc-reveal" hidden={!d.hasWebhook} aria-pressed={!!link} onClick={async e => {
          if (link) return hide();
          const url = await busy(e.currentTarget, () => call('revealDiscordWebhook')) as string | undefined;
          if (!url) return;
          setShown({ url, hint: d.webhookHint });
          clearTimeout(hideTimer.current);
          hideTimer.current = window.setTimeout(hide, 30_000);
        }}>{link ? <><Icon name="eyeOff" />{t('dc.hide')}</> : <><Icon name="eye" />{t('dc.show')}</>}</button>
        <button className="btn sm ghost" data-dc="remove" id="s-dc-remove" hidden={!d.hasWebhook}>{t('dc.remove')}</button>
      </Row>
      <div id="s-dc-status">{d.hasWebhook ? <div className="set-row">
        <div>
          <div className="t">{d.lastSent ? <Ago iso={d.lastSent} tpl="dc.last" /> : t('dc.never')}</div>
          {d.error ? <div className="d err">{d.error}</div> : null}
          {d.queued ? <div className="d">{t('dc.queued', { n: d.queued })}</div> : null}
        </div>
        <div className="ctl"><button className="btn sm" data-dc="test">{t('dc.test')}</button></div>
      </div> : null}</div>
    </Group>
    <Group id="dc-summary" title={t('dc.summary')}>
      <Row title={t('dc.sendAuto')}>
        <select className="select sm" id="s-dc-when" aria-label={t('dc.sendAuto')} value={when} onChange={e => {
          const v = e.target.value;
          setWhen(v);
          set({ summaryDaily: v === 'daily' || v === 'both', summaryWeekly: v === 'weekly' || v === 'both' });
        }}>
          {[['none', 'dc.send.none'], ['daily', 'dc.daily'], ['weekly', 'dc.weekly'], ['both', 'dc.send.both']].map(([v, k]) => <option key={v} value={v}>{t(k)}</option>)}
        </select>
      </Row>
      <div className="dc-when" id="s-dc-daily-row" hidden={when !== 'daily' && when !== 'both'}>
        <Row title={t('dc.dailyAt')}>{timeBox('s-dc-daily-time', t('dc.dailyAt'), s.dailyTime, time('summaryDailyTime'))}</Row>
      </div>
      <div className="dc-when" id="s-dc-weekly-row" hidden={when !== 'weekly' && when !== 'both'}>
        <Row title={t('dc.weeklyAt')}>
          <select className="select sm" id="s-dc-weekday" aria-label={t('dc.weekday')} value={weekday}
            onChange={e => { setWeekday(e.target.value); set({ summaryWeekday: +e.target.value }); }}>
            {weekdays.map(n => <option key={n} value={n}>{new Date(2026, 9, 11 + n).toLocaleDateString(locale, { weekday: 'long' })}</option>)}
          </select>
          {timeBox('s-dc-weekly-time', t('dc.weeklyAt'), s.weeklyTime, time('summaryWeeklyTime'))}
        </Row>
      </div>
      <Row title={t('dc.after')} desc={t('dc.afterDesc')}>
        <Switch id="s-dc-after" label={t('dc.after')} checked={after} onChange={v => { setAfter(v); set({ summaryAfterSchedule: v }); }} />
      </Row>
      <Row title={t('dc.period')} desc={t('dc.periodDesc')}>
        <select className="select sm" id="s-dc-days" aria-label={t('dc.period')} value={days} onChange={e => { setDays(e.target.value); set({ summaryDays: +e.target.value }); }}>
          {[1, 7, 30].map(n => <option key={n} value={n}>{t(`dc.days.${n}`)}</option>)}
        </select>
      </Row>
      <Row title={t('dc.include')}>
        <button className="btn sm" data-dc="parts" id="s-dc-parts" aria-haspopup="dialog">{tn('dc.partsN', (s.parts || []).length)}</button>
      </Row>
      <Row title={t('dc.accounts')} desc={<span id="s-dc-acc">{picked ? t('dc.accN', { n: picked }) : t('dc.accAll')}</span>}>
        <button className="btn sm" data-dc="accounts">{t('dc.choose')}</button>
      </Row>
      <Row title={t('dc.ping')}>{ping('summary')}</Row>
      <Row title={t('dc.sendNow')} desc={t('dc.sendNowDesc')}>
        <button className="btn sm" data-dc="summary-now" id="s-dc-now">{t('dc.sendNowBtn')}</button>
      </Row>
    </Group>
    <Group id="dc-events" title={t('dc.events')}>
      {/* Picking kinds one by one is in the closed part under it. */}
      <Row title={t('dc.alertsOn')} desc={t('dc.eventsHint')}>
        <Switch id="s-dc-alerts" label={t('dc.alertsOn')} checked={alertsWant ?? NOTIFY_KINDS.some(k => events.includes(k))} disabled={alertsWant !== null}
          onChange={async enabled => {
            setAlertsWant(enabled);
            for (const kind of NOTIFY_KINDS) {
              if (events.includes(kind) !== enabled && await call('setDiscord', { event: { kind, enabled } }) === undefined) break;
            }
            setAlertsWant(null);
            putBack();
          }} />
      </Row>
      <Row title={t('dc.ping')}>{ping('alerts')}</Row>
      <More id="s-dc-kinds" title={t('dc.kinds')}>
        <div className="set-sub">{NOTIFY_KINDS.map(k => <Row key={k} title={t(`set.notify.${k}`)}>
          {ping(k)}
          <Switch id={`s-dc-ev-${k}`} label={t(`set.notify.${k}`)} checked={kinds.includes(k)} onChange={v => {
            setKinds(v ? [...kinds, k] : kinds.filter(x => x !== k));
            set({ event: { kind: k, enabled: v } });
          }} />
        </Row>)}</div>
      </More>
      <p className="set-hint">{t('dc.idHelp')}</p>
    </Group>
    <Group id="dc-board">
      <Row title={t('dc.boardOn')} desc={t('dc.boardDesc')}>
        <select className="select sm" id="s-dc-minutes" aria-label={t('dc.every')} value={minutes}
          onChange={e => { setMinutes(e.target.value); set({ boardMinutes: +e.target.value }); }}>
          {[2, 5, 10, 15, 30, 60].map(m => <option key={m} value={m}>{t('dc.everyN', { n: m })}</option>)}
        </select>
        <Switch id="s-dc-board" label={t('dc.boardOn')} checked={board} onChange={v => { setBoard(v); set({ board: v }); }} />
      </Row>
      <Row title={t('dc.quietOn')} desc={t('dc.quietDesc')}>
        {timeBox('s-dc-quiet-from', t('dc.from'), d.quiet?.from, quietTime('quietFrom'), quietFrom)}
        <span className="dc-dash">–</span>
        {timeBox('s-dc-quiet-to', t('dc.to'), d.quiet?.to, quietTime('quietTo'), quietTo)}
        <Switch id="s-dc-quiet" label={t('dc.quietOn')} checked={quiet} onChange={v => { setQuiet(v); set({ quiet: v }); }} />
      </Row>
    </Group>
    <Group id="dc-look">
      <More id="s-dc-look" title={t('dc.look')}>
        <Row title={t('dc.name')} desc={t('dc.nameDesc')}>
          <TextBox className="field sm" id="s-dc-name" maxLength={80} spellCheck={false} autoComplete="off" aria-label={t('dc.name')}
            saved={d.username ?? ''} again={again} commit={value => {
              const v = value.trim();
              if (v.length > 80 || /discord|clyde/i.test(v)) { toast(t('dc.badName'), 'warn'); return false; }
              set({ username: v });
            }} />
        </Row>
        <Row title={t('dc.avatar')} desc={t('dc.avatarDesc')}>
          <TextBox className="field sm" id="s-dc-avatar" maxLength={2048} spellCheck={false} autoComplete="off" placeholder="https://…" aria-label={t('dc.avatar')}
            saved={d.avatarUrl ?? ''} again={again} commit={value => {
              const v = value.trim();
              if (v && !(/^https:\/\/\S+$/i.test(v) && v.length <= 2048)) { toast(t('dc.badAvatar'), 'warn'); return false; }
              set({ avatarUrl: v });
            }} />
        </Row>
        <Row title={t('dc.lang')}>
          <select className="select sm" id="s-dc-lang" aria-label={t('dc.lang')} value={lang} onChange={e => { setLang(e.target.value); set({ language: e.target.value }); }}>
            <option value="app">{t('dc.lang.app')}</option><option value="en">English</option><option value="vi">Tiếng Việt</option>
          </select>
        </Row>
        <Row title={t('dc.style')} desc={t('dc.styleDesc')}>
          <select className="select sm" id="s-dc-style" aria-label={t('dc.style')} value={style} onChange={e => { setStyle(e.target.value); set({ style: e.target.value }); }}>
            <option value="embed">{t('dc.style.embed')}</option><option value="plain">{t('dc.style.plain')}</option>
          </select>
        </Row>
        <Row title={t('dc.hideNames')} desc={t('dc.hideNamesDesc')}>
          <Switch id="s-dc-hidenames" label={t('dc.hideNames')} checked={hideNames} onChange={v => { setHideNames(v); set({ hideNames: v }); }} />
        </Row>
      </More>
    </Group>
  </Page>;
}

// Who a message pings: nobody, @here and @everyone save at once; a user or a role waits for its ID in the box beside the
// list ("alerts" can also read "Varies by alert", not to be picked).
function Ping({ kind, saved, again, setPing, putBack }: {
  kind: string; saved: string; again: number; setPing: (kind: string, value: string) => void; putBack: () => void;
}) {
  const { t, toast } = useHost();
  const [savedMode, savedId = ''] = saved.split(':');
  const [mode, setMode] = useState(savedMode);
  const [id, setId] = useState(savedId);
  const [idShown, setIdShown] = useState(!!savedId);
  const [bad, setBad] = useState(false);
  // A user or role picked, no ID yet: the app's state mustn't put the old choice back meanwhile.
  const pending = useRef(false);
  const [focusId, setFocusId] = useState(false);
  const list = useRef<HTMLSelectElement>(null), box = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (pending.current) return;
    setMode(savedMode);
    setId(savedId);
    setIdShown(!!savedId);
    setBad(false);
  }, [saved, again]); // savedMode and savedId come from saved
  useLayoutEffect(() => {
    if (!focusId) return;
    setFocusId(false);
    box.current!.focus();
  }, [focusId]);

  // The ID is saved when the box is left or on Enter (its change event), once it looks like one.
  const latest = useRef({ mode, setPing });
  latest.current = { mode, setPing };
  useEffect(() => {
    const el = box.current!;
    const changed = () => {
      const value = el.value.trim();
      if (!value) return;
      const ok = DC_ID.test(value);
      setBad(!ok);
      if (!ok) return toast(t(latest.current.mode === 'role' ? 'dc.badRole' : 'dc.badUser'), 'warn');
      pending.current = false;
      latest.current.setPing(kind, `${latest.current.mode}:${value}`);
    };
    // Left without an ID: the saved choice comes back (unless the person went back to the list itself).
    const left = () => setTimeout(() => {
      if (!pending.current || el.value.trim() || document.activeElement === list.current || document.activeElement === el) return;
      pending.current = false;
      putBack();
    });
    el.addEventListener('change', changed);
    el.addEventListener('blur', left);
    return () => { el.removeEventListener('change', changed); el.removeEventListener('blur', left); };
  }, [kind, t, toast, putBack]);

  return <div className="dc-ping">
    <select className="select sm" data-ping={kind} aria-label={t('dc.ping')} ref={list} value={mode} onChange={e => {
      const v = e.target.value, needsId = v === 'user' || v === 'role';
      setMode(v);
      setIdShown(needsId);
      if (!needsId) {
        pending.current = false;
        return setPing(kind, v);
      }
      // The ID box starts empty: a person's ID kept as a role's (or the other way round) would ping the wrong one.
      setId('');
      setBad(false);
      pending.current = true;
      setFocusId(true);
    }}>
      {PING_MODES.map(v => <option key={v} value={v}>{t(`dc.ping.${v}`)}</option>)}
      {kind === 'alerts' ? <option value="mixed" disabled hidden>{t('dc.pingMixed')}</option> : null}
    </select>
    <input className={`field sm dc-id${bad ? ' bad' : ''}`} data-ping-id={kind} ref={box} inputMode="numeric" maxLength={20} spellCheck={false} autoComplete="off"
      placeholder={t('dc.idPh')} aria-label="ID" hidden={!idShown} value={id} onChange={e => setId(e.target.value)} />
  </div>;
}

// Control from Discord (AppController.Remote.cs): on, this PC takes /pc commands from the Discord account that typed its
// code into /pc link. The code shows until linked, then who it's linked to. Off unlinks it.
function RemoteGroup() {
  const { t, call, busy } = useHost();
  const r = useAppState().remote || {};
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (!r.code) return;
    const timer = window.setInterval(() => setNow(Date.now()), 15_000);
    return () => clearInterval(timer);
  }, [r.code]);
  const left = r.codeUntil ? Math.max(0, Math.ceil((r.codeUntil - now) / 60_000)) : 0;
  return <Group id="dc-remote">
    <Row title={t('dc.remote')} desc={t('dc.remoteDesc')}>
      <Switch id="s-dc-remote" label={t('dc.remote')} checked={!!r.on} onChange={v => call('setRemote', { enabled: v })} />
    </Row>
    {r.on ? <div className="set-row" id="s-dc-remote-state">
      <div>
        {!r.connected ? <div className="t">{t('dc.remoteConnecting')}</div>
          : r.linked ? <>
            <div className="t">{r.user ? t('dc.remoteLinkedTo', { user: r.user }) : t('dc.remoteLinked')}</div>
            <div className="d">{t('dc.remoteUse', { pc: r.pc })}</div>
          </>
          : r.code && left > 0 ? <>
            <div className="t">{t('dc.remoteCode')} <code className="dc-code" id="s-dc-code">{r.code}</code></div>
            <div className="d">{t('dc.remoteCodeHelp', { n: left })}</div>
          </>
          : <><div className="t">{t('dc.remoteNoCode')}</div>{r.busy ? <div className="d warn-text">{t('dc.remoteBusy')}</div> : null}</>}
      </div>
      <div className="ctl">
        {r.connected && !r.linked ? <button className="btn sm" id="s-dc-remote-code" onClick={e => busy(e.currentTarget, () => call('remoteCode'))}>{t('dc.remoteNewCode')}</button> : null}
        {!r.linked ? <button className="btn sm ghost" id="s-dc-remote-add" onClick={() => call('openDiscordApp')}>{t('dc.remoteAdd')}</button> : null}
      </div>
    </div> : null}
  </Group>;
}
