// What the React parts use of the page around them (app.js uiHost()): words, icons, commands to the app, link targets.
import { createContext, useContext, useEffect, useState, useSyncExternalStore } from 'react';

/** What the app last pushed (its state event: accounts, settings…), as app.js keeps it in S.app. */
export type AppState = Record<string, any>;

/** Settings › Appearance › Custom: background and accent (#rrggbb), fade over the picture 0–90, blur 0–30 px, the picture (or video). */
export interface CustomTheme { background: string; accent: string; fade: number; blur?: number; picture: string | null; video?: boolean }
export interface SavedTheme { id: string; name: string; look: CustomTheme }

/** The live figures the app sends every few seconds (its metrics event): Roblox's windows and the PC's memory. */
export type Metrics = {
  accounts: { id: string; ramMb: number }[]; robloxCount: number; monitorCount?: number;
  totalMb: number; systemMb: number; availableMb?: number; usedPct?: number;
};
/** The page's own state, kept by app.js (S): the tab shown, the live figures… A new object whenever any of it changes. */
export interface PageState {
  view: string;
  /** The accounts chosen (Play, Close… act on them), and the Accounts tab's search, status chip, group, layout, sort. */
  selected: Set<string>;
  query: string; filter: string; group: string; layout: 'cards' | 'list'; sort: string;
  /** The cards have been shown once (they fade in only the first time). */
  introShown: boolean;
  /** The game in the dock's box, as the app found it ({ loading } meanwhile), and what a launch is doing now. */
  game: Record<string, any> | null;
  progress: string | null;
  /** Something is in the game box (Play goes there; without, it opens Roblox's home screen). */
  hasGame: boolean;
  /** The game box's list (app.js P): open, browsing a tab or showing a search's results, the row chosen with the keys. */
  picker: { open: boolean; mode: 'browse' | 'search'; tab: string; lists: Record<string, any[]> | null; results: any[]; query: string; index: number; loading: boolean; refreshing?: boolean };
}

