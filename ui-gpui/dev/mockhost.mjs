// A stand-in for the C# app, for looking at the GPUI window on a PC where nothing may pop up: runs ui/dev/mock.js (the
// web page's fake app, with its made-up accounts and games) in Node, serves it over the two named pipes the app makes
// (link.rs), and starts bloxkeep-ui with --shot, so the window is drawn hidden into a picture.
//
//   node ui-gpui/dev/mockhost.mjs --shot out.png [--size 1280x800] [--query "lang=vi&kicked=1"] [--page accounts]
//                                 [--exe path] [--wait 2500] [--online]
//
// Game pictures come from site/demo-pics (the website demo's copies) unless --online: Roblox's own addresses can't be
// reached from every network.
import fs from 'node:fs';
import net from 'node:net';
import path from 'node:path';
import vm from 'node:vm';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, '../..');
const argv = process.argv.slice(2);
const opt = (k, d) => { const i = argv.indexOf(k); return i >= 0 ? argv[i + 1] : d; };
const shot = opt('--shot');
const size = opt('--size', '1280x800');
const query = opt('--query', '');
const page = opt('--page');
const wait = opt('--wait', '2500');
const exe = opt('--exe', 'E:/Rust/target/bloxkeep-ui-shot/release/bloxkeep-ui.exe');
const online = argv.includes('--online');

// The fake app, as the browser runs it.
const sandbox = {
  console, setTimeout, setInterval, clearTimeout, clearInterval, URLSearchParams, URL, encodeURIComponent, decodeURIComponent,
  location: { search: query ? `?${query}` : '' },
  document: { querySelector: () => null, querySelectorAll: () => [], hidden: false, addEventListener: () => {} },
  addEventListener: () => {}, removeEventListener: () => {},
  // The preview's own files (ui/dev/wiki-sample.json…) from the repo; anything else from the web.
  fetch: async u => {
    if (typeof u === 'string' && u.startsWith('/')) {
      const text = fs.readFileSync(path.join(repo, u.split('?')[0]), 'utf8');
      return { ok: true, json: async () => JSON.parse(text), text: async () => text };
    }
    return fetch(u);
  },
};
sandbox.window = sandbox;
vm.createContext(sandbox);
vm.runInContext(fs.readFileSync(path.join(repo, 'ui/dev/mock.js'), 'utf8'), sandbox, { filename: 'mock.js' });
const mock = sandbox.__mockBridge;

// Game icons and covers from the demo's copies, by place id.
const pics = path.join(repo, 'site/demo-pics');
const map = online ? null : JSON.parse(fs.readFileSync(path.join(pics, 'map.json'), 'utf8'));
const local = v => {
  if (!map || v === null || typeof v !== 'object') return v;
  if (Array.isArray(v)) return v.map(local);
  const out = {};
  for (const [k, x] of Object.entries(v)) out[k] = local(x);
  const g = map.games?.[out.placeId];
  if (g?.icon && typeof out.iconUrl === 'string') out.iconUrl = path.join(pics, g.icon);
  return out;
};

const name = `bloxkeep-ui-mock-${process.pid}`;
let down = null;
const queue = [];
const post = msg => { const line = JSON.stringify(local(msg)) + '\n'; down ? down.write(line) : queue.push(line); };
mock.attach((ev, data) => post({ type: 'event', name: ev, data: data ?? null }));

const serve = (pipe, onSocket) => new Promise(res => net.createServer(onSocket).listen(`\\\\.\\pipe\\${pipe}`, res));
await serve(name, s => { down = s; queue.splice(0).forEach(l => s.write(l)); s.on('error', () => {}); });
await serve(`${name}-up`, s => {
  let buf = '';
  s.on('error', () => {});
  s.on('data', async chunk => {
    buf += chunk;
    let i;
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i); buf = buf.slice(i + 1);
      let msg; try { msg = JSON.parse(line); } catch { continue; }
      if (msg.type === 'hello') { console.log(`[mockhost] window ${msg.hwnd}`); continue; }
      if (!msg.cmd) continue;
      if (process.env.MOCK_LOG) console.log('[mockhost] <-', line.slice(0, 200));
      try {
        let result = (await mock.call(msg.cmd, msg.args ?? {})) ?? null;
        // gameLooks answers by place: its icon and cover from the demo's copies when they're there.
        if (msg.cmd === 'gameLooks' && map) {
          result = { ...(result || {}) };
          for (const place of msg.args?.places || []) {
            const g = map.games?.[place];
            if (g) result[place] = { ...(result[place] || {}), ...(g.icon ? { icon: path.join(pics, g.icon) } : {}), ...(g.cover ? { cover: path.join(pics, g.cover) } : {}) };
          }
        }
        if (process.env.MOCK_LOG && msg.cmd === 'gameLooks') console.log('[mockhost] ->', JSON.stringify(result).slice(0, 400));
        post({ type: 'reply', id: msg.id, ok: true, result });
      }
      catch (e) { post({ type: 'reply', id: msg.id, ok: false, error: String(e?.message ?? e) }); }
    }
  });
});

const args = ['--pipe', name, '--size', size, '--wait', wait];
// --cache DIR: where the interface may keep what Roblox said (the app gives it its cache folder).
if (opt('--cache')) args.push('--cache', opt('--cache'));
if (shot) args.push('--shot', path.resolve(shot));
const ui = spawn(exe, args, {
  stdio: ['ignore', 'inherit', 'inherit'],
  env: { ...process.env, BLOXKEEP_UI_ASSETS: path.join(repo, 'ui-gpui/assets'), ...(page ? { BLOXKEEP_UI_PAGE: page } : {}) },
});
ui.on('exit', code => process.exit(code ?? 0));
