// The tray's page (tray.html in TrayWindow.cs's window): draws the quick panel or menu (Tray.tsx) from the app's "tray"
// event, and sends back clicks ("act") and its size ("size").
import { createRef } from 'react';
import { TrayView, type TrayState, type TrayViewApi } from '../Tray';
import { startPage } from './page';

const view = createRef<TrayViewApi>();
startPage<{ mode: 'panel' | 'menu'; state: TrayState; opened: number; language?: string; theme?: string; customTheme?: unknown }>({
  event: 'tray',
  root: 'tray',
  inWindow: false,
  part: send => <TrayView handle={view} api={{
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h, opened) => send('size', { w, h, opened }),
  }} />,
  show: d => view.current!.show(d.mode, d.state, d.opened),
});
