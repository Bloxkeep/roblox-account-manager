// The Macros tab (Beta): a row a macro (its name and one line saying what it is doing, in words), its panel shown or
// not, Run or Stop, and a menu (Edit, Duplicate, Export, move, Delete); the order by dragging the handle. New macro:
// Record, From a template (its cards: what each does and what it needs), Blank, Import file (the same three as large tiles
// while there is none). Stop all, and the recording under way. A failed run's Fix is in its menu: the form opens on the
// step that stopped. The form is MacroEditor.tsx, in app.js's dialog (host.editMacro).
import { Fragment, useEffect, useMemo, useRef, useState, type DragEvent } from 'react';
import { Icon, useAppState, useHost, usePage, type Host } from './host';
import { Pop } from './MacroPop';
import { richNodes } from './rich';
import { MAX_MACROS, OPENERS, STEP_ICON, blankMacro, durText, isLive, keyName, recording, valueOf, type Ctx, type Macro, type Step, type MacroRun, type MacroTemplate, type Recording, type Templates } from './macros';

const NONE: Macro[] = [];
const NO_RUNS: MacroRun[] = [];
type Account = { id: string; label: string; username?: string; avatarUrl?: string | null; state?: string; session?: { pid?: number } | null };
type T = (k: string, v?: Record<string, string | number>) => string;
/** The form, opened on a step (the list's Fix: where the last run stopped, and why). */
const editAt = (host: Host, m: Macro, at?: { step?: number; section?: string | null; reason?: string }) =>
  (host.editMacro as (macro: object, isNew: boolean, opts?: { step?: number; section?: string | null; reason?: string }) => void)(m, false, at);

/** An account with a Roblox window open here: one a macro can run on, record or take a picture of. */
export const hasWindow = (a: Account) => a.state === 'running' && (a.session?.pid || 0) > 0;

/** The app's built-in templates (macroTemplates), asked for once the Macros page is first opened. */
let templatesNow: Templates | null = null;
let templatesAsked: Promise<Templates | null> | null = null;
export function useTemplates(want: boolean): Templates | null {
  const { call } = useHost();
  const [got, setGot] = useState(templatesNow);
  useEffect(() => {
    if (templatesNow || !want) return;
    templatesAsked ||= (call('macroTemplates', {}, { silent: true }) as Promise<Templates | undefined>).then(r => (templatesNow = r || null));
    let alive = true;
    templatesAsked.then(r => { if (alive) setGot(r); if (!r) templatesAsked = null; });
    return () => { alive = false; };
  }, [want]);
  return got;
}

/** One account's run in words: "Main running", "Farm 1 waits its turn", "Farm 2 stopped: didn't see what it looks for". */
export function runText(t: T, r: MacroRun, name: string): string {
  const why = r.reason ? t(`macro.why.${r.reason}`) : '';
  if (r.status === 'waiting' && (!r.reason || r.reason === 'busy')) return t('macro.line.turn', { a: name });
  if (r.status === 'running' && r.reason === 'recovering') return t('macro.line.recovering', { a: name });
  return t(`macro.line.${r.status}${why && r.status !== 'running' && r.status !== 'done' ? 'Why' : ''}`, { a: name, why });
}
/** When it runs and on whom, in words: "Runs when you press Run · Selected accounts". */
export function whenText(t: T, m: Macro): string {
  const when = m.trigger.kind === 'interval' ? t('macro.trig.every', { d: durText(t, m.trigger.everySeconds || 60) }) : t(`macro.when.${m.trigger.kind}`);
  const who = m.targets === 'accounts' ? t(m.accountIds.length === 1 ? 'macro.tg.n1' : 'macro.tg.n', { n: m.accountIds.length }) : t(`macro.tg.${m.targets}`);
  return [when, who, m.hotkey ? `Ctrl+Alt+${m.hotkey}` : ''].filter(Boolean).join(' · ');
}

