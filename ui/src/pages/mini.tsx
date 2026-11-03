// The mini window's page (mini.html in MiniWindow.cs's window): draws the running accounts (mini/MiniView.tsx) from the
// app's "mini" event, and sends back clicks ("act"), its size ("size"), a drag of its title row ("drag") and of its
// bottom edge ("resize").
import { createRef } from 'react';
import { MiniRoot, type MiniData, type MiniViewApi } from '../mini/MiniView';
import { startPage } from './page';

const view = createRef<MiniViewApi>();
startPage<MiniData>({
  event: 'mini',
  root: 'mini',
  inWindow: true,
  part: send => <MiniRoot handle={view} on={{
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h) => send('size', { w, h }),
    drag: () => send('drag'),
    resize: () => send('resize'),
  }} />,
  show: d => view.current!.show(d),
});
