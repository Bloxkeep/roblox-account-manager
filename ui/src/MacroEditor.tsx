// The macro form (new, from a template, recorded, imported, or edit). On top: its name, one folded line for when and
// where it runs, and Try it. Left: the steps as numbered sentences; If and Repeat are blocks (their steps indented on a
// rail, folded by the opener's chevron, ended by a slim "end if" line); the open one shows its few values in the sentence
// itself, each with a "Player can change" tick that puts it on the panel (an option: its label from the sentence, its
// range and unit from the value, never asked). A step may carry a note (why it is there). Areas are picked by name.
// Add step and the "+" between rows open one searchable list. What stops a save is said on its own row, with the fix
// one click away; the dialog's foot counts them. Under the steps, what the player sees: the panel, its labels editable
// in place. Right: Edit on game (the dots and boxes drawn on the Roblox window, dragged there), the same dots and boxes
// on a picture of the window, and Try it's results as plain lines (the step running, or where it stopped, marked on its
// row too). app.js's dialog keeps the title and buttons; it asks this form what to save (collect) and whether anything
// changed (dirty).
import { Fragment, useEffect, useImperativeHandle, useLayoutEffect, useRef, useState, type ReactNode, type Ref } from 'react';
import { createPortal, flushSync } from 'react-dom';
import { Icon, useAppState, useHost } from './host';
import { Pop } from './MacroPop';
import { Avatar } from './Avatar';
import { ColorInput } from './ColorField';
import { GameBox } from './PresetForm';
import { GameFrame, TryLines, type Picture, type Seen } from './MacroPicture';
import { PlayerSees } from './MacroPanel';
import { hasWindow, useCountdown } from './MacrosView';
import {
  AREA_COLOURS, FREE, MAX_FEATURES, MAX_NOTE, OPENERS, asPoint, KEYS, LOOK_STEPS, MAX_AREAS, MAX_OPTIONS, MAX_TABS, MOUSE_KEYS, STEP_GROUPS, STEP_ICON, SYNONYMS,
  blockEnd, camDir, counterName, counters, durText, endOf, featureProblems, freeName, isArea, isOpt, isLive, keyCap, keyFromEvent, keyName, kindsAt, newStep, newTest,
  pointOf, problems, recording, safeEnds, stepText, stepsToAdd, withAreas,
  type Area, type Feature, type Macro, type MacroRun, type Option, type Problem, type Rect, type Step, type Test,
} from './macros';

type Account = { id: string; label: string; username?: string; avatarUrl?: string | null; state?: string; session?: { pid?: number } | null };
export interface MacroEditorApi {
  /** What Save sends (saveMacro's macro), or the first thing stopping it and the box to show. */
  collect(): { macro: Macro } | { error: string; box: HTMLElement | null };
  dirty(): boolean;
}
/** A step as the form holds it: its own key for React (taken off before saving). */
type Held = Step & { _k: number };
/** A feature as the form holds it: its steps are kept in the form's lists, by its id. */
type FeatMeta = Omit<Feature, 'steps'>;
/** A step somewhere in the macro: in a feature's steps (sec, its id) or the main ones (null). */
type At = { sec: string | null; i: number };

let seq = 0;
const hold = (s: Step): Held => ({ ...s, _k: ++seq });
const strip = ({ _k, ...s }: Held): Step => { void _k; return s; };
const getAt = (s: Step, path: string): any => path.split('.').reduce<any>((o, k) => o?.[k], s);
const setAt = <S extends Step>(s: S, path: string, v: unknown): S => {
  const [a, b] = path.split('.');
  return (b ? { ...s, [a]: { ...s[a], [b]: v } } : { ...s, [a]: v }) as S;
};
/** Words compared without case or Vietnamese marks ("cho" finds "Chờ"). */
const plain = (s: string) => s.normalize('NFD').replace(/[̀-ͯ]/g, '').replace(/đ/g, 'd').replace(/Đ/g, 'D').toLowerCase();
/** A counter's name (its key on the panel and in the run): letters, digits and _, not starting with a digit. */
const COUNTER_NAME = /^[A-Za-z_][A-Za-z0-9_]{0,31}$/;

/** The values a maker may hand to the player, by step kind and field: what the option is, its range and unit, and the
 * label it starts with (the player sees it on the panel; the maker can rename it there). */
