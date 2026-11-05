// A small thing under its button (Add step's list, a block's "delete it all?", New counter, the template cards): inside
// its dialog when it has one, so the dialog's keys and focus still hold; the menus' look.
import { useCallback, useEffect, useLayoutEffect, useRef, useState, type ReactNode } from 'react';
import { createPortal } from 'react-dom';

/** Something under (or over, near the bottom) its button, inside its dialog so the dialog's keys and focus still hold;
 * the menus' look. Closes on its button again, a click outside, Escape (before the dialog hears it), or a resize. */
export function Pop({ anchor, close, label, className, children, focusFirst = true }: {
  anchor: HTMLElement; close: () => void; label: string; className?: string; children: ReactNode; focusFirst?: boolean;
}) {
  const panel = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<{ left: number; top: number; max?: number } | null>(null);
  // The side it first opened on stays while its size changes (a search narrowing a list): it doesn't jump over its button.
  const side = useRef<'below' | 'above' | null>(null);
  const placeIt = useCallback(() => {
    const p = panel.current;
    if (!p) return;
    const a = anchor.getBoundingClientRect();
    // Its whole height, not what a cap left of it (a list inside may shrink to the cap rather than the panel scrolling).
    const cap = p.style.maxHeight;
    p.style.maxHeight = 'none';
    const height = p.offsetHeight, width = p.offsetWidth;
    p.style.maxHeight = cap;
    const below = a.bottom + 6, roomBelow = innerHeight - 8 - below, roomAbove = a.top - 6 - 8;
    // Never over its button: under it when it fits, else over it when that fits, else the roomier side, scrolled.
    side.current ||= height <= roomBelow ? 'below' : height <= roomAbove ? 'above' : roomBelow >= roomAbove ? 'below' : 'above';
    const room = Math.max(80, side.current === 'below' ? roomBelow : roomAbove);
    const tall = Math.min(height, room);
    const top = side.current === 'below' ? below : a.top - 6 - tall;
    const left = Math.max(8, Math.min(a.left, innerWidth - 8 - width));
    const max = height > room ? room : undefined;
    setPlace(old => old && old.left === left && old.top === top && old.max === max ? old : { left, top, max });
  }, [anchor]);
  useLayoutEffect(() => {
    placeIt();
    const watch = new ResizeObserver(() => placeIt());
    watch.observe(panel.current!);
    return () => watch.disconnect();
  }, [placeIt]);
  useEffect(() => {
    if (anchor.hasAttribute('aria-haspopup')) anchor.setAttribute('aria-expanded', 'true');
    if (focusFirst) panel.current?.querySelector<HTMLElement>('[autofocus], input, button:not(:disabled)')?.focus({ preventScroll: true });
    const key = (e: KeyboardEvent) => {
      if (e.key !== 'Escape') return;
      e.stopImmediatePropagation();
      e.preventDefault();
      close();
      anchor.focus({ preventScroll: true });
    };
    // The button's own click toggles it; any other press outside closes it.
    const press = (e: MouseEvent) => {
      const at = e.target as Node;
      if (!panel.current?.contains(at) && !anchor.contains(at)) close();
    };
    const scrolled = (e: Event) => { if (!(e.target instanceof Node && panel.current?.contains(e.target))) placeIt(); };
    addEventListener('keydown', key, true);
    addEventListener('mousedown', press, true);
    addEventListener('resize', close);
    addEventListener('scroll', scrolled, { capture: true, passive: true });
    return () => {
      if (anchor.hasAttribute('aria-haspopup')) anchor.setAttribute('aria-expanded', 'false');
      removeEventListener('keydown', key, true);
      removeEventListener('mousedown', press, true);
      removeEventListener('resize', close);
      removeEventListener('scroll', scrolled, { capture: true });
    };
  }, [anchor, close, placeIt]);
  const host = anchor.closest('.overlay') || document.body;
  return createPortal(<div className={`mc-pop${className ? ' ' + className : ''}${place?.max ? ' scroll' : ''}`} ref={panel} role="dialog" aria-label={label}
    style={place ? { left: place.left, top: place.top, maxHeight: place.max } : { opacity: 0, left: 0, top: 0 }}>{children}</div>, host);
}
