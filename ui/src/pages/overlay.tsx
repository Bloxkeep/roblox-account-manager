// The overlay's page (overlay.html in OverlayWindow.cs's window, over a game or a monitor): draws the macro's dots and
// areas, or the frozen picture to pick on (overlay/OverlayView.tsx), from the app's "overlay" event, and sends back where
// things went ("act": dot, area, pick, done, cancel) in parts of the picture.
import { createRef } from 'react';
import { OverlayRoot, type OverlayData, type OverlayViewApi } from '../overlay/OverlayView';
import { startPage } from './page';

const view = createRef<OverlayViewApi>();
startPage<OverlayData & { language?: string; theme?: string; customTheme?: unknown }>({
  event: 'overlay',
  root: 'overlay',
  // In its window the dimming is the window's own (see-through as a whole); a browser preview draws it (app.css).
  inWindow: true,
  part: send => <OverlayRoot handle={view} on={{ act: (what, args = {}) => send('act', { what, ...args }) }} />,
  before: d => { document.body.classList.toggle('ov-dim', d.mode === 'edit' || d.mode === 'areas'); },
  show: d => view.current!.show(d),
});