type Meta = { kind: Option['kind']; min?: number; max?: number; unit?: Option['unit']; label: string };
const META: Record<string, Meta> = {
  'key.key': { kind: 'key', label: 'macro.opt.key' },
  'click.point': { kind: 'point', label: 'macro.opt.point' },
  'move.point': { kind: 'point', label: 'macro.opt.point' },
  'click.holdMs': { kind: 'number', min: 30, max: 10000, unit: 's', label: 'macro.opt.hold' },
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
/** A block's rail: an If's in the info colour, a repeat's in a quiet accent. */
const RAIL: Record<string, string> = { if: 'var(--online)', loop: 'color-mix(in srgb, var(--accent) 40%, var(--surface))' };
const railOf = (kind: string) => RAIL[kind === 'if' ? 'if' : 'loop'];
/** How many kinds Add step remembers as used lately. */
const RECENT = 5;

/** Each row of the list as drawn: how deep it sits, the blocks around it (outermost first), and for an opener where its
 * block ends and how many steps it holds; an end row's opener; the folded block hiding it, if any. */
type Shape = { depth: number; rails: string[]; end: number; opener: number; inner: number; hidden: boolean; around: number[] };
function shapes(steps: Held[], folded: Set<number>): Shape[] {
  const open: { at: number; kind: string }[] = [];
  return steps.map((s, i) => {
    const k = s.kind;
    let opener = -1;
    if ((k === 'endIf' && open.at(-1)?.kind === 'if') || (k === 'endLoop' && open.length && open.at(-1)!.kind !== 'if')) opener = open.pop()!.at;
    const around = open.map(o => o.at);
    const hidden = around.some(a => folded.has(steps[a]._k)) || (opener >= 0 && folded.has(steps[opener]._k));
    const shape: Shape = { depth: open.length, rails: open.map(o => o.kind), end: -1, opener, inner: 0, hidden, around };
    if (OPENERS.has(k)) {
      shape.end = blockEnd(steps, i);
      const last = shape.end >= 0 ? shape.end : steps.length;
      shape.inner = steps.slice(i + 1, last).filter(x => !['else', 'endIf', 'endLoop'].includes(x.kind)).length;
      open.push({ at: i, kind: k });
    }
    return shape;
  });
}

/** The form, with what its steps' sentences need: values, writing them, the ticks, the areas, the counters. */
type Ed = {
  val: (i: number, path: string) => any; put: (i: number, path: string, v: unknown) => void; set: (i: number, p: Partial<Step>) => void;
  ticked: (i: number, path: string) => boolean; tick: (i: number, path: string, on: boolean) => void;
  areas: Area[]; newArea: () => string | null; pick: (i: number, what: 'point' | 'colour' | 'target' | 'marker', path?: string) => void; cut: (i: number) => void;
  canPick: boolean; capture: number | null; setCapture: (k: number | null) => void;
  /** The counters (Set and Add steps' names), what each is called, and the settings an If can read. */
  counters: string[]; label: (name: string) => string; readable: Option[];
  /** "New counter…": its small dialog, under the list it was picked in; done gets the name. */
  newCounter: (anchor: HTMLElement, done: (name: string) => void) => void;
};

export function MacroEditor({ base, accounts, macros, api, openStep, openSection, stoppedReason, goAccounts }: {
  base: Macro; isNew?: boolean; accounts: Account[]; macros: Macro[]; api: Ref<MacroEditorApi>;
  /** The Macros list's Fix: this step open and in sight (in this feature's steps, if one), marked where its last run stopped (and why). */
  openStep?: number; openSection?: string | null; stoppedReason?: string;
  /** The empty picture's "Go to Accounts": closes the form (asking first when something changed) and shows Accounts. */
  goAccounts?: () => void;
}) {
  const host = useHost();
  const { t, tn, call, busy, toast, on } = host;
  const app = useAppState();

  // A macro from before areas gets its drawn boxes as areas: the form counts that as where it started.
  const [start] = useState(() => withAreas(t, base));
  const [m, setM] = useState<Macro>(() => ({ ...start, trigger: { ...start.trigger }, accountIds: [...start.accountIds] }));
  const patch = (p: Partial<Macro>) => setM(x => ({ ...x, ...p }));
  // Every list of steps by its key (the main steps: ''); the one shown is `steps`, its entry here stale while it is.
  const [init] = useState(() => {
    const own = Object.fromEntries((start.features || []).map(f => [f.id, f.steps.map(hold)]));
    return { lists: { '': start.steps.map(hold), ...own } as Record<string, Held[]>, sec: openSection && own[openSection] ? openSection : null };
  });
  const [feats, setFeats] = useState<FeatMeta[]>(() => (start.features || []).map(({ steps: _s, ...f }) => { void _s; return f; }));
  const [lists, setLists] = useState(init.lists);
  const [sec, setSec] = useState<string | null>(init.sec);
  const [steps, setSteps] = useState<Held[]>(() => init.lists[init.sec ?? '']);
  const stepsNow = useRef(steps);
  stepsNow.current = steps;
  const secKey = sec ?? '';
  const listOf = (key: string | null) => (key ?? '') === secKey ? steps : lists[key ?? ''] || [];
  const allHeld = () => ['', ...feats.map(f => f.id)].flatMap(listOf);
  /** Each step, everywhere, changed the same way (an option handed back, a counter renamed). */
  const mapAll = (fn: (s: Held) => Held) => {
    setSteps(list => list.map(fn));
    setLists(ls => Object.fromEntries(Object.entries(ls).map(([k, list]) => [k, list.map(fn)])));
  };
  const [cursor, setCursor] = useState(-1);
  const [tried, setTried] = useState(false);
  const [pending, setPending] = useState<{ steps: Step[]; w: number; h: number } | null>(null);
  const [capture, setCapture] = useState<number | null>(null); // the step whose "Press a key" box is listening
  const [openKey, setOpenKey] = useState<number | null>(() => openStep != null && steps[openStep] ? steps[openStep]._k : null);
  const [note, setNote] = useState('');
  const [whenOpen, setWhenOpen] = useState(false);
  const [folded, setFolded] = useState<Set<number>>(() => new Set());
  const [noteFor, setNoteFor] = useState<number | null>(null); // the step whose note is being written
  // The small things that open under a button: Add step's list, a block's "delete it all?", New counter.
  const [adding, setAdding] = useState<{ anchor: HTMLElement; at: number } | null>(null);
  const [asking, setAsking] = useState<{ anchor: HTMLElement; i: number } | null>(null);
  const [naming, setNaming] = useState<{ anchor: HTMLElement; done: (name: string) => void } | null>(null);
  // What can't be taken away while steps still use it: an area, or a switch or list an If or a text step reads.
  const [inUse, setInUse] = useState<{ what: string; label: string; steps: At[] } | null>(null);
  // Where Try it (or the run the list's Fix came from) stopped, on that step's row.
  const [stopped, setStopped] = useState<{ k: number; reason: string } | null>(() =>
    openStep != null && steps[openStep] && stoppedReason ? { k: steps[openStep]._k, reason: stoppedReason } : null);
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
  const rowEl = (i: number) => form.current?.querySelector<HTMLElement>(`.mc-step[data-i="${i}"]`) || null;

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
  const shownOpts = opts.filter(keeps(used(allHeld())));
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
      patch({ options: [...opts.filter(keeps(used(allHeld()))), o] });
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
  // The panel's own fields changed one at a time, so a change to one (its game, showing by itself) keeps the others (its tabs).
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
  /** Where each step is that does something: in the main steps, then each feature's. */
  const findAll = (yes: (s: Held) => boolean): At[] => [null, ...feats.map(f => f.id)].flatMap(sc => listOf(sc).flatMap((s, i) => yes(s) ? [{ sec: sc, i }] : []));
  /** The steps that read a switch, a choice or a list made on the panel. */
  const readers = (name: string) => findAll(s => {
    const t2 = s.test as Test | undefined;
    return (t2?.kind === 'var' && t2.name === name) || s.words === '$' + name || (t2?.kind === 'text' && t2.words === '$' + name);
  });

  /* ── counters (Set and Add steps' names; what the panel calls each) ── */
  const counterList = counters({ steps: allHeld() });
  const labelOf = (name: string) => counterName(m, name);
  const setCounterLabel = (name: string, label: string) => setPanel({ counterLabels: { ...(m.panel?.counterLabels || {}), [name]: label } });

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
  // An area a step still looks in stays: said under it, with links to those steps.
  const delArea = (name: string) => {
    const by = findAll(s => s.region === '@' + name || s.test?.region === '@' + name);
    if (by.length) return setInUse({ what: 'area:' + name, label: areas.find(a => a.name === name)?.label || '', steps: by });
    setInUse(null);
    patch({ areas: areas.filter(a => a.name !== name) });
  };

  // A template's "to do": the fields it marked on its steps, each done once changed from what the template had (an
  // area once its box was moved on the game).
  const sig = (st: Step, field: string, mm: Macro) => field === 'region'
    ? JSON.stringify((mm.areas || []).find(a => '@' + a.name === st.region)?.rect ?? st.region ?? null)
    : JSON.stringify(isOpt(st[field]) ? (mm.options || []).find(o => '$' + o.name === st[field])?.value ?? null : st[field] ?? null);
  const todos = useRef(init.lists[''].flatMap(st => ((st.todo as string[] | undefined) || []).map(field => ({ k: st._k, field, was: sig(st, field, m) }))));
  const todoDone = (x: { k: number; field: string; was: string }) => {
    const st = allHeld().find(y => y._k === x.k);
    return !st || sig(st, x.field, m) !== x.was;
  };
  /** A step as saved: its note trimmed, the template's to-dos still not done. */
  const clean = (st: Held): Step => {
    const { todo: _t, note: n0, ...rest } = strip(st);
    void _t;
    const n1 = String(n0 || '').trim().slice(0, MAX_NOTE);
    const still = todos.current.filter(x => x.k === st._k && !todoDone(x)).map(x => x.field);
    return { ...rest, ...(n1 ? { note: n1 } : {}), ...(still.length ? { todo: still } : {}) };
  };
  // The features in their switches' order on the panel: the order they play in.
  const featsInOrder = [...feats].sort((a, b) => opts.findIndex(o => o.name === a.option) - opts.findIndex(o => o.name === b.option));
  const full = (): Macro => {
    const every = allHeld();
    const refs = used(every);
    const names = new Set(counters({ steps: every }));
    // Labels of counters no step keeps any more go; the panel keeps only its own.
    const labels = Object.fromEntries(Object.entries(m.panel?.counterLabels || {}).filter(([k, v]) => names.has(k) && v.trim()));
    const { counterLabels: _l, ...panel } = m.panel || { autoShow: false, placeId: null };
    void _l;
    return { ...m, name: m.name.trim(), options: opts.filter(keeps(refs)),
      panel: Object.keys(labels).length ? { ...panel, counterLabels: labels } : panel,
      steps: listOf(null).map(clean),
      features: featsInOrder.map(f => ({ ...f, steps: listOf(f.id).map(clean) })) };
  };
  /** The macro with the steps shown as its own (Edit on game and the picture draw these; a feature's Try plays them). */
  const shownMacro = (): Macro => sec ? { ...full(), steps: steps.map(clean), features: [] } : full();
  const snapshot = () => JSON.stringify(full());
  const initial = useRef('');
  useEffect(() => { initial.current = snapshot(); }, []); // once, as it opened
  const whole = full();
  const all = [...problems(t, whole, macros), ...featureProblems(t, whole)];
  const errors = all.filter(p => !p.warn);
  const latest = useRef({ snapshot, collect: () => ({}) as ReturnType<MacroEditorApi['collect']>, shownMacro });
  latest.current.snapshot = snapshot;
  latest.current.shownMacro = shownMacro;
  latest.current.collect = () => {
    setTried(true);
    const first = errors[0];
    if (!first) return { macro: full() };
    return { error: first.text, box: show(first) };
  };
  useImperativeHandle(api, () => ({ collect: () => latest.current.collect(), dirty: () => latest.current.snapshot() !== initial.current }), []);

  /** The place a problem is about, brought into sight: its step open (its block unfolded), the When line or the panel's
   * tab opened; the box to focus. */
  const show = (p: Problem): HTMLElement | null => {
    // A feature's problem: its steps first.
    if ((p.sec ?? null) !== sec && (p.step !== null || p.sec)) switchTo(p.sec ?? null);
    if (p.sec && p.step === null) return form.current?.querySelector<HTMLElement>(p.field === 'every' ? '#mc-feat-every' : '#mc-feat-name') || null;
    if (p.step !== null) {
      unfoldTo(p.step);
      const st = stepsNow.current[p.step];
      if (st) flushSync(() => setOpenKey(st._k));
      const row = rowEl(p.step);
      return row?.querySelector<HTMLElement>('[aria-invalid="true"], .mc-sent :is(input, select, button)') || row;
    }
    if (p.field === 'name') return nameBox.current;
    if (p.field === 'accounts' || p.field === 'every') {
      flushSync(() => setWhenOpen(true));
      return form.current?.querySelector<HTMLElement>(p.field === 'every' ? '#mc-every' : '#mc-accs input') || null;
    }
    if (p.field === 'options') {
      // A setting with no name may be on another tab: that tab first.
      const o = opts.find(x => !x.label.trim());
      if (o && tabs.length) flushSync(() => setTabSel(o.tab && tabs.includes(o.tab) ? o.tab : tabs[0]));
      return [...(form.current?.querySelectorAll<HTMLInputElement>('.mp-label') || [])].find(x => !x.value.trim()) || null;
    }
    if (p.field?.startsWith('area:')) return form.current?.querySelector<HTMLElement>(`#mc-area-${p.field.slice(5)}`) || null;
    return form.current?.querySelector<HTMLElement>('#mc-add') || null;
  };
  const goTo = (p: Problem) => {
    setTried(true);
    const box = show(p);
    box?.scrollIntoView({ block: 'center' });
    box?.focus({ preventScroll: true });
  };

  // A recording this form started: its steps come in here (added to the end or in place of the steps there, as chosen).
  useEffect(() => on('macroRecorded', (d?: { steps: Step[]; w: number; h: number }) => {
    if (recording.for !== 'editor') return;
    recording.for = null;
    if (!d?.steps?.length) return toast(t('macro.nothingRecorded'), 'info');
    if (stepsNow.current.length) return setPending(d);
    setSteps(d.steps.map(hold));
    patch({ recordedSize: { w: d.w, h: d.h } });
  }), [on]);
  const rec = app.macroRecording || null;

  /* ── features (a switch on the panel with steps of its own) ── */
  /** Shows another list of steps (null: the main ones). Rows are keyed the same everywhere, so what is open or folded stays. */
  const switchTo = (to: string | null) => {
    if (to === sec) return;
    const cur = steps;
    flushSync(() => {
      setLists(ls => ({ ...ls, [secKey]: cur }));
      setSteps(lists[to ?? ''] || []);
      setSec(to);
      setCursor(-1); setCutFor(null); setAdding(null); setAsking(null); setInUse(null); setCapture(null); setDragging(null); setDropping(null);
    });
  };
  /** A step somewhere in the macro, brought into sight and open. */
  const goStep = (at: At) => { switchTo(at.sec); focusRow(at.i); };
  const featOf = (id: string | null) => feats.find(f => f.id === id);
  const featLabel = (f?: FeatMeta) => opts.find(o => o.name === f?.option)?.label.trim() || t('macro.feat.unnamed');
  const setFeat = (id: string, p: Partial<FeatMeta>) => setFeats(list => list.map(f => f.id === id ? { ...f, ...p } : f));
  const addFeature = () => {
    if (feats.length >= MAX_FEATURES) return toast(t('macro.feat.full', { n: MAX_FEATURES }), 'warn');
    if (shownOpts.length >= MAX_OPTIONS) return toast(t('macro.opt.full', { n: MAX_OPTIONS }), 'warn');
    let n = feats.length + 1;
    while (opts.some(o => o.label === t('macro.feat.n', { n }))) n++;
    // Its switch starts on, so Try plays it at once; the player turns it off on the panel.
    const o: Option = { name: freeName('o', opts), label: t('macro.feat.n', { n }), kind: 'check', value: 1, default: 1, ...onTab() };
    const id = freeName('f', feats.map(f => ({ name: f.id })));
    patch({ options: [...shownOpts, o] });
    setFeats(list => [...list, { id, option: o.name, everyMs: null, onFail: 'skip' }]);
    setLists(ls => ({ ...ls, [id]: [] }));
    switchTo(id);
    setTimeout(() => { const box = form.current?.querySelector<HTMLInputElement>('#mc-feat-name'); box?.focus(); box?.select(); });
  };
  // Deleting a feature with steps: asked first, under the button.
  const [dropping, setDropping] = useState<{ anchor: HTMLElement; id: string } | null>(null);
  const dropFeature = (id: string, anchor?: HTMLElement, sure = false) => {
    const f = featOf(id);
    if (!f) return;
    if (!sure && listOf(id).length && anchor) return setDropping({ anchor, id });
    setDropping(null);
    if (sec === id) switchTo(null);
    setFeats(list => list.filter(x => x.id !== id));
    setLists(ls => { const { [id]: _gone, ...rest } = ls; void _gone; return rest; });
    // Its switch goes too, unless a step still reads it (then it stays a plain switch).
    if (!readers(f.option).length) setM(x => ({ ...x, options: (x.options || []).filter(o => o.name !== f.option) }));
  };
  /** A step (an If or a repeat with its whole block) taken to the end of another list. */
  const moveToList = (i: number, to: string | null) => {
    const [a, b] = span(steps, i);
    const moved = steps.slice(a, b + 1);
    flushSync(() => {
      setSteps(list => list.filter((_, k) => k < a || k > b));
      setLists(ls => ({ ...ls, [to ?? '']: [...(ls[to ?? ''] || []), ...moved] }));
      setCursor(-1);
    });
    toast(t('macro.feat.moved', { n: moved.length, name: to ? featLabel(featOf(to)) : t('macro.feat.main') }), 'info');
  };
  const curFeat = featOf(sec);

  /* ── steps ── */
  const folds = shapes(steps, folded);
  /** Unfolds every block around step i, so its row is drawn. */
  const unfoldTo = (i: number) => {
    const list = stepsNow.current;
    const around = shapes(list, new Set()).at(i)?.around || [];
    const keys = around.map(a => list[a]._k);
    if (keys.some(k => folded.has(k))) flushSync(() => setFolded(f => new Set([...f].filter(k => !keys.includes(k)))));
  };
  const focusRow = async (i: number, open = true) => {
    unfoldTo(i);
    const st = stepsNow.current[i];
    if (open && st) setOpenKey(st._k);
    await new Promise(r => setTimeout(r));
    const row = rowEl(i);
    const el = row?.querySelector<HTMLElement>('.mc-sent :is(input, select, button)') || row?.querySelector<HTMLElement>('.mc-step-head');
    if (el) el.focus({ preventScroll: true }); else if (row) { row.tabIndex = -1; row.focus({ preventScroll: true }); }
    row?.scrollIntoView({ block: 'nearest' });
  };
  /** The steps a step brings with it when moved, copied or deleted: an If or a repeat its whole block. */
  const span = (list: Step[], i: number): [number, number] => {
    const end = OPENERS.has(list[i]?.kind) ? blockEnd(list, i) : -1;
    return [i, end >= 0 ? end : i];
  };
  const remember = (kind: string) => {
    const was = host.store.get('macro.recentKinds', '').split(',').filter(Boolean);
    host.store.set('macro.recentKinds', [kind, ...was.filter(k => k !== kind)].slice(0, RECENT).join(','));
  };
  const add = (kind: string, where?: number) => {
    setAdding(null);
    remember(kind);
    // If stuck ends the macro's own steps: it goes last, and turns on starting again and the stuck watch.
    const at = kind === 'recover' ? steps.length : where ?? (cursor >= 0 && cursor < steps.length ? cursor + 1 : steps.length);
    if (kind === 'recover') patch({ onFail: 'restart', stuckMinutes: m.stuckMinutes || 5 });
    const added = stepsToAdd(kind).map(hold);
    flushSync(() => setSteps(list => [...list.slice(0, at), ...added, ...list.slice(at)]));
    setCursor(at);
    setOpenKey(['else', 'break', 'stop', 'rejoin', 'recover'].includes(kind) ? null : added[0]._k);
    focusRow(at, false); // opened above (this closure's steps are from before the insert)
  };
  const openAdd = (anchor: HTMLElement, at?: number) => {
    const where = at ?? (cursor >= 0 && cursor < steps.length ? cursor + 1 : steps.length);
    setAdding(a => a && a.anchor === anchor ? null : { anchor, at: where });
  };
  const moveStep = (i: number, dir: number, btn: HTMLElement) => {
    const [a, b] = span(steps, i);
    if (dir < 0 ? a === 0 : b >= steps.length - 1) return;
    const next = [...steps];
    // The step past the block's edge goes round it: the block moves one place.
    if (dir < 0) { const [x] = next.splice(a - 1, 1); next.splice(b, 0, x); }
    else { const [x] = next.splice(b + 1, 1); next.splice(a, 0, x); }
    const to = a + dir;
    flushSync(() => setSteps(next));
    setCursor(to);
    const same = rowEl(to)?.querySelector<HTMLElement>(`[data-move="${dir}"]:not(:disabled)`);
    (same || btn).focus();
    rowEl(to)?.scrollIntoView({ block: 'nearest' });
  };
  const dupStep = (i: number) => {
    const [a, b] = span(steps, i);
    flushSync(() => setSteps(list => [...list.slice(0, b + 1), ...list.slice(a, b + 1).map(s => hold(strip(s))), ...list.slice(b + 1)]));
    setCursor(b + 1);
    rowEl(b + 1)?.querySelector<HTMLElement>('[data-st="more"]')?.focus();
  };
  const delStep = (i: number, anchor?: HTMLElement, sure = false) => {
    const [a, b] = span(steps, i);
    // An If or a repeat with steps in it: asked first, under its menu.
    if (!sure && b > a && folds[i]?.inner && anchor) return setAsking({ anchor, i });
    setAsking(null);
    flushSync(() => setSteps(list => list.filter((_, k) => k < a || k > b)));
    if (cutFor !== null && cutFor >= a && cutFor <= b) setCutFor(null);
    const next = rowEl(Math.min(a, steps.length - (b - a) - 2));
    (next?.querySelector<HTMLElement>('[data-st="more"]') || form.current?.querySelector<HTMLElement>('#mc-add'))?.focus();
  };
  const toggleFold = (k: number) => setFolded(f => { const n = new Set(f); if (n.has(k)) n.delete(k); else n.add(k); return n; });
  const setNoteOf = (i: number, text: string) => setSteps(list => list.map((s, k) => {
    if (k !== i) return s;
    const { note: _n, ...rest } = s;
    void _n;
    return (text.trim() ? { ...rest, note: text.slice(0, MAX_NOTE) } : rest) as Held;
  }));
  // Dragging by the handle: a step, or an If or repeat with its whole block, shown in its new place as it goes.
  const [dragging, setDragging] = useState<{ from: number; len: number } | null>(null);
  const dragOver = (j: number) => {
    if (!dragging || (j >= dragging.from && j < dragging.from + dragging.len)) return;
    const next = [...steps];
    const moved = next.splice(dragging.from, dragging.len);
    const to = j < dragging.from ? j : j - dragging.len + 1;
    next.splice(to, 0, ...moved);
    setSteps(next);
    setDragging({ from: to, len: dragging.len });
  };

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
    if (!windowId) return toast(t('macro.need.game'), 'info');
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
    if (!windowId && !picture) return toast(t('macro.need.game'), 'info');
    setCutFor(i);
    if (!picture && windowId) take(form.current!.querySelector<HTMLElement>('#mc-pic-take')!);
    setTimeout(() => form.current?.querySelector<HTMLElement>('.mc-pic-frame')?.focus({ preventScroll: true }), 0);
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
  /** A step's own area (or its If's) dragged on the game. */
  const moveRegion = (i: number, test: boolean, r: Rect) => {
    const region = { x: +r.x.toFixed(4), y: +r.y.toFixed(4), w: +r.w.toFixed(4), h: +r.h.toFixed(4) };
    setSteps(list => list.map((s, k) => k !== i ? s : test ? { ...s, test: { ...s.test, region } } : { ...s, region }));
  };
  const dots = steps.flatMap((s, i) => { const p = pointOf(s, opts); return p ? [{ i, n: i + 1, ...p }] : []; });

  // Edit on game: the dots and boxes drawn over the window itself, dragged there; each change comes back here and the
  // overlay is drawn again from the form. Done (or Esc) on the game, or this button, ends it; so does closing the form.
  const [onGame, setOnGame] = useState(false);
  const draft = snapshot();
  useEffect(() => {
    if (!onGame || !windowId) return;
    const timer = setTimeout(() => call('editOnGame', { id: m.id, accountId: windowId, macro: latest.current.shownMacro() }, { silent: true }), 120);
    return () => clearTimeout(timer);
  }, [onGame, windowId, draft]);
  useEffect(() => {
    if (!onGame) return;
    return () => { call('editOnGame', { accountId: null }, { silent: true }); };
  }, [onGame, windowId]);
  useEffect(() => on('onGameEdit', (d?: { step?: number; x?: number; y?: number; area?: string; region?: number; test?: boolean; rect?: Rect; done?: boolean }) => {
    if (!d) return;
    if (d.done) return setOnGame(false);
    if (d.step != null && d.x != null && d.y != null) moveDot(d.step, d.x, d.y);
    if (d.area && d.rect) moveBox(d.area, d.rect);
    if (d.region != null && d.rect) moveRegion(d.region, !!d.test, d.rect);
  }), [on]);
  const editOnGame = () => { if (!windowId) return toast(t('macro.need.game'), 'info'); setOnGame(x => !x); };

  // Try it: the macro as it is in the form, once, on the window chosen (macroTryRun). Its run (trial) shows here, not in
  // the list: each step it gets past is a line ("found" for those that look for something), and how it ended; the step
  // running is marked on its row, and where it stopped. Meanwhile the app sends what it sees on that window (macroSeen:
  // the picture, what it found, a note).
  const [seen, setSeen] = useState<Seen | null>(null);
  const [lines, setLines] = useState<Seen[] | null>(null);
  // sec: the steps it plays (a feature's Try plays them alone); the lines say what happened to those.
  const [trial, setTrial] = useState<{ id: string; accountId: string; since: number; sec: string | null } | null>(null);
  const [linesSec, setLinesSec] = useState<string | null>(null);
  const trying = !!trial;
  const lastStep = useRef<number | null>(null);
  useEffect(() => on('macroSeen', (d?: Seen) => {
    if (!d || (d.accountId && d.accountId !== windowId)) return;
    if (d.image) setSeen(d);
  }), [on, windowId]);
  const run = trial ? ((app.macroRuns || []) as MacroRun[]).filter(r => r.trial && r.macroId === trial.id && r.accountId === trial.accountId && Date.parse(r.startedAt) >= trial.since).at(-1) : undefined;
  useEffect(() => {
    if (!trial || !run) return;
    const tried = listOf(trial.sec);
    const line = (d: Seen) => setLines(list => [...(list || []).filter(x => x.step == null || x.step !== d.step), d]);
    const passed = (k: number) => { if (tried[k] && !BLOCKS.has(tried[k].kind)) line({ w: 0, h: 0, step: k, outcome: LOOKS.has(tried[k].kind) ? 'found' : 'done' }); };
    // While a feature plays in the middle of the main steps, the lines wait where those were.
    const at = run.section ? lastStep.current : run.step ?? null;
    if (lastStep.current !== null && at !== lastStep.current && !isLive(run)) {
      // ended: what stopped it, then how
      if (run.status === 'failed' && (run.reason === 'missingValue' || run.reason === 'notFound' || run.reason === 'timeout'))
        line({ w: 0, h: 0, step: lastStep.current, outcome: run.reason === 'missingValue' ? 'missingValue' : 'notFound' });
      else if (run.status === 'done') passed(lastStep.current);
    } else if (lastStep.current !== null && at !== null && at !== lastStep.current) passed(lastStep.current);
    if (!isLive(run)) {
      // Where it stopped, on its row: the app says (lastStep), else the last step seen running.
      const where = run.section ? run.lastStep ?? null : run.lastStep ?? lastStep.current;
      const list = run.section ? listOf(run.section) : tried;
      if (run.status === 'failed' && where !== null && list[where]) {
        setStopped({ k: list[where]._k, reason: run.reason || '' });
        if ((run.section ?? trial.sec) === sec) {
          unfoldTo(where);
          setTimeout(() => rowEl(where)?.scrollIntoView({ block: 'nearest' }));
        }
      }
      line({ w: 0, h: 0, outcome: run.status === 'done' ? 'done' : 'stopped' });
      setTrial(null);
    }
    lastStep.current = at;
  }, [run?.step, run?.status, run?.section]);
  // The step playing, on its row when its list is the one shown (a feature's, while it plays in the main steps' run).
  const runningAt = trial && run && isLive(run) && run.step != null && (run.section ?? trial.sec) === sec ? run.step : null;
  useEffect(() => { if (runningAt !== null) unfoldTo(runningAt); }, [runningAt]);
  const tryIt = (btn: HTMLElement) => busy(btn, async () => {
    if (trial) { setTrial(null); return call('stopMacros', { id: trial.id, accountId: trial.accountId, trial: true }); }
    if (!windowId) return toast(t('macro.need.try'), 'info');
    // A feature's Try plays its steps alone: the main steps' problems don't stop it.
    const stops = sec ? errors.filter(p => p.sec === sec || (!p.sec && p.step === null && p.field)) : errors;
    if (stops.length) { setTried(true); goTo(stops[0]); return toast(t('macro.tryFix'), 'warn'); }
    setLines([]);
    setLinesSec(sec);
    setStopped(null);
    lastStep.current = null;
    const r = await call('macroTryRun', { macro: shownMacro(), accountId: windowId }) as { queued: number; macroId: string } | undefined;
    if (r?.macroId) setTrial({ id: r.macroId, accountId: windowId, since: Date.now() - 2000, sec });
  });

  const record = (btn: HTMLElement) => busy(btn, async () => {
    if (!windowId) return toast(t('macro.need.record'), 'info');
    recording.for = 'editor';
    if (await call('recordMacro', { accountId: windowId }) === undefined) recording.for = null;
  });

  // The list's Fix: the step that stopped, open and in sight.
  useEffect(() => {
    if (openStep == null || !steps[openStep]) return;
    unfoldTo(openStep);
    setTimeout(() => rowEl(openStep)?.scrollIntoView({ block: 'center' }), 60);
  }, []);

  // The dialog's foot: how many problems stop Save (a click goes to the first), and Save marked as not ready yet.
  const [foot, setFoot] = useState<HTMLElement | null>(null);
  useLayoutEffect(() => { setFoot(form.current?.closest('.modal')?.querySelector<HTMLElement>('.modal-foot') || null); }, []);
  useEffect(() => {
    const save = foot?.querySelector<HTMLElement>('.btn.primary');
    if (!save) return;
    save.setAttribute('aria-disabled', errors.length ? 'true' : 'false');
    if (errors.length) save.title = errors[0].text; else save.removeAttribute('title');
  });

  /* ── the form ── */
  const taken = new Map(macros.filter(o => o.id !== m.id && o.hotkey).map(o => [o.hotkey!, o.name]));
  const accTicked = new Set(m.accountIds);
  const shown = (p: { step: number | null }) => tried || p.step !== null;
  const inSec = (p: Problem) => (p.sec ?? null) === sec;
  const stepProblems = (i: number) => all.filter(p => p.step === i && inSec(p) && shown(p));
  const formProblems = (field: string) => all.filter(p => p.step === null && !p.sec && p.field === field && shown(p));
  const who = m.targets === 'accounts' ? tn('macro.tg.n', m.accountIds.length) : t(`macro.tg.${m.targets}`);
  const when = m.trigger.kind === 'interval' ? t('macro.trig.every', { d: durText(t, m.trigger.everySeconds || 60) }) : t(`macro.when.${m.trigger.kind}`);
  const ed: Ed = { val, put, set: setStep, ticked, tick, areas, newArea, pick, cut, canPick: !!windowId, capture, setCapture,
    counters: counterList, label: labelOf, readable: shownOpts.filter(o => o.kind === 'check' || o.kind === 'choice' || o.kind === 'number'),
    newCounter: (anchor, done) => setNaming(n => n?.anchor === anchor ? null : { anchor, done }) };
  const place = m.panel?.placeId ? `https://www.roblox.com/games/${m.panel.placeId}` : '';
  const [placeText, setPlaceText] = useState(place);
  // "Every": a number and its unit (seconds or minutes), kept in seconds.
  const every = m.trigger.everySeconds;
  const [unit, setUnit] = useState<'s' | 'min'>(() => every && every >= 60 && every % 60 === 0 ? 'min' : 's');
  const everyShown = typeof every === 'number' && Number.isFinite(every) ? +(unit === 'min' ? every / 60 : every).toFixed(2) : '';
  /** The fix a problem offers on its row, as a button. */
  const fixOf = (p: Problem): { label: string; run: () => void; off?: string } | null => {
    const i = p.step;
    switch (p.fix) {
      case 'addEnd': {
        if (i === null) return null;
        const kind = steps[i].kind;
        return { label: t(kind === 'if' ? 'macro.fix.addEndIf' : 'macro.fix.addEndLoop'), run: () => {
          // The block takes what follows it in its own level: it ends where the level does.
          const close = shapes(steps, new Set());
          let at = steps.length;
          for (let j = i + 1; j < steps.length; j++) if (close[j].depth < close[i].depth + 1 && close[j].opener !== i) { at = j; break; }
          const added = hold(newStep(endOf(kind)));
          flushSync(() => setSteps(list => [...list.slice(0, at), added, ...list.slice(at)]));
          focusRow(i, false);
        } };
      }
      case 'pick': return i === null ? null : { label: t('macro.fix.pick'), off: windowId ? undefined : t('macro.need.game'), run: () => pick(i, 'point') };
      case 'area': return i === null ? null : { label: t('macro.fix.area'), run: () => {
        const s = steps[i];
        const isTest = s.kind === 'if' || s.kind === 'loopUntil';
        if (!areas.length) {
          const r = newArea();
          if (r) setStep(i, isTest ? { test: { ...s.test, region: r } } : { region: r });
          return focusRow(i);
        }
        unfoldTo(i);
        flushSync(() => setOpenKey(s._k));
        const box = rowEl(i)?.querySelector<HTMLElement>('select.mc-where');
        box?.focus();
        box?.scrollIntoView({ block: 'nearest' });
      } };
      case 'name': return { label: t('macro.fix.name'), run: () => { const box = show(p); box?.focus(); box?.scrollIntoView({ block: 'nearest' }); } };
    }
    return null;
  };

  const todoList = todos.current;
  const nameErr = formProblems('name');
  const whenErr = [...formProblems('accounts'), ...formProblems('every')];
  const stepsErr = all.filter(p => p.step === null && !p.field && !p.sec && shown(p));
  const featErr = curFeat ? all.filter(p => p.sec === curFeat.id && p.step === null && shown(p)) : [];
  // The main steps' safe points (where the features on play), marked on the list; If stuck comes after the last.
  const safe = !sec && feats.length ? new Set(safeEnds(steps)) : new Set<number>();
  // The end's mark, unless the last of them is a repeat's end, marked already.
  const endAt = sec || !feats.length ? -2 : steps.findIndex(s => s.kind === 'recover');
  const endMarked = safe.has((endAt >= 0 ? endAt : steps.length) - 1);
  const safeRow = (key: string) => <div key={key} className="mc-safe" role="listitem"><Icon name="layers" /><span>{t('macro.feat.here')}</span></div>;
  const lists4Move = feats.length ? [{ key: null as string | null, label: t('macro.feat.main') }, ...featsInOrder.map(f => ({ key: f.id as string | null, label: featLabel(f) }))]
    .filter(x => x.key !== sec) : [];
  const optsErr = formProblems('options');
  return <div className="mc-form" ref={form}>
    {todoList.length ? <div className="mc-todo" role="group" aria-labelledby="mc-todo-h">
      <b id="mc-todo-h">{todoList.every(todoDone) ? t('macro.todo.allDone') : t('macro.todo.title')}</b>
      <ul>{todoList.map((x, k) => {
        const main = listOf(null);
        const i = main.findIndex(y => y._k === x.k);
        const done = todoDone(x);
        const area = x.field === 'region' ? areas.find(a => '@' + a.name === main[i]?.region)?.label : '';
        return <li key={k} className={done ? 'done' : ''}><Icon name={done ? 'check' : 'record'} />
          <span>{area ? t('macro.todo.area', { area }) : t(`macro.todo.${x.field}`)}{i >= 0 ? <small> · {t('macro.stepN', { n: i + 1 })}</small> : null}</span>
          {done || i < 0 ? null : x.field === 'region'
            ? <button type="button" className="btn sm" onClick={() => { if (!onGame) editOnGame(); }}><Icon name="screens" /><span>{t('macro.game.edit')}</span></button>
            : <button type="button" className="btn sm" onClick={() => goStep({ sec: null, i })}>{t('macro.todo.go')}</button>}</li>;
      })}</ul>
    </div> : null}
    <div className="mc-cols">
      <div className="mc-main">
        <div className="form mc-top">
          <div className="mc-name-row">
            <label>{t('macro.name')}<input className="field" id="mc-name" ref={nameBox} maxLength={60} placeholder={t('macro.namePh')} value={m.name}
              aria-invalid={nameErr.some(p => !p.warn) || undefined} aria-describedby={nameErr.length ? 'mc-name-err' : undefined}
              onChange={e => patch({ name: e.target.value })} /></label>
            <button type="button" className={`btn mc-try-it${trying ? ' on' : ''}`} id="mc-try" aria-pressed={trying} disabled={!windowId}
              title={windowId ? t('macro.tryItTip', { name: windowName }) : t('macro.need.try')} onClick={e => tryIt(e.currentTarget)}>
              <Icon name={trying ? 'stop' : 'play'} /><span>{t(trying ? 'macro.tryStop' : sec ? 'macro.feat.try' : 'macro.tryIt')}</span></button>
          </div>
          {nameErr.length ? <FieldErr id="mc-name-err" list={nameErr} /> : null}
          <details className="mc-when" open={whenOpen} onToggle={e => setWhenOpen((e.target as HTMLDetailsElement).open)}>
            <summary id="mc-when"><span>{[when, who, m.hotkey ? `Ctrl+Alt+${m.hotkey}` : t('macro.hk.noneLine')].join(' · ')}</span>
              {whenErr.some(p => !p.warn) && !whenOpen ? <i className="mc-dot-bad" aria-hidden="true" /> : null}<Icon name="chevronDown" /></summary>
            <div className="mc-when-body">
              <div className="mc-grid">
                <label>{t('macro.trigger')}<select className="select" id="mc-trigger" value={m.trigger.kind} onChange={e => {
                  const kind = e.target.value as Macro['trigger']['kind'];
                  patch({ trigger: kind === 'interval' ? { kind, everySeconds: m.trigger.everySeconds || 300 } : { kind } });
                  if (kind === 'interval' && !m.trigger.everySeconds) setUnit('min');
                }}>{['manual', 'interval', 'join', 'rejoin'].map(k => <option key={k} value={k}>{t(`macro.trigOpt.${k}`)}</option>)}</select></label>
                {m.trigger.kind === 'interval' ? <label>{t('macro.everyMin')}<span className="mc-every">
                  <input className="field num" id="mc-every" type="number" min={unit === 'min' ? 1 : 10} max={unit === 'min' ? 1440 : 86400}
                    aria-invalid={whenErr.some(p => p.field === 'every') || undefined}
                    value={everyShown} onChange={e => patch({ trigger: { kind: 'interval', everySeconds: e.target.value === '' ? NaN : Math.round(+e.target.value * (unit === 'min' ? 60 : 1)) } })} />
                  <select className="select" id="mc-every-unit" aria-label={t('macro.every.unit')} value={unit} onChange={e => {
                    const u = e.target.value as 's' | 'min';
                    setUnit(u);
                    if (typeof everyShown === 'number') patch({ trigger: { kind: 'interval', everySeconds: Math.round(everyShown * (u === 'min' ? 60 : 1)) } });
                  }}><option value="s">{t('macro.every.s')}</option><option value="min">{t('macro.every.min')}</option></select></span></label> : <span />}
                <label>{t('macro.targets')}<select className="select" id="mc-targets" value={m.targets} onChange={e => patch({ targets: e.target.value as Macro['targets'] })}>
                  {['selected', 'all', 'accounts'].map(k => <option key={k} value={k}>{t(`macro.tg.${k}`)}</option>)}</select></label>
                <label>{t('macro.hotkey')}<select className="select" id="mc-hotkey" value={m.hotkey ? String(m.hotkey) : ''} onChange={e => patch({ hotkey: e.target.value ? +e.target.value : null })}>
                  <option value="">{t('macro.hk.none')}</option>
                  {[1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => <option key={n} value={String(n)} disabled={taken.has(n)}>
                    {taken.has(n) ? t('macro.hk.taken', { k: `Ctrl+Alt+${n}`, name: taken.get(n)! }) : `Ctrl+Alt+${n}`}</option>)}</select></label>
              </div>
              {whenErr.length ? <FieldErr list={whenErr} /> : null}
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
                    {t(k === 'restart' && listOf(null).some(s => s.kind === 'recover') ? 'macro.fail.recover' : `macro.fail.${k}`)}</option>)}</select></label>
                {allHeld().some(s => LOOK_STEPS.includes(s.kind)) ? <label>{t('macro.stuckL')}<select className="select" id="mc-stuck" value={String(m.stuckMinutes || 0)}
                  onChange={e => patch({ stuckMinutes: +e.target.value || null })}>{[...new Set([0, 2, 5, 10, 30, m.stuckMinutes || 0])].sort((x, y) => x - y)
                    .map(n => <option key={n} value={String(n)}>{n ? t('macro.stuck.after', { n }) : t('macro.stuck.off')}</option>)}</select></label> : <span />}
              </div>
              <div className="mc-grid mc-when-nums">
                <label>{t('macro.e.stopAfterL')}<NumField id="mc-max" value={m.maxMinutes} min={1} max={240} onChange={v => patch({ maxMinutes: v })} /></label>
                <label>{t('macro.e.randomMs')}<span className="mc-unit wide"><input className="field num" id="mc-jms" type="number" min={0} max={0.5} step="0.01"
                  value={Number.isFinite(m.jitterMs) ? +(m.jitterMs / 1000).toFixed(3) : ''}
                  onChange={e => patch({ jitterMs: e.target.value === '' ? NaN : Math.round(+e.target.value * 1000) })} /><span aria-hidden="true">{t('macro.unit.s')}</span></span></label>
                <label>{t('macro.e.randomPx')}<span className="mc-unit wide"><NumField id="mc-jpx" value={m.jitterPx} min={0} max={20} onChange={v => patch({ jitterPx: v })} />
                  <span aria-hidden="true">px</span></span></label>
              </div>
            </div>
          </details>
        </div>

        <section className="mc-steps-sec" aria-labelledby="mc-steps-h">
          <div className="mc-steps-head">
            <b id="mc-steps-h">{t('macro.steps')} <span className="muted">({steps.length})</span></b>
            <button type="button" className="btn sm" id="mc-record" disabled={!windowId || !!rec} title={windowId ? t('macro.rec.tip') : t('macro.need.record')}
              onClick={e => record(e.currentTarget)}><Icon name="record" /><span>{t('macro.recordBtn')}</span></button>
            <button type="button" className="btn sm" id="mc-add" aria-haspopup="dialog" aria-expanded={adding?.anchor.id === 'mc-add'}
              onClick={e => openAdd(e.currentTarget)}><Icon name="plus" /><span>{t('macro.addStep')}</span></button>
            {feats.length ? null : <button type="button" className="btn sm ghost" id="mc-feat-add" title={t('macro.feat.addTip')} onClick={addFeature}>
              <Icon name="layers" /><span>{t('macro.feat.add')}</span></button>}
          </div>
          {feats.length ? <FeatTabs feats={featsInOrder} sec={sec} label={featLabel} on={f => !!opts.find(o => o.name === f.option)?.value}
            bad={key => errors.some(p => (p.sec ?? null) === key)} count={key => listOf(key).length} go={switchTo}
            add={feats.length < MAX_FEATURES ? addFeature : null} /> : null}
          {curFeat ? <FeatHead f={curFeat} o={opts.find(o => o.name === curFeat.option)} errs={featErr} set={p => setFeat(curFeat.id, p)}
            setOpt={p => { const o = opts.find(x => x.name === curFeat.option); if (o) setOpt(o.name, p); }}
            drop={btn => dropFeature(curFeat.id, btn)} /> : null}
          {rec ? <EditorRecording startsAt={rec.startsAt} count={rec.steps} /> : null}
          {pending ? <div className="mc-pending" role="group" aria-label={t('macro.rec.done', { n: pending.steps.length })}>
            <span>{tn('macro.rec.done', pending.steps.length)}</span>
            <button type="button" className="btn sm primary" onClick={() => { setSteps(list => [...list, ...pending.steps.map(hold)]); setPending(null); }}>{t('macro.rec.append')}</button>
            <button type="button" className="btn sm" onClick={() => { setSteps(pending.steps.map(hold)); patch({ recordedSize: { w: pending.w, h: pending.h } }); setPending(null); }}>{t('macro.rec.replace')}</button>
            <button type="button" className="btn sm ghost" onClick={() => setPending(null)}>{t('macro.rec.discard')}</button>
          </div> : null}
          {!steps.length ? <p className="mc-steps-none">{t(sec ? 'macro.feat.noSteps' : 'macro.noSteps')}</p> : null}
          {stepsErr.length ? <FieldErr list={stepsErr} /> : null}
          <div className="mc-steps" role="list" aria-labelledby="mc-steps-h"
            onDragOver={e => {
              if (!dragging) return;
              e.preventDefault();
              e.dataTransfer.dropEffect = 'move';
              const over = (e.target as HTMLElement).closest<HTMLElement>('.mc-step');
              if (over?.dataset.i) dragOver(+over.dataset.i);
            }}
            onDrop={e => e.preventDefault()} onDragEnd={() => setDragging(null)}>
            {steps.map((s, i) => {
              const sh = folds[i];
              if (sh.hidden) return null;
              const inside = sh.end >= 0 && folded.has(s._k) ? all.filter(p => !p.warn && inSec(p) && p.step !== null && p.step > i && p.step <= sh.end).length : 0;
              const mark = (safe.has(i) || (sh.end >= 0 && folded.has(s._k) && safe.has(sh.end))) ? safeRow('safe-' + s._k) : null;
              return <Fragment key={s._k}>{i === endAt && !endMarked ? safeRow('safe-end') : null}<StepRow s={s} i={i} m={m} count={steps.length} shape={sh} problems={stepProblems(i)} fixOf={fixOf}
                dragging={!!dragging && i >= dragging.from && i < dragging.from + dragging.len}
                open={openKey === s._k} toggle={() => { setOpenKey(openKey === s._k ? null : s._k); setCursor(i); }} ed={ed}
                folded={folded.has(s._k)} fold={() => toggleFold(s._k)} hiddenBad={inside}
                running={runningAt === i} stopped={stopped?.k === s._k ? stopped.reason : null}
                noting={noteFor === s._k} setNoting={on => setNoteFor(on ? s._k : null)} setNote={text => setNoteOf(i, text)}
                focus={() => setCursor(i)} move={(dir, btn) => moveStep(i, dir, btn)} dup={() => dupStep(i)} del={btn => delStep(i, btn)} cutting={cutFor === i}
                insert={(btn, at) => openAdd(btn, at)} dragStart={() => { const [a, b] = span(steps, i); setDragging({ from: a, len: b - a + 1 }); }}
                moveTargets={s.kind === 'else' || s.kind === 'recover' ? [] : lists4Move} moveOut={to => moveToList(i, to)} />{mark}</Fragment>;
            })}
            {endAt === -1 && steps.length && !endMarked ? safeRow('safe-end') : null}
          </div>
        </section>

        <PlayerSees m={m} options={shownOpts} counts={counterList.map(labelOf)} areas={areas.length}
          features={Object.fromEntries(feats.map(f => [f.option, { id: f.id, steps: listOf(f.id).length }]))} openFeature={id => {
            switchTo(id);
            form.current?.querySelector('.mc-steps-sec')?.scrollIntoView({ block: 'start' });
          }} addFeature={feats.length < MAX_FEATURES ? addFeature : null}
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
          remove={(name, btn) => {
            const o = opts.find(x => x.name === name);
            if (!o) return;
            // A feature's switch: the feature goes with it (asked first when it has steps).
            const f = feats.find(x => x.option === name);
            if (f) return dropFeature(f.id, btn);
            if (FREE.has(o.kind)) {
              // A switch or list a step still reads stays: said here, with links to those steps.
              const by = readers(name);
              if (by.length) return setInUse({ what: 'opt:' + name, label: o.label, steps: by });
              setInUse(null);
              return patch({ options: shownOpts.filter(x => x.name !== name) });
            }
            // A ticked value: the steps get it back as their own.
            const back = (k: string, v: unknown) => v !== '$' + name ? v : o.kind === 'point' ? asPoint(o.value)?.[k as 'x' | 'y'] : o.value;
            mapAll(s => ({ ...Object.fromEntries(Object.entries(s).map(([k, v]) => [k, back(k, v)])),
              ...(s.test ? { test: Object.fromEntries(Object.entries(s.test).map(([k, v]) => [k, back(k, v)])) } : {}) } as Held));
          }}>
          {optsErr.length ? <FieldErr list={optsErr} /> : null}
          {inUse?.what.startsWith('opt:') ? <InUse label={inUse.label} steps={inUse.steps} name={id => featLabel(featOf(id))} go={goStep} close={() => setInUse(null)} /> : null}
          <label className="mc-switch-row">
            <span className="switch"><input type="checkbox" id="mc-panel-auto" checked={!!m.panel?.autoShow}
              onChange={e => setPanel({ autoShow: e.target.checked })} /><span /></span>
            <span><b>{t('macro.panel.auto')}</b><small>{t('macro.panel.autoDesc')}</small></span></label>
          {m.panel?.autoShow ? <div className="mc-panel-game"><GameBox place={placeText} setPlace={v => {
            setPlaceText(v);
            const id = /(?:games\/|^)(\d{3,})/.exec(v.trim())?.[1];
            setPanel({ placeId: id ? +id : null });
          }} chose={g => setPanel({ placeId: g.placeId })} /></div> : null}
        </PlayerSees>
      </div>

      <aside className="mc-side">
        <section className="mc-game" aria-labelledby="mc-game-h">
          <div className="mc-pic-head">
            <b id="mc-game-h">{t('macro.game.title')}</b>
            {live.length > 1 ? <select className="select sm" id="mc-pic-acc" value={windowId} aria-label={t('macro.pic.account')} onChange={e => setAccountId(e.target.value)}>
              {live.map(a => <option key={a.id} value={a.id}>{a.label}</option>)}</select> : null}
            <button type="button" className={`btn sm${onGame ? ' on' : ''}`} id="mc-ongame" aria-pressed={onGame} disabled={!windowId}
              title={windowId ? undefined : t('macro.need.game')} onClick={editOnGame}>
              <Icon name={onGame ? 'check' : 'screens'} /><span>{t(onGame ? 'macro.game.done' : 'macro.game.edit')}</span></button>
          </div>
          <p className={`mc-game-say${onGame ? ' on' : ''}`} role="status">{!windowId ? t('macro.game.noWindow')
            : onGame ? t('macro.game.editing', { name: windowName }) : t('macro.game.how', { name: windowName })}</p>
          <GameFrame picture={seen?.image ? { image: seen.image.startsWith('data:') ? seen.image : `data:image/png;base64,${seen.image}`, w: seen.w, h: seen.h } : picture}
            size={m.recordedSize} dots={dots} boxes={areas} found={seen?.boxes || []} cutting={cutFor !== null}
            cancelCut={() => setCutFor(null)} onDot={moveDot} onBox={moveBox} onCut={cutDone} goAccounts={!windowId ? goAccounts : undefined} />
          <div className="mc-pic-acts">
            <button type="button" className="btn sm" id="mc-pic-take" disabled={!windowId || taking} title={windowId ? undefined : t('macro.need.game')}
              onClick={e => take(e.currentTarget)}><Icon name="image" /><span>{t(picture ? 'macro.pic.again' : 'macro.pic.take')}</span></button>
            <small>{picture ? t('macro.pic.size', { w: picture.w, h: picture.h }) : t('macro.pic.why')}</small>
          </div>
          {note ? <p className="mc-note" role="status"><Icon name="info" /><span>{note}</span></p> : null}
          <AreaList areas={areas} set={setArea} del={delArea} add={() => newArea()} problems={all.filter(p => p.field?.startsWith('area:'))}
            inUse={inUse?.what.startsWith('area:') ? inUse : null} name={id => featLabel(featOf(id))} go={goStep} closeInUse={() => setInUse(null)} />
        </section>
        {lines ? <TryLines lines={lines} trying={trying} steps={listOf(linesSec)} m={m} name={windowName} note={seen?.note} /> : null}
      </aside>
    </div>
    {adding ? <AddList anchor={adding.anchor} at={adding.at} steps={steps} inFeature={!!sec} add={add} close={() => setAdding(null)} /> : null}
    {dropping ? <Pop anchor={dropping.anchor} close={() => setDropping(null)} label={t('macro.feat.del')} className="mc-ask">
      <p>{tn('macro.feat.delAsk', listOf(dropping.id).length, { name: featLabel(featOf(dropping.id)) })}</p>
      <div className="mc-pop-acts">
        <button type="button" className="btn sm ghost" onClick={() => setDropping(null)}>{t('common.cancel')}</button>
        <button type="button" className="btn sm danger" autoFocus onClick={() => dropFeature(dropping.id, undefined, true)}>{t('macro.delete')}</button>
      </div>
    </Pop> : null}
    {asking ? <Pop anchor={asking.anchor} close={() => setAsking(null)} label={t('macro.delStep')} className="mc-ask">
      <p>{tn('macro.block.deleteAsk', folds[asking.i]?.inner || 0, { what: t(steps[asking.i]?.kind === 'if' ? 'macro.block.if' : 'macro.block.loop') })}</p>
      <div className="mc-pop-acts">
        <button type="button" className="btn sm ghost" onClick={() => setAsking(null)}>{t('common.cancel')}</button>
        <button type="button" className="btn sm danger" autoFocus onClick={() => delStep(asking.i, undefined, true)}>{t('macro.delete')}</button>
      </div>
    </Pop> : null}
    {naming ? <NewCounter anchor={naming.anchor} taken={counterList} close={() => setNaming(null)} done={(name, label) => {
      if (label.trim() && label.trim() !== name) setCounterLabel(name, label.trim());
      naming.done(name);
      setNaming(null);
    }} /> : null}
    {foot && errors.length ? createPortal(<button type="button" className="mc-probs-badge" onClick={() => goTo(errors[0])} title={errors[0].text}>
      <Icon name="alert" /><span>{tn('macro.problemsN', errors.length)}</span></button>, foot) : null}
  </div>;
}

