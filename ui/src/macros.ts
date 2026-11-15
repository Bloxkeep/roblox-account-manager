// Macros (Beta): what a macro and its steps are (the app's AppSettings.Macros, the shared contract), the keys a step
// may press, the settings a player may change on its panel (options) and the boxes it looks in (areas), the plain words
// for a trigger or a step, and the checks before Save. The app runs them: one Roblox window at a time, each brought to
// the front for its turn.

export type Rect = { x: number; y: number; w: number; h: number };
/** A number or colour may be one of the macro's options instead ("$o1"), an area one of its areas ("@a1"). */
type V<X> = X | string;
export type Test =
  | { kind: 'image'; image: string; region?: V<Rect>; threshold?: V<number>; not?: boolean }
  /** words: any of a list the player ticks ("$o1") counts, in place of the text. */
  | { kind: 'text'; text: string; words?: string; region?: V<Rect>; not?: boolean }
  | { kind: 'colour'; x: V<number>; y: V<number>; colour: string; tolerance: V<number>; region?: V<Rect>; not?: boolean }
  | { kind: 'var'; name: string; op: string; value: number | string };
/** A step as the editor holds it: its kind and that kind's fields (the contract's Step). */
export type Step = { kind: string; [field: string]: any };
export type Trigger = { kind: 'manual' | 'interval' | 'join' | 'rejoin'; everySeconds?: number };
export type Macro = {
  id: string; name: string; steps: Step[]; targets: 'selected' | 'all' | 'accounts'; accountIds: string[]; trigger: Trigger;
  enabled: boolean; hotkey: number | null; jitterMs: number; jitterPx: number; maxMinutes: number; onFail: 'stop' | 'skip' | 'restart' | 'rejoin' | 'alert';
  /** Stuck when what its steps look for hasn't been seen this long (then onFail); null: never. */
  stuckMinutes?: number | null;
  recordedSize?: { w: number; h: number };
  /** What the player may change, on the macro's panel in this order (12 at most). */
  options?: Option[];
  /** Named boxes on the game the steps look in (8 at most). */
  areas?: Area[];
  /** Its small window beside the game: shown by itself while an account plays placeId. */
  /** tabs: the panel's tabs, in order (fewer than two: none). */
  /** counterLabels: what the panel and the editor call a counter (its name stays its key). */
  panel?: { autoShow: boolean; placeId: number | null; tabs?: string[]; counterLabels?: Record<string, string> };
  /** Switches on the panel with steps of their own (10 at most), played whole where the main steps reach a safe point. */
  features?: Feature[];
};
/** A feature: its switch (a "check" option), how often it may play (none: at every safe point), what a wait that runs
 * out does in it (skip: not this time; macro: the macro's own onFail), and its steps. */
export type Feature = { id: string; option: string; everyMs?: number | null; onFail: 'skip' | 'macro'; steps: Step[] };
export const MAX_FEATURES = 10;
/** The main steps' safe points: the "End repeat" of each repeat inside no other block (before If stuck), and their end. */
export function safeEnds(steps: Step[]): number[] {
  const out: number[] = [];
  let depth = 0;
  for (let i = 0; i < steps.length && steps[i].kind !== 'recover'; i++) {
    const k = steps[i].kind;
    if (OPENERS.has(k)) depth++;
    else if (k === 'endIf') depth--;
    else if (k === 'endLoop' && depth-- === 1) out.push(i);
  }
  return out;
}
/** A setting the player may change on the panel. The page makes it from a step's value the maker ticked: its name is
 * never shown, its label comes from the step's sentence, its kind, range and unit from the field. A switch, a choice and a
 * heading are added on the panel itself (FREE): an If reads the first two; a heading is a title over the rows below it.
 * A point is {x, y}, parts of the window; tab: the panel's tab it shows on (none: the first). */
export type Option = {
  name: string; label: string; kind: 'number' | 'colour' | 'check' | 'key' | 'choice' | 'point' | 'list' | 'heading';
  value: OptValue; default: OptValue; min?: number; max?: number; unit?: 'ms' | 's' | 'px' | '%' | ''; choices?: string[]; tab?: string;
};
export type OptValue = number | string | string[] | { x: number; y: number } | null;
/** The settings made on the panel, not from a step's value: kept while no step names them. A list's value is the words
 * the player ticked of its choices; a step reading text takes any of them. */
/** A point setting's value ({x, y}), or null. */
export const asPoint = (v: OptValue | undefined) => v && typeof v === 'object' && !Array.isArray(v) ? v : null;
export const FREE = new Set<Option['kind']>(['check', 'choice', 'list', 'heading']);
export const MAX_TABS = 6;
export type Area = { name: string; label: string; colour: string; rect: Rect };
export type RunStatus = 'waiting' | 'running' | 'paused' | 'done' | 'failed' | 'stopped';
/** A run of a macro on one account; trial: the editor's Try it (shown there, not in the list). */
/** step: where it is while live; lastStep: where it ended (a failed run: the step that stopped it). */
/** section: the feature playing (or, ended, where it was); none for the main steps. */
export type MacroRun = { macroId: string; accountId: string; status: RunStatus; reason?: string; step?: number; lastStep?: number; section?: string | null; loops?: number; startedAt: string; endsAt?: string; trial?: boolean };
export type Recording = { accountId: string; startsAt: string; steps: number } | null;

