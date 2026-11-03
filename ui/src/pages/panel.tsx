// A macro panel's page (panel.html in PanelWindow.cs's window): draws the panel (panel/PanelView.tsx) from the app's
// "panel" event, says a refused change from "panelError", and sends back clicks ("act"), its size ("size") and a drag of
// its title row ("drag").
import { createRef } from 'react';
import { PanelRoot } from '../panel/PanelView';
import type { PanelData, PanelError, PanelViewApi } from '../panel/panelHost';
import { startPage } from './page';

const view = createRef<PanelViewApi>();
startPage<PanelData>({
  event: 'panel',
  root: 'panel',
  inWindow: true,
  part: send => <PanelRoot handle={view} on={{
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h, pill) => send('size', { w, h, pill }),
    drag: () => send('drag'),
  }} />,
  show: d => view.current!.show(d),
  also: { panelError: (e: PanelError | null) => { if (e) view.current!.error(e); } },
});