/** Problems that belong to the form rather than a step (its name, when it runs, the panel), under their fields. */
function FieldErr({ list, id }: { list: Problem[]; id?: string }) {
  return <ul className="mc-ferr" id={id} role="status">{list.map((p, k) => <li key={k} className={p.warn ? 'warn' : ''}><Icon name="alert" /><span>{p.text}</span></li>)}</ul>;
}

/** The main steps and each feature's, as tabs over the list: a feature's switch on or off, a dot where something stops
 * a save. They play left to right (the order of their switches on the panel). */
function FeatTabs({ feats, sec, label, on, bad, count, go, add }: {
  feats: FeatMeta[]; sec: string | null; label: (f: FeatMeta) => string; on: (f: FeatMeta) => boolean;
  bad: (key: string | null) => boolean; count: (key: string | null) => number; go: (key: string | null) => void; add: (() => void) | null;
}) {
  const { t } = useHost();
  const tab = (key: string | null, name: string, lit: boolean | null) => <button key={key ?? ''} type="button" role="tab" aria-selected={sec === key}
    className={`mc-ftab${sec === key ? ' on' : ''}`} onClick={() => go(key)}>
    {lit === null ? null : <i className={`mc-ftab-sw${lit ? ' lit' : ''}`} title={t(lit ? 'macro.feat.isOn' : 'macro.feat.isOff')} aria-hidden="true" />}
    <span>{name}</span><small>{count(key)}</small>{bad(key) ? <i className="mc-dot-bad" aria-hidden="true" /> : null}</button>;
  return <div className="mc-ftabs-wrap">
    <div className="mc-ftabs" role="tablist" aria-label={t('macro.feat.tabs')}>
      {tab(null, t('macro.feat.main'), null)}
      {feats.map(f => tab(f.id, label(f), on(f)))}
      {add ? <button type="button" className="mc-ftab add" id="mc-feat-add" title={t('macro.feat.addTip')} onClick={add}><Icon name="plus" /><span>{t('macro.feat.add')}</span></button> : null}
    </div>
    {feats.length > 1 ? <small className="mc-ftabs-say">{t('macro.feat.order')}</small> : null}
  </div>;
}