/** The app's limits (contract): 100 macros, 5000 steps, names up to 60 characters, typed text up to 200. */
export const MAX_MACROS = 100;
export const MAX_STEPS = 5000;
export const MAX_OPTIONS = 30;
export const MAX_AREAS = 8;
/** Each new area's colour, in turn: told apart on the game and on the picture. */
export const AREA_COLOURS = ['#22c55e', '#f59e0b', '#3b82f6', '#ec4899', '#a855f7', '#14b8a6', '#ef4444', '#eab308'];
/** An image step's picture, cut from a window picture: at most this many pixels each way. */
export const MAX_CUT = 256;

const range = (from: number, to: number, f: (i: number) => string) => Array.from({ length: to - from + 1 }, (_, i) => f(from + i));
/** The keys a step may press, by the names the app knows (it sends their scan codes). */
export const KEYS = [
  ...range(65, 90, c => String.fromCharCode(c)), ...range(0, 9, String), ...range(1, 12, n => `F${n}`),
  'Space', 'Enter', 'Tab', 'Backspace', 'Escape', 'Shift', 'Ctrl', 'Alt', 'Up', 'Down', 'Left', 'Right',
  'Insert', 'Delete', 'Home', 'End', 'PageUp', 'PageDown', 'Minus', 'Equals', 'LeftBracket', 'RightBracket',
  'Semicolon', 'Quote', 'Comma', 'Period', 'Slash', 'Backslash', 'Backquote', ...range(0, 9, n => `Numpad${n}`),
];
/** How a key reads on a key cap ("[" rather than LeftBracket). */
const CAPS: Record<string, string> = {
  Minus: '-', Equals: '=', LeftBracket: '[', RightBracket: ']', Semicolon: ';', Quote: "'", Comma: ',', Period: '.', Slash: '/',
  Backslash: '\\', Backquote: '`', Up: '↑', Down: '↓', Left: '←', Right: '→', PageUp: 'Page Up', PageDown: 'Page Down',
};
export const keyCap = (k: string) => CAPS[k] || (k.startsWith('Numpad') ? `Num ${k.slice(6)}` : k);
/** Mouse buttons a Hold or Let go step may take instead of a key (charge-and-release casts). */
export const MOUSE_KEYS = ['MouseLeft', 'MouseRight', 'MouseMiddle'];
/** A key's or mouse button's name in words ("left mouse", "Space"). */
export const keyName = (t: T, k: string) => MOUSE_KEYS.includes(k) ? t(`macro.mk.${k}`) : keyCap(k);

/** A key pressed in the "Press a key" box, as a KEYS name; '' for one a macro can't press (Windows, Print Screen). */
export function keyFromEvent(e: { code: string; key: string }): string {
  const c = e.code;
  if (/^Key[A-Z]$/.test(c)) return c.slice(3);
  if (/^Digit\d$/.test(c)) return c.slice(5);
  if (/^Numpad\d$/.test(c)) return c;
  if (/^F([1-9]|1[0-2])$/.test(c)) return c;
  const map: Record<string, string> = {
    Space: 'Space', Enter: 'Enter', NumpadEnter: 'Enter', Tab: 'Tab', Backspace: 'Backspace', Escape: 'Escape', ShiftLeft: 'Shift', ShiftRight: 'Shift',
    ControlLeft: 'Ctrl', ControlRight: 'Ctrl', AltLeft: 'Alt', AltRight: 'Alt', ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
    Insert: 'Insert', Delete: 'Delete', Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown', Minus: 'Minus', Equal: 'Equals',
    BracketLeft: 'LeftBracket', BracketRight: 'RightBracket', Semicolon: 'Semicolon', Quote: 'Quote', Comma: 'Comma', Period: 'Period',
    Slash: 'Slash', Backslash: 'Backslash', Backquote: 'Backquote',
  };
  return map[c] || '';
}

