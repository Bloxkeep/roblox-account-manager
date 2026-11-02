// The macro form (new, from a template, recorded, imported, or edit). On top: its name, one folded line for when and
// where it runs, and Try it. Left: the steps as numbered sentences; the open one shows its few values in the sentence
// itself, each with a "Player can change" tick that puts it on the panel (an option: its label from the sentence, its
// range and unit from the value, never asked). Areas are picked by name. Under the steps, what the player sees: the
// panel, its labels editable in place. Right: Edit on game (the dots and boxes drawn on the Roblox window, dragged
// there), the same dots and boxes on a picture of the window, and Try it's results as plain lines. app.js's dialog keeps
// the title and buttons; it asks this form what to save (collect) and whether anything changed (dirty).
import { Fragment, useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from 'react';
import { flushSync } from 'react-dom';
import { Icon, useAppState, useHost } from './host';
import { Avatar } from './Avatar';
import { ColorInput } from './ColorField';
import { GameBox } from './PresetForm';
import { GameFrame, TryLines, type Picture, type Seen } from './MacroPicture';
import { PlayerSees } from './MacroPanel';
import { hasWindow, useCountdown } from './MacrosView';
import {
  AREA_COLOURS, COMMON_STEPS, FREE, asPoint, KEYS, LOOK_STEPS, MAX_AREAS, MAX_OPTIONS, MAX_TABS, MOUSE_KEYS, STEP_GROUPS, STEP_ICON, camDir, counters, depths, durText, freeName, isArea, isOpt,
  isLive, keyCap, keyFromEvent, keyName, newTest, pointOf, problems, recording, stepText, stepsToAdd, withAreas,
  type Area, type Macro, type MacroRun, type Option, type Rect, type Step, type Test,
} from './macros';

type Account = { id: string; label: string; username?: string; avatarUrl?: string | null; state?: string; session?: { pid?: number } | null };
export interface MacroEditorApi {
  /** What Save sends (saveMacro's macro), or the first thing stopping it and the box to show. */
  collect(): { macro: Macro } | { error: string; box: HTMLElement | null };
  dirty(): boolean;
}
/** A step as the form holds it: its own key for React (taken off before saving). */
type Held = Step & { _k: number };

let seq = 0;
const hold = (s: Step): Held => ({ ...s, _k: ++seq });
const strip = ({ _k, ...s }: Held): Step => { void _k; return s; };
const getAt = (s: Step, path: string): any => path.split('.').reduce<any>((o, k) => o?.[k], s);
const setAt = <S extends Step>(s: S, path: string, v: unknown): S => {
  const [a, b] = path.split('.');
  return (b ? { ...s, [a]: { ...s[a], [b]: v } } : { ...s, [a]: v }) as S;
};

/** The values a maker may hand to the player, by step kind and field: what the option is, its range and unit, and the
 * label it starts with (the player sees it on the panel; the maker can rename it there). */
type Meta = { kind: Option['kind']; min?: number; max?: number; unit?: Option['unit']; label: string };
const META: Record<string, Meta> = {
  'key.key': { kind: 'key', label: 'macro.opt.key' },
  'click.point': { kind: 'point', label: 'macro.opt.point' },
  'move.point': { kind: 'point', label: 'macro.opt.point' },
  'click.holdMs': { kind: 'number', min: 0, max: 10000, unit: 's', label: 'macro.opt.hold' },
  'clickColour.eachAfterMs': { kind: 'number', min: 0, max: 10000, unit: 's', label: 'macro.opt.after' },
  'wait.ms': { kind: 'number', min: 0, max: 3_600_000, unit: 's', label: 'macro.opt.wait' },
  'waitColour.colour': { kind: 'colour', label: 'macro.opt.colour' },
  'clickColour.colour': { kind: 'colour', label: 'macro.opt.colour' },
  'follow.target': { kind: 'colour', label: 'macro.opt.target' },
  'follow.marker': { kind: 'colour', label: 'macro.opt.marker' },
  'loop.times': { kind: 'number', min: 1, max: 100000, unit: '', label: 'macro.opt.times' },
  'if.test.value': { kind: 'number', min: 0, max: 100000, unit: '', label: 'macro.opt.count' },
  'loopUntil.test.value': { kind: 'number', min: 0, max: 100000, unit: '', label: 'macro.opt.count' },
};
/** A camera turn's size in words, as pixels of mouse drag. */
const CAM = { little: 100, some: 250, lot: 500 } as const;
const OPS = ['>=', '<=', '=', '>', '<', '!='];
/** Steps that look for something on the window: Try it says it found it once they're passed. */
const LOOKS = new Set(['waitImage', 'clickImage', 'waitColour', 'clickColour', 'waitText', 'if', 'loopUntil', 'follow']);
const BLOCKS = new Set(['loop', 'endLoop', 'else', 'endIf', 'break']);

/** The form, with what its steps' sentences need: values, writing them, the ticks, the areas. */
type Ed = {
  val: (i: number, path: string) => any; put: (i: number, path: string, v: unknown) => void; set: (i: number, p: Partial<Step>) => void;
  ticked: (i: number, path: string) => boolean; tick: (i: number, path: string, on: boolean) => void;
  areas: Area[]; newArea: () => string | null; pick: (i: number, what: 'point' | 'colour' | 'target' | 'marker', path?: string) => void; cut: (i: number) => void;
  canPick: boolean; capture: number | null; setCapture: (k: number | null) => void;
  /** The counters (Set and Add steps' names) and the settings an If can read. */
  counters: string[]; readable: Option[];
};

export function MacroEditor({ base, accounts, macros, api }: {
  base: Macro; isNew?: boolean; accounts: Account[]; macros: Macro[]; api: Ref<MacroEditorApi>;
}) {
  const host = useHost();
  const { t, tn, call, busy, toast, on } = host;
  const app = useAppState();

  // A macro from before areas gets its drawn boxes as areas: the form counts that as where it started.
  const [start] = useState(() => withAreas(t, base));
  const [m, setM] = useState<Macro>(() => ({ ...start, trigger: { ...start.trigger }, accountIds: [...start.accountIds] }));
  const patch = (p: Partial<Macro>) => setM(x => ({ ...x, ...p }));
  const [steps, setSteps] = useState<Held[]>(() => start.steps.map(hold));
  const [cursor, setCursor] = useState(-1);
  const [tried, setTried] = useState(false);
  const [pending, setPending] = useState<{ steps: Step[]; w: number; h: number } | null>(null);
  const [capture, setCapture] = useState<number | null>(null); // the step whose "Press a key" box is listening
  const [openKey, setOpenKey] = useState<number | null>(null);
  const [note, setNote] = useState('');
  const [whenOpen, setWhenOpen] = useState(false);
  const opts = m.options || [];
  const areas = m.areas || [];

  // The window the picture, Edit on game, Try it and Record use: one of the macro's own accounts with a window, else any.
  const live: Account[] = (app.accounts || []).filter(hasWindow);
  const [accountId, setAccountId] = useState(() => (live.find(a => start.accountIds.includes(a.id)) || live[0])?.id || '');
  const windowId = live.some(a => a.id === accountId) ? accountId : live[0]?.id || '';
  const windowName = live.find(a => a.id === windowId)?.label || '';
  const [picture, setPicture] = useState<Picture | null>(null);
  const [taking, setTaking] = useState(false);
  const [cutFor, setCutFor] = useState<number | null>(null);

  const form = useRef<HTMLDivElement>(null);
  const nameBox = useRef<HTMLInputElement>(null);
  const rowEl = (i: number) => form.current?.querySelectorAll<HTMLElement>('.mc-step')[i] || null;

  /* ── options (what the player may change) ── */
  const optOf = (v: unknown) => isOpt(v) ? opts.find(o => '$' + o.name === v) : undefined;
  const used = (list: Held[]) => {
    const refs = new Set<string>();
    const scan = (o: unknown) => { if (isOpt(o)) refs.add(o.slice(1)); else if (o && typeof o === 'object') Object.values(o).forEach(scan); };
    list.forEach(s => scan(strip(s)));
    return refs;
  };
  // A switch, a choice or a heading is made on the panel and stays while no step names it; the rest go with their tick.
  const keeps = (refs: Set<string>) => (o: Option) => FREE.has(o.kind) || refs.has(o.name);
  const shownOpts = opts.filter(keeps(used(steps)));
  const setOpt = (name: string, p: Partial<Option>) => setM(x => ({ ...x, options: (x.options || []).map(o => o.name === name ? { ...o, ...p } : o) }));
  // The panel's tabs (fewer than two: none); a new setting goes on the one shown.
  const tabs = (m.panel?.tabs?.length || 0) >= 2 ? m.panel!.tabs! : [];
  const [tabSel, setTabSel] = useState<string | null>(null);
  const curTab = tabs.length ? (tabSel && tabs.includes(tabSel) ? tabSel : tabs[0]) : null;
  const onTab = () => curTab && curTab !== tabs[0] ? { tab: curTab } : {};
  const freeLabel = (first: string) => {
    let label = first;
    for (let n = 2; shownOpts.some(o => o.label === label); n++) label = `${first} ${n}`;
    return label;
  };
  // "point" is a click's x and y together: both name one point setting.
  const rawAt = (s: Step, path: string) => getAt(s, path === 'point' ? 'x' : path);
  const val = (i: number, path: string) => { const raw = rawAt(steps[i], path); const o = optOf(raw); return o ? o.value : raw; };
  const setStep = (i: number, p: Partial<Step>) => setSteps(list => list.map((s, k) => k === i ? { ...s, ...p } : s));
  const put = (i: number, path: string, v: unknown) => {
    const o = optOf(getAt(steps[i], path));
    if (o) setOpt(o.name, { value: v as Option['value'], default: v as Option['value'] });
    else setSteps(list => list.map((s, k) => k === i ? setAt(s, path, v) : s));
  };
  const ticked = (i: number, path: string) => !!optOf(rawAt(steps[i], path));
  const tick = (i: number, path: string, want: boolean) => {
    const s = steps[i];
    const raw = path === 'point' ? { x: Number(s.x) || 0.5, y: Number(s.y) || 0.5 } : getAt(s, path);
    if (want) {
      if (shownOpts.length >= MAX_OPTIONS) return toast(t('macro.opt.full', { n: MAX_OPTIONS }), 'warn');
      const meta = META[`${s.kind}.${path}`];
      if (!meta) return;
      const name = freeName('o', opts);
      const o: Option = { name, label: freeLabel(t(meta.label)), kind: meta.kind, value: raw, default: raw, ...onTab(),
        ...(meta.min !== undefined ? { min: meta.min, max: meta.max } : {}), ...(meta.unit !== undefined ? { unit: meta.unit } : {}) };
      patch({ options: [...opts.filter(keeps(used(steps))), o] });
      setSteps(list => list.map((x, k) => k !== i ? x : path === 'point' ? { ...x, x: '$' + name, y: '$' + name } : setAt(x, path, '$' + name)));
    } else {
      const o = optOf(rawAt(s, path));
      if (!o) return;
      const p = o.value as { x: number; y: number };
      setSteps(list => list.map((x, k) => k !== i ? x : path === 'point' ? { ...x, x: p.x, y: p.y } : setAt(x, path, o.value)));
    }
  };
  /** A switch, a choice or a heading added on the panel itself. */
  const addFree = (kind: 'check' | 'choice' | 'list' | 'heading') => {
    if (shownOpts.length >= MAX_OPTIONS) return toast(t('macro.opt.full', { n: MAX_OPTIONS }), 'warn');
    const choices = [t('macro.opt.choiceN', { n: 1 }), t('macro.opt.choiceN', { n: 2 })];
    const value = kind === 'check' ? 0 : kind === 'choice' ? choices[0] : kind === 'list' ? [] : null;
    const o: Option = { name: freeName('o', opts), label: freeLabel(t(`macro.opt.new.${kind}`)), kind, value, default: value, ...onTab(),
      ...(kind === 'choice' || kind === 'list' ? { choices } : {}) };
    patch({ options: [...shownOpts, o] });
  };
  const setPanel = (p: Partial<NonNullable<Macro['panel']>>) => setM(x => ({ ...x, panel: { autoShow: false, placeId: null, ...x.panel, ...p } }));
  const addTab = () => {
    if (tabs.length >= MAX_TABS) return toast(t('macro.tab.full', { n: MAX_TABS }), 'warn');
    const next = tabs.length ? [...tabs] : [t('macro.tab.first')];
    let n = next.length + 1;
    while (next.some(x => x.toLowerCase() === t('macro.tab.n', { n }).toLowerCase())) n++;
    next.push(t('macro.tab.n', { n }));
    setPanel({ tabs: next });
    setTabSel(next[next.length - 1]);
  };
  const renameTab = (old: string, name: string) => setM(x => ({ ...x,
    panel: { autoShow: false, placeId: null, ...x.panel, tabs: (x.panel?.tabs || []).map(y => y === old ? name : y) },
    options: (x.options || []).map(o => o.tab === old ? { ...o, tab: name } : o) }));
  // A tab taken away: its settings go to the first; one tab left is no tabs.
  const removeTab = (name: string) => {
    const next = tabs.filter(x => x !== name);
    setM(x => ({ ...x, panel: { autoShow: false, placeId: null, ...x.panel, tabs: next.length >= 2 ? next : [] },
      options: (x.options || []).map(o => {
        const { tab, ...rest } = o;
        return next.length >= 2 && tab && tab !== name && tab !== next[0] ? o : rest;
      }) }));
    if (curTab === name) setTabSel(null);
  };

  /* ── areas (named boxes on the game) ── */
  const newArea = () => {
    if (areas.length >= MAX_AREAS) { toast(t('macro.area.full', { n: MAX_AREAS }), 'warn'); return null; }
    const name = freeName('a', areas);
    let n = areas.length + 1;
    while (areas.some(a => a.label === t('macro.area.n', { n }))) n++;
    const a: Area = { name, label: t('macro.area.n', { n }), colour: AREA_COLOURS[areas.length % AREA_COLOURS.length], rect: { x: 0.35, y: 0.35, w: 0.3, h: 0.3 } };
    patch({ areas: [...areas, a] });
    return '@' + name;
  };
  const setArea = (name: string, p: Partial<Area>) => setM(x => ({ ...x, areas: (x.areas || []).map(a => a.name === name ? { ...a, ...p } : a) }));
  const delArea = (name: string) => {
    patch({ areas: areas.filter(a => a.name !== name) });
    const off = (v: unknown) => v === '@' + name ? undefined : v;
    setSteps(list => list.map(s => ({ ...s, ...(s.region !== undefined ? { region: off(s.region) } : {}), ...(s.test?.region !== undefined ? { test: { ...s.test, region: off(s.test.region) } } : {}) })));
  };

  // A template's "to do": the fields it marked on its steps, each done once changed from what the template had (an
  // area once its box was moved on the game).
  const sig = (st: Step, field: string, mm: Macro) => field === 'region'
    ? JSON.stringify((mm.areas || []).find(a => '@' + a.name === st.region)?.rect ?? st.region ?? null)
    : JSON.stringify(isOpt(st[field]) ? (mm.options || []).find(o => '$' + o.name === st[field])?.value ?? null : st[field] ?? null);
  const todos = useRef(steps.flatMap(st => ((st.todo as string[] | undefined) || []).map(field => ({ k: st._k, field, was: sig(st, field, m) }))));
  const todoDone = (x: { k: number; field: string; was: string }) => {
    const st = steps.find(y => y._k === x.k);
    return !st || sig(st, x.field, m) !== x.was;
  };
  const full = (): Macro => {
    const refs = used(steps);
    return { ...m, name: m.name.trim(), options: opts.filter(keeps(refs)), steps: steps.map(st => {
      const { todo: _t, ...rest } = strip(st);
      void _t;
      const still = todos.current.filter(x => x.k === st._k && !todoDone(x)).map(x => x.field);
      return still.length ? { ...rest, todo: still } : rest;
    }) };
  };
  const snapshot = () => JSON.stringify(full());
  const initial = useRef('');
  useEffect(() => { initial.current = snapshot(); }, []); // once, as it opened
  const all = problems(t, full(), macros);
  const errors = all.filter(p => !p.warn);
  const latest = useRef({ snapshot, collect: () => ({}) as ReturnType<MacroEditorApi['collect']>, full });
  latest.current.snapshot = snapshot;
  latest.current.full = full;
  latest.current.collect = () => {
    setTried(true);
    const first = errors[0];
    if (!first) return { macro: full() };
    if (first.step !== null) flushSync(() => setOpenKey(steps[first.step!]._k));
    const box = first.field === 'name' ? nameBox.current
      : first.field === 'accounts' || first.field === 'every' ? (flushSync(() => setWhenOpen(true)), form.current?.querySelector<HTMLElement>(first.field === 'every' ? '#mc-every' : '#mc-accs input') || null)
      : first.field === 'options' ? form.current?.querySelector<HTMLElement>('.mp-label') || null
      : first.step !== null ? rowEl(first.step)?.querySelector<HTMLElement>('.mc-sent :is(input, button)') || rowEl(first.step) : null;
    return { error: first.text, box };
  };
  useImperativeHandle(api, () => ({ collect: () => latest.current.collect(), dirty: () => latest.current.snapshot() !== initial.current }), []);

  // A recording this form started: its steps come in here (added to the end or in place of the steps there, as chosen).
  const stepsNow = useRef(steps);
  stepsNow.current = steps;
  useEffect(() => on('macroRecorded', (d?: { steps: Step[]; w: number; h: number }) => {
    if (recording.for !== 'editor') return;
    recording.for = null;
    if (!d?.steps?.length) return toast(t('macro.nothingRecorded'), 'info');
    if (stepsNow.current.length) return setPending(d);
    setSteps(d.steps.map(hold));
    patch({ recordedSize: { w: d.w, h: d.h } });
  }), [on]);
  const rec = app.macroRecording || null;

  /* ── steps ── */
  const focusRow = async (i: number) => {
    if (steps[i]) setOpenKey(steps[i]._k);
    await new Promise(r => setTimeout(r));
    const row = rowEl(i);
    const el = row?.querySelector<HTMLElement>('.mc-sent :is(input, select, button)');
    if (el) el.focus(); else if (row) { row.tabIndex = -1; row.focus(); }
    row?.scrollIntoView({ block: 'nearest' });
  };
  const add = (kind: string) => {
    // If stuck ends the macro's own steps: it goes last, and turns on starting again and the stuck watch.
    const at = kind !== 'recover' && cursor >= 0 && cursor < steps.length ? cursor + 1 : steps.length;
    if (kind === 'recover') patch({ onFail: 'restart', stuckMinutes: m.stuckMinutes || 5 });
    const added = stepsToAdd(kind).map(hold);
    setSteps(list => [...list.slice(0, at), ...added, ...list.slice(at)]);
    setCursor(at);
    setOpenKey(added[0]._k);
    focusRow(at);
  };
  // The common kinds first; the rest one click away under More steps, in their groups.
  const addMenu = (btn: HTMLElement) => host.menu(btn, [
    ...COMMON_STEPS.map(k => ({ icon: STEP_ICON[k], text: t(`macro.kind.${k}`), run: () => add(k) })),
    '-', { icon: 'more', text: t('macro.moreSteps'), run: () => setTimeout(() => moreMenu(btn)) },
  ]);
  const moreMenu = (btn: HTMLElement) => host.menu(btn, STEP_GROUPS.flatMap(([g, kinds], gi) => {
    const rest = kinds.filter(k => !COMMON_STEPS.includes(k) && !(k === 'recover' && steps.some(s => s.kind === 'recover')));
    return rest.length ? [...(gi ? ['-' as const] : []), { head: true, text: t(`macro.grp.${g}`) },
      ...rest.map(k => ({ icon: STEP_ICON[k], text: t(`macro.kind.${k}`), run: () => add(k) }))] : [];
  }));
  const moveStep = (i: number, dir: number, btn: HTMLElement) => {
    const j = i + dir;
    if (j < 0 || j >= steps.length) return;
    const next = [...steps];
    [next[i], next[j]] = [next[j], next[i]];
    flushSync(() => setSteps(next));
    setCursor(j);
    const same = rowEl(j)?.querySelector<HTMLElement>(`[data-move="${dir}"]:not(:disabled)`);
    (same || btn).focus();
    rowEl(j)?.scrollIntoView({ block: 'nearest' });
  };
  const dupStep = (i: number) => {
    flushSync(() => setSteps(list => [...list.slice(0, i + 1), hold(strip(list[i])), ...list.slice(i + 1)]));
    setCursor(i + 1);
    rowEl(i + 1)?.querySelector<HTMLElement>('[data-st="dup"]')?.focus();
  };
  const delStep = (i: number) => {
    flushSync(() => setSteps(list => list.filter((_, k) => k !== i)));
    if (cutFor === i) setCutFor(null);
    const next = rowEl(Math.min(i, steps.length - 2));
    (next?.querySelector<HTMLElement>('[data-st="del"]') || form.current?.querySelector<HTMLElement>('#mc-add'))?.focus();
  };
  const [dragging, setDragging] = useState<number | null>(null);

  /* ── the game: a picture of it, Edit on game, picking on it ── */
  const take = (btn: HTMLElement) => busy(btn, async () => {
    setTaking(true);
    const p = await call('macroPicture', { accountId: windowId }) as Picture | undefined;
    setTaking(false);
    if (!p) return;
    setPicture(p);
    setSeen(null);
    if (!m.recordedSize) patch({ recordedSize: { w: p.w, h: p.h } });
  });
  // A point or colour picked by clicking on the game itself (the click doesn't reach Roblox; Esc gives up).
  const pick = async (i: number, what: 'point' | 'colour' | 'target' | 'marker', path?: string) => {
    if (!windowId) return toast(t('macro.needWindow'), 'info');
    setNote(t('macro.pickNow', { name: windowName }));
    const r = await call('pickOnGame', { accountId: windowId }) as { x: number; y: number; colour: string } | null | undefined;
    setNote('');
    if (!r) return;
    const s = steps[i];
    const xy = { x: +r.x.toFixed(4), y: +r.y.toFixed(4) };
    const test = s.kind === 'if' || s.kind === 'loopUntil';
    if (what === 'point') setXY(i, xy);
    else if (what === 'colour') {
      // A colour at a point: the point goes with it (Wait for a colour, If a colour shows, without an area).
      const at = test ? !s.test?.region : s.kind === 'waitColour' && !s.region;
      if (at) setXY(i, xy);
      put(i, test ? 'test.colour' : 'colour', r.colour);
    } else put(i, path || what, r.colour);
    focusRow(i);
  };
  const cut = (i: number) => {
    setCutFor(i);
    if (!picture && windowId) take(form.current!.querySelector<HTMLElement>('#mc-pic-take')!);
    setTimeout(() => form.current?.querySelector<HTMLElement>('.mc-pic-frame')?.focus({ preventScroll: true }), 0);
    form.current?.querySelector('.mc-game')?.scrollIntoView({ block: 'nearest' });
  };
  const cutDone = (r: Rect, image: string, cutDown: boolean) => {
    void r;
    setNote(cutDown ? t('macro.pic.cutMax') : '');
    if (cutFor === null) return;
    const i = cutFor;
    setCutFor(null);
    const s = steps[i];
    if (s.kind === 'if' || s.kind === 'loopUntil') setSteps(list => list.map((x, k) => k === i ? { ...x, test: { ...x.test, image } } : x));
    else setStep(i, { image });
    focusRow(i);
  };
  // A step's point; one the player sets (a point setting) moves the setting's value, as the maker's.
  const setXY = (i: number, xy: { x: number; y: number }) => {
    const s = steps[i];
    const o = s.kind === 'if' || s.kind === 'loopUntil' ? undefined : optOf(s.x);
    if (o?.kind === 'point') setOpt(o.name, { value: xy, default: xy });
    else setSteps(list => list.map((x, k) => k !== i ? x : (x.kind === 'if' || x.kind === 'loopUntil') ? { ...x, test: { ...x.test, ...xy } } : { ...x, ...xy }));
  };
  const moveDot = (i: number, x: number, y: number) => setXY(i, { x: +x.toFixed(4), y: +y.toFixed(4) });
  const moveBox = (name: string, r: Rect) => setArea(name, { rect: { x: +r.x.toFixed(4), y: +r.y.toFixed(4), w: +r.w.toFixed(4), h: +r.h.toFixed(4) } });
  const dots = steps.flatMap((s, i) => { const p = pointOf(s, opts); return p ? [{ i, n: i + 1, ...p }] : []; });

  // Edit on game: the dots and boxes drawn over the window itself, dragged there; each change comes back here and the
  // overlay is drawn again from the form. Done (or Esc) on the game, or this button, ends it; so does closing the form.
  const [onGame, setOnGame] = useState(false);
  const draft = snapshot();
  useEffect(() => {
    if (!onGame || !windowId) return;
    const timer = setTimeout(() => call('editOnGame', { id: m.id, accountId: windowId, macro: latest.current.full() }, { silent: true }), 120);
    return () => clearTimeout(timer);
  }, [onGame, windowId, draft]);
  useEffect(() => {
    if (!onGame) return;
    return () => { call('editOnGame', { accountId: null }, { silent: true }); };
  }, [onGame, windowId]);
  useEffect(() => on('onGameEdit', (d?: { step?: number; x?: number; y?: number; area?: string; rect?: Rect; done?: boolean }) => {
    if (!d) return;
    if (d.done) return setOnGame(false);
    if (d.step != null && d.x != null && d.y != null) moveDot(d.step, d.x, d.y);
    if (d.area && d.rect) moveBox(d.area, d.rect);
  }), [on]);
  const editOnGame = () => { if (!windowId) return toast(t('macro.needWindow'), 'info'); setOnGame(x => !x); };

  // Try it: the macro as it is in the form, once, on the window chosen (macroTryRun). Its run (trial) shows here, not in
  // the list: each step it gets past is a line ("found" for those that look for something), and how it ended. Meanwhile
  // the app sends what it sees on that window (macroSeen: the picture, what it found, a note).
  const [seen, setSeen] = useState<Seen | null>(null);
  const [lines, setLines] = useState<Seen[] | null>(null);
  const [trial, setTrial] = useState<{ id: string; accountId: string; since: number } | null>(null);
  const trying = !!trial;
  const lastStep = useRef<number | null>(null);
  useEffect(() => on('macroSeen', (d?: Seen) => {
    if (!d || (d.accountId && d.accountId !== windowId)) return;
    if (d.image) setSeen(d);
  }), [on, windowId]);
  const run = trial ? ((app.macroRuns || []) as MacroRun[]).filter(r => r.trial && r.macroId === trial.id && r.accountId === trial.accountId && Date.parse(r.startedAt) >= trial.since).at(-1) : undefined;
  useEffect(() => {
    if (!trial || !run) return;
    const line = (d: Seen) => setLines(list => [...(list || []).filter(x => x.step == null || x.step !== d.step), d]);
    const passed = (k: number) => { if (steps[k] && !BLOCKS.has(steps[k].kind)) line({ w: 0, h: 0, step: k, outcome: LOOKS.has(steps[k].kind) ? 'found' : 'done' }); };
    const at = run.step ?? null;
    if (lastStep.current !== null && at !== lastStep.current && !isLive(run)) {
      // ended: what stopped it, then how
      if (run.status === 'failed' && (run.reason === 'missingValue' || run.reason === 'notFound' || run.reason === 'timeout'))
        line({ w: 0, h: 0, step: lastStep.current, outcome: run.reason === 'missingValue' ? 'missingValue' : 'notFound' });
      else if (run.status === 'done') passed(lastStep.current);
    } else if (lastStep.current !== null && at !== null && at !== lastStep.current) passed(lastStep.current);
    lastStep.current = at;
    if (!isLive(run)) {
      line({ w: 0, h: 0, outcome: run.status === 'done' ? 'done' : 'stopped' });
      setTrial(null);
    }
  }, [run?.step, run?.status]);
  // Its results are at the foot of the form, far below the button: kept in sight as it starts and as lines come.
  useEffect(() => {
    if (trial || lines?.length) form.current?.closest('.modal-body')?.querySelector('.mc-tries')?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
  }, [trial?.since, lines?.length]);
  const tryIt = (btn: HTMLElement) => busy(btn, async () => {
    if (trial) { setTrial(null); return call('stopMacros', { id: trial.id, accountId: trial.accountId, trial: true }); }
    if (!windowId) return toast(t('macro.needWindow'), 'info');
    if (errors.length) { setTried(true); return toast(t('macro.tryFix'), 'warn'); }
    setLines([]);
    lastStep.current = null;
    const r = await call('macroTryRun', { macro: full(), accountId: windowId }) as { queued: number; macroId: string } | undefined;
    if (r?.macroId) setTrial({ id: r.macroId, accountId: windowId, since: Date.now() - 2000 });
  });

  const record = (btn: HTMLElement) => busy(btn, async () => {
    if (!windowId) return toast(t('macro.needWindow'), 'info');
    recording.for = 'editor';
    if (await call('recordMacro', { accountId: windowId }) === undefined) recording.for = null;
  });

  /* ── the form ── */
  const d = depths(steps);
  const taken = new Map(macros.filter(o => o.id !== m.id && o.hotkey).map(o => [o.hotkey!, o.name]));
  const accTicked = new Set(m.accountIds);
  const shown = (p: { step: number | null; field?: string }) => tried || p.step !== null;
  const listed = all.filter(shown);
  const stepProblems = (i: number) => all.filter(p => p.step === i && shown(p));
  const who = m.targets === 'accounts' ? tn('macro.tg.n', m.accountIds.length) : t(`macro.tg.${m.targets}`);
  const when = m.trigger.kind === 'interval' ? t('macro.trig.every', { d: durText(t, m.trigger.everySeconds || 60) }) : t(`macro.when.${m.trigger.kind}`);
  const ed: Ed = { val, put, set: setStep, ticked, tick, areas, newArea, pick, cut, canPick: !!windowId, capture, setCapture,
    counters: counters({ steps }), readable: shownOpts.filter(o => o.kind === 'check' || o.kind === 'choice' || o.kind === 'number') };
  const place = m.panel?.placeId ? `https://www.roblox.com/games/${m.panel.placeId}` : '';
  const [placeText, setPlaceText] = useState(place);

  const todoList = todos.current;
  return <div className="mc-form" ref={form}>
    {todoList.length ? <div className="mc-todo" role="group" aria-labelledby="mc-todo-h">
      <b id="mc-todo-h">{todoList.every(todoDone) ? t('macro.todo.allDone') : t('macro.todo.title')}</b>
      <ul>{todoList.map((x, k) => {
        const i = steps.findIndex(y => y._k === x.k);
        const done = todoDone(x);
        const area = x.field === 'region' ? areas.find(a => '@' + a.name === steps[i]?.region)?.label : '';
        return <li key={k} className={done ? 'done' : ''}><Icon name={done ? 'check' : 'record'} />
          <span>{area ? t('macro.todo.area', { area }) : t(`macro.todo.${x.field}`)}{i >= 0 ? <small> · {t('macro.stepN', { n: i + 1 })}</small> : null}</span>
          {done || i < 0 ? null : x.field === 'region'
            ? <button type="button" className="btn sm" onClick={() => { if (!onGame) editOnGame(); }}><Icon name="screens" /><span>{t('macro.game.edit')}</span></button>
            : <button type="button" className="btn sm" onClick={() => focusRow(i)}>{t('macro.todo.go')}</button>}</li>;
      })}</ul>
    </div> : null}
    <div className="mc-cols">
      <div className="mc-main">
        <div className="form mc-top">
          <div className="mc-name-row">
            <label>{t('macro.name')}<input className="field" id="mc-name" ref={nameBox} maxLength={60} placeholder={t('macro.namePh')} value={m.name}
              onChange={e => patch({ name: e.target.value })} /></label>
            <button type="button" className={`btn mc-try-it${trying ? ' on' : ''}`} id="mc-try" aria-pressed={trying} disabled={!windowId}
              title={windowId ? t('macro.tryItTip', { name: windowName }) : t('macro.needWindow')} onClick={e => tryIt(e.currentTarget)}>
              <Icon name={trying ? 'stop' : 'play'} /><span>{t(trying ? 'macro.tryStop' : 'macro.tryIt')}</span></button>
          </div>
          <details className="mc-when" open={whenOpen} onToggle={e => setWhenOpen((e.target as HTMLDetailsElement).open)}>
            <summary id="mc-when"><span>{[when, who, m.hotkey ? `Ctrl+Alt+${m.hotkey}` : t('macro.hk.noneLine')].join(' · ')}</span><Icon name="chevronDown" /></summary>
            <div className="mc-when-body">
              <div className="mc-grid">
                <label>{t('macro.trigger')}<select className="select" id="mc-trigger" value={m.trigger.kind} onChange={e => {
                  const kind = e.target.value as Macro['trigger']['kind'];
                  patch({ trigger: kind === 'interval' ? { kind, everySeconds: m.trigger.everySeconds || 300 } : { kind } });
                }}>{['manual', 'interval', 'join', 'rejoin'].map(k => <option key={k} value={k}>{t(`macro.trigOpt.${k}`)}</option>)}</select></label>
                {m.trigger.kind === 'interval' ? <label>{t('macro.everyMin')}<input className="field num" id="mc-every" type="number" min={1} max={1440}
                  value={Math.round((m.trigger.everySeconds || 300) / 60)} onChange={e => patch({ trigger: { kind: 'interval', everySeconds: e.target.value === '' ? NaN : Math.round(+e.target.value * 60) } })} /></label> : <span />}
                <label>{t('macro.targets')}<select className="select" id="mc-targets" value={m.targets} onChange={e => patch({ targets: e.target.value as Macro['targets'] })}>
                  {['selected', 'all', 'accounts'].map(k => <option key={k} value={k}>{t(`macro.tg.${k}`)}</option>)}</select></label>
                <label>{t('macro.hotkey')}<select className="select" id="mc-hotkey" value={m.hotkey ? String(m.hotkey) : ''} onChange={e => patch({ hotkey: e.target.value ? +e.target.value : null })}>
                  <option value="">{t('macro.hk.none')}</option>
                  {[1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => <option key={n} value={String(n)} disabled={taken.has(n)}>
                    {taken.has(n) ? t('macro.hk.taken', { k: `Ctrl+Alt+${n}`, name: taken.get(n)! }) : `Ctrl+Alt+${n}`}</option>)}</select></label>
              </div>
              {m.hotkey && app.hotkeys && !app.hotkeys.on ? <p className="mc-note"><Icon name="info" /><span>{t('macro.hk.off')}</span></p> : null}
              {m.targets === 'accounts' ? <div className="rc-list mc-accs" id="mc-accs" role="group" aria-label={t('macro.targets')}>{accounts.map(a =>
                <label key={a.id} className="rc-item"><input type="checkbox" checked={accTicked.has(a.id)} aria-label={a.label} onChange={e =>
                  patch({ accountIds: e.target.checked ? [...m.accountIds, a.id] : m.accountIds.filter(x => x !== a.id) })} />
                  <div className="avatar sm"><Avatar a={a} /></div><span><b>{a.label}</b>{a.username ? <small>@{a.username}</small> : null}</span></label>)}</div> : null}
              {m.trigger.kind !== 'manual' ? <label className="mc-switch-row">
                <span className="switch"><input type="checkbox" id="mc-enabled" checked={m.enabled} onChange={e => patch({ enabled: e.target.checked })} /><span /></span>
                <span><b>{t('macro.auto')}</b><small>{t('macro.autoDesc')}</small></span></label> : null}
              {/* What happens when it goes wrong, then its limits: fields like the ones above, not sentences. */}
              <div className="mc-grid mc-when-more">
                <label>{t('macro.onFail')}<select className="select" id="mc-onfail" value={m.onFail} onChange={e => patch({ onFail: e.target.value as Macro['onFail'] })}>
                  {['stop', 'skip', 'restart', 'rejoin', 'alert'].map(k => <option key={k} value={k}>
                    {t(k === 'restart' && steps.some(s => s.kind === 'recover') ? 'macro.fail.recover' : `macro.fail.${k}`)}</option>)}</select></label>
                {steps.some(s => LOOK_STEPS.includes(s.kind)) ? <label>{t('macro.stuckL')}<select className="select" id="mc-stuck" value={String(m.stuckMinutes || 0)}
                  onChange={e => patch({ stuckMinutes: +e.target.value || null })}>{[...new Set([0, 2, 5, 10, 30, m.stuckMinutes || 0])].sort((x, y) => x - y)
                    .map(n => <option key={n} value={String(n)}>{n ? t('macro.stuck.after', { n }) : t('macro.stuck.off')}</option>)}</select></label> : <span />}
              </div>
              <div className="mc-grid mc-when-nums">
                <label>{t('macro.e.stopAfterL')}<NumField id="mc-max" value={m.maxMinutes} min={1} max={240} onChange={v => patch({ maxMinutes: v })} /></label>
                <label>{t('macro.e.randomMs')}<NumField id="mc-jms" value={m.jitterMs} min={0} max={500} onChange={v => patch({ jitterMs: v })} /></label>
                <label>{t('macro.e.randomPx')}<NumField id="mc-jpx" value={m.jitterPx} min={0} max={20} onChange={v => patch({ jitterPx: v })} /></label>
              </div>
            </div>
          </details>
        </div>

        <section className="mc-steps-sec" aria-labelledby="mc-steps-h">
          <div className="mc-steps-head">
            <b id="mc-steps-h">{t('macro.steps')} <span className="muted">({steps.length})</span></b>
            <button type="button" className="btn sm" id="mc-record" disabled={!windowId || !!rec} title={windowId ? t('macro.rec.tip') : t('macro.needWindow')}
              onClick={e => record(e.currentTarget)}><Icon name="record" /><span>{t('macro.recordBtn')}</span></button>
            <button type="button" className="btn sm primary" id="mc-add" aria-haspopup="menu" onClick={e => addMenu(e.currentTarget)}>
              <Icon name="plus" /><span>{t('macro.addStep')}</span></button>
          </div>
          {rec ? <EditorRecording startsAt={rec.startsAt} count={rec.steps} /> : null}
          {pending ? <div className="mc-pending" role="group" aria-label={t('macro.rec.done', { n: pending.steps.length })}>
            <span>{tn('macro.rec.done', pending.steps.length)}</span>
            <button type="button" className="btn sm primary" onClick={() => { setSteps(list => [...list, ...pending.steps.map(hold)]); setPending(null); }}>{t('macro.rec.append')}</button>
            <button type="button" className="btn sm" onClick={() => { setSteps(pending.steps.map(hold)); patch({ recordedSize: { w: pending.w, h: pending.h } }); setPending(null); }}>{t('macro.rec.replace')}</button>
            <button type="button" className="btn sm ghost" onClick={() => setPending(null)}>{t('macro.rec.discard')}</button>
          </div> : null}
          {!steps.length ? <p className="mc-steps-none">{t('macro.noSteps')}</p> : null}
          <div className="mc-steps" role="list" aria-labelledby="mc-steps-h"
            onDragOver={e => {
              if (dragging === null) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              const over = (e.target as HTMLElement).closest<HTMLElement>('.mc-step');
              const j = over ? [...over.parentElement!.children].indexOf(over) : -1;
              if (j < 0 || j === dragging) return;
              const next = [...steps];
              const [s] = next.splice(dragging, 1);
              next.splice(j, 0, s);
              setSteps(next);
              setDragging(j);
            }}
            onDrop={e => e.preventDefault()} onDragEnd={() => setDragging(null)}>
            {steps.map((s, i) => <StepRow key={s._k} s={s} i={i} m={m} count={steps.length} depth={d[i]} problems={stepProblems(i)} dragging={dragging === i}
              open={openKey === s._k} toggle={() => { setOpenKey(openKey === s._k ? null : s._k); setCursor(i); }} ed={ed}
              focus={() => setCursor(i)} move={(dir, btn) => moveStep(i, dir, btn)} dup={() => dupStep(i)} del={() => delStep(i)} cutting={cutFor === i}
              dragStart={() => setDragging(i)} />)}
          </div>
          {listed.length ? <div className="mc-problems" role="status" aria-live="polite">
            <b>{t(errors.some(shown) ? 'macro.err.title' : 'macro.warn.title')}</b>
            <ul>{listed.map((p, k) => <li key={k} className={p.warn ? 'warn' : ''}>
              {p.step !== null ? <button type="button" className="link" onClick={() => focusRow(p.step!)}>{p.text}</button> : p.text}</li>)}</ul>
          </div> : null}
        </section>

        <PlayerSees m={m} options={shownOpts} counts={counters({ steps })} areas={areas.length}
          tabs={tabs} tab={curTab} setTab={setTabSel} addTab={addTab} renameTab={renameTab} removeTab={removeTab}
          moveTo={(name, tab) => setM(x => ({ ...x, options: (x.options || []).map(o => {
            if (o.name !== name) return o;
            const { tab: _was, ...rest } = o;
            void _was;
            return tab === tabs[0] ? rest : { ...rest, tab };
          }) }))}
          add={addFree} setChoices={(name, choices) => {
            const o = opts.find(x => x.name === name);
            const keep = (v: Option['value']) => o?.kind === 'list' ? (Array.isArray(v) ? v.filter(x => choices.includes(x)) : [])
              : typeof v === 'string' && choices.includes(v) ? v : choices[0];
            if (o) setOpt(name, { choices, value: keep(o.value), default: keep(o.default) });
          }}
          setLabel={(name, label) => setOpt(name, { label })}
          move={(name, dir) => {
            // Up and down among the rows of its tab.
            const list = [...shownOpts];
            const tabOf = (o: Option) => tabs.length && o.tab && tabs.includes(o.tab) ? o.tab : tabs[0];
            const i = list.findIndex(o => o.name === name);
            if (i < 0) return;
            let j = i + dir;
            while (j >= 0 && j < list.length && tabOf(list[j]) !== tabOf(list[i])) j += dir;
            if (j < 0 || j >= list.length) return;
            [list[i], list[j]] = [list[j], list[i]];
            patch({ options: list });
          }}
          remove={name => {
            const o = opts.find(x => x.name === name);
            if (!o) return;
            if (FREE.has(o.kind)) return patch({ options: shownOpts.filter(x => x.name !== name) });
            // A ticked value: the steps get it back as their own.
            const back = (k: string, v: unknown) => v !== '$' + name ? v : o.kind === 'point' ? asPoint(o.value)?.[k as 'x' | 'y'] : o.value;
            setSteps(list => list.map(s => ({ ...Object.fromEntries(Object.entries(s).map(([k, v]) => [k, back(k, v)])),
              ...(s.test ? { test: Object.fromEntries(Object.entries(s.test).map(([k, v]) => [k, back(k, v)])) } : {}) } as Held)));
          }}>
          <label className="mc-switch-row">
            <span className="switch"><input type="checkbox" id="mc-panel-auto" checked={!!m.panel?.autoShow}
              onChange={e => patch({ panel: { autoShow: e.target.checked, placeId: m.panel?.placeId ?? null } })} /><span /></span>
            <span><b>{t('macro.panel.auto')}</b><small>{t('macro.panel.autoDesc')}</small></span></label>
          {m.panel?.autoShow ? <div className="mc-panel-game"><GameBox place={placeText} setPlace={v => {
            setPlaceText(v);
            const id = /(?:games\/|^)(\d{3,})/.exec(v.trim())?.[1];
            patch({ panel: { autoShow: true, placeId: id ? +id : null } });
          }} chose={g => patch({ panel: { autoShow: true, placeId: g.placeId } })} /></div> : null}
        </PlayerSees>
      </div>

      <aside className="mc-side">
        <section className="mc-game" aria-labelledby="mc-game-h">
          <div className="mc-pic-head">
            <b id="mc-game-h">{t('macro.game.title')}</b>
            {live.length > 1 ? <select className="select sm" id="mc-pic-acc" value={windowId} aria-label={t('macro.pic.account')} onChange={e => setAccountId(e.target.value)}>
              {live.map(a => <option key={a.id} value={a.id}>{a.label}</option>)}</select> : null}
            <button type="button" className={`btn sm${onGame ? ' on' : ' primary'}`} id="mc-ongame" aria-pressed={onGame} disabled={!windowId} onClick={editOnGame}>
              <Icon name={onGame ? 'check' : 'screens'} /><span>{t(onGame ? 'macro.game.done' : 'macro.game.edit')}</span></button>
          </div>
          <p className={`mc-game-say${onGame ? ' on' : ''}`} role="status">{!windowId ? t('macro.game.noWindow')
            : onGame ? t('macro.game.editing', { name: windowName }) : t('macro.game.how', { name: windowName })}</p>
          <GameFrame picture={seen?.image ? { image: seen.image.startsWith('data:') ? seen.image : `data:image/png;base64,${seen.image}`, w: seen.w, h: seen.h } : picture}
            size={m.recordedSize} dots={dots} boxes={areas} found={seen?.boxes || []} cutting={cutFor !== null}
            cancelCut={() => setCutFor(null)} onDot={moveDot} onBox={moveBox} onCut={cutDone} />
          <div className="mc-pic-acts">
            <button type="button" className="btn sm" id="mc-pic-take" disabled={!windowId || taking} onClick={e => take(e.currentTarget)}>
              <Icon name="image" /><span>{t(picture ? 'macro.pic.again' : 'macro.pic.take')}</span></button>
            <small>{picture ? t('macro.pic.size', { w: picture.w, h: picture.h }) : t('macro.pic.why')}</small>
          </div>
          {note ? <p className="mc-note" role="status"><Icon name="info" /><span>{note}</span></p> : null}
          <AreaList areas={areas} set={setArea} del={delArea} add={() => newArea()} />
        </section>
        {lines ? <TryLines lines={lines} trying={trying} steps={steps} m={m} name={windowName} note={seen?.note} /> : null}
      </aside>
    </div>
  </div>;
}

/** The macro's areas: each one's colour on the game, its name (what the steps say: "in Fish area"), and removing it. */
function AreaList({ areas, set, del, add }: { areas: Area[]; set: (name: string, p: Partial<Area>) => void; del: (name: string) => void; add: () => void }) {
  const { t } = useHost();
  return <div className="mc-areas" role="group" aria-labelledby="mc-areas-h">
    <div className="mc-areas-head"><b id="mc-areas-h">{t('macro.area.title')}</b>
      <button type="button" className="btn sm" id="mc-area-add" onClick={add}><Icon name="plus" /><span>{t('macro.area.new')}</span></button></div>
    {!areas.length ? <p className="mc-pic-none">{t('macro.area.none')}</p> : <ul>{areas.map(a => <li key={a.name}>
      <i style={{ borderColor: a.colour }} aria-hidden="true" />
      <input className="field sm" value={a.label} maxLength={30} aria-label={t('macro.area.name')} onChange={e => set(a.name, { label: e.target.value })} />
      <button type="button" className="icon-btn" title={t('macro.area.del')} aria-label={`${t('macro.area.del')}: ${a.label}`} onClick={() => del(a.name)}><Icon name="trash" /></button>
    </li>)}</ul>}
  </div>;
}

/** The recording this form started: its countdown, then its steps so far. */
function EditorRecording({ startsAt, count }: { startsAt: string; count: number }) {
  const { t, call, busy } = useHost();
  const left = useCountdown(startsAt);
  return <div className="mc-rec in-form" role="status">
    <span className="mc-rec-dot" aria-hidden="true" />
    <b>{left > 0 ? t('macro.rec.startIn', { n: left }) : t('macro.rec.count', { n: count })}</b>
    <small>{t('macro.rec.hint')}</small>
    <button type="button" className="btn sm" onClick={e => busy(e.currentTarget, () => call('stopRecording', {}))}><Icon name="stop" /><span>{t('macro.rec.stop')}</span></button>
  </div>;
}

function StepRow({ s, i, m, count, depth, problems, dragging, ed, focus, move, dup, del, cutting, dragStart, open, toggle }: {
  s: Held; i: number; m: Macro; count: number; depth: number; problems: { text: string; warn?: boolean }[]; dragging: boolean; open: boolean; toggle: () => void;
  ed: Ed; focus: () => void; move: (dir: number, btn: HTMLElement) => void; dup: () => void; del: () => void; cutting: boolean; dragStart: () => void;
}) {
  const { t } = useHost();
  const n = i + 1;
  const [armed, setArmed] = useState(false);
  const body = open ? <StepBody s={s} i={i} m={m} ed={ed} cutting={cutting} /> : null;
  const plain = ['else', 'endIf', 'endLoop', 'break', 'stop', 'rejoin', 'recover'].includes(s.kind);
  const bad = problems.some(p => !p.warn);
  const say = stepText(t, s, m, n);
  return <div className={`mc-step${dragging ? ' dragging' : ''}${bad ? ' bad' : ''}${open ? ' open' : ''}`} role="listitem" aria-label={`${t('macro.stepN', { n })}: ${say}`}
    style={{ '--d': depth } as React.CSSProperties} draggable={armed} onFocus={focus}
    onDragStart={e => { if (!armed) return; dragStart(); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(s._k)); }}
    onDragEnd={() => setArmed(false)} onPointerUp={() => setArmed(false)}>
    <span className="ord-grip" title={t('lo.drag')} onPointerDown={() => setArmed(true)}><Icon name="grip" /></span>
    <span className="mc-step-n" aria-hidden="true">{n}</span>
    {plain ? <span className={`mc-step-head k-${s.kind}`}><Icon name={STEP_ICON[s.kind] || 'info'} /><span className="mc-say">{say}</span></span>
      : <button type="button" className={`mc-step-head k-${s.kind}`} aria-expanded={open} onClick={toggle}>
        <Icon name={STEP_ICON[s.kind] || 'info'} /><span className="mc-say">{say}</span><Icon name={open ? 'chevronUp' : 'chevronDown'} /></button>}
    <span className="mc-step-acts">
      <button type="button" className="icon-btn" data-move="-1" title={t('lo.up')} aria-label={`${t('lo.up')}: ${t('macro.stepN', { n })}`} disabled={i === 0} onClick={e => move(-1, e.currentTarget)}><Icon name="chevronUp" /></button>
      <button type="button" className="icon-btn" data-move="1" title={t('lo.down')} aria-label={`${t('lo.down')}: ${t('macro.stepN', { n })}`} disabled={i >= count - 1} onClick={e => move(1, e.currentTarget)}><Icon name="chevronDown" /></button>
      <button type="button" className="icon-btn" data-st="dup" title={t('macro.dupStep')} aria-label={`${t('macro.dupStep')}: ${t('macro.stepN', { n })}`} onClick={dup}><Icon name="copy" /></button>
      <button type="button" className="icon-btn" data-st="del" title={t('macro.delStep')} aria-label={`${t('macro.delStep')}: ${t('macro.stepN', { n })}`} onClick={del}><Icon name="trash" /></button>
    </span>
    {body ? <div className="mc-step-fields">{body}</div> : null}
    {problems.length ? <ul className="mc-step-probs">{problems.map((p, k) => <li key={k} className={p.warn ? 'warn' : ''}><Icon name="alert" />{p.text}</li>)}</ul> : null}
  </div>;
}