export interface Host {
  /** The interface's words for an i18n key (wwwroot/i18n.js), {placeholders} filled from vars. */
  t(key: string, vars?: Record<string, string | number>): string;
  /** The same, the "1" form when n is 1 (key1). */
  tn(key: string, n: number, vars?: Record<string, string | number>): string;
  /** The words for a key in a given language, whichever is chosen (each language's own name in the switch). */
  tIn(lang: 'en' | 'vi', key: string): string;
  lang: 'en' | 'vi';
  /** "en-US" or "vi-VN", for dates and capitals. */
  locale: string;
  /** An icon's drawing (the inside of its 24×24 <svg>), by its name in app.js's ICONS. */
  iconPaths(name: string): string;
  logo: string;
  /** A command to the app (the bridge); failures show a toast there and resolve to undefined. */
  call(cmd: string, args?: Record<string, unknown>, opts?: { silent?: boolean }): Promise<unknown>;
  /** The same command, its failure left to the caller: rejects with the app's words (no toast). */
  ask(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Keeps the page as it is for the app to bring back (app.js saveView), before something that reloads it. */
  saveView(restart?: boolean): void;
  /** Multi-Roblox on (after the risk warning) or off (app.js setMulti); whether it was done. */
  setMulti(on: boolean): Promise<boolean>;
  strength(password: string): 'weak' | 'ok' | 'strong';
  ago(iso: string): string;
  /** A Settings page was drawn (again): app.js searches it again if the search box has words in it. */
  settingsDrawn(): void;
  /** The themes by group (app.js THEMES): basic, scenery, color, style, season. */
  themes: Record<string, string[]>;
  /** The theme picked with the arrow keys and not saved yet (null when none); subscribe tells when it changes. */
  themePending(): string | null;
  /** Shows a theme at once and saves it once the keys rest (app.js previewTheme). */
  previewTheme(id: string): void;
  /** Settings › Appearance › Custom: its colours and picture as shown now (a change not saved yet included); subscribe tells when they change. */
  customTheme(): CustomTheme | null;
  customVars(theme: CustomTheme): { tone: 'dark' | 'light'; vars: Record<string, string>; swatch: string };
  /** Shows a change to the custom theme at once and saves it once the changes rest (app.js previewCustom). */
  previewCustom(change: Partial<CustomTheme>): void;
  /** The dialog asking for a Discord webhook link; thenTurnOn: sending goes on once it is saved. */
  discordLink(thenTurnOn: boolean): void;
  /** fn runs on each app.js signal: hideDiscordLink (Settings, its page or the window left), clearPassword (Settings
   * left), presetQuery (its search put back), presetsShown, statsShown / statsHidden, settled (after an unlock or for a
   * new page: tabs not open yet get ready). Returns how to stop. */
  on(signal: 'hideDiscordLink' | 'clearPassword' | 'presetQuery' | 'presetsShown' | 'statsShown' | 'statsHidden' | 'settled', fn: () => void): () => void;
  /** The Presets tab's search, as app.js keeps it (a page restarted comes back with it), and a new one typed. */
  presetQuery(): string;
  setPresetQuery(q: string): void;
  /** "Mon–Fri 20:00", "Every day 20:00" or a one-off's date (app.js scheduleText). */
  scheduleText(schedule: { time: string; date?: string; days?: number[] }): string;
  nextRunText(preset: { nextRun?: string }): string;
  /** The options a preset sets differently from the usual settings, in one line (app.js presetOptionsText). */
  presetOptionsText(preset: object): string;
  /** Copies an account's username, PID or authenticator code (app.js copy: the app copies; a toast says what). */
  copyField(id: string, field: string): void;
  /** Runs a preset (app.js runPreset: one at a time, asking when Roblox is busy). */
  runPreset(id: string): Promise<unknown>;
  deletePreset(preset: object): Promise<unknown>;
  editPreset(preset: object): void;
  /** The browser's own storage for this page's small choices (app.js store; never fails: the fallback instead). */
  store: { get(key: string, fallback: string): string; set(key: string, value: string): void };
  /** Short weekday names from the browser's own calendar (0 = Sunday, like the app; app.js weekdayName). */
  weekdayName(day: number): string;
  fmtNum(n: number): string;
  /** A link, Place ID or roblox:// address rather than a name to search (app.js looksLikeTarget). */
  looksLikeTarget(value: string): boolean;
  /** The game lists (recent, pinned, favourites, popular) the dock's picker already loaded, or null; and loading them. */
  gameListsCached(): object | null;
  loadGameLists(): Promise<object | undefined>;
  /** A search's results from the last few minutes (shared with the dock's box), or null; and keeping new ones. */
  searchCached(query: string): unknown[] | null;
  rememberSearch(query: string, rows: unknown[]): void;
  /** Where a control is (app.js focusKey: what it does, its row, its place), to find it, or one in its place, again. */
  focusKey(node: Element, root: Element): unknown;
  refocus(root: Element, key: unknown): void;
  /** Somewhere a button leads: "settings:<page or group>", "guide", "web" (roblox.com), "feedback", "github". */
  go(target: string): void;
  /** Runs fn with the button turned off and its spinner showing (app.js busy), its answer when done. */
  busy(button: HTMLElement, fn: () => Promise<unknown>): Promise<unknown>;
  toast(text: string, kind?: 'info' | 'ok' | 'warn' | 'error'): void;
  /** The app's state now, and a way to hear when a new one comes (returns how to stop listening). */
  state(): AppState;
  subscribe(listener: () => void): () => void;
  /** The page's own state now (app.js S), and a way to hear when it changes. */
  page(): PageState;
  subscribePage(listener: () => void): () => void;
  /** The live figures now (the last metrics event), and a way to hear when new ones come. */
  metrics(): Metrics;
  subscribeMetrics(listener: () => void): () => void;
  fmtMb(mb: number): string;
  /** How long since a time, "47 min" or "2 h 5 min" (app.js duration). */
  duration(iso: string): string;
  /** Two names in the language's order, "Farm 2" before "Farm 10" (app.js compareText). */
  compareText(a: string, b: string): number;
  /** The accounts the Accounts tab shows now: its search, chip and group, in its sort (app.js visible). */
  visible(): Record<string, any>[];
  /** The cards have been shown: later drawings don't fade them in again. */
  introShown(): void;
  /** Makes one card the list's Tab stop (the card with the focus, else the first selected, else the first; app.js setTabStop). */
  setTabStop(box: Element, stop?: Element | null): void;
}

export function useAppState(): AppState {
  const { state, subscribe } = useHost();
  return useSyncExternalStore(subscribe, state);
}

/** A part of the page's own state, drawn again when that part changes (pick returns the same object until then). */
export function usePage<T>(pick: (page: PageState) => T): T {
  const { page, subscribePage } = useHost();
  return useSyncExternalStore(subscribePage, () => pick(page()));
}

export function useMetrics(): Metrics {
  const { metrics, subscribeMetrics } = useHost();
  return useSyncExternalStore(subscribeMetrics, metrics);
}

export const HostContext = createContext<Host | null>(null);

export function useHost(): Host {
  const host = useContext(HostContext);
  if (!host) throw new Error('A React part was drawn without the page (HostContext)');
  return host;
}

/** One of the page's icons, drawn the way app.js draws them (<svg class="i …">), so the stylesheet treats it the same. */
export function Icon({ name, className = '' }: { name: string; className?: string }) {
  const { iconPaths } = useHost();
  return <svg className={`i ${className}`} viewBox="0 0 24 24" aria-hidden="true" dangerouslySetInnerHTML={{ __html: iconPaths(name) }} />;
}

/** How long ago a time was, kept fresh while shown; tpl's {t} places it in a sentence ("Last backup: {t}").
 * Not data-ago: app.js's patchTimes would write over React's text. */
export function Ago({ iso, tpl }: { iso: string; tpl?: string }) {
  const { t, ago } = useHost();
  const [, tick] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => tick(n => n + 1), 30000);
    return () => clearInterval(timer);
  }, []);
  return <span>{tpl ? t(tpl, { t: ago(iso) }) : ago(iso)}</span>;
}
