// Drawing a React part into an element of a page: the app's own page (main.tsx, for app.js) and the small pages
// (page.tsx) both draw this way.
import { StrictMode, type ReactNode } from 'react';
import { flushSync } from 'react-dom';
import { createRoot } from 'react-dom/client';
import { HostContext, type Host } from '../host';

/** Draws into el now (app.js focuses the first control right after, so it must be there). keep: keepFocus on redraws;
 * a frame drawn once (a dialog's, a menu) needs none. */
export function mount(el: HTMLElement, host: Host, node: ReactNode, keep = true): () => void {
  // Marked as React's: the page-wide fallback for broken game pictures (app.js) leaves its elements alone.
  el.dataset.react = '';
  const root = createRoot(el);
  flushSync(() => root.render(<StrictMode><HostContext.Provider value={host}>{node}</HostContext.Provider></StrictMode>));
  const stop = keep ? keepFocus(el, host) : () => {};
  // The element may be the page's own (#app, the lock screen's): what app.js draws there next isn't React's.
  return () => { stop(); root.unmount(); delete el.dataset.react; };
}

// A control with the focus that React draws away (a list redrawn, a row moved: Edge drops a moved element's focus too)
// hands it back: to itself if still there, else to the same control drawn anew or the one in its place, as app.js's
// setHtml does. Only a loss the redraw itself caused is undone; focus the person moved elsewhere stays.
function keepFocus(el: HTMLElement, host: Host): () => void {
  let last: Element | null = null, key: unknown = null, lost: Element | null = null;
  const focused = (e: FocusEvent) => { last = e.target as Element; key = host.focusKey(last, el); };
  // A redraw that takes the element out tells the observer in a microtask; a loss nothing redrew is forgotten after.
  const left = (e: FocusEvent) => {
    if (e.target !== last || e.relatedTarget) return;
    lost = last;
    setTimeout(() => { if (lost === e.target) lost = null; });
  };
  const watch = new MutationObserver(records => {
    const was = lost;
    const from = was && records.find(r => [...r.removedNodes].some(n => n === was || n.contains(was)));
    if (!was || !from) return;
    lost = null;
    const active = document.activeElement;
    if (active && active !== document.body) return;
    if (was.isConnected && !(was as HTMLButtonElement).disabled) return (was as HTMLElement).focus({ preventScroll: true });
    host.refocus(from.target.isConnected ? from.target as Element : el, key);
  });
  el.addEventListener('focusin', focused);
  el.addEventListener('focusout', left);
  watch.observe(el, { childList: true, subtree: true });
  return () => { el.removeEventListener('focusin', focused); el.removeEventListener('focusout', left); watch.disconnect(); };
}
