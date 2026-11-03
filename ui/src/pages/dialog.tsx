// The app's message box (dialog.html in Forms/Dialog.cs's window): draws the "dialog" event (dialog/DialogView.tsx), and
// sends back the choice ("act"), its size ("size") and a drag of its head ("drag").
import { createRef } from 'react';
import { DialogRoot, type DialogData, type DialogViewApi } from '../dialog/DialogView';
import { startPage } from './page';

const view = createRef<DialogViewApi>();
startPage<DialogData>({
  event: 'dialog',
  root: 'root',
  inWindow: true,
  part: send => <DialogRoot handle={view} on={{
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h) => send('size', { w, h }),
    drag: () => send('drag'),
  }} />,
  show: d => view.current!.show(d),
});