/** The kinds a step can be, in the Add step menu's groups. Adding if or a loop adds its end too. */
export const STEP_GROUPS: [string, string[]][] = [
  ['keys', ['key', 'down', 'up', 'type']],
  ['mouse', ['click', 'clickImage', 'clickColour', 'move', 'camera', 'scroll', 'follow']],
  ['wait', ['wait', 'waitImage', 'waitText', 'waitColour', 'waitStill']],
  ['check', ['if', 'else']],
  ['flow', ['loop', 'loopUntil', 'break', 'stop', 'recover']],
  ['other', ['setVar', 'addVar', 'read', 'write', 'call', 'rejoin', 'alert']],
];
/** Each kind's icon (the page's own icon set). */
export const STEP_ICON: Record<string, string> = {
  key: 'keyboard', down: 'keyboard', up: 'keyboard', type: 'message', click: 'mouse', clickImage: 'image', move: 'mouse', camera: 'rotate', scroll: 'sort',
  wait: 'clock', waitImage: 'image', waitText: 'search', waitColour: 'palette', waitStill: 'clock', if: 'branch', else: 'branch', endIf: 'cornerUpLeft',
  loop: 'repeat', loopUntil: 'repeat', endLoop: 'cornerUpLeft', break: 'logOut', stop: 'stop', setVar: 'tag', addVar: 'tag', read: 'eye', write: 'edit', call: 'play', rejoin: 'refresh', alert: 'bell',
  follow: 'gauge', clickColour: 'palette', recover: 'wrench',
};
/** Other words a person may search Add step for, by kind (the kinds' own names and groups are searched too). */
export const SYNONYMS: Record<string, string[]> = {
  key: ['press', 'hit', 'bấm', 'nhấn', 'phím'], down: ['press', 'bấm', 'nhấn'], click: ['tap', 'mouse', 'bấm', 'nhấn', 'chuột'],
  setVar: ['variable', 'formula', 'math', 'random', 'biến', 'công thức', 'tính'], addVar: ['variable', 'biến', 'cộng'],
  read: ['ocr', 'number', 'coins', 'money', 'clipboard', 'file', 'web', 'đọc', 'số', 'tiền'], write: ['clipboard', 'file', 'web', 'webhook', 'log', 'save', 'ghi', 'lưu', 'chép'],
  call: ['function', 'sub', 'run', 'macro', 'gọi', 'chạy'],
  wait: ['sleep', 'delay', 'pause', 'chờ', 'đợi'], waitImage: ['chờ', 'đợi'], waitText: ['chờ', 'đợi'], waitColour: ['chờ', 'đợi'], waitStill: ['chờ', 'đợi'],
  loop: ['loop', 'again', 'lặp'], loopUntil: ['loop', 'lặp'], break: ['exit', 'leave', 'thoát'], if: ['when', 'check', 'nếu'], else: ['otherwise', 'ngược lại'],
};
/** A step's note: a line under its sentence saying why it is there (kept with the step as `note`). */
export const MAX_NOTE = 80;
/** The kinds that open a block (If, Repeat, Repeat until), and the step that ends each. */
export const OPENERS = new Set(['if', 'loop', 'loopUntil']);
export const endOf = (kind: string) => kind === 'if' ? 'endIf' : 'endLoop';
/** Where the block opened at i ends (its End if / End repeat), or -1 when it never does. */
export function blockEnd(steps: Step[], i: number): number {
  if (!OPENERS.has(steps[i]?.kind)) return -1;
  let d = 0;
  for (let j = i; j < steps.length; j++) {
    const k = steps[j].kind;
    if (OPENERS.has(k)) d++;
    else if (k === 'endIf' || k === 'endLoop') { if (--d === 0) return k === endOf(steps[i].kind) ? j : -1; }
  }
  return -1;
}
/** The blocks around a place in the steps (a step about to go in at index at), innermost last. */
export function blocksAt(steps: Step[], at: number): { at: number; kind: string; hasElse: boolean }[] {
  const open: { at: number; kind: string; hasElse: boolean }[] = [];
  for (let j = 0; j < Math.min(at, steps.length); j++) {
    const k = steps[j].kind;
    if (OPENERS.has(k)) open.push({ at: j, kind: k, hasElse: false });
    else if (k === 'else' && open.at(-1)?.kind === 'if') open.at(-1)!.hasElse = true;
    else if ((k === 'endIf' && open.at(-1)?.kind === 'if') || (k === 'endLoop' && open.length && open.at(-1)!.kind !== 'if')) open.pop();
  }
  return open;
}
/** The kinds Add step offers at a place: Otherwise only straight inside an If without one, Leave the repeat inside a
 * repeat, If stuck once. */
export function kindsAt(steps: Step[], at: number): string[] {
  const around = blocksAt(steps, at);
  const inner = around.at(-1);
  return STEP_GROUPS.flatMap(([, kinds]) => kinds).filter(k =>
    k === 'else' ? inner?.kind === 'if' && !inner.hasElse
      : k === 'break' ? around.some(b => b.kind !== 'if')
      : k === 'recover' ? !steps.some(s => s.kind === 'recover')
      : true);
}
/** Steps that look for something on the game: only a macro with one can be stuck. */
export const LOOK_STEPS = ['waitImage', 'waitText', 'waitColour', 'waitStill', 'clickImage', 'clickColour', 'follow'];

/** A new step of a kind, with the contract's defaults. */
export function newStep(kind: string): Step {
  switch (kind) {
    case 'key': return { kind, key: 'Space', ms: 60 };
    case 'down': case 'up': return { kind, key: 'W' };
    case 'click': return { kind, x: 0.5, y: 0.5, button: 'left', double: false };
    case 'clickImage': return { kind, image: '', button: 'left', threshold: 0.9, timeoutMs: 5000 };
    case 'camera': return { kind, dx: 200, dy: 0, ms: 300, button: 'right' };
    case 'scroll': return { kind, notches: -3 };
    case 'move': return { kind, x: 0.5, y: 0.5 };
    case 'wait': return { kind, ms: 1000 };
    case 'waitImage': return { kind, image: '', threshold: 0.9, timeoutMs: 30000, gone: false };
    case 'waitText': return { kind, text: '', timeoutMs: 30000, gone: false };
    case 'waitColour': return { kind, x: 0.5, y: 0.5, colour: '#ffffff', tolerance: 24, timeoutMs: 30000, gone: false };
    case 'waitStill': return { kind, ms: 2000, timeoutMs: 30000 };
    case 'if': return { kind, test: { kind: 'text', text: '' } };
    case 'loop': return { kind, times: 0 };
    case 'loopUntil': return { kind, test: { kind: 'text', text: '' }, max: 0 };
    case 'type': return { kind, text: '' };
    case 'setVar': return { kind, name: 'count', value: 0 };
    case 'addVar': return { kind, name: 'count', value: 1 };
    // Reads a number (coins, a level) or the words in an area into a variable, tried again until one shows.
    case 'read': return { kind, name: 'coins', mode: 'number', from: 'screen', timeoutMs: 10000 };
    case 'write': return { kind, to: 'file', fileName: 'Bloxkeep log', text: '' };
    case 'call': return { kind };
    case 'alert': return { kind, text: '', picture: false };
    // Keeps a moving marker on a target (a reel or charge bar): the area to watch and the two colours are set after.
    case 'follow': return { kind, target: '#22c55e', marker: '#ffffff', tolerance: 24, button: 'left', mode: 'onOff', predict: false, kp: 1, kd: 0.4, fps: 60, untilGoneMs: 1000, timeoutMs: 30000 };
    // Clicks a patch of a colour wherever it shows in the area (buttons popping up at random spots), again while it keeps showing.
    case 'clickColour': return { kind, colour: '#ffffff', tolerance: 24, minSize: 4, button: 'left', timeoutMs: 30000 };
    default: return { kind };
  }
}
/** What Add step puts in: a block's opening step with its end. */
export const stepsToAdd = (kind: string): Step[] =>
  kind === 'if' ? [newStep('if'), newStep('endIf')] : kind === 'loop' || kind === 'loopUntil' ? [newStep(kind), newStep('endLoop')] : [newStep(kind)];

