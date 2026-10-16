// What the React parts use of the page around them (RobloxManager/wwwroot/app.js hands it over as uiHost()): its words
// in the chosen language, its icons, the commands to the app, and where its buttons lead.
import { createContext, useContext, useSyncExternalStore } from 'react';

/** What the app last pushed (its state event: accounts, settings…), as app.js keeps it in S.app. */
export type AppState = Record<string, any>;

export interface Host {
  /** The interface's words for an i18n key (wwwroot/i18n.js), {placeholders} filled from vars. */
  t(key: string, vars?: Record<string, string | number>): string;
  /** The same, the "1" form when n is 1 (key1). */
  tn(key: string, n: number, vars?: Record<string, string | number>): string;
  lang: 'en' | 'vi';
  /** "en-US" or "vi-VN", for dates and capitals. */
  locale: string;
  /** An icon's drawing (the inside of its 24×24 <svg>), by its name in app.js's ICONS. */
  iconPaths(name: string): string;
  /** The app's logo (an image path inside the page). */
  logo: string;
  /** A command to the app (the bridge); failures show a toast there and resolve to undefined. */
  call(cmd: string, args?: Record<string, unknown>): Promise<unknown>;
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