/** The feature shown: its name (its switch's label on the panel), on or off, when it plays, what a failure does in it. */
function FeatHead({ f, o, errs, set, setOpt, drop }: {
  f: FeatMeta; o?: Option; errs: Problem[]; set: (p: Partial<FeatMeta>) => void; setOpt: (p: Partial<Option>) => void; drop: (btn: HTMLElement) => void;
}) {
  const { t } = useHost();
  const every = f.everyMs;
  const [unit, setUnit] = useState<'s' | 'min'>(() => every && every >= 60000 && every % 60000 === 0 ? 'min' : 's');
  const size = unit === 'min' ? 60000 : 1000;
  const shown = typeof every === 'number' && Number.isFinite(every) ? +(every / size).toFixed(2) : '';
  return <div className="mc-feat form" role="group" aria-label={o?.label || t('macro.feat.unnamed')}>
    <div className="mc-feat-row">
      <label className="mc-feat-name">{t('macro.feat.name')}<input className="field" id="mc-feat-name" maxLength={30} value={o?.label || ''}
        aria-invalid={!o?.label.trim() || undefined} onChange={e => setOpt({ label: e.target.value })} /></label>
      <label className="mc-switch-row mc-feat-on">
        <span className="switch"><input type="checkbox" checked={!!o?.value} onChange={e => setOpt({ value: e.target.checked ? 1 : 0, default: e.target.checked ? 1 : 0 })} /><span /></span>
        <span>{t(o?.value ? 'macro.feat.isOn' : 'macro.feat.isOff')}</span></label>
      <button type="button" className="icon-btn" title={t('macro.feat.del')} aria-label={`${t('macro.feat.del')}: ${o?.label || ''}`} onClick={e => drop(e.currentTarget)}><Icon name="trash" /></button>
    </div>
    <div className="mc-grid">
      <label>{t('macro.feat.when')}<span className="mc-every">
        <select className="select" id="mc-feat-when" value={every == null ? 'round' : 'every'}
          onChange={e => set({ everyMs: e.target.value === 'round' ? null : 300000 })}>
          <option value="round">{t('macro.feat.eachRound')}</option><option value="every">{t('macro.feat.everyOpt')}</option></select>
        {every == null ? null : <>
          <input className="field num" id="mc-feat-every" type="number" min={1} max={unit === 'min' ? 1440 : 86400}
            aria-label={t('macro.feat.everyL')} aria-invalid={errs.some(p => p.field === 'every') || undefined} value={shown}
            onChange={e => set({ everyMs: e.target.value === '' ? NaN : Math.round(+e.target.value * size) })} />
          <select className="select" aria-label={t('macro.every.unit')} value={unit} onChange={e => {
            const u = e.target.value as 's' | 'min';
            setUnit(u);
            if (typeof shown === 'number') set({ everyMs: Math.round(shown * (u === 'min' ? 60000 : 1000)) });
          }}><option value="s">{t('macro.every.s')}</option><option value="min">{t('macro.every.min')}</option></select></>}</span></label>
      <label>{t('macro.feat.onFail')}<select className="select" id="mc-feat-fail" value={f.onFail} onChange={e => set({ onFail: e.target.value as FeatMeta['onFail'] })}>
        <option value="skip">{t('macro.feat.fail.skip')}</option><option value="macro">{t('macro.feat.fail.macro')}</option></select></label>
    </div>
    <p className="mc-feat-say">{t(every == null ? 'macro.feat.sayRound' : 'macro.feat.sayEvery')}</p>
    {errs.length ? <FieldErr list={errs} /> : null}
  </div>;
}