export function MacrosView() {
  const host = useHost();
  const { t, call, busy, toast, on } = host;
  const app = useAppState();
  const all: Macro[] = app.settings?.macros || NONE;
  // Try it's runs are the editor's: the list shows the real ones.
  const runs: MacroRun[] = useMemo(() => ((app.macroRuns || NO_RUNS) as MacroRun[]).filter(r => !r.trial), [app.macroRuns]);
  const rec: Recording = app.macroRecording || null;
  const accounts: Account[] = app.accounts || [];
  const panels = new Set<string>(app.panelsShown || []);
  const byId = new Map(accounts.map(a => [a.id, a]));
  const selected = usePage(p => p.selected);
  // Macros on the selected accounts that still have somewhere to run with none selected (the game's, or the front window).
  const canRun = new Set<string>(app.macroCanRun || []);
  const list = useRef<HTMLDivElement>(null);

  // New macro › Record finished: the steps open in a new macro's form.
  useEffect(() => on('macroRecorded', (d?: { steps: object[]; w: number; h: number }) => {
    if (recording.for !== 'new') return;
    recording.for = null;
    if (!d?.steps?.length) return toast(t('macro.nothingRecorded'), 'info');
    host.editMacro({ ...blankMacro(), steps: d.steps, recordedSize: { w: d.w, h: d.h } }, true);
  }), [on, host]);

  // Run changes are said aloud (a screen reader), not only shown: the newest few, in plain words.
  const [said, setSaid] = useState('');
  const before = useRef<Map<string, string> | null>(null);
  useEffect(() => {
    const now = new Map(runs.map(r => [`${r.macroId}|${r.accountId}`, `${r.status}|${r.reason || ''}`]));
    const was = before.current;
    before.current = now;
    if (!was) return;
    const changed = runs.filter(r => was.get(`${r.macroId}|${r.accountId}`) !== now.get(`${r.macroId}|${r.accountId}`));
    if (changed.length) setSaid(changed.slice(-3).map(r => `${all.find(m => m.id === r.macroId)?.name || ''}: ${runText(t, r, byId.get(r.accountId)?.label || '')}`).join('. '));
  }, [runs]);

  const templates = useTemplates(usePage(p => p.view) === 'macros');
  const tpls: MacroTemplate[] = templates?.macros || [];
  const full = () => all.length >= MAX_MACROS && (toast(t('macro.full', { n: MAX_MACROS }), 'warn'), true);
  const startNew = (m: Macro) => { if (!full()) host.editMacro(m, true); };
  const fromTemplate = (tp: MacroTemplate) => {
    const { about: _a, icon: _i, ...macro } = tp;
    void _a; void _i;
    startNew({ ...blankMacro(), ...macro, id: '', steps: macro.steps.map(x => ({ ...x })) });
  };
  // The templates as cards under the button: what each does and what it needs from the game. Its button again closes them.
  const [cards, setCards] = useState<HTMLElement | null>(null);
  const templateCards = (btn: HTMLElement) => setCards(was => was === btn ? null : btn);
  // Recording follows one window: the only one open, or the one picked from a list of those open.
  const record = (anchor: HTMLElement) => {
    if (full()) return;
    const windows = accounts.filter(hasWindow);
    const start = async (accountId: string) => {
      recording.for = 'new';
      if (await call('recordMacro', { accountId }) === undefined) recording.for = null;
    };
    if (!windows.length) return toast(t('macro.need.record'), 'info');
    if (windows.length === 1) return start(windows[0].id);
    host.menu(anchor, [{ head: true, text: t('macro.recordWhich') }, ...windows.map(a => ({ icon: 'user', text: a.label, run: () => start(a.id) }))]);
  };
  const importFile = async () => {
    if (full()) return;
    const m = await call('importMacro', {}) as Macro | null | undefined;
    if (m) host.editMacro({ ...blankMacro(), ...m, id: '' }, true);
  };
  const newMenu = (btn: HTMLElement) => host.menu(btn, [
    { icon: 'record', text: t('macro.record'), hint: 'F8', run: () => record(btn) },
    { icon: 'layers', text: t('macro.fromTemplate'), run: () => setTimeout(() => templateCards(btn)) },
    { icon: 'plus', text: t('macro.empty'), run: () => startNew(blankMacro()) },
    { icon: 'folder', text: t('macro.import'), run: () => importFile() },
  ]);
  const anyLive = runs.some(isLive);
  const stopAll = (btn: HTMLElement) => busy(btn, async () => {
    if (await call('stopMacros', {}) !== undefined) toast(t('macro.stoppedAll'), 'ok');
  });

  // Dragging by the handle only, as in Presets: the rows show the new order until the app answers; Esc or a drop outside undoes.
  const [armed, setArmed] = useState<string | null>(null);
  const [drag, setDrag] = useState<{ id: string; order: string[]; done?: boolean } | null>(null);
  const rows = drag ? drag.order.map(id => all.find(m => m.id === id)).filter((m): m is Macro => !!m) : all;
  const dragOver = (e: DragEvent<HTMLDivElement>) => {
    if (!drag || drag.done) return;
    e.preventDefault();
    e.dataTransfer.dropEffect = 'move';
    const over = (e.target as HTMLElement).closest<HTMLElement>('.mc-item');
    if (!over || over.dataset.id === drag.id) return;
    const r = over.getBoundingClientRect();
    const order = drag.order.filter(id => id !== drag.id);
    order.splice(order.indexOf(over.dataset.id!) + (e.clientY < r.top + r.height / 2 ? 0 : 1), 0, drag.id);
    if (order.join() !== drag.order.join()) setDrag({ ...drag, order });
  };
  const dragEnd = async (e: DragEvent<HTMLDivElement>) => {
    setArmed(null);
    if (!drag) return;
    const to = drag.order.indexOf(drag.id);
    if (e.dataTransfer.dropEffect === 'none' || to === all.findIndex(m => m.id === drag.id)) return setDrag(null);
    setDrag({ ...drag, done: true });
    await call('moveMacro', { id: drag.id, to });
    setDrag(null);
  };
  const move = async (id: string, dir: number) => {
    const i = all.findIndex(m => m.id === id), to = i + dir;
    if (i < 0 || to < 0 || to >= all.length) return;
    await call('moveMacro', { id, to });
  };

  const live = new Set(runs.filter(isLive).map(r => r.macroId)).size;
  const sub = all.length ? [t(all.length === 1 ? 'macro.count1' : 'macro.countN', { n: all.length }), live ? t('macro.runningN', { n: live }) : ''].filter(Boolean) : [];

  return <>
    <header className="top">
      <div className="top-title">
        <h1>{t('macro.title')}<span className="state-pill warn mc-beta" title={`${t('macro.betaTip')} ${t('macro.honest')}`}>{t('macro.beta')}</span></h1>
        <p id="mc-sub">{sub.map((s, i) => <Fragment key={i}>{i ? <span className="sep">·</span> : null}{richNodes(s)}</Fragment>)}</p>
      </div>
      <div className="top-tools">
        <button className="btn mc-stop-all" disabled={!anyLive && !rec} title={t('macro.stopAllTip')} onClick={e => stopAll(e.currentTarget)}>
          <Icon name="stop" /><span>{t('macro.stopAll')}</span><kbd>Ctrl+Alt+0</kbd></button>
        <button className="btn primary" id="mc-new" aria-haspopup="menu" onClick={e => { if (cards === e.currentTarget) return setCards(null); newMenu(e.currentTarget); }}>
          <Icon name="plus" /><span>{t('macro.new')}</span><Icon name="chevronDown" /></button>
      </div>
    </header>
    {rec ? <RecordingBar rec={rec} label={byId.get(rec.accountId)?.label || ''} /> : null}
    <p className="mc-live" role="status" aria-live="polite">{said}</p>
    <div className="scroll">
      <div className="pv-list mc-list" id="mc-list" role="list" ref={list} onPointerUp={() => setArmed(null)} onDragOver={dragOver}
        onDrop={e => e.preventDefault()} onDragEnd={dragEnd}>
        {!all.length ? <div className="empty mc-empty">
          <span className="pv-empty-icon"><Icon name="keyboard" /></span><h2>{t('macro.emptyTitle')}</h2><p>{t('macro.emptyText')}</p>
          <div className="mc-tiles">
            <button type="button" className="mc-tile" data-start="record" onClick={e => record(e.currentTarget)}><Icon name="record" />
              <b>{t('macro.record')}</b><small>{t('macro.tile.record')}</small></button>
            <button type="button" className="mc-tile" data-start="template" aria-haspopup="dialog" aria-expanded={!!cards && cards.dataset.start === 'template'} onClick={e => templateCards(e.currentTarget)}><Icon name="layers" />
              <b>{t('macro.fromTemplate')}</b><small>{t('macro.tile.template')}</small></button>
            <button type="button" className="mc-tile" data-start="blank" onClick={() => startNew(blankMacro())}><Icon name="plus" />
              <b>{t('macro.empty')}</b><small>{t('macro.tile.blank')}</small></button>
          </div>
          <button type="button" className="link" onClick={() => importFile()}>{t('macro.importLink')}</button>
        </div>
          : rows.map((m, i) => <MacroRow key={m.id} m={m} i={i} count={rows.length} runs={runs.filter(r => r.macroId === m.id)} byId={byId}
            shown={panels.has(m.id)} selected={selected} canRun={canRun.has(m.id)} dragging={drag?.id === m.id && !drag.done} draggable={armed === m.id} arm={() => setArmed(m.id)} move={move}
            dragStart={e => {
              if (armed !== m.id) return;
              setDrag({ id: m.id, order: rows.map(x => x.id) });
              e.dataTransfer.effectAllowed = 'move';
              e.dataTransfer.setData('text/plain', m.id); // Chromium needs some data for the drag to start
            }} />)}
      </div>
    </div>
    {cards ? <Pop anchor={cards} close={() => setCards(null)} label={t('macro.fromTemplate')} className="menu mc-tpl-pop">
      {tpls.length ? tpls.map(tp => <button key={tp.name} type="button" className="mc-tpl" onClick={() => { setCards(null); fromTemplate(tp); }}>
        <Icon name={TPL_ICON[tp.icon || ''] || tp.icon || 'keyboard'} /><span><b>{tp.name}</b>{tp.about ? <small>{tp.about}</small> : null}
          {tp.steps.length ? <em>{needsText(t, host.locale, tp)}</em> : null}</span></button>)
        : <p className="mc-add-none">{t('macro.noTemplates')}</p>}
    </Pop> : null}
  </>;
}

