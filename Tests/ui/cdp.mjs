// A small headless-Edge driver over the Chrome DevTools Protocol. Node 22+ (global WebSocket and fetch): no packages.
// Edge is on every Windows 10/11 PC and on GitHub's Windows machines.
import { spawn } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

export const sleep = ms => new Promise(r => setTimeout(r, ms));

function findEdge() {
  const candidates = [
    process.env.EDGE_PATH,
    'C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe',
    'C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe',
  ].filter(Boolean);
  const found = candidates.find(p => fs.existsSync(p));
  if (!found) throw new Error(`Microsoft Edge not found (looked in ${candidates.join(', ')}); set EDGE_PATH`);
  return found;
}

/** Starts Edge with a throwaway profile and returns a driver for its one tab. */
export async function launch({ width = 1280, height = 800, scale = 1, locale = 'en-US', timezone = 'Asia/Ho_Chi_Minh' } = {}) {
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'bloxkeep-ui-'));
  // No password manager: where it's on, Edge marks a new-password box aria-autocomplete="list" itself, which isn't
  // the page's doing and differs from PC to PC (it did between this PC and GitHub's).
  fs.mkdirSync(path.join(profile, 'Default'));
  fs.writeFileSync(path.join(profile, 'Default', 'Preferences'), JSON.stringify({
    credentials_enable_service: false, profile: { password_manager_enabled: false }, autofill: { enabled: false, profile_enabled: false, credit_card_enabled: false },
  }));
  // Port 0: Edge picks a free one and writes it into the profile (DevToolsActivePort), so several can run side by side.
  const proc = spawn(findEdge(), ['--headless=new', '--remote-debugging-port=0', `--user-data-dir=${profile}`,
    '--hide-scrollbars', '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--font-render-hinting=none',
    '--disable-features=msEdgeSidebarV2,msUndersideButton', '--disable-background-timer-throttling',
    `--window-size=${width},${height}`, `--lang=${locale}`, 'about:blank'], { stdio: 'ignore' });
  let page;
  for (let i = 0; i < 150 && !page; i++) {
    await sleep(150);
    try {
      const port = fs.readFileSync(path.join(profile, 'DevToolsActivePort'), 'utf8').split('\n')[0].trim();
      const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
      page = list.find(t => t.type === 'page');
    } catch { /* not up yet */ }
  }
  if (!page) throw new Error('Edge did not start');
  const ws = new WebSocket(page.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = rej; });
  let id = 0;
  const waiting = new Map(), listeners = [];
  ws.onmessage = m => {
    const msg = JSON.parse(m.data);
    if (msg.id && waiting.has(msg.id)) {
      const { res, rej } = waiting.get(msg.id);
      waiting.delete(msg.id);
      msg.error ? rej(new Error(msg.error.message)) : res(msg.result);
    } else if (msg.method) listeners.forEach(f => f(msg));
  };
  // Edge gone (closed, crashed): what was still waiting for an answer fails instead of waiting forever.
  ws.onclose = () => {
    for (const { rej } of waiting.values()) rej(new Error('Edge closed'));
    waiting.clear();
  };
  const send = (method, params = {}) => new Promise((res, rej) => {
    if (ws.readyState !== WebSocket.OPEN) return rej(new Error('Edge closed'));
    const i = ++id;
    waiting.set(i, { res, rej });
    ws.send(JSON.stringify({ id: i, method, params }));
  });
  await send('Page.enable');
  await send('Runtime.enable');
  const problems = [];
  listeners.push(m => {
    if (m.method === 'Runtime.consoleAPICalled' && m.params.type === 'error')
      problems.push(`console.error: ${m.params.args.map(a => a.value ?? a.description).join(' ')}`);
    if (m.method === 'Runtime.exceptionThrown')
      problems.push(`exception: ${m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text}`);
  });
  const b = {
    send, problems,
    /** fn hears every event the page sends (a screencast's frames, say). */
    on: fn => listeners.push(fn),
    async goto(url) {
      const loaded = new Promise(res => {
        const f = m => { if (m.method === 'Page.loadEventFired') { listeners.splice(listeners.indexOf(f), 1); res(); } };
        listeners.push(f);
      });
      await send('Page.navigate', { url });
      await loaded;
    },
    async eval(expr) {
      const r = await send('Runtime.evaluate', { expression: expr, awaitPromise: true, returnByValue: true });
      if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description || r.exceptionDetails.text);
      return r.result.value;
    },
    /** Key presses as Edge gets them from a keyboard: key is e.g. 'Escape', 'Enter', 'k' with modifiers ['ctrl']. */
    async key(key, modifiers = []) {
      const bits = { alt: 1, ctrl: 2, meta: 4, shift: 8 };
      const mod = modifiers.reduce((m, k) => m | bits[k], 0);
      const named = { Escape: 27, Enter: 13, Tab: 9, ArrowDown: 40, ArrowUp: 38, ArrowLeft: 37, ArrowRight: 39, Backspace: 8, Delete: 46, Home: 36, End: 35, ' ': 32 };
      const code = named[key] ?? key.toUpperCase().charCodeAt(0);
      // Enter types "\r" as a keyboard does: without it, Edge neither submits a form nor ends a text box's change.
      const text = key === 'Enter' ? '\r' : key.length === 1 && !(mod & 2) ? key : undefined;
      const base = { key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, windowsVirtualKeyCode: code, modifiers: mod };
      await send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', ...base, text });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
    },
    async type(text) {
      await send('Input.insertText', { text });
    },
    /** A mouse click at (x, y): { button: 'right' }, { double: true }, { modifiers: ['shift'] }. */
    async click(x, y, { button = 'left', double = false, modifiers = [] } = {}) {
      const bits = { alt: 1, ctrl: 2, meta: 4, shift: 8 };
      const mod = modifiers.reduce((m, k) => m | bits[k], 0);
      await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x, y, modifiers: mod });
      for (let n = 1; n <= (double ? 2 : 1); n++)
        for (const type of ['mousePressed', 'mouseReleased'])
          await send('Input.dispatchMouseEvent', { type, x, y, button, clickCount: n, modifiers: mod });
    },
    /**
     * A drag with the mouse from (x1, y1) to (x2, y2), as a person makes it: Edge hands the drag to us (no window of
     * its own to drag in, headless), and we take it over the page and drop it there.
     */
    async drag(x1, y1, x2, y2) {
      await send('Input.setInterceptDrags', { enabled: true });
      const started = new Promise(res => {
        const f = m => { if (m.method === 'Input.dragIntercepted') { listeners.splice(listeners.indexOf(f), 1); res(m.params.data); } };
        listeners.push(f);
      });
      await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x1, y: y1 });
      await send('Input.dispatchMouseEvent', { type: 'mousePressed', x: x1, y: y1, button: 'left', buttons: 1, clickCount: 1 });
      for (let i = 1; i <= 4; i++) await send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: x1, y: y1 + 3 * i, button: 'left', buttons: 1 });
      const data = await Promise.race([started, sleep(2000).then(() => null)]);
      try {
        if (!data) throw new Error('the drag did not start');
        await send('Input.dispatchDragEvent', { type: 'dragEnter', x: x1, y: y1 + 12, data });
        for (let i = 1; i <= 6; i++) await send('Input.dispatchDragEvent', { type: 'dragOver', x: x1 + (x2 - x1) * i / 6, y: y1 + 12 + (y2 - y1 - 12) * i / 6, data });
        await send('Input.dispatchDragEvent', { type: 'drop', x: x2, y: y2, data });
      } finally {
        await send('Input.dispatchMouseEvent', { type: 'mouseReleased', x: x2, y: y2, button: 'left', clickCount: 1 });
        await send('Input.setInterceptDrags', { enabled: false });
      }
    },
    /** Ends Edge at once (a scenario that never finished): no answer waited for. */
    kill() {
      try { ws.close(); } catch { /* closed */ }
      try { proc.kill(); } catch { /* exited */ }
    },
    async shot(file) {
      const r = await send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(file, Buffer.from(r.data, 'base64'));
    },
    async close() {
      // Edge may close before it answers: its answer isn't waited for long (a run on GitHub waited 28 minutes for it).
      await Promise.race([send('Browser.close').catch(() => {}), sleep(2000)]);
      ws.close();
      await sleep(600);
      try { proc.kill(); } catch { /* exited */ }
      for (let i = 0; i < 10; i++) {
        try { fs.rmSync(profile, { recursive: true, force: true }); break; } catch { await sleep(400); }
      }
    },
  };
  await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: scale, mobile: false });
  await send('Emulation.setFocusEmulationEnabled', { enabled: true });
  await send('Emulation.setLocaleOverride', { locale });
  await send('Emulation.setTimezoneOverride', { timezoneId: timezone });
  // Windows set to dark, whatever this PC is set to: the "system" theme follows it.
  await send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-color-scheme', value: 'dark' }] });
  return b;
}