/** "Used by steps 3 and 4": what can't go yet, each step a link to it (a feature's step with the feature's name). */
function InUse({ label, steps, name, go, close }: { label: string; steps: At[]; name: (id: string) => string; go: (at: At) => void; close: () => void }) {
  const { t, tn, locale } = useHost();
  const MARK = '\u0001';
  const nums = steps.map(x => x.sec ? `${name(x.sec)} ${x.i + 1}` : String(x.i + 1));
  const list = new Intl.ListFormat(locale, { style: 'long', type: 'conjunction' }).formatToParts(nums.map((_, k) => `${MARK}${k}${MARK}`))
    .map(p => p.value).join('');
  const text = tn('macro.ref.used', steps.length, { list, name: label });
  return <p className="mc-inuse" role="status"><Icon name="info" /><span>{text.split(MARK).map((b, k) => k % 2
    ? <button key={k} type="button" className="link" onClick={() => go(steps[+b])}>{nums[+b]}</button> : b ? <Fragment key={k}>{b}</Fragment> : null)}</span>
    <button type="button" className="icon-btn" title={t('common.close')} aria-label={t('common.close')} onClick={close}><Icon name="x" /></button></p>;
}

/** The macro's areas: each one's colour on the game, its name (what the steps say: "in Fish area"), and removing it. */
function AreaList({ areas, set, del, add, problems: probs, inUse, name, go, closeInUse }: {
  areas: Area[]; set: (name: string, p: Partial<Area>) => void; del: (name: string) => void; add: () => void; problems: Problem[];
  inUse: { what: string; label: string; steps: At[] } | null; name: (id: string) => string; go: (at: At) => void; closeInUse: () => void;
}) {
  const { t } = useHost();
  return <div className="mc-areas" role="group" aria-labelledby="mc-areas-h">
    <div className="mc-areas-head"><b id="mc-areas-h">{t('macro.area.title')}</b>
      <button type="button" className="btn sm" id="mc-area-add" onClick={add}><Icon name="plus" /><span>{t('macro.area.new')}</span></button></div>
    {!areas.length ? <p className="mc-pic-none">{t('macro.area.none')}</p> : <ul>{areas.map(a => {
      const bad = probs.find(p => p.field === `area:${a.name}`);
      return <li key={a.name}>
        <div className="mc-area-row">
          <i style={{ borderColor: a.colour }} aria-hidden="true" />
          <input className="field sm" id={`mc-area-${a.name}`} value={a.label} maxLength={30} aria-label={t('macro.area.name')} aria-invalid={!!bad || undefined}
            onChange={e => set(a.name, { label: e.target.value })} />
          <button type="button" className="icon-btn" title={t('macro.area.del')} aria-label={`${t('macro.area.del')}: ${a.label}`} onClick={() => del(a.name)}><Icon name="trash" /></button>
        </div>
        {bad ? <p className="mc-ferr-line"><Icon name="alert" /><span>{bad.text}</span></p> : null}
        {inUse?.what === 'area:' + a.name ? <InUse label={a.label} steps={inUse.steps} name={name} go={go} close={closeInUse} /> : null}
      </li>;
    })}</ul>}
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

/* ── small popovers under a button: Add step's list, a block's "delete it all?", New counter ── */

/** Add step: a search box, the kinds used lately, then every kind in its group; only those that fit where it goes
 * (Otherwise straight inside an If, Leave the repeat inside a repeat, one If stuck). Enter adds the first match. */
function AddList({ anchor, at, steps, inFeature, add, close }: {
  anchor: HTMLElement; at: number; steps: Step[]; inFeature: boolean; add: (kind: string, at: number) => void; close: () => void;
}) {
  const { t, store } = useHost();
  const [q, setQ] = useState('');
  const list = useRef<HTMLDivElement>(null);
  // If stuck belongs to the main steps only.
  const fits = new Set(kindsAt(steps, at).filter(k => !(inFeature && k === 'recover')));
  const name = (k: string) => t(`macro.kind.${k}`);
  const words = plain(q.trim()).split(/\s+/).filter(Boolean);
  const hay = (k: string, g: string) => plain([name(k), t(`macro.grp.${g}`), ...(SYNONYMS[k] || [])].join(' '));
  const matches = words.length ? STEP_GROUPS.flatMap(([g, kinds]) => kinds.filter(k => fits.has(k) && words.every(w => hay(k, g).includes(w))))
    .sort((a, b) => Number(!plain(name(a)).startsWith(words[0])) - Number(!plain(name(b)).startsWith(words[0]))) : [];
  const recent = store.get('macro.recentKinds', '').split(',').filter(k => fits.has(k));
  const item = (k: string, key = k) => <button key={key} type="button" role="menuitem" data-kind={k} onClick={() => add(k, at)}>
    <Icon name={STEP_ICON[k] || 'info'} /><span>{name(k)}</span></button>;
  // The arrows go through the list; from the box, Down goes in.
  const keys = (e: React.KeyboardEvent) => {
    if (e.key !== 'ArrowDown' && e.key !== 'ArrowUp') return;
    const items = [...(list.current?.querySelectorAll<HTMLElement>('button') || [])];
    if (!items.length) return;
    e.preventDefault();
    const i = items.indexOf(document.activeElement as HTMLElement);
    const next = e.key === 'ArrowDown' ? (i < 0 ? 0 : Math.min(items.length - 1, i + 1)) : i <= 0 ? -1 : i - 1;
    (next < 0 ? anchor.closest('.overlay')?.querySelector<HTMLElement>('.mc-add-q') : items[next])?.focus();
  };
  return <Pop anchor={anchor} close={close} label={t('macro.addStep')} className="menu mc-add-pop">
    <div onKeyDown={keys}>
      <div className="mc-add-search"><Icon name="search" />
        <input className="field sm mc-add-q" type="search" value={q} placeholder={t('macro.search')} aria-label={t('macro.search')} spellCheck={false}
          onChange={e => setQ(e.target.value)} onKeyDown={e => { if (e.key === 'Enter' && matches[0]) { e.preventDefault(); add(matches[0], at); } }} /></div>
      <div className="mc-add-list" ref={list} role="menu" aria-label={t('macro.addStep')}>
        {words.length ? (matches.length ? matches.map(k => item(k)) : <p className="mc-add-none">{t('macro.search.none')}</p>)
          : <>{recent.length ? <><div className="menu-head">{t('macro.recent')}</div>{recent.map(k => item(k, 'r-' + k))}<hr /></> : null}
            {STEP_GROUPS.map(([g, kinds], gi) => {
              const here = kinds.filter(k => fits.has(k));
              return here.length ? <Fragment key={g}>{gi ? <hr /> : null}<div className="menu-head">{t(`macro.grp.${g}`)}</div>{here.map(k => item(k))}</Fragment> : null;
            })}</>}
      </div>
    </div>
  </Pop>;
}

/** New counter…: its name (what steps and an If use) and what the panel calls it. */
function NewCounter({ anchor, taken, close, done }: { anchor: HTMLElement; taken: string[]; close: () => void; done: (name: string, label: string) => void }) {
  const { t } = useHost();
  const [name, setName] = useState('');
  const [label, setLabel] = useState('');
  const bad = name !== '' && !COUNTER_NAME.test(name);
  const dup = taken.includes(name);
  const ok = name !== '' && !bad && !dup;
  return <Pop anchor={anchor} close={close} label={t('macro.counter.new')} className="mc-counter-pop">
    <form className="form" onSubmit={e => { e.preventDefault(); if (ok) done(name, label); }}>
      <label>{t('macro.e.counterL')}<input className="field sm" value={name} maxLength={32} spellCheck={false} autoComplete="off"
        aria-invalid={bad || dup || undefined} aria-describedby="mc-counter-hint" onChange={e => setName(e.target.value.replace(/\s/g, '_'))} /></label>
      <small id="mc-counter-hint" className={bad || dup ? 'bad' : ''}>{dup ? t('macro.counter.taken') : t('macro.counter.nameHint')}</small>
      <label>{t('macro.counter.label')}<input className="field sm" value={label} maxLength={30} placeholder={name || undefined} onChange={e => setLabel(e.target.value)} /></label>
      <div className="mc-pop-acts">
        <button type="button" className="btn sm ghost" onClick={close}>{t('common.cancel')}</button>
        <button type="submit" className="btn sm primary" aria-disabled={!ok} title={ok ? undefined : t('macro.counter.nameHint')}>{t('macro.opt.add')}</button>
      </div>
    </form>
  </Pop>;
}

/* ── a step's row ── */

function StepRow({ s, i, m, count, shape, problems: probs, fixOf, dragging, ed, focus, move, dup, del, cutting, dragStart, open, toggle, folded, fold, hiddenBad,
  running, stopped, noting, setNoting, setNote, insert, moveTargets, moveOut }: {
  s: Held; i: number; m: Macro; count: number; shape: Shape; problems: Problem[]; fixOf: (p: Problem) => { label: string; run: () => void; off?: string } | null;
  dragging: boolean; open: boolean; toggle: () => void; folded: boolean; fold: () => void; hiddenBad: number; running: boolean; stopped: string | null;
  noting: boolean; setNoting: (on: boolean) => void; setNote: (text: string) => void; insert: (btn: HTMLElement, at: number) => void;
  ed: Ed; focus: () => void; move: (dir: number, btn: HTMLElement) => void; dup: () => void; del: (btn: HTMLElement) => void; cutting: boolean; dragStart: () => void;
  /** The other lists it can go to (the main steps, the features'), at their end. */
  moveTargets: { key: string | null; label: string }[]; moveOut: (to: string | null) => void;
}) {
  const host = useHost();
  const { t, tn } = host;
  const n = i + 1;
  const [armed, setArmed] = useState(false);
  const isEnd = s.kind === 'endIf' || s.kind === 'endLoop';
  const opener = OPENERS.has(s.kind) && shape.end >= 0;
  const body = open ? <StepBody s={s} i={i} m={m} ed={ed} cutting={cutting} invalid={new Set(probs.filter(p => !p.warn && p.field).map(p => p.field!))} /> : null;
  const plainRow = ['else', 'break', 'stop', 'rejoin', 'recover'].includes(s.kind);
  const bad = probs.some(p => !p.warn);
  const say = stepText(t, s, m, n);
  // Where the row sits: Otherwise one level out (a divider in its If), the rest at their depth. Each block around it draws its rail.
  const at = s.kind === 'else' && shape.depth ? shape.depth - 1 : shape.depth;
  const rails = shape.rails.map((k, L) => <span key={L} className="mc-rail" aria-hidden="true" style={{ left: `${8 + (L - at) * 22}px`, background: railOf(k) }} />);
  const ins = <button type="button" className="mc-ins" tabIndex={-1} aria-haspopup="dialog" title={t('macro.insHere')} aria-label={t('macro.insHere')}
    onClick={e => insert(e.currentTarget, i)}><span className="mc-ins-dot"><Icon name="plus" /></span></button>;
  const common = { 'data-i': i, style: { '--d': at } as React.CSSProperties };
  const ctrlEnter = (e: React.KeyboardEvent<HTMLElement>) => {
    if (e.key === 'Enter' && e.ctrlKey) { e.preventDefault(); insert(e.currentTarget.querySelector<HTMLElement>('.mc-step-head') || e.currentTarget, i + 1); }
  };

  // The end of a block: a slim line under it, not moved or deleted by itself (its opener takes it along).
  if (isEnd && shape.opener >= 0) return <div className={`mc-step mc-end${dragging ? ' dragging' : ''}`} role="listitem" {...common} onKeyDown={ctrlEnter}
    aria-label={`${t('macro.stepN', { n })}: ${t(s.kind === 'endIf' ? 'macro.block.endIf' : 'macro.block.endLoop')}`}>
    {rails}{ins}
    <span className="mc-step-n" aria-hidden="true">{n}</span>
    <span className="mc-step-head"><Icon name={STEP_ICON[s.kind]} /><span className="mc-say">{t(s.kind === 'endIf' ? 'macro.block.endIf' : 'macro.block.endLoop')}</span></span>
  </div>;

  const menu = (btn: HTMLElement) => host.menu(btn, [
    { icon: 'edit', text: t(s.note ? 'macro.note.edit' : 'macro.note.add'), run: () => setNoting(true) },
    { icon: 'copy', text: t('macro.dupStep'), run: dup },
    ...(moveTargets.length ? ['-' as const, { head: true, text: t('macro.feat.moveTo') },
      ...moveTargets.map(x => ({ icon: 'chevronRight', text: x.label, run: () => moveOut(x.key) }))] : []),
    '-', { icon: 'trash', text: t('macro.delStep'), danger: true, run: () => del(btn) },
  ], { icons: true });
  const chip = running ? <em className="mc-chip run">{t('macro.run.here')}</em> : null;
  const stopText = stopped !== null ? (stopped === 'notFound' || stopped === 'timeout' || !stopped ? t('macro.stop.here') : t('macro.stop.hereWhy', { why: t(`macro.why.${stopped}`) })) : '';
  return <div className={`mc-step${dragging ? ' dragging' : ''}${bad ? ' bad' : ''}${open ? ' open' : ''}${s.kind === 'else' ? ' mc-else' : ''}${opener ? ' mc-opener' : ''}${running ? ' running' : ''}${stopped !== null ? ' stopped' : ''}`}
    role="listitem" aria-label={`${t('macro.stepN', { n })}: ${say}`} {...common} draggable={armed} onFocus={focus} onKeyDown={ctrlEnter}
    onDragStart={e => { if (!armed) return; dragStart(); e.dataTransfer.effectAllowed = 'move'; e.dataTransfer.setData('text/plain', String(s._k)); }}
    onDragEnd={() => setArmed(false)} onPointerUp={() => setArmed(false)}>
    {rails}{ins}
    <span className="ord-grip" title={t('lo.drag')} onPointerDown={() => setArmed(true)}><Icon name="grip" /></span>
    <span className="mc-step-n" aria-hidden="true">{n}</span>
    {opener ? <button type="button" className="icon-btn mc-fold" aria-expanded={!folded} title={t(folded ? 'macro.block.unfold' : 'macro.block.fold')}
      aria-label={`${t(folded ? 'macro.block.unfold' : 'macro.block.fold')}: ${t('macro.stepN', { n })}`} onClick={fold}><Icon name="caretRight" /></button> : null}
    {plainRow ? <span className={`mc-step-head k-${s.kind}`}><Icon name={STEP_ICON[s.kind] || 'info'} /><span className="mc-say">{say}</span>{chip}</span>
      : <button type="button" className={`mc-step-head k-${s.kind}`} aria-expanded={open} onClick={toggle}>
        <Icon name={STEP_ICON[s.kind] || 'info'} /><span className="mc-say">{say}{folded ? <small> {tn('macro.block.steps', shape.inner)}</small> : null}</span>
        {hiddenBad ? <em className="mc-bad-count" title={tn('macro.block.probs', hiddenBad)}>{hiddenBad}</em> : null}{chip}
        <Icon name={open ? 'chevronUp' : 'chevronDown'} /></button>}
    <span className="mc-step-acts">
      <button type="button" className="icon-btn" data-move="-1" title={t('lo.up')} aria-label={`${t('lo.up')}: ${t('macro.stepN', { n })}`} disabled={i === 0} onClick={e => move(-1, e.currentTarget)}><Icon name="chevronUp" /></button>
      <button type="button" className="icon-btn" data-move="1" title={t('lo.down')} aria-label={`${t('lo.down')}: ${t('macro.stepN', { n })}`} disabled={(opener ? shape.end : i) >= count - 1} onClick={e => move(1, e.currentTarget)}><Icon name="chevronDown" /></button>
      <button type="button" className="icon-btn" data-st="more" aria-haspopup="menu" title={t('macro.moreActs')} aria-label={`${t('macro.moreActs')}: ${t('macro.stepN', { n })}`} onClick={e => menu(e.currentTarget)}><Icon name="more" /></button>
    </span>
    {noting ? <NoteBox value={s.note || ''} done={text => { setNote(text); setNoting(false); }} />
      : s.note ? <p className="mc-step-note"><button type="button" className="link" title={t('macro.note.edit')} onClick={() => setNoting(true)}>{s.note}</button></p> : null}
    {stopText ? <p className="mc-stop-line" role="status"><Icon name="alert" /><span>{stopText}</span></p> : null}
    {probs.length ? <ul className="mc-step-probs">{probs.map((p, k) => {
      const fix = p.warn ? null : fixOf(p);
      return <li key={k} className={p.warn ? 'warn' : ''}><Icon name="alert" /><span>{p.bare}</span>
        {fix ? <button type="button" className="btn sm" disabled={!!fix.off} title={fix.off} onClick={fix.run}>{fix.label}</button> : null}</li>;
    })}</ul> : null}
    {body ? <div className="mc-step-fields">{body}</div> : null}
  </div>;
}

/** A step's note being written: Enter or leaving the box keeps it, Escape leaves it as it was; empty takes it off. */
function NoteBox({ value, done }: { value: string; done: (text: string) => void }) {
  const { t } = useHost();
  const [text, setText] = useState(value);
  const box = useRef<HTMLInputElement>(null);
  useEffect(() => { box.current?.focus(); }, []);
  return <p className="mc-step-note editing"><input ref={box} className="field sm" value={text} maxLength={MAX_NOTE} placeholder={t('macro.note.ph')} aria-label={t('macro.note.label')}
    onChange={e => setText(e.target.value)} onBlur={() => done(text)}
    onKeyDown={e => {
      if (e.key === 'Enter') { e.preventDefault(); done(text); }
      if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); done(value); }
    }} /></p>;
}