/** The templates' icon words (MacroTemplates.cs) as the page's icons. */
const TPL_ICON: Record<string, string> = { pointer: 'mouse', bar: 'gauge', rejoin: 'refresh', blank: 'plus' };

/** What a template needs from the game before it works (a picture of the button, an area on it), or that it is ready as it is. */
function needsText(t: T, locale: string, tp: MacroTemplate): string {
  const todo = new Set(tp.steps.flatMap(s => (s.todo as string[] | undefined) || []));
  const pic = todo.has('image') || tp.steps.some(s => (s.kind === 'clickImage' || s.kind === 'waitImage') && !s.image);
  const area = todo.has('region') || !!tp.areas?.length;
  const what = [pic ? t('macro.tpl.needs.pic') : '', area ? t('macro.tpl.needs.area') : ''].filter(Boolean);
  return what.length ? t('macro.tpl.needs', { what: new Intl.ListFormat(locale, { type: 'conjunction' }).format(what) }) : t('macro.tpl.needs.none');
}

/** The recording under way: its countdown, then how many steps so far; F8 or Stop ends it. */
function RecordingBar({ rec, label }: { rec: NonNullable<Recording>; label: string }) {
  const { t, call, busy } = useHost();
  const left = useCountdown(rec.startsAt);
  return <div className="mc-rec" role="status">
    <span className="mc-rec-dot" aria-hidden="true" />
    <b>{left > 0 ? t('macro.rec.starting', { name: label, n: left }) : t('macro.rec.on', { name: label, n: rec.steps })}</b>
    <small>{t('macro.rec.hint')}</small>
    <button className="btn sm" onClick={e => busy(e.currentTarget, () => call('stopRecording', {}))}><Icon name="stop" /><span>{t('macro.rec.stop')}</span></button>
  </div>;
}

