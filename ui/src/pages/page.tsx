// The one glue of the app's small pages, each a small window's whole face or top: the tray's (tray.html, TrayWindow.cs),
// a macro's panel (panel.html, PanelWindow.cs), the mini window (mini.html, MiniWindow.cs), the overlay (overlay.html,
// OverlayWindow.cs), the message box (dialog.html, Forms/Dialog.cs) and the top of the browser windows (browser.html,
// Forms/WebsiteForm.cs, BrowserLoginForm.cs). Each page has a bundle of its own (ui/build.mjs: ui-<page>.js, from
// pages/<page>.tsx) that draws its part with startPage: the app's event of the page's name drawn, and clicks ("act"), its
// size ("size") and the like sent back in the same messages as the app's own page.
import type { ReactNode } from 'react';
// The icons, the logo and the custom theme's colours: the same file the app's page loads (window.BloxkeepShared).
import '../../../RobloxManager/wwwroot/shared.js';
import type { Host } from '../host';
import { mount } from './mount';

type Dict = Record<string, Record<string, string>>;
interface Shared {
  ICONS: Record<string, string>;
  LOGO: string;
  customVars(theme: object): { tone: 'dark' | 'light'; vars: Record<string, string> };
}
interface Bridge {
  call(cmd: string, args: Record<string, unknown>): void;
  attach(listener: (name: string, data: any) => void): void;
}
type PageWindow = Window & {
  BloxkeepShared: Shared;
  chrome?: { webview?: { postMessage(m: unknown): void; addEventListener(type: 'message', fn: (e: { data: any }) => void): void } };
  /** The preview pages' fake app (ui/dev/<page>-mock.js). */
  __mockBridge?: Bridge;
};

/** A command to the app. */
export type Send = (cmd: string, args?: Record<string, unknown>) => void;

/** What a small page is: see startPage. */
export interface PageOptions<D> {
  /** The app's event this page draws ("tray", "panel"…). */
  event: string;
  /** The element's id the part draws into (tray, panel, mini, overlay, root). */
  root: string;
  /** In its window Windows draws the corners and the hairline (a browser preview draws the card's own): the page says
   * so with "in-window" on <html>. The tray's card draws its own. */
  inWindow: boolean;
  /** The part, drawn once, given the way to send commands to the app. */
  part(send: Send): ReactNode;
  /** Draws an event of the page's name (through the part's handle). */
  show(d: D): void;
  /** Other events of the app this page hears ("panelError"). */
  also?: Record<string, (data: any) => void>;
  /** Before each drawing (the overlay dims the page while dots or areas are moved). */
  before?(d: D): void;
}

/**
 * Starts a small page: its part drawn into its element, then each of the app's events of its name drawn in the app's
 * language and theme. Says "ready" once drawn, so the app sends the first event.
 */
export function startPage<D extends { language?: string; theme?: string; customTheme?: unknown }>(o: PageOptions<D>) {
  const w = window as unknown as PageWindow;
  const { ICONS, LOGO, customVars } = w.BloxkeepShared;
  const DICT = window.I18N as Dict;
  let lang = 'en';
  // A "1" key the language lacks (Vietnamese has no plural) is its plural key in that language, never English.
  function t(key: string, vars?: Record<string, string | number>): string {
    const own = DICT[lang] || {};
    let s = own[key] ?? (lang !== 'en' && key.endsWith('1') ? own[key.slice(0, -1)] : undefined) ?? DICT.en[key] ?? key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? String(vars[k]) : m));
    return s;
  }
  const tn = (key: string, n: number, vars: Record<string, string | number> = {}) => t(n === 1 && `${key}1` in DICT[lang] ? `${key}1` : key, { n, ...vars });
  const locale = () => (lang === 'vi' ? 'vi-VN' : 'en-US');
  const fmtNum = (n: number) => new Intl.NumberFormat(locale()).format(n);
  const fmtMb = (mb: number) => mb >= 1024 ? `${new Intl.NumberFormat(locale(), { maximumFractionDigits: 1 }).format(mb / 1024)} GB` : `${Math.round(mb)} MB`;

  const wv = w.chrome && w.chrome.webview, mock = w.__mockBridge;
  if (wv && o.inWindow) document.documentElement.classList.add('in-window');
  let seq = 0;
  const send: Send = (cmd, args = {}) => {
    if (wv) wv.postMessage({ id: ++seq, cmd, args });
    else if (mock) mock.call(cmd, args);
  };

  // A part of the app's page in its own window: only the host's words, numbers and pictures are there.
  const host = {
    t, tn, fmtNum, fmtMb, get lang() { return lang; }, get locale() { return locale(); }, logo: LOGO, iconPaths: (name: string) => ICONS[name] || '',
  } as unknown as Host;
  mount(document.getElementById(o.root)!, host, o.part(send), false);

  /** The app's theme, and a custom one's colours (without its picture: these pages are plain cards). */
  function theme(id: string | undefined, custom: unknown) {
    const root = document.documentElement;
    root.dataset.theme = id || 'dark';
    const vars = id === 'custom' && custom ? customVars({ ...(custom as object), picture: null }) : null;
    if (vars) {
      root.dataset.tone = vars.tone;
      Object.entries(vars.vars).forEach(([k, v]) => root.style.setProperty(k, v));
    } else {
      delete root.dataset.tone;
      root.removeAttribute('style');
    }
  }

  let shownTheme: string | null = null;
  function show(d: D) {
    if (!d) return;
    if (d.language && DICT[d.language]) lang = d.language;
    document.documentElement.lang = lang;
    const look = JSON.stringify([d.theme, d.customTheme]);
    if (look !== shownTheme) { shownTheme = look; theme(d.theme, d.customTheme); }
    o.before?.(d);
    o.show(d);
  }
  const hear = (name: string, data: any) => {
    if (name === o.event) show(data);
    else if (o.also && Object.hasOwn(o.also, name)) o.also[name](data);
  };
  if (wv) wv.addEventListener('message', e => { const m = e.data; if (m && m.type === 'event') hear(m.name, m.data); });
  else if (mock) mock.attach(hear);
  send('ready');
}
