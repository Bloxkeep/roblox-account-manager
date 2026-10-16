// The parts of Bloxkeep's interface drawn with React, for app.js to put where it wants them: each draws into an element
// app.js made (a dialog's body…) and returns how to take it down again. While the move to React goes on, app.js still
// draws the rest; it reaches these as window.BloxkeepUI (ui.js loads before it).
import { StrictMode, createRef, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { Feedback, type FeedbackApi } from './Feedback';
import { HostContext, type Host } from './host';
import { JoinPlayer, type JoinPlayerApi } from './JoinPlayer';
import { MultiHelp } from './MultiHelp';
import { PickList, type PickItem, type PickListApi } from './PickList';
import { AppearancePage } from './settings/AppearancePage';
import { BackupPage } from './settings/BackupPage';
import { DiscordPage } from './settings/DiscordPage';
import { LaunchPage } from './settings/LaunchPage';
import { NotifyPage } from './settings/NotifyPage';
import { PerfPage } from './settings/PerfPage';
import { RobloxPage } from './settings/RobloxPage';
import { SecurityPage } from './settings/SecurityPage';
import { UpdatesPage } from './settings/UpdatesPage';
import { WhatsNew, type Release } from './WhatsNew';

/** Draws into el now (app.js focuses the first control right after, so the content must be there), until unmount. */
function mount(el: HTMLElement, host: Host, node: ReactNode): () => void {
  const root = createRoot(el);
  flushSync(() => root.render(<StrictMode><HostContext.Provider value={host}>{node}</HostContext.Provider></StrictMode>));
  const stop = keepFocus(el, host);
  return () => { stop(); root.unmount(); };
}

/**
 * A control with the focus that is drawn away (a list redrawn without it, Play turned into Stop) hands the focus on to
 * the same control drawn anew, or the one now in its place, as app.js's setHtml does: not back to the top of the window.
 */
function keepFocus(el: HTMLElement, host: Host): () => void {
  let last: Element | null = null, key: unknown = null;
  const focused = (e: FocusEvent) => { last = e.target as Element; key = host.focusKey(last, el); };
  const watch = new MutationObserver(() => {
    if (!last || last.isConnected) return;
    const active = document.activeElement;
    last = null;
    if (!active || active === document.body) host.refocus(el, key);
  });
  el.addEventListener('focusin', focused);
  watch.observe(el, { childList: true, subtree: true });
  return () => { el.removeEventListener('focusin', focused); watch.disconnect(); };
}

const BloxkeepUI = {
  /** What's new into the body of the dialog app.js opened for it; close closes that dialog. */
  whatsNew(el: HTMLElement, host: Host, releases: Release[], close: () => void) {
    return mount(el, host, <WhatsNew releases={releases} close={close} />);
  },
  /** Send feedback's form into its dialog's body; the footer's buttons use form (FeedbackApi). */
  feedback(el: HTMLElement, host: Host, locked: boolean) {
    const form = createRef<FeedbackApi>();
    const unmount = mount(el, host, <Feedback locked={locked} api={form} />);
    return { unmount, form: form.current! };
  },
  /** Join a player's form into its dialog's body; "Find and join" (submit) uses form (JoinPlayerApi). */
  joinPlayer(el: HTMLElement, host: Host, count: number, submit: () => void) {
    const form = createRef<JoinPlayerApi>();
    const unmount = mount(el, host, <JoinPlayer count={count} submit={submit} api={form} />);
    return { unmount, form: form.current! };
  },
  /** Multi-Roblox's help into its dialog's body; enable turns it on (app.js closes the dialog first). */
  multiHelp(el: HTMLElement, host: Host, enable: () => void) {
    return mount(el, host, <MultiHelp enable={enable} />);
  },
  /** A Settings page into its section (app.js makes the section; the page's title comes from there too). */
  settingsPage(el: HTMLElement, host: Host, page: string, title: string) {
    const pages: Record<string, ReactNode> = {
      launch: <LaunchPage title={title} />, roblox: <RobloxPage title={title} />, perf: <PerfPage title={title} />, notify: <NotifyPage title={title} />,
      discord: <DiscordPage title={title} />, appearance: <AppearancePage title={title} />, security: <SecurityPage title={title} />, backup: <BackupPage title={title} />, updates: <UpdatesPage title={title} />,
    };
    return mount(el, host, pages[page]);
  },
  /** A list to tick into a picker's body; the footer's buttons read list.values(). */
  pickList(el: HTMLElement, host: Host, text: string, items: PickItem[]) {
    const list = createRef<PickListApi>();
    const unmount = mount(el, host, <PickList text={text} items={items} api={list} />);
    return { unmount, list: list.current! };
  },
};

declare global {
  interface Window { BloxkeepUI: typeof BloxkeepUI }
}
window.BloxkeepUI = BloxkeepUI;