/** Whole seconds until a time, counting down while shown. */
export function useCountdown(iso: string): number {
  const [, tick] = useState(0);
  const left = Math.max(0, Math.ceil((new Date(iso).getTime() - Date.now()) / 1000));
  useEffect(() => {
    if (left <= 0) return;
    const timer = setTimeout(() => tick(n => n + 1), 250);
    return () => clearTimeout(timer);
  });
  return left;
}

function MacroRow({ m, i, count, runs, byId, shown, selected, canRun, dragging, draggable, arm, dragStart, move }: {
  m: Macro; i: number; count: number; runs: MacroRun[]; byId: Map<string, Account>; shown: boolean; selected: Set<string>; canRun: boolean; dragging: boolean;
  draggable: boolean; arm: () => void; dragStart: (e: DragEvent<HTMLDivElement>) => void; move: (id: string, dir: number) => void;
}) {
  const host = useHost();
  const { t, call, busy, toast } = host;
  const auto = m.trigger.kind !== 'manual';
  const liveRuns = runs.filter(isLive);
  const accs = m.targets === 'accounts' ? m.accountIds.filter(id => byId.has(id)) : [];
  // On the selected accounts: those selected, else where the app would run it (the game's, the front window); with
  // neither it would only be refused, so the button says why instead.
  const sel = [...selected].filter(id => byId.has(id));
  const cantRun = (m.targets === 'selected' && !sel.length && !canRun) || (m.targets === 'accounts' && !accs.length);
  // One line: what each account is doing (a dot in its colour each), or (none running) when it runs and on whom.
  const failed = runs.find(r => r.status === 'failed');
  const said = runs.map(r => runText(t, r, byId.get(r.accountId)?.label || '?'));
  const line = runs.length ? said.join(' · ') : [whenText(t, m), auto && !m.enabled ? t('macro.autoOff') : ''].filter(Boolean).join(' · ');
  const fix = () => editAt(host, m, failed ? { step: failed.lastStep ?? failed.step, section: failed.section, reason: failed.reason } : undefined);

  const panel = (btn: HTMLElement) => busy(btn, () => call(shown ? 'hidePanel' : 'showPanel', { id: m.id }));
  const menu = (btn: HTMLElement) => host.menu(btn, [
    ...(failed ? [{ icon: 'wrench', text: t('macro.fix'), run: fix }] : []),
    { icon: 'edit', text: t('macro.edit'), run: () => host.editMacro(m, false) },
    { icon: 'copy', text: t('macro.duplicate'), run: async () => { if (await call('duplicateMacro', { id: m.id }) !== undefined) toast(t('macro.duplicated'), 'ok'); } },
    { icon: 'download', text: t('macro.export'), run: async () => { if (await call('exportMacro', { id: m.id })) toast(t('macro.exported'), 'ok'); } },
    ...(auto ? [{ icon: m.enabled ? 'pause' : 'play', text: t(m.enabled ? 'macro.autoPause' : 'macro.autoResume'), run: () => { call('saveMacro', { macro: { ...m, enabled: !m.enabled } }); } }] : []),
    ...(count > 1 ? ['-' as const,
      { icon: 'chevronUp', text: t('lo.up'), disabled: i === 0, run: () => move(m.id, -1) },
      { icon: 'chevronDown', text: t('lo.down'), disabled: i >= count - 1, run: () => move(m.id, 1) }] : []),
    '-', { icon: 'trash', text: t('macro.delete'), danger: true, run: () => { host.deleteMacro(m); } },
  ]);

  return <div className={`pv-item mc-item${dragging ? ' dragging' : ''}`} role="listitem" data-id={m.id} draggable={draggable} onDragStart={dragStart} aria-label={m.name}>
    {count > 1 ? <span className="ord-grip pv-grip" title={t('lo.drag')} onPointerDown={arm}><Icon name="grip" /></span> : null}
    <span className={`pv-icon mc-icon${liveRuns.length ? ' on' : ''}`}><Icon name={auto ? 'repeat' : 'keyboard'} />
      {failed ? <i className="mc-icon-bad" title={t('macro.fixTip')} aria-hidden="true" /> : null}</span>
    <div className="pv-main">
      <b title={m.name}>{m.name}</b>
      <span className={`mc-line${failed ? ' bad' : liveRuns.length ? ' on' : ''}`} title={line}>{runs.length
        ? runs.map((r, k) => <span key={r.accountId} className={`mc-acc s-${r.status}`}><i aria-hidden="true" />{said[k]}</span>) : line}</span>
    </div>
    <Shape m={m} />
    <button type="button" className={`btn sm mc-panel${shown ? ' on' : ''}`} data-mc="panel" aria-pressed={shown} title={t(shown ? 'macro.panelHide' : 'macro.panelShow')}
      aria-label={`${t('macro.panel')}: ${m.name}`} onClick={e => panel(e.currentTarget)}>{shown ? <i className="mc-panel-dot" aria-hidden="true" /> : <Icon name="screens" />}
      <span>{t('macro.panel')}</span></button>
    {/* Run and Stop are two buttons (keyed), as Presets' Run and Close: Enter on Stop doesn't start it again. */}
    {liveRuns.length
      ? <button key="stop" className="btn sm pv-run mc-stop" data-mc="stop" aria-label={`${t('macro.stop')}: ${m.name}`}
        onClick={e => busy(e.currentTarget, async () => { if (await call('stopMacros', { id: m.id }) !== undefined) toast(t('macro.stopped'), 'ok'); })}>
        <Icon name="stop" /><span>{t('macro.stop')}</span></button>
      : <button key="run" className="btn sm pv-run" data-mc="run" aria-label={`${t('macro.run')}: ${m.name}`} disabled={cantRun}
        title={cantRun ? t(m.targets === 'selected' ? 'macro.needSelect' : 'macro.noAccountsLeft') : undefined}
        onClick={e => busy(e.currentTarget, () => host.runMacro(m.id, m.targets === 'selected' && sel.length ? sel : undefined))}><Icon name="play" /><span>{t('macro.run')}</span></button>}
    <button type="button" className="icon-btn" data-mc="more" aria-haspopup="menu" title={t('macro.moreActs')} aria-label={`${t('macro.moreActs')}: ${m.name}`}
      onClick={e => menu(e.currentTarget)}><Icon name="more" /></button>
  </div>;
}