export const newTest = (kind: string): Test =>
  kind === 'image' ? { kind, image: '', threshold: 0.9 } : kind === 'colour' ? { kind, x: 0.5, y: 0.5, colour: '#ffffff', tolerance: 24 }
    : kind === 'var' ? { kind, name: 'count', op: '>=', value: 1 } : { kind: 'text', text: '' };

/** A new, empty macro: by hand, on the accounts selected in the Accounts tab. */
export const blankMacro = (): Macro => ({
  id: '', name: '', steps: [], targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null,
  jitterMs: 40, jitterPx: 3, maxMinutes: 10, onFail: 'stop', stuckMinutes: 5, options: [], areas: [], panel: { autoShow: false, placeId: null },
});

/** The built-in templates, the app's own (macroTemplates): macros with their options and areas filled in. A template's
 * steps may name fields the player still has to set for their game (todo: 'region', 'colour'…): the form lists those
 * on top until done. */
export type MacroTemplate = Macro & { about?: string; icon?: string };
export type Templates = { macros: MacroTemplate[] };

/** A setting or area reference in a step: "$o1" (an option), "@a1" (an area). */
export const isOpt = (v: unknown): v is string => typeof v === 'string' && /^\$[A-Za-z_][A-Za-z0-9_]{0,23}$/.test(v);
export const isArea = (v: unknown): v is string => typeof v === 'string' && /^@[A-Za-z_][A-Za-z0-9_]{0,23}$/.test(v);
/** The first free name with this prefix: o1, o2… or a1, a2… */
export const freeName = (prefix: string, taken: { name: string }[]) => {
  for (let n = 1; ; n++) if (!taken.some(x => x.name === prefix + n)) return prefix + n;
};
/** The counters a macro keeps (its Set and Add steps' names): each shows on its panel. */
export const counters = (m: Pick<Macro, 'steps'>): string[] => [...new Set(m.steps.filter(s => s.kind === 'setVar' || s.kind === 'addVar' || s.kind === 'read').map(s => String(s.name || '').trim()).filter(Boolean))];
/** The steps with a point on the window (a numbered dot): its number is the step's. */
export const pointOf = (s: Step, options: Option[] = []): { x: number; y: number } | null =>
  ['click', 'move'].includes(s.kind) || (s.kind === 'waitColour' && !s.region) ? (typeof s.x === 'number' && typeof s.y === 'number' ? { x: s.x, y: s.y }
    : isOpt(s.x) ? asPoint(options.find(o => '$' + o.name === s.x && o.kind === 'point')?.value) : null)
    : (s.kind === 'if' || s.kind === 'loopUntil') && s.test?.kind === 'colour' && !s.test.region && typeof s.test.x === 'number' && typeof s.test.y === 'number' ? { x: s.test.x, y: s.test.y }
    : null;

/** A macro from before areas: each box drawn on a step becomes one of its areas ("Area 1"…), the step pointing at it. */
export function withAreas(t: T, m: Macro): Macro {
  const areas = [...(m.areas || [])];
  const box = (r: unknown) => {
    if (!r || typeof r !== 'object') return r;
    const name = freeName('a', areas);
    areas.push({ name, label: t('macro.area.n', { n: areas.length + 1 }), colour: AREA_COLOURS[areas.length % AREA_COLOURS.length], rect: r as Rect });
    return '@' + name;
  };
  const steps = m.steps.map(s => {
    let out = s;
    if (s.region && typeof s.region === 'object') out = { ...out, region: box(s.region) };
    if (s.test?.region && typeof s.test.region === 'object') out = { ...out, test: { ...s.test, region: box(s.test.region) } };
    return out;
  });
  return { ...m, steps, areas, options: m.options || [], panel: m.panel || { autoShow: false, placeId: null } };
}

type T = (key: string, vars?: Record<string, string | number>) => string;

/** "45 s", "5 min", "1 h 30 min": a length of time in plain words. */
export function durText(t: T, seconds: number): string {
  const s = Math.round(seconds);
  if (s < 60) return t('macro.dur.s', { n: s });
  const h = Math.floor(s / 3600), m = Math.floor((s % 3600) / 60), r = s % 60;
  return [h ? t('macro.dur.h', { n: h }) : '', m ? t('macro.dur.min', { n: m }) : '', r ? t('macro.dur.s', { n: r }) : ''].filter(Boolean).join(' ');
}
/** A time in words, kept in milliseconds, said in seconds ("0.6 s", "2 min"). */
export const msText = (t: T, ms: number) => ms < 60000 && ms % 1000 ? t('macro.dur.s', { n: +(ms / 1000).toFixed(2) }) : durText(t, ms / 1000);