/** The open step: its sentence again, with its values in place; under it the ticks that hand values to the player. */
function StepBody({ s, i, m, ed, cutting }: { s: Held; i: number; m: Macro; ed: Ed; cutting: boolean }) {
  const { t } = useHost();
  const n = i + 1;
  const id = (f: string) => `mc-s${s._k}-${f}`;
  const v = (path: string) => ed.val(i, path);
  const put = (path: string) => (x: unknown) => ed.put(i, path, x);
  const test = s.test as Test | undefined;
  const isTest = s.kind === 'if' || s.kind === 'loopUntil';
  const pre = isTest ? 'test.' : '';

  const btnSel = (path: string, kinds: string[]) => <Pick id={id('b')} label={t('macro.e.buttonL')} value={String(v(path) || 'left')}
    options={kinds.map(b => [b, t(`macro.s.b.${b}`)])} onChange={put(path)} />;
  const secs = (path: string, label: string) => <Secs id={id(path.replace('.', '-'))} label={label} ms={v(path)} onChange={put(path)} />;
  const point = <button type="button" className="mc-pt" title={t('macro.e.pickPoint')} disabled={!ed.canPick} onClick={() => ed.pick(i, 'point')}>
    <span aria-hidden="true">{n}</span><span className="mc-vh">{t('macro.e.pickPoint')}</span></button>;
  const colour = (path: string, what: 'colour' | 'target' | 'marker', label: string) => <span className="mc-colourf">
    <ColorInput id={id(path.replace('.', '-'))} value={/^#[0-9a-f]{6}$/i.test(String(v(path))) ? String(v(path)) : '#ffffff'} label={label} onChange={put(path)} />
    <button type="button" className="btn sm mc-pick" disabled={!ed.canPick} title={ed.canPick ? t('macro.e.pickTip') : t('macro.needWindow')}
      onClick={() => ed.pick(i, what, path)}><Icon name="mouse" /><span>{t('macro.e.pick')}</span></button></span>;
  const where = (path: string, withPoint: boolean) => {
    const raw = getAt(s, path);
    const cur = isArea(raw) ? raw : withPoint ? 'point' : 'anywhere';
    return <Pick id={id('where')} label={t('macro.e.whereL')} value={cur} className="mc-where"
      options={[withPoint ? ['point', t('macro.s.atPoint', { n })] : ['anywhere', t('macro.s.anywhere')],
        ...ed.areas.map(a => ['@' + a.name, t('macro.s.inArea', { area: a.label })]),
        ...(isArea(raw) && !ed.areas.some(a => '@' + a.name === raw) ? [[raw, t('macro.s.inArea', { area: t('macro.s.gone') })]] : []),
        ['new', t('macro.area.newIn')]]}
      onChange={x => {
        const r = x === 'new' ? ed.newArea() : x === 'point' || x === 'anywhere' ? undefined : x;
        if (x === 'new' && !r) return;
        ed.set(i, isTest ? { test: { ...s.test, region: r } } : { region: r });
      }} />;
  };
  const shows = (path: string, gone: boolean) => <Pick id={id('gone')} label={t('macro.e.showsL')} value={gone ? 'gone' : 'shows'}
    options={[['shows', t('macro.e.shows')], ['gone', t('macro.e.gone')]]} onChange={x => ed.set(i, path === 'test.not' ? { test: { ...s.test, not: x === 'gone' } } : { gone: x === 'gone' })} />;
  const pic = (img: string) => <span className="mc-imgf">
    {img ? <img className="mc-thumb" src={`data:image/png;base64,${img}`} alt={t('macro.e.thePic')} /> : <span className="mc-thumb none" aria-hidden="true"><Icon name="image" /></span>}
    <button type="button" className={`btn sm mc-cut${cutting ? ' on' : ''}`} aria-pressed={cutting} onClick={() => ed.cut(i)}><Icon name="image" />
      <span>{t(cutting ? 'macro.e.cutting' : img ? 'macro.e.recut' : 'macro.e.cut')}</span></button></span>;
  const until = (path = 'timeoutMs') => <Say k="macro.e.giveUp" parts={{ d: secs(path, t('macro.e.giveUpL')) }} />;
  const textIn = (path: string, max: number, label: string, wide = false) => <TextIn id={id(path.replace('.', '-'))} label={label} value={String(v(path) ?? '')} max={max} wide={wide} onChange={put(path)} />;
  // A text to read, or any word the player ticks in a list setting (once the panel has one).
  const lists = (m.options || []).filter(o => o.kind === 'list');
  const textOrWords = (at: '' | 'test.') => {
    const words = String(getAt(s, at + 'words') ?? '');
    const box = textIn(at + 'text', 100, t('macro.e.textL'));
    if (!lists.length && !words) return box;
    const pick = <Pick id={id(at + 'words')} label={t('macro.e.readL')} value={words}
      options={[['', t('macro.e.thisText')], ...lists.map(o => ['$' + o.name, t('macro.e.oneOf', { name: o.label })]),
        ...(words && !lists.some(o => '$' + o.name === words) ? [[words, t('macro.e.oneOf', { name: t('macro.s.gone') })]] : [])]}
      onChange={x => ed.set(i, at ? { test: { ...s.test, words: x || undefined } } : { words: x || undefined })} />;
    return words ? pick : <>{pick}{box}</>;
  };
  const num = (path: string, label: string, min: number, max: number) => <Count id={id(path.replace('.', '-'))} label={label} value={v(path)} min={min} max={max} onChange={put(path)} />;
  const tune = (kids: ReactNode) => <details className="mc-tune"><summary>{t('macro.e.tune')}</summary><div>{kids}</div></details>;
  const closeness = (path: string) => <Say k="macro.e.closeness" parts={{ n: num(path, t('macro.e.closenessL'), 0, 255) }} />;
  const match = (path: string) => <Say k="macro.e.match" parts={{ n: <Count id={id('th')} label={t('macro.e.matchL')} value={Math.round((Number(v(path)) || 0.9) * 100)}
    min={50} max={100} onChange={x => ed.put(i, path, x / 100)} /> }} />;

  let lines: ReactNode = null;
  switch (s.kind) {
    case 'key': lines = <>
      <Say k="macro.e.key" parts={{ key: <KeyBox id={id('key')} value={String(v('key'))} k={s._k} ed={ed} onChange={put('key')} /> }} />
      {tune(<Say k="macro.e.keyHold" parts={{ d: secs('ms', t('macro.e.keyHoldL')) }} />)}</>; break;
    case 'down': lines = <Say k="macro.e.down" parts={{ key: <KeyBox mouse id={id('key')} value={s.key} k={s._k} ed={ed} onChange={put('key')} /> }} />; break;
    case 'up': lines = <Say k="macro.e.up" parts={{ key: <KeyBox mouse id={id('key')} value={s.key} k={s._k} ed={ed} onChange={put('key')} /> }} />; break;
    case 'type': lines = <><Say k="macro.e.type" parts={{ text: textIn('text', 200, t('macro.e.typeL'), true) }} /><small className="mc-f-note">{t('macro.typeNote')}</small></>; break;
    case 'click': {
      const how = s.holdMs ? 'hold' : s.double ? 'double' : 'click';
      const howSel = <Pick id={id('how')} label={t('macro.e.howL')} value={how} options={['click', 'double', 'hold'].map(k => [k, t(`macro.e.how.${k}`)])}
        onChange={x => ed.set(i, { double: x === 'double', holdMs: x === 'hold' ? (ed.val(i, 'holdMs') || 500) : undefined })} />;
      lines = how === 'hold'
        ? <Say k="macro.e.clickHold" parts={{ how: howSel, b: btnSel('button', ['left', 'right', 'middle']), d: secs('holdMs', t('macro.e.holdL')), n: point }} />
        : <Say k="macro.e.click" parts={{ how: howSel, b: btnSel('button', ['left', 'right', 'middle']), n: point }} />;
      break;
    }
    case 'move': lines = <Say k="macro.e.move" parts={{ n: point }} />; break;
    case 'camera': {
      const dir = camDir(s);
      const size = Math.max(Math.abs(Number(s.dx) || 0), Math.abs(Number(s.dy) || 0));
      const amount = size <= 150 ? 'little' : size <= 350 ? 'some' : 'lot';
      const turn = (d: string, a: keyof typeof CAM) => ed.set(i, {
        dx: d === 'left' ? -CAM[a] : d === 'right' ? CAM[a] : 0, dy: d === 'up' ? -CAM[a] : d === 'down' ? CAM[a] : 0, button: 'right', ms: s.ms ?? 300 });
      lines = <Say k="macro.e.camera" parts={{
        dir: <Pick id={id('dir')} label={t('macro.e.dirL')} value={dir} options={['left', 'right', 'up', 'down'].map(k => [k, t(`macro.e.dir.${k}`)])} onChange={x => turn(x, amount)} />,
        amount: <Pick id={id('amt')} label={t('macro.e.amountL')} value={amount} options={(['little', 'some', 'lot'] as const).map(k => [k, t(`macro.e.amount.${k}`)])} onChange={x => turn(dir, x as keyof typeof CAM)} /> }} />;
      break;
    }
    case 'scroll': {
      const up = Number(s.notches) > 0;
      lines = <Say k="macro.e.scroll" parts={{
        dir: <Pick id={id('dir')} label={t('macro.e.dirL')} value={up ? 'up' : 'down'} options={[['up', t('macro.e.dir.up')], ['down', t('macro.e.dir.down')]]}
          onChange={x => ed.set(i, { notches: (x === 'up' ? 1 : -1) * Math.abs(Number(s.notches) || 3) })} />,
        n: <Count id={id('n')} label={t('macro.e.timesL')} value={Math.abs(Number(s.notches) || 0)} min={1} max={50} onChange={x => ed.set(i, { notches: (up ? 1 : -1) * x })} /> }} />;
      break;
    }
    case 'wait': lines = <Say k="macro.e.wait" parts={{ d: secs('ms', t('macro.e.waitL')) }} />; break;
    case 'waitStill': lines = <>{<Say k="macro.e.waitStill" parts={{ d: secs('ms', t('macro.e.waitL')), where: where('region', false) }} />}{until()}</>; break;
    case 'clickImage': lines = <>
      <Say k="macro.e.clickImage" parts={{ pic: pic(s.image), where: where('region', false), b: btnSel('button', ['left', 'right']) }} />
      {until()}{tune(match('threshold'))}</>; break;
    case 'waitImage': lines = <>
      <Say k="macro.e.waitImage" parts={{ pic: pic(s.image), shows: shows('gone', !!s.gone), where: where('region', false) }} />
      {until()}{tune(match('threshold'))}</>; break;
    case 'waitText': lines = <>
      <Say k="macro.e.waitText" parts={{ text: textOrWords(''), shows: shows('gone', !!s.gone), where: where('region', false) }} />{until()}</>; break;
    case 'waitColour': lines = <>
      <Say k="macro.e.waitColour" parts={{ c: colour('colour', 'colour', t('macro.e.colourL')), shows: shows('gone', !!s.gone), where: where('region', true) }} />
      {until()}{tune(closeness('tolerance'))}</>; break;
    case 'clickColour': lines = <>
      <Say k="macro.e.clickColour" parts={{ c: colour('colour', 'colour', t('macro.e.colourL')), where: where('region', false) }} />
      <Say k="macro.e.clickHow" parts={{ how: <Pick id={id('each')} label={t('macro.e.clickHowL')} value={s.eachAfterMs != null ? 'each' : s.repeatUntilGoneMs ? 'again' : 'once'}
        options={[['once', t('macro.e.clickOnce')], ['again', t('macro.e.again')], ['each', t('macro.e.clickEach')]]}
        onChange={x => ed.set(i, x === 'each' ? { eachAfterMs: 300, repeatUntilGoneMs: s.repeatUntilGoneMs || 1000 }
          : { eachAfterMs: undefined, repeatUntilGoneMs: x === 'again' ? s.repeatUntilGoneMs || 1000 : undefined })} /> }} />
      {s.eachAfterMs != null ? <Say k="macro.e.eachAfter" parts={{ d: secs('eachAfterMs', t('macro.e.eachAfterL')) }} /> : null}
      {until()}{tune(<>{closeness('tolerance')}<Say k="macro.e.minSize" parts={{ n: num('minSize', t('macro.e.minSizeL'), 0, 10000) }} /></>)}</>; break;
    case 'if': case 'loopUntil': {
      const kind = test?.kind || 'text';
      // A counter, or a setting the player sets: a switch is on or off, a choice one of its options.
      const varTest = () => {
        const name = String(test?.kind === 'var' ? test.name : '');
        const o = ed.readable.find(x => x.name === name);
        const names = [...ed.counters.map(c => [c, c]), ...ed.readable.map(x => [x.name, x.label])];
        if (name && !names.some(([k]) => k === name)) names.push([name, name]);
        const nameIn = ed.readable.length ? <Pick id={id('test-name')} label={t('macro.e.counterL')} value={name} options={names}
          onChange={x => {
            const to = ed.readable.find(y => y.name === x);
            ed.set(i, { test: { kind: 'var', name: x, op: to?.kind === 'check' || to?.kind === 'choice' ? '=' : String(test && 'op' in test ? test.op : '>='),
              value: to?.kind === 'check' ? 1 : to?.kind === 'choice' ? to.choices?.[0] ?? '' : 1 } });
          }} /> : textIn('test.name', 30, t('macro.e.counterL'));
        if (o?.kind === 'check') return <Say k="macro.e.testSwitch" vars={{ lead: head }} parts={{ what: lead, name: nameIn,
          v: <Pick id={id('v')} label={t('macro.e.valueL')} value={test?.kind === 'var' && test.value ? '1' : '0'}
            options={[['1', t('macro.e.switchOn')], ['0', t('macro.e.switchOff')]]} onChange={x => ed.put(i, 'test.value', +x)} /> }} />;
        const op = <Pick id={id('op')} label={t('macro.e.opL')} value={String(test && 'op' in test ? test.op : '>=')}
          options={(o?.kind === 'choice' ? ['=', '!='] : OPS).map(x => [x, t(`macro.e.op.${x}`)])} onChange={put('test.op')} />;
        const value = o?.kind === 'choice' ? <Pick id={id('v')} label={t('macro.e.valueL')} value={String(test?.kind === 'var' ? test.value : '')}
          options={(o.choices || []).map(c => [c, c])} onChange={put('test.value')} /> : num('test.value', t('macro.e.valueL'), -1e9, 1e9);
        return <Say k="macro.e.testVar" vars={{ lead: head }} parts={{ what: lead, name: nameIn, op, v: value }} />;
      };
      const lead = <Pick id={id('tk')} label={t('macro.e.checkL')} value={kind} options={['image', 'text', 'colour', 'var'].map(k => [k, t(`macro.e.check.${k}`)])}
        onChange={x => ed.set(i, { test: newTest(x) })} />;
      const head = t(s.kind === 'if' ? 'macro.s.if' : 'macro.s.until');
      const rest = kind === 'image' ? <Say k="macro.e.testImage" vars={{ lead: head }} parts={{ what: lead, pic: pic(String(test?.kind === 'image' ? test.image : '')), shows: shows('test.not', !!(test && 'not' in test && test.not)), where: where('test.region', false) }} />
        : kind === 'text' ? <Say k="macro.e.testText" vars={{ lead: head }} parts={{ what: lead, text: textOrWords('test.'), shows: shows('test.not', !!(test && 'not' in test && test.not)), where: where('test.region', false) }} />
        : kind === 'colour' ? <Say k="macro.e.testColour" vars={{ lead: head }} parts={{ what: lead, c: colour(`${pre}colour`, 'colour', t('macro.e.colourL')), shows: shows('test.not', !!(test && 'not' in test && test.not)), where: where('test.region', true) }} />
        : varTest();
      lines = <>{rest}{s.kind === 'loopUntil' ? <Say k="macro.e.atMost" parts={{ n: num('max', t('macro.e.atMostL'), 0, 100000) }} /> : null}</>;
      break;
    }
    case 'loop': {
      const times = Number(v('times')) || 0;
      lines = <Say k={times ? 'macro.e.loopN' : 'macro.e.loop'} parts={{
        mode: <Pick id={id('mode')} label={t('macro.e.loopL')} value={times ? 'n' : 'forever'} options={[['forever', t('macro.e.loopForever')], ['n', t('macro.e.loopCount')]]}
          onChange={x => ed.put(i, 'times', x === 'n' ? 5 : 0)} />,
        n: num('times', t('macro.e.timesL'), 1, 100000) }} />;
      break;
    }
    case 'setVar': lines = <Say k="macro.e.setVar" parts={{ name: textIn('name', 30, t('macro.e.counterL')), v: num('value', t('macro.e.valueL'), -1e9, 1e9) }} />; break;
    case 'addVar': lines = <Say k="macro.e.addVar" parts={{ name: textIn('name', 30, t('macro.e.counterL')), v: num('value', t('macro.e.valueL'), -1e9, 1e9) }} />; break;
    case 'alert': lines = <>
      <Say k="macro.e.alert" parts={{ text: textIn('text', 200, t('macro.e.alertL'), true) }} />
      <Check id={id('pic')} label={t('macro.e.alertPic')} checked={!!s.picture} onChange={x => ed.set(i, { picture: x })} />
      <small className="mc-f-note">{t('macro.alertNote')}</small></>; break;
    case 'follow': {
      const edges = !!s.marker && typeof s.marker === 'object';
      lines = <>
        <Say k="macro.e.follow" parts={{
          marker: edges ? <span className="mc-colourf">{t('macro.e.edges')}</span> : colour('marker', 'marker', t('macro.e.markerL')),
          target: colour('target', 'target', t('macro.e.targetL')), where: where('region', false),
          b: <Pick id={id('b')} label={t('macro.e.buttonL')} value={s.button === 'key' ? 'key' : String(s.button || 'left')}
            options={[['left', t('macro.s.b.left')], ['right', t('macro.s.b.right')], ['key', t('macro.e.aKey')]]}
            onChange={x => ed.set(i, x === 'key' ? { button: x, key: s.key || 'Space' } : { button: x })} /> }} />
        {s.button === 'key' ? <Say k="macro.e.followKey" parts={{ key: <KeyBox id={id('key')} value={s.key} k={s._k} ed={ed} onChange={put('key')} /> }} /> : null}
        <Say k="macro.e.followEnd" parts={{ d: secs('untilGoneMs', t('macro.e.goneL')) }} />
        {tune(<>
          <Say k="macro.e.markerIs" parts={{ kind: <Pick id={id('mk')} label={t('macro.e.markerIsL')} value={edges ? 'edges' : 'one'}
            options={[['one', t('macro.e.markerOne')], ['edges', t('macro.e.markerEdges')]]}
            onChange={x => ed.set(i, { marker: x === 'edges' ? { left: String(ed.val(i, 'marker') || '#ffffff'), right: String(ed.val(i, 'marker') || '#ffffff') } : (edges ? s.marker.left : s.marker) })} /> }} />
          {edges ? <Say k="macro.e.edgeColours" parts={{ l: colour('marker.left', 'marker', t('macro.e.leftL')), r: colour('marker.right', 'marker', t('macro.e.rightL')) }} /> : null}
          <Say k="macro.e.moves" parts={{ how: <Pick id={id('mode')} label={t('macro.e.movesL')} value={s.mode || 'onOff'}
            options={[['onOff', t('macro.e.modeOnOff')], ['smooth', t('macro.e.modeSmooth')]]} onChange={put('mode')} /> }} />
          <Check id={id('pred')} label={t('macro.e.predict')} checked={!!s.predict} onChange={x => ed.set(i, { predict: x })} />
          {closeness('tolerance')}
          <Say k="macro.e.fps" parts={{ n: num('fps', t('macro.e.fpsL'), 15, 240) }} />
          {until()}</>)}
        <small className="mc-f-note">{t('macro.followNote')}</small></>;
      break;
    }
  }

  // The values this step may hand to the player, ticked or not.
  const can = Object.keys(META).filter(k => k.startsWith(s.kind + '.')).map(k => k.slice(s.kind.length + 1))
    .filter(path => !(path === 'holdMs' && !s.holdMs) && !(path === 'marker' && typeof s.marker === 'object') && !(path === 'test.value' && test?.kind !== 'var'));
  return <>
    {lines}
    {can.length ? <div className="mc-ticks" role="group" aria-label={t('macro.opt.can')}><span>{t('macro.opt.can')}</span>
      {can.map(path => {
        const raw = getAt(s, path === 'point' ? 'x' : path);
        const o = isOpt(raw) ? (m.options || []).find(x => '$' + x.name === raw) : undefined;
        return <Check key={path} id={id(`tick-${path.replace('.', '-')}`)} label={o?.label || t(META[`${s.kind}.${path}`].label)} checked={ed.ticked(i, path)} onChange={x => ed.tick(i, path, x)} />;
      })}</div> : null}
  </>;
}

/* ── the sentence and its values ── */

/** A sentence with its values in place: t(k)'s {names} become the given parts (in any language's order). */
export function Say({ k, parts, vars }: { k: string; parts: Record<string, ReactNode>; vars?: Record<string, string | number> }) {
  const { t } = useHost();
  const MARK = '\u0001';
  const raw = t(k, { ...vars, ...Object.fromEntries(Object.keys(parts).map(p => [p, MARK + p + MARK])) });
  return <p className="mc-sent">{raw.split(MARK).map((b, i) => i % 2 ? <Fragment key={i}>{parts[b]}</Fragment> : b ? <span key={i}>{b}</span> : null)}</p>;
}

function Pick({ id, label, value, options, onChange, className }: { id: string; label: string; value: string; options: string[][]; onChange: (v: string) => void; className?: string }) {
  return <select className={`select sm${className ? ' ' + className : ''}`} id={id} aria-label={label} value={value} onChange={e => onChange(e.target.value)}>
    {options.map(([v, text]) => <option key={v} value={v}>{text}</option>)}</select>;
}

/** A time in seconds (kept in milliseconds): "0.5 s". */
function Secs({ id, label, ms, onChange }: { id: string; label: string; ms: unknown; onChange: (v: number) => void }) {
  const { t } = useHost();
  const n = typeof ms === 'number' && Number.isFinite(ms) ? +(ms / 1000).toFixed(3) : '';
  return <span className="mc-unit"><input className="field sm num" id={id} type="number" min={0} step="0.1" aria-label={label} value={n}
    onChange={e => onChange(e.target.value === '' ? NaN : Math.round(+e.target.value * 1000))} /><span aria-hidden="true">{t('macro.unit.s')}</span></span>;
}

/** A whole number in a field of the form (a label above it). */
function NumField({ id, value, min, max, onChange }: { id: string; value: unknown; min: number; max: number; onChange: (v: number) => void }) {
  return <input className="field num" id={id} type="number" min={min} max={max}
    value={typeof value === 'number' && Number.isFinite(value) ? value : ''} onChange={e => onChange(e.target.value === '' ? NaN : +e.target.value)} />;
}

function Count({ id, label, value, min, max, onChange }: { id: string; label: string; value: unknown; min: number; max: number; onChange: (v: number) => void }) {
  return <input className="field sm num mc-count" id={id} type="number" min={min} max={max} aria-label={label}
    value={typeof value === 'number' && Number.isFinite(value) ? value : ''} onChange={e => onChange(e.target.value === '' ? NaN : +e.target.value)} />;
}

function TextIn({ id, label, value, max, wide, onChange }: { id: string; label: string; value: string; max: number; wide?: boolean; onChange: (v: string) => void }) {
  return <input className={`field sm mc-text${wide ? ' wide' : ''}`} id={id} maxLength={max} spellCheck={false} aria-label={label} value={value}
    placeholder={label} onChange={e => onChange(e.target.value)} />;
}

export function Check({ id, label, checked, onChange }: { id: string; label: string; checked: boolean; onChange: (v: boolean) => void }) {
  return <label className="mc-check"><input type="checkbox" id={id} checked={checked} onChange={e => onChange(e.target.checked)} /><span>{label}</span></label>;
}

/** A key from the list, or the next key pressed in the "Press a key" box. Escape there is taken as the key too (the
 * dialog's own Escape stands back while the box says it is listening, aria-expanded). */
function KeyBox({ id, value, onChange, k, ed, mouse }: { id: string; value: string; onChange: (k: string) => void; k: number; ed: Ed; mouse?: boolean }) {
  const { t, toast } = useHost();
  const cap = useRef<HTMLButtonElement>(null);
  const capture = ed.capture === k;
  const setCapture = (on: boolean) => ed.setCapture(on ? k : null);
  useEffect(() => { if (capture) cap.current?.focus(); }, [capture]);
  const known = KEYS.includes(value) || (mouse && MOUSE_KEYS.includes(value));
  return <span className="mc-keyf">
    <select className="select sm" id={id} value={known ? value : ''} aria-label={t('macro.f.key')} onChange={e => onChange(e.target.value)}>
      {known ? null : <option value="">—</option>}
      {mouse ? MOUSE_KEYS.map(x => <option key={x} value={x}>{keyName(t, x)}</option>) : null}
      {KEYS.map(x => <option key={x} value={x}>{keyCap(x)}</option>)}</select>
    <button type="button" ref={cap} className={`btn sm mc-keycap${capture ? ' on' : ''}`} aria-expanded={capture} aria-pressed={capture}
      title={t('macro.f.pressTip')} onClick={() => setCapture(!capture)} onBlur={() => setCapture(false)}
      onKeyDown={e => {
        if (!capture) return;
        if (e.key === 'Tab') return setCapture(false);
        e.preventDefault();
        e.stopPropagation();
        const key = keyFromEvent(e);
        if (!key) return toast(t('macro.f.badKey'), 'warn');
        onChange(key);
        setCapture(false);
      }}><Icon name="keyboard" /><span>{t(capture ? 'macro.f.pressing' : 'macro.f.press')}</span></button></span>;
}

