// The top of Bloxkeep's browser windows (browser.html in Forms/WebsiteForm.cs and BrowserLoginForm.cs): draws the
// "browser" event (browser/BrowserChrome.tsx), and sends back clicks ("act") and its size ("size").
import { createRef } from 'react';
import { BrowserRoot, type BrowserChromeApi, type BrowserData } from '../browser/BrowserChrome';
import { startPage } from './page';

const view = createRef<BrowserChromeApi>();
startPage<BrowserData>({
  event: 'browser',
  root: 'root',
  inWindow: true,
  part: send => <BrowserRoot handle={view} on={{
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h) => send('size', { w, h }),
  }} />,
  show: d => view.current!.show(d),
});