/** What a step's sentence needs of its macro: the values of its options and the labels of its areas. */
export type Ctx = Pick<Macro, 'options' | 'areas'> & { panel?: Macro['panel']; macroNames?: Record<string, string> };
/** A counter as the panel and the editor call it: its label, else its name. */
export const counterName = (m: Ctx | undefined, name: string) => m?.panel?.counterLabels?.[name] || name;
/** A value, or the option it stands for. */
export const valueOf = (m: Ctx | undefined, v: unknown) => isOpt(v) ? m?.options?.find(o => '$' + o.name === v)?.value ?? '' : v;
/** Where a step looks, in words: "in Fish area", or "anywhere" on the window. */
export function areaText(t: T, r: unknown, m?: Ctx): string {
  if (isArea(r)) return t('macro.s.inArea', { area: m?.areas?.find(a => '@' + a.name === r)?.label || t('macro.s.gone') });
  return t('macro.s.anywhere');
}
const button = (t: T, b: unknown) => t(`macro.s.b.${b === 'right' || b === 'middle' ? b : 'left'}`);
/** A camera turn's direction from its drag (dx, dy). */
export const camDir = (s: Step) => Math.abs(Number(s.dx) || 0) >= Math.abs(Number(s.dy) || 0) ? (Number(s.dx) < 0 ? 'left' : 'right') : (Number(s.dy) < 0 ? 'up' : 'down');

/** What a step does as a sentence ("Hold the left button for 0.6 s at point 2", "Wait until the colour shows in Fish
 * area"): its row's text. Points are numbered after their step (n). */
/** Where a Read step reads, or a Write step writes: the area on the game, the clipboard, a file by name, a web address's host. */
const fromText = (t: T, s: Step, where: string) => s.from === 'clipboard' ? t('macro.s.fromClip')
  : s.from === 'file' ? t('macro.s.fromFile', { name: String(s.fileName || '…') })
  : s.from === 'web' ? t('macro.s.fromWeb', { host: hostOf(String(s.url || '')) || '…' }) : where;
const hostOf = (url: string) => { try { return new URL(url).host; } catch { return ''; } };

export function stepText(t: T, s: Step, m?: Ctx, n = 0): string {
  const v = (x: unknown) => valueOf(m, x);
  const ms = (x: unknown) => { const y = v(x); return typeof y === 'number' ? msText(t, y) : '…'; };
  const where = areaText(t, s.region, m);
  const shows = s.gone ? 'gone' : 'shows';
  switch (s.kind) {
    case 'key': return t('macro.s.key', { k: keyName(t, String(v(s.key))) });
    case 'down': return t('macro.s.down', { k: keyName(t, s.key) });
    case 'up': return t('macro.s.up', { k: keyName(t, s.key) });
    case 'type': return t('macro.s.type', { text: String(s.text || '…') });
    case 'click': return s.holdMs ? t('macro.s.clickHold', { b: button(t, s.button), d: ms(s.holdMs), n })
      : t(s.double ? 'macro.s.dblClick' : 'macro.s.click', { b: button(t, s.button), n });
    case 'clickImage': return t('macro.s.clickImage', { where });
    case 'clickColour': return t(s.eachAfterMs != null ? 'macro.s.clickEach' : 'macro.s.clickColour', { where });
    case 'move': return t('macro.s.move', { n });
    case 'camera': return t('macro.s.camera', { dir: t(`macro.e.dir.${camDir(s)}`) });
    case 'scroll': return t(Number(s.notches) > 0 ? 'macro.s.scrollUp' : 'macro.s.scrollDown', { n: Math.abs(Number(s.notches) || 0) });
    case 'wait': return t('macro.s.wait', { d: ms(s.ms) });
    case 'waitImage': return t(`macro.s.waitImage.${shows}`, { where });
    case 'waitText': return s.words ? t(`macro.s.waitWords.${shows}`, { name: listName(s.words, m), where })
      : t(`macro.s.waitText.${shows}`, { text: String(s.text || '…'), where });
    case 'waitColour': return t(`macro.s.waitColour.${shows}`, { where: s.region ? where : t('macro.s.atPoint', { n }) });
    case 'waitStill': return t('macro.s.waitStill', { d: ms(s.ms) });
    case 'if': case 'loopUntil': {
      const test = s.test as Test | undefined;
      const lead = t(s.kind === 'if' ? 'macro.s.if' : 'macro.s.until');
      if (!test) return lead;
      if (test.kind === 'var') {
        // A switch or choice the player sets: by its label, "on"/"off" for a switch.
        const o = m?.options?.find(x => x.name === test.name);
        if (o?.kind === 'check') return `${lead} ${t(test.value ? 'macro.s.switchOn' : 'macro.s.switchOff', { name: o.label })}`;
        return `${lead} ${t(o ? 'macro.s.settingIs' : 'macro.s.counterIs', { name: o?.label || (test.name ? counterName(m, test.name) : '…'), op: t(`macro.e.op.${test.op}`), v: String(v(test.value)) })}`;
      }
      const at = test.region ? areaText(t, test.region, m) : test.kind === 'colour' ? t('macro.s.atPoint', { n }) : t('macro.s.anywhere');
      const what = test.kind === 'text' ? (test.words ? t('macro.s.theWords', { name: listName(test.words, m) }) : t('macro.s.theText', { text: test.text || '…' })) : t(`macro.s.the.${test.kind}`);
      return `${lead} ${t(test.not ? 'macro.s.isGone' : 'macro.s.shows', { what, where: at })}`;
    }
    case 'loop': return s.times ? t('macro.s.loopN', { n: String(v(s.times)) }) : t('macro.s.loop');
    case 'setVar': return typeof s.text === 'string' ? t('macro.s.setText', { name: s.name ? counterName(m, s.name) : '…', text: s.text })
      : t('macro.s.setVar', { name: s.name ? counterName(m, s.name) : '…', v: s.expr ?? String(v(s.value)) });
    case 'addVar': return t('macro.s.addVar', { name: s.name ? counterName(m, s.name) : '…', v: s.expr ?? String(v(s.value)) });
    case 'read': return t(`macro.s.read.${s.mode === 'text' ? 'text' : 'number'}`, { name: s.name ? counterName(m, s.name) : '…', where: fromText(t, s, where) });
    case 'write': return t('macro.s.write', { text: String(s.text || '…'), to: s.to === 'clipboard' ? t('macro.s.toClip')
      : s.to === 'web' ? t('macro.s.toWeb', { host: hostOf(String(s.url || '')) || '…' }) : t('macro.s.toFile', { name: String(s.fileName || '…') }) });
    case 'call': return t('macro.s.call', { name: (s.macroId && m?.macroNames?.[s.macroId]) || '…' });
    case 'alert': return t(s.picture ? 'macro.s.alertPic' : 'macro.s.alert');
    case 'follow': return t('macro.s.follow', { where });
    case 'recover': return t('macro.s.recover');
    default: return t(`macro.kind.${s.kind}`);
  }
}