/** The most steps a row's shape shows; more say how many. */
const SHAPE_MAX = 14;

/** A macro at a glance: its steps in order as small chips, a key's own label (E, Space), a wait's seconds and a loop's
 * count on theirs, a block's end a short bar; a macro is known by its shape before its text is read. */
function Shape({ m }: { m: Ctx & { steps: Step[] } }) {
  const { t } = useHost();
  const steps = m.steps;
  if (!steps.length) return null;
  // A key or a wait set by one of the macro's choices ("$o2") shows the choice's value.
  const v = (x: unknown) => valueOf(m, x);
  const secs = (ms: number) => `${Math.round(ms / 100) / 10}s`;
  return <span className="mc-shape" aria-hidden="true">{steps.slice(0, SHAPE_MAX).map((s, i) => {
    if (s.kind === 'endIf' || s.kind === 'endLoop') return <i key={i} className="mc-sh-end" />;
    const key = v(s.key), ms = v(s.ms);
    const label = (s.kind === 'key' || s.kind === 'down' || s.kind === 'up') && typeof key === 'string' && key ? keyName(t, key)
      : s.kind === 'wait' && typeof ms === 'number' ? secs(ms)
      : s.kind === 'loop' ? (typeof s.times === 'number' && s.times > 0 ? `×${s.times}` : s.times ? '' : '∞') : '';
    // A key or a wait says itself; a loop keeps its icon beside its count.
    const icon = !label || s.kind === 'loop';
    return <span key={i} className={`mc-sh${OPENERS.has(s.kind) ? ' opens' : ''}`}>{icon ? <Icon name={STEP_ICON[s.kind] || 'info'} /> : null}
      {label ? <b>{label}</b> : null}</span>;
  })}{steps.length > SHAPE_MAX ? <small>+{steps.length - SHAPE_MAX}</small> : null}</span>;
}