/** The open step: its sentence again, with its values in place; under it the ticks that hand values to the player. */
function StepBody({ s, i, m, ed, cutting, invalid }: { s: Held; i: number; m: Macro; ed: Ed; cutting: boolean; invalid: Set<string> }) {
  const { t } = useHost();
  const n = i + 1;
  const id = (f: string) => `mc-s${s._k}-${f}`;
  const v = (path: string) => ed.val(i, path);
  const put = (path: string) => (x: unknown) => ed.put(i, path, x);
  const test = s.test as Test | undefined;
  const isTest = s.kind === 'if' || s.kind === 'loopUntil';
  const pre = isTest ? 'test.' : '';
  const bad = (field: string) => invalid.has(field) || invalid.has(field.replace(/^test\./, ''));

  const btnSel = (path: string, kinds: string[]) => <Pick id={id('b')} label={t('macro.e.buttonL')} value={String(v(path) || 'left')}
    options={kinds.map(b => [b, t(`macro.s.b.${b}`)])} onChange={put(path)} />;
  const secs = (path: string, label: string) => <Secs id={id(path.replace('.', '-'))} label={label} ms={v(path)} onChange={put(path)} invalid={bad(path)} />;
  const point = <button type="button" className="mc-pt" title={ed.canPick ? t('macro.e.pickPoint') : t('macro.need.game')} disabled={!ed.canPick} aria-invalid={bad('x') || undefined}
    onClick={() => ed.pick(i, 'point')}><span aria-hidden="true">{n}</span><span className="mc-vh">{t('macro.e.pickPoint')}</span></button>;
  const colour = (path: string, what: 'colour' | 'target' | 'marker', label: string) => <span className="mc-colourf">
    <ColorInput id={id(path.replace('.', '-'))} value={/^#[0-9a-f]{6}$/i.test(String(v(path))) ? String(v(path)) : '#ffffff'} label={label} onChange={put(path)} />
    <button type="button" className="btn sm mc-pick" disabled={!ed.canPick} title={ed.canPick ? t('macro.e.pickTip') : t('macro.need.game')}
      onClick={() => ed.pick(i, what, path)}><Icon name="mouse" /><span>{t('macro.e.pick')}</span></button></span>;
  const where = (path: string, withPoint: boolean) => {
    const raw = getAt(s, path);
    const cur = isArea(raw) ? raw : withPoint ? 'point' : 'anywhere';
    return <Pick id={id('where')} label={t('macro.e.whereL')} value={cur} className="mc-where" invalid={bad('region')}
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
    <button type="button" className={`btn sm mc-cut${cutting ? ' on' : ''}`} aria-pressed={cutting} aria-invalid={bad('image') || undefined} onClick={() => ed.cut(i)}><Icon name="image" />
      <span>{t(cutting ? 'macro.e.cutting' : img ? 'macro.e.recut' : 'macro.e.cut')}</span></button></span>;
  const until = (path = 'timeoutMs') => <Say k="macro.e.giveUp" parts={{ d: secs(path, t('macro.e.giveUpL')) }} />;
  const textIn = (path: string, max: number, label: string, wide = false) => <TextIn id={id(path.replace('.', '-'))} label={label} value={String(v(path) ?? '')} max={max} wide={wide}
    invalid={bad(path)} onChange={put(path)} />;
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
  const num = (path: string, label: string, min: number, max: number) => <Count id={id(path.replace('.', '-'))} label={label} value={v(path)} min={min} max={max} invalid={bad(path)} onChange={put(path)} />;
  const tune = (kids: ReactNode) => <details className="mc-tune"><summary><span>{t('macro.e.tune')}</span><Icon name="chevronDown" /></summary><div>{kids}</div></details>;
  // How close a colour must be: a share from exact to loose (the app keeps 0–255).
  const closeness = (path: string) => {
    const tol = Number(v(path));
    const pct = Number.isFinite(tol) ? Math.round(tol / 2.55) : 9;
    return <p className="mc-sent mc-close"><span>{t('macro.tune.close')}</span>
      <span className="mc-range"><small aria-hidden="true">{t('macro.tune.exact')}</small>
        <input type="range" id={id('close')} min={0} max={100} step={1} value={pct} aria-label={t('macro.tune.close')} aria-valuetext={`${pct} %`}
          onChange={e => ed.put(i, path, Math.round(+e.target.value * 2.55))} />
        <small aria-hidden="true">{t('macro.tune.loose')}</small></span><output htmlFor={id('close')}>{pct} %</output></p>;
  };
  const match = (path: string) => <Say k="macro.e.match" parts={{ n: <Count id={id('th')} label={t('macro.e.matchL')} value={Math.round((Number(v(path)) || 0.9) * 100)}
    min={50} max={100} onChange={x => ed.put(i, path, x / 100)} /> }} />;
  // A counter by what the panel calls it, or a new one (its own small dialog); for an If, the panel's settings too.
  const counterPick = (path: string, withSettings: boolean, onPick: (name: string) => void) => {
    const name = String(getAt(s, path) ?? '');
    const options = [...ed.counters.map(c => [c, ed.label(c)]), ...(withSettings ? ed.readable.map(x => [x.name, x.label]) : [])];
    if (name && !options.some(([k]) => k === name)) options.unshift([name, ed.label(name)]);
    const pid = id(path.replace('.', '-'));
    return <Pick id={pid} label={t('macro.e.counterL')} value={name} invalid={bad('name')}
      options={[...(name ? [] : [['', '—']]), ...options, ['\u0000new', t('macro.counter.new')]]}
      onChange={x => {
        if (x !== '\u0000new') return onPick(x);
        const el = document.getElementById(pid);
        if (el) ed.newCounter(el, onPick);
      }} />;
  };

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
        const nameIn = counterPick('test.name', true, x => {
          const to = ed.readable.find(y => y.name === x);
          ed.set(i, { test: { kind: 'var', name: x, op: to?.kind === 'check' || to?.kind === 'choice' ? '=' : String(test && 'op' in test ? test.op : '>='),
            value: to?.kind === 'check' ? 1 : to?.kind === 'choice' ? to.choices?.[0] ?? '' : (test?.kind === 'var' && typeof test.value === 'number' ? test.value : 1) } });
        });
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
      // At most so many rounds, or no limit (0).
      const most = Number(v('max')) || 0;
      lines = <>{rest}{s.kind === 'loopUntil' ? <div className="mc-most">
        <Say k="macro.e.atMost" parts={{ n: most ? num('max', t('macro.e.atMostL'), 1, 100000)
          : <input className="field sm num mc-count" type="number" disabled aria-label={t('macro.e.atMostL')} value="" /> }} />
        <Check id={id('nolimit')} label={t('macro.tune.noLimit')} checked={!most} onChange={x => ed.put(i, 'max', x ? 0 : 10)} /></div> : null}</>;
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
    case 'setVar': lines = <Say k="macro.e.setVar" parts={{ name: counterPick('name', false, x => ed.set(i, { name: x })), v: num('value', t('macro.e.valueL'), -1e9, 1e9) }} />; break;
    case 'addVar': lines = <Say k="macro.e.addVar" parts={{ name: counterPick('name', false, x => ed.set(i, { name: x })), v: num('value', t('macro.e.valueL'), -1e9, 1e9) }} />; break;
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

function Pick({ id, label, value, options, onChange, className, invalid }: { id: string; label: string; value: string; options: string[][]; onChange: (v: string) => void; className?: string; invalid?: boolean }) {
  return <select className={`select sm${className ? ' ' + className : ''}`} id={id} aria-label={label} value={value} aria-invalid={invalid || undefined} onChange={e => onChange(e.target.value)}>
    {options.map(([v, text]) => <option key={v} value={v}>{text}</option>)}</select>;
}

/** A time in seconds (kept in milliseconds): "0.5 s". */
function Secs({ id, label, ms, onChange, invalid }: { id: string; label: string; ms: unknown; onChange: (v: number) => void; invalid?: boolean }) {
  const { t } = useHost();
  const n = typeof ms === 'number' && Number.isFinite(ms) ? +(ms / 1000).toFixed(3) : '';
  return <span className="mc-unit"><input className="field sm num" id={id} type="number" min={0} step="0.1" aria-label={label} value={n} aria-invalid={invalid || undefined}
    onChange={e => onChange(e.target.value === '' ? NaN : Math.round(+e.target.value * 1000))} /><span aria-hidden="true">{t('macro.unit.s')}</span></span>;
}

/** A whole number in a field of the form (a label above it). */
function NumField({ id, value, min, max, onChange }: { id: string; value: unknown; min: number; max: number; onChange: (v: number) => void }) {
  return <input className="field num" id={id} type="number" min={min} max={max}
    value={typeof value === 'number' && Number.isFinite(value) ? value : ''} onChange={e => onChange(e.target.value === '' ? NaN : +e.target.value)} />;
}

function Count({ id, label, value, min, max, onChange, invalid }: { id: string; label: string; value: unknown; min: number; max: number; onChange: (v: number) => void; invalid?: boolean }) {
  return <input className="field sm num mc-count" id={id} type="number" min={min} max={max} aria-label={label} aria-invalid={invalid || undefined}
    value={typeof value === 'number' && Number.isFinite(value) ? value : ''} onChange={e => onChange(e.target.value === '' ? NaN : +e.target.value)} />;
}

function TextIn({ id, label, value, max, wide, onChange, invalid }: { id: string; label: string; value: string; max: number; wide?: boolean; onChange: (v: string) => void; invalid?: boolean }) {
  return <input className={`field sm mc-text${wide ? ' wide' : ''}`} id={id} maxLength={max} spellCheck={false} aria-label={label} value={value} aria-invalid={invalid || undefined}
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