/** What stops a save (or a warning), tied to its step and field. bare: the words without "Step n:" (said on the step's
 * own row). fix: the one-click way out the form offers (Add End if, Pick on game, Choose area, Name it). */
/** sec: the feature whose steps it is about (none: the main steps). */
export type Problem = { step: number | null; text: string; bare: string; warn?: boolean; field?: string; fix?: 'addEnd' | 'pick' | 'area' | 'name'; sec?: string };
const okNum = (v: unknown, min: number, max: number) => isOpt(v) || (typeof v === 'number' && isFinite(v) && v >= min && v <= max);
const okColour = (v: unknown) => isOpt(v) || (typeof v === 'string' && /^#[0-9a-f]{6}$/i.test(v));
const imageKinds = ['clickImage', 'waitImage'];

/** What stops it from saving (and a warning or two that don't), each tied to its step: the app checks the same. */
export function problems(t: T, m: Macro, macros: Macro[]): Problem[] {
  const out: Problem[] = [];
  const add = (step: number | null, key: string, vars?: Record<string, string | number>, warn = false, field?: string, fix?: Problem['fix']) => {
    const text = t(key, { ...vars, n: step === null ? '' : step + 1 });
    const lead = step === null ? '' : `${t('macro.stepN', { n: step + 1 })}:`;
    const rest = lead && text.startsWith(lead) ? text.slice(lead.length).trim() : text;
    out.push({ step, text, bare: rest.charAt(0).toLocaleUpperCase() + rest.slice(1), warn, field, ...(fix ? { fix } : {}) });
  };
  if (!m.name.trim()) add(null, 'macro.err.name', {}, false, 'name');
  else if (macros.some(o => o.id !== m.id && o.name.trim().toLowerCase() === m.name.trim().toLowerCase())) add(null, 'macro.err.sameName', {}, true, 'name');
  if (m.targets === 'accounts' && !m.accountIds.length) add(null, 'macro.err.noAccounts', {}, false, 'accounts');
  if (m.trigger.kind === 'interval' && !okNum(m.trigger.everySeconds, 10, 86400)) add(null, 'macro.err.every', {}, false, 'every');
  if (!m.steps.length) add(null, 'macro.err.noSteps');
  if (m.steps.length > MAX_STEPS) add(null, 'macro.err.tooMany', { max: MAX_STEPS });

  // Blocks: each if and loop closed, else and break where they belong. Keys held down let go before the end.
  const open: { kind: string; at: number; hasElse?: boolean }[] = [];
  const held = new Map<string, number>();
  m.steps.forEach((s, i) => {
    switch (s.kind) {
      case 'if': open.push({ kind: 'if', at: i }); break;
      case 'loop': case 'loopUntil': open.push({ kind: 'loop', at: i }); break;
      case 'else': {
        const top = open[open.length - 1];
        if (top?.kind !== 'if') add(i, 'macro.err.elseAlone');
        else if (top.hasElse) add(i, 'macro.err.elseTwice');
        else top.hasElse = true;
        break;
      }
      case 'endIf': if (open[open.length - 1]?.kind === 'if') open.pop(); else add(i, 'macro.err.endIfAlone'); break;
      case 'endLoop': if (open[open.length - 1]?.kind === 'loop') open.pop(); else add(i, 'macro.err.endLoopAlone'); break;
      case 'break': if (!open.some(o => o.kind === 'loop')) add(i, 'macro.err.breakAlone'); break;
      // The macro's own steps end here; what follows runs only to get unstuck.
      case 'recover':
        if (open.length) add(i, 'macro.err.recoverInside');
        else if (m.steps.findIndex(x => x.kind === 'recover') !== i) add(i, 'macro.err.recoverTwice');
        else if (held.size) add(i, 'macro.err.heldAtRecover', { k: keyCap([...held.keys()][0]) });
        else if (m.onFail !== 'restart' && m.onFail !== 'skip' && !m.stuckMinutes) add(i, 'macro.warn.recoverOff', {}, true);
        break;
      case 'down':
        if (held.has(s.key)) add(i, 'macro.err.downTwice', { k: keyCap(s.key) });
        else held.set(s.key, i);
        break;
      case 'up':
        if (!held.has(s.key)) add(i, 'macro.err.upAlone', { k: keyCap(s.key) });
        else held.delete(s.key);
        break;
    }
    if (['key', 'down', 'up'].includes(s.kind) && !(s.kind === 'key' && isOpt(s.key)) && !KEYS.includes(s.key) && !(s.kind !== 'key' && MOUSE_KEYS.includes(s.key))) add(i, 'macro.err.key');
    if (s.kind === 'key' && s.key === 'Alt') add(i, 'macro.err.altTap');
    if (s.kind === 'key' && !okNum(s.ms, 30, 10000)) add(i, 'macro.err.hold', {}, false, 'ms');
    if ((s.kind === 'key' || s.kind === 'down') && s.key === 'Escape') add(i, 'macro.warn.escape', {}, true);
    if (['click', 'move', 'waitColour'].includes(s.kind) && !(s.kind === 'waitColour' && s.region) && !(okNum(s.x, 0, 1) && okNum(s.y, 0, 1))) add(i, 'macro.err.point', {}, false, 'x', 'pick');
    if (imageKinds.includes(s.kind) && !s.image) add(i, 'macro.err.image', {}, false, 'image');
    if (s.kind === 'waitText' && !s.words && !String(s.text || '').trim()) add(i, 'macro.err.text', {}, false, 'text');
    if (s.kind === 'wait' && !okNum(s.ms, 0, 3_600_000)) add(i, 'macro.err.wait');
    if (['clickImage', 'waitImage', 'waitText', 'waitColour', 'waitStill'].includes(s.kind) && !okNum(s.timeoutMs, 100, 3_600_000)) add(i, 'macro.err.timeout');
    if (s.kind === 'type' && !String(s.text || '').trim()) add(i, 'macro.err.typeEmpty', {}, false, 'text');
    if (s.kind === 'type' && String(s.text || '').length > 200) add(i, 'macro.err.typeLong');
    if ((s.kind === 'setVar' || s.kind === 'addVar' || s.kind === 'read') && !String(s.name || '').trim()) add(i, 'macro.err.varName');
    if ((s.kind === 'setVar' || s.kind === 'addVar') && typeof s.text !== 'string' && (s.expr !== undefined ? !String(s.expr).trim() : !(isOpt(s.value) || okNum(s.value, -1e12, 1e12))))
      add(i, 'macro.err.formula', {}, false, 'value');
    if (s.kind === 'read' && (s.from || 'screen') === 'screen' && !s.region) add(i, 'macro.err.readArea', {}, false, 'region', 'area');
    const where = s.kind === 'read' ? s.from : s.kind === 'write' ? s.to : '';
    if (where === 'file' && !/^[\p{L}\p{N} _\-]{1,40}$/u.test(String(s.fileName || '').trim())) add(i, 'macro.err.fileName', {}, false, 'fileName');
    if (where === 'web' && !/^https:\/\/\S+$/.test(String(s.url || '').trim())) add(i, 'macro.err.url', {}, false, 'url');
    if (s.kind === 'write' && !String(s.text || '').trim()) add(i, 'macro.err.writeEmpty', {}, false, 'text');
    if (s.kind === 'call' && !s.macroId) add(i, 'macro.err.callPick', {}, false, 'macroId');
    if (s.kind === 'call' && s.macroId && !macros.some(x => x.id === s.macroId)) add(i, 'macro.err.callGone', {}, false, 'macroId');
    if (s.kind === 'read' && !okNum(s.timeoutMs, 100, 3_600_000)) add(i, 'macro.err.timeout');
    if (s.kind === 'alert' && !String(s.text || '').trim()) add(i, 'macro.err.alertEmpty', {}, false, 'text');
    if (s.kind === 'follow') {
      if (!s.region) add(i, 'macro.err.followArea', {}, false, 'region', 'area');
      const edges = s.marker && typeof s.marker === 'object';
      if (!okColour(s.target) || (edges ? !okColour(s.marker.left) || !okColour(s.marker.right) : !okColour(s.marker))) add(i, 'macro.err.colour');
      if (s.mode === 'smooth' && (!okNum(s.kp ?? 1, 0, 10) || !okNum(s.kd ?? 0.4, 0, 10))) add(i, 'macro.err.gains');
      if (!okNum(s.tolerance, 0, 255)) add(i, 'macro.err.tolerance');
      if (s.button === 'key' && !KEYS.includes(s.key)) add(i, 'macro.err.key');
      if (!okNum(s.untilGoneMs, 100, 60000)) add(i, 'macro.err.untilGone');
      if (!okNum(s.fps ?? 60, 15, 240)) add(i, 'macro.err.fps');
      if (!okNum(s.timeoutMs, 100, 3_600_000)) add(i, 'macro.err.timeout');
    }
    if (s.kind === 'waitColour' && !okColour(s.colour)) add(i, 'macro.err.colour');
    if (s.kind === 'click' && s.holdMs !== undefined && s.holdMs !== 0 && !okNum(s.holdMs, 30, 10000)) add(i, 'macro.err.holdMs', {}, false, 'holdMs');
    if (s.kind === 'clickColour') {
      if (!okColour(s.colour)) add(i, 'macro.err.colour');
      if (!okNum(s.tolerance, 0, 255)) add(i, 'macro.err.tolerance');
      if (s.minSize !== undefined && !okNum(s.minSize, 0, 10000)) add(i, 'macro.err.minSize');
      if (!okNum(s.timeoutMs, 100, 3_600_000)) add(i, 'macro.err.timeout');
      if (s.repeatUntilGoneMs !== undefined && !okNum(s.repeatUntilGoneMs, 0, 600000)) add(i, 'macro.err.untilGone');
    }
    // A setting or area the macro no longer has: the run would stop there.
    for (const v of [...Object.values(s), ...(s.test ? Object.values(s.test) : []), ...(s.marker && typeof s.marker === 'object' ? Object.values(s.marker) : [])]) {
      if (isOpt(v) && !(m.options || []).some(o => '$' + o.name === v)) add(i, 'macro.err.noOption');
      if (isArea(v) && !(m.areas || []).some(a => '@' + a.name === v)) add(i, 'macro.err.noArea', {}, false, 'region', 'area');
    }
    if (s.kind === 'if' || s.kind === 'loopUntil') {
      const test = s.test as Test;
      if (test.kind === 'image' && !test.image) add(i, 'macro.err.image', {}, false, 'image');
      if (test.kind === 'text' && !test.words && !String(test.text || '').trim()) add(i, 'macro.err.text', {}, false, 'text');
      if (test.kind === 'var' && !test.name.trim()) add(i, 'macro.err.varName');
    }
  });
  for (const o of open) add(o.at, o.kind === 'if' ? 'macro.err.ifOpen' : 'macro.err.loopOpen', {}, false, undefined, 'addEnd');
  for (const o of m.options || []) if (!o.label.trim()) add(null, 'macro.err.optLabel', {}, false, 'options');
  for (const a of m.areas || []) if (!a.label.trim()) add(null, 'macro.err.areaName', {}, false, `area:${a.name}`, 'name');
  for (const [k, i] of held) add(i, 'macro.err.neverUp', { k: keyCap(k) });
  return out;
}

/** What stops a feature from saving, and its warnings: its steps checked as the main ones are (their blocks closed and
 * keys let go within them), no If stuck in them, how often it plays; on the main steps, a key held where a feature would
 * come in. Each tied to its feature (sec). */
export function featureProblems(t: T, m: Macro): Problem[] {
  const out: Problem[] = [];
  const feats = m.features || [];
  if (!feats.length) return out;
  const name = (f: Feature) => m.options?.find(o => o.name === f.option)?.label || '…';
  for (const f of feats) {
    const lead = name(f);
    for (const p of problems(t, { ...m, name: lead, steps: f.steps, onFail: 'restart', stuckMinutes: 5 }, [])) {
      if (p.step === null) continue;
      out.push({ ...p, sec: f.id, text: `${lead}: ${p.text}` });
    }
    f.steps.forEach((s, i) => { if (s.kind === 'recover') out.push(mk(t, i, 'macro.feat.errRecover', f.id, lead)); });
    if (!f.steps.length) out.push({ ...mk(t, null, 'macro.feat.warnEmpty', f.id, lead), warn: true, field: 'feature' });
    if (f.everyMs != null && !(Number.isFinite(f.everyMs) && f.everyMs >= 1000 && f.everyMs <= 86_400_000)) out.push({ ...mk(t, null, 'macro.feat.errEvery', f.id, lead), field: 'every' });
  }
  // Main steps: a key still held at a pass's end keeps the features out.
  const held = new Set<string>();
  const ends = new Set(safeEnds(m.steps));
  m.steps.forEach((s, i) => {
    if (s.kind === 'down') held.add(s.key);
    if (s.kind === 'up') held.delete(s.key);
    if (ends.has(i) && held.size) out.push({ ...mk(t, i, 'macro.feat.warnHeld', undefined, '', { k: keyCap([...held][0]) }), warn: true });
  });
  return out;
}

const mk = (t: T, step: number | null, key: string, sec: string | undefined, name: string, vars: Record<string, string | number> = {}): Problem => {
  const bare = t(key, { ...vars, name, n: step === null ? '' : step + 1 });
  const text = step === null ? bare : `${t('macro.stepN', { n: step + 1 })}: ${bare}`;
  return { step, text: name && step !== null ? `${name}: ${text}` : text, bare, ...(sec ? { sec } : {}) };
};

/** The label of the list a text step reads ("$o1"). */
const listName = (ref: string, m?: Ctx) => m?.options?.find(o => '$' + o.name === ref)?.label || '…';

/** A run still going (it holds its window's turn, or waits for one). */
export const isLive = (r: MacroRun) => r.status === 'waiting' || r.status === 'running' || r.status === 'paused';

/** Who started the recording now under way: the macro form (its steps get them) or New macro › Record (a new form). */
export const recording = { for: null as 'editor' | 'new' | null };
