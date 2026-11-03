// The macro panel's protocol with the app (PanelWindow.cs and AppController.Panels.cs): what the app sends the panel's
// page (the "panel" event: the macro, what it does now, the theme and the language; "panelError": a change refused), and
// what the page sends back ("ready", "size", "drag", "act"). The page's glue (ui-panel.js, pages/panel.tsx) passes them through; the parts here draw them.

/** A setting the player may change, as the app sends it: its value, and the value in words as the app shows it. */
export interface PanelOption {
  name: string;
  label: string;
  kind: 'number' | 'colour' | 'check' | 'key' | 'choice' | 'point' | 'list' | 'heading';
  tab?: string | null;
  value: number | string | string[] | { x: number; y: number } | null;
  choices?: string[] | null;
  /** The value in words, in the app's language ("0,5 giây", "2 of 5", "Space"). */
  shown: string;
  /** A number: its range, what − and + add, what it is divided by to show it, and its unit as written after it. */
  min?: number | null;
  max?: number | null;
  step?: number | null;
  scale?: number | null;
  suffix?: string | null;
  minShown?: string | null;
  maxShown?: string | null;
}

export interface PanelMacro {
  id: string;
  name: string;
  /** Ctrl+Alt+<digit> runs it, when set. */
  hotkey: number | null;
  /** Its tabs, in order (fewer than two: none). */
  tabs: string[];
  areasCount: number;
  /** The variables it counts, with the maker's label for each. */
  counters: { name: string; label: string }[];
  options: PanelOption[];
}

export type PanelState = 'ready' | 'noGame' | 'running' | 'paused' | 'stopped' | 'recording';

export interface PanelNow {
  state: PanelState;
  /** Its runs going on: whose window (as names are shown), how it goes, and a pause's reason in words. */
  runs: { name: string; status: 'waiting' | 'running' | 'recovering' | 'paused'; reason: string | null }[];
  counters: Record<string, number>;
  /** Since when the first run goes on (the page ticks the clock). */
  startedAt: string | null;
  /** The last run's error, until the next run or 10 minutes: its reason in words and the step it stopped at (0-based). */
  lastError: { text: string; step: number } | null;
  /** Its areas are being moved on the game now. */
  editingAreas: boolean;
  /** Bloxkeep's hotkeys are on (Ctrl+Alt+<digit> is shown only then). */
  hotkeysOn: boolean;
  /** Run has a window to run in. */
  hasGame: boolean;
  /** Any Roblox window is open (to pick a spot or move areas on). */
  hasWindow: boolean;
  /** A recording going on (every run waits meanwhile). */
  recording: { account: string } | null;
  /** "Reset all" can still be undone. */
  undoable: boolean;
  /** Some value differs from how the macro was made. */
  changed: boolean;
}

/** The app's "panel" event. */
export interface PanelData {
  macro: PanelMacro | null;
  now: PanelNow | null;
  theme: string;
  customTheme: unknown;
  language: string;
  /** Folded to the pill (the window keeps it across a page restart). */
  pill: boolean;
}

/** A change refused (under its row; name null: the status line). */
export interface PanelError { name: string | null; text: string }

/** What the panel's page sends the app. */
export interface PanelOn {
  /** run, stop, set {name, value} or {name, key}, nudge {name, by}, typed {name, text}, reset, undoReset, pickColour {name},
   * pickPoint {name}, areas, hide, edit {step?}, snap, away (the pointer left after a change: the keyboard may go back). */
  act(what: string, args?: Record<string, unknown>): void;
  /** The page's size in CSS pixels (its natural height, rows not scrolled), and whether it is the pill. */
  size(width: number, height: number, pill: boolean): void;
  /** The title row is being dragged: the window moves with the pointer. */
  drag(): void;
}

/** What the panel page (ui-panel.js) holds: show draws the app's latest "panel" event, error a refused change. */
export interface PanelViewApi {
  show(data: PanelData): void;
  error(e: PanelError): void;
}

/** h:mm:ss since a time. */
export function elapsed(since: string | null, now: number): string {
  if (!since) return '';
  const s = Math.max(0, Math.floor((now - Date.parse(since)) / 1000));
  const h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60, x = s % 60;
  return `${h}:${String(m).padStart(2, '0')}:${String(x).padStart(2, '0')}`;
}
