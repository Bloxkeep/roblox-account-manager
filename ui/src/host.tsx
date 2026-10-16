// What the React parts use of the page around them (RobloxManager/wwwroot/app.js hands it over as uiHost()): its words
// in the chosen language, its icons, the commands to the app, and where its buttons lead.
import { createContext, useContext, useEffect, useState, useSyncExternalStore } from 'react';

/** What the app last pushed (its state event: accounts, settings…), as app.js keeps it in S.app. */
export type AppState = Record<string, any>;

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
  /** The app's logo (an image path inside the page). */
  logo: string;
  /** A command to the app (the bridge); failures show a toast there and resolve to undefined. */
  call(cmd: string, args?: Record<string, unknown>, opts?: { silent?: boolean }): Promise<unknown>;
  /** The same command, its failure left to the caller: rejects with the app's words (no toast). */
  ask(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
  /** Keeps the page as it is for the app to bring back (app.js saveView), before something that reloads it. */
  saveView(restart?: boolean): void;
  /** Multi-Roblox on (after the risk warning) or off (app.js setMulti); whether it was done. */
  setMulti(on: boolean): Promise<boolean>;
  /** A password's strength, only a guide (app.js strengthLevel). */
  strength(password: string): 'weak' | 'ok' | 'strong';
  /** How long ago a time was, in words ("5 min ago"; app.js ago). */
  ago(iso: string): string;
  /** A Settings page was drawn (again): app.js searches it again if the search box has words in it. */
  settingsDrawn(): void;
  /** The themes by group (app.js THEMES): basic, scenery, color, style, season. */
  themes: Record<string, string[]>;
  /** The theme picked with the arrow keys and not saved yet (null when none); subscribe tells when it changes. */
  themePending(): string | null;
  /** Shows a theme at once and saves it once the keys rest (app.js previewTheme). */
  previewTheme(id: string): void;
  /** The dialog asking for a Discord webhook link; thenTurnOn: sending goes on once it is saved. */
  discordLink(thenTurnOn: boolean): void;
  /**
   * fn runs each time app.js gives the signal: "hideDiscordLink" (a shown webhook link goes: Settings or its page left,
   * the window left), "clearPassword" (typed passwords go: Settings left), "presetQuery" (the Presets search was put
   * back: presetQuery), "presetsShown" (the Presets tab was opened), "statsShown" / "statsHidden" (the Statistics tab
   * came on screen or went). Returns how to stop.
   */
  on(signal: 'hideDiscordLink' | 'clearPassword' | 'presetQuery' | 'presetsShown' | 'statsShown' | 'statsHidden', fn: () => void): () => void;
  /** The Presets tab's search, as app.js keeps it (a page restarted comes back with it), and a new one typed. */
  presetQuery(): string;
  setPresetQuery(q: string): void;
  /** "Mon–Fri 20:00", "Every day 20:00" or a one-off's date (app.js scheduleText). */
  scheduleText(schedule: { time: string; date?: string; days?: number[] }): string;
  /** "Next: Today 20:00" for a scheduled preset, empty otherwise (app.js nextRunText). */
  nextRunText(preset: { nextRun?: string }): string;
  /** The options a preset sets differently from the usual settings, in one line (app.js presetOptionsText). */
  presetOptionsText(preset: object): string;
  /** An account's picture, or its initials, as the cards draw it (app.js avatarInner: HTML). */
  avatarHtml(account: object): string;
  /** Copies an account's username, PID or authenticator code (app.js copy: the app copies; a toast says what). */
  copyField(id: string, field: string): void;
  /** Runs a preset (app.js runPreset: one at a time, asking when Roblox is busy). */
  runPreset(id: string): Promise<unknown>;
  /** Deletes a preset after asking (app.js deletePreset). */
  deletePreset(preset: object): Promise<unknown>;
  /** The preset editor for a preset (app.js presetModal). */
  editPreset(preset: object): void;
  /** The browser's own storage for this page's small choices (app.js store; never fails: the fallback instead). */
  store: { get(key: string, fallback: string): string; set(key: string, value: string): void };
  /** Short weekday names from the browser's own calendar (0 = Sunday, like the app; app.js weekdayName). */
  weekdayName(day: number): string;
  /** A number as the interface writes it (thousands separated; app.js fmtNum). */
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
  /** Focuses that control again within root, or the one now in its place (app.js refocus). */
  refocus(root: Element, key: unknown): void;
  /** Somewhere a button leads: "settings:<page or group>", "guide", "web" (roblox.com), "feedback", "github". */
  go(target: string): void;
  /** Runs fn with the button turned off and its spinner showing (app.js busy), its answer when done. */
  busy(button: HTMLElement, fn: () => Promise<unknown>): Promise<unknown>;
  /** A message in the corner (app.js toast). */
  toast(text: string, kind?: 'info' | 'ok' | 'warn' | 'error'): void;
  /** The app's state now, and a way to hear when a new one comes (returns how to stop listening). */
  state(): AppState;
  subscribe(listener: () => void): () => void;
}

/** The app's state, drawn again whenever the app pushes a new one. */
export function useAppState(): AppState {
  const { state, subscribe } = useHost();
  return useSyncExternalStore(subscribe, state);
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

/**
 * How long ago a time was, in words, kept fresh while shown. tpl, if given, places it in a sentence: that key's {t}
 * ("Last backup: {t}"). Not marked data-ago: app.js's patchTimes would write over React's text.
 */
export function Ago({ iso, tpl }: { iso: string; tpl?: string }) {
  const { t, ago } = useHost();
  const [, tick] = useState(0);
  useEffect(() => {
    const timer = setInterval(() => tick(n => n + 1), 30000);
    return () => clearInterval(timer);
  }, []);
  return <span>{tpl ? t(tpl, { t: ago(iso) }) : ago(iso)}</span>;
}
