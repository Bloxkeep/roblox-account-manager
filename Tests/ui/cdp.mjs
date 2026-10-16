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
  const port = 9400 + Math.floor(Math.random() * 500);
  const profile = fs.mkdtempSync(path.join(os.tmpdir(), 'bloxkeep-ui-'));
  const proc = spawn(findEdge(), ['--headless=new', `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`,
    '--hide-scrollbars', '--no-first-run', '--no-default-browser-check', '--disable-extensions', '--font-render-hinting=none',
    '--disable-features=msEdgeSidebarV2,msUndersideButton', '--disable-background-timer-throttling',
    `--window-size=${width},${height}`, `--lang=${locale}`, 'about:blank'], { stdio: 'ignore' });
  let page;
  for (let i = 0; i < 150 && !page; i++) {
    await sleep(150);
    try {
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
  const send = (method, params = {}) => new Promise((res, rej) => {
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
      const text = key.length === 1 && !(mod & 2) ? key : undefined;
      const base = { key, code: key.length === 1 ? `Key${key.toUpperCase()}` : key, windowsVirtualKeyCode: code, modifiers: mod };
      await send('Input.dispatchKeyEvent', { type: text ? 'keyDown' : 'rawKeyDown', ...base, text });
      await send('Input.dispatchKeyEvent', { type: 'keyUp', ...base });
    },
    async type(text) {
      await send('Input.insertText', { text });
    },
    async click(x, y) {
      for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased'])
        await send('Input.dispatchMouseEvent', { type, x, y, button: 'left', clickCount: 1 });
    },
    async shot(file) {
      const r = await send('Page.captureScreenshot', { format: 'png' });
      fs.writeFileSync(file, Buffer.from(r.data, 'base64'));
    },
    async close() {
      try { await send('Browser.close'); } catch { /* already gone */ }
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
  return b;
}
