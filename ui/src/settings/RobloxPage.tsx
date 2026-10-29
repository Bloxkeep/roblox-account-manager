// Settings › Roblox version & graphics: Roblox closing windows, the installed and newest builds, what the watch found in
// the next builds, builds seen here (with release-notes links); then the settings Bloxkeep gives Roblox before it opens.
import { useLayoutEffect, useRef, type ReactNode } from 'react';
import { Icon, useAppState, useHost } from '../host';
import { Group, Page, Row, useLocal } from './parts';

/** Roblox's id for a build ("version-cec3ad5889b447cf"), shortened to its first 8 characters. */
const shortBuild = (b: string) => String(b || '').replace(/^version-/i, '').slice(0, 8);

/** A pill: a short state in colour (ok, warn, off). */
const Pill = ({ tone, title, children }: { tone: string; title?: string; children: ReactNode }) =>
  <span className={`state-pill ${tone}`} title={title}>{children}</span>;

const RC_LISTS: Record<string, [string, string | null][]> = {
  mode: [['off', null], ['all', null], ['marked', null]],
  graphics: [['0', null], ...[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(n => [String(n), String(n)] as [string, string])],
  fps: [['0', null], ...[30, 60, 144, 240].map(n => [String(n), `${n} FPS`] as [string, string])],
  volume: [['-1', null], ['0', null], ...[25, 50, 75, 100].map(n => [String(n), `${n}%`] as [string, string])],
};

export function RobloxPage({ title }: { title: string }) {
  const { t, tn, lang, locale } = useHost();
  const app = useAppState();
  const ru = app.robloxUpdates || {}, rv = app.robloxVersion || {};

  /** A date and time in the app's language ("Oct 13, 2026, 8:25 PM"; 24-hour in Vietnamese). */
  const fmtWhen = (iso: string) => new Date(iso).toLocaleString(locale, lang === 'vi'
    ? { dateStyle: 'medium', timeStyle: 'short', hourCycle: 'h23' } : { dateStyle: 'medium', timeStyle: 'short' });
  /** The watch's verdict on a build as [tone, words]; two windows having played together on it here outweigh it; null if unchecked. */
  const watchWord = (check: string | null, worked?: boolean, where = 'rw'): [string, string] | null => {
    const key = worked ? 'worked' : check === 'same' || check === 'first' ? 'same' : ['changed', 'renamed', 'gone'].includes(check || '') ? check : null;
    return key ? [key === 'worked' || key === 'same' ? 'ok' : 'warn', t(`${where}.${key}`)] : null;
  };

  const w = ru.watch || {};
  // The watch found the lock changed in the build Multi-Roblox meets next, and it hasn't worked on it here since.
  const watchWarns = !!ru.watch && ['changed', 'renamed', 'gone'].includes(w.check) && !w.worked;
  const word = watchWord(w.check, w.worked) || ['off', t(w.readAt ? 'rw.unchecked' : 'rbx.unknown')];
  const watchDesc = [w.updatesFirst && w.version ? t('rw.updatesTo', { v: w.version }) : '', t('rw.desc'),
    w.held?.length ? t('rw.held', { n: w.held.join(', ') }) : ''].filter(Boolean).join(' ');
  const nextWord = w.next && watchWord(w.next.check);
  const c = ru.closed;
  const inst = ru.installed;
  const hist: any[] = ru.history || [];
  // Out of date: the launcher that keeps this copy of Roblox can install the new one now (Update now), when there is one.
  const canUpdate = rv.upToDate === false && !!rv.updater;
  const rvText = !rv.installed ? t('rbx.notInstalled')
    : canUpdate ? t('rbx.outdatedUpdate', { v: rv.latest, l: rv.updater })
    : rv.upToDate === false ? t(rv.store ? 'rbx.outdatedStore' : rv.direct ? 'rbx.outdatedDirect' : 'rbx.outdated', { v: rv.latest })
    : rv.upToDate ? t('rbx.latest') : t('rbx.unknown');

  // The warning went (Hide) with the focus in it: on to the page's next control, not back to the top of the window.
  const closedBox = useRef<HTMLDivElement>(null);
  const closedFocused = useRef(false);
  closedFocused.current = !!closedBox.current?.contains(document.activeElement);
  useLayoutEffect(() => {
    const box = closedBox.current!, active = document.activeElement;
    if (!closedFocused.current || box.contains(active) || (active && active !== document.body)) return;
    const page = box.closest<HTMLElement>('.set-page')!;
    const next = [...page.querySelectorAll<HTMLElement>('button, input, select, a[href]')]
      .find(x => box.compareDocumentPosition(x) & Node.DOCUMENT_POSITION_FOLLOWING && !(x as HTMLButtonElement).disabled && x.offsetParent);
    (next || document.querySelector<HTMLElement>(`#view-settings [data-set-page="${page.dataset.page}"]`))?.focus();
  });

  const rc = app.settings?.robloxConfig || { mode: 'off', graphics: 0, fps: 0, volume: -1 };
  const chosen = (app.accounts || []).filter((a: { useRobloxConfig?: boolean }) => a.useRobloxConfig).length;

  return <Page id="roblox" title={title}>
    <Group id="rbxupd" title={t('ru.title')}>
      <div id="s-rbxclosed" ref={closedBox}>
        {c ? <div className="set-note"><Icon name="alert" /><span>
          {tn('ru.closed', c.count, { t: fmtWhen(c.at) })}{c.updatedAt ? ` ${t('ru.closedUpdate', { t: fmtWhen(c.updatedAt) })}` : ''}
          {' '}<button type="button" className="link" data-act="ru-hide">{t('ru.hide')}</button>
        </span></div> : null}
      </div>
      <div id="s-rbxver">
        <Row title={t('rbx.title')} desc={rvText}><Pill tone={rv.upToDate === false ? 'warn' : rv.upToDate ? 'ok' : 'off'}>{rv.installed || '—'}</Pill>
          {canUpdate ? <button type="button" className="btn sm" data-act="rbx-update" disabled={!!rv.updating}>{t(rv.updating ? 'rbx.updating' : 'rbx.update')}</button> : null}</Row>
      </div>
      <div id="s-rbxupd">
        <Row title={t('ru.installed')} desc={inst?.source ? t('ru.via', { s: inst.source }) : ''}>
          {inst ? <span className="ru-build" title={inst.build}>{t('ru.build', { b: shortBuild(inst.build) })}</span> : '—'}
        </Row>
        {/* The newest version only when it isn't the one installed (the row above already shows that number). */}
        {rv.latest && rv.latest === rv.installed ? null
          : <Row title={t('ru.newest')}><Pill tone={rv.latest ? 'off' : 'warn'}>{rv.latest || t('rbx.unknown')}</Pill></Row>}
        <Row title={t('rw.title')} desc={<>{watchDesc} <button type="button" className="link" data-act="rw-how">{t('rw.how')}</button></>}>
          <Pill tone={word[0]} title={w.version ? `Roblox ${w.version}` : undefined}>{word[1]}</Pill>
        </Row>
        {nextWord ? <Row title={t('rw.next')} desc={t('rw.nextDesc', { v: w.next.version })}><Pill tone={nextWord[0]}>{nextWord[1]}</Pill></Row> : null}
        {watchWarns || c ? <div className="set-note"><Icon name="info" /><span>
          {t(c ? 'rw.todoClosed' : 'rw.todo')} <button type="button" className="link" data-act="rw-check">{t('rw.checkBloxkeep')}</button>
        </span></div> : null}
        <div className="ru-head">{t('ru.history')}</div>
        {hist.length ? <ul className="ru-list">{hist.map((h, i) => {
          const older = hist[i + 1];
          const note = h.first ? t('ru.first') : older && older.version === h.version ? t('ru.sameVersion') : '';
          const check = watchWord(h.check, h.worked, 'rw.h');
          return <li className="ru-item" key={`${h.build}-${h.at}`}>
            <div className="ru-text">
              <div className="ru-main"><b>{h.version}</b>{note ? <span className="ru-note">{note}</span> : null}{check ? <span className={`ru-note ${check[0]}`}>{check[1]}</span> : null}</div>
              <div className="ru-sub">{fmtWhen(h.at)} · <span title={h.build}>{t('ru.build', { b: shortBuild(h.build) })}</span>{h.source ? ` · ${h.source}` : ''}</div>
            </div>
            <button type="button" className="link ru-link" data-act="ru-notes" data-v={h.version} title={`${t('ru.notes')} — ${t('ru.notesHint')}`}
              aria-label={`${t('ru.notes')} ${h.version}`}>{t('ru.notesShort')}</button>
          </li>;
        })}</ul> : <p className="set-hint">{t('ru.none')}</p>}
      </div>
    </Group>
    <Group id="rc" title={t('rc.title')}>
      <Row title={t('rc.mode')} desc={<span id="s-rc-hint">{rc.mode === 'off' ? '' : rc.mode === 'all' ? t('rc.hint.all')
        : <>{tn('rc.hint.chosen', chosen)} <button type="button" className="link" data-act="rc-pick">{t('rc.pick')}</button></>}</span>}>
        <RcSelect name="mode" rc={rc} />
      </Row>
      <Row title={t('rc.graphics')}><RcSelect name="graphics" rc={rc} /></Row>
      <Row title={t('rc.fps')}><RcSelect name="fps" rc={rc} /></Row>
      <Row title={t('rc.volume')}><RcSelect name="volume" rc={rc} /></Row>
    </Group>
  </Page>;
}

/** A Roblox setting: a value its list doesn't offer shows the first choice ("leave as is"); all but the mode wait while it's off. */
function RcSelect({ name, rc }: { name: string; rc: Record<string, string | number> }) {
  const { t, call } = useHost();
  const list = RC_LISTS[name];
  const saved = String(rc[name]);
  const [value, setValue] = useLocal(list.some(([v]) => v === saved) ? saved : list[0][0]);
  const words = (v: string, own: string | null) => own ?? (name === 'mode' ? t(`rc.mode.${v}`) : v === '0' && name === 'volume' ? t('rc.mute') : t('rc.keep'));
  return <select className="select" id={`s-rc-${name}`} aria-label={t(`rc.${name}`)} value={value} disabled={name !== 'mode' && rc.mode === 'off'} onChange={e => {
    setValue(e.target.value);
    call('setSettings', { robloxConfig: { [name]: name === 'mode' ? e.target.value : +e.target.value } });
  }}>
    {list.map(([v, own]) => <option key={v} value={v}>{words(v, own)}</option>)}
  </select>;
}
