// Flashes: a frame the page shows for a moment that looks like neither what was there before nor what stays after
// (a spinner, an empty state or a plain background passing by while a tab, a list or a figure changes). The real page
// with the fake app behind it (ui/dev/mock.js), in a headless Edge: every frame it paints is recorded (a screencast),
// and a frame with more than a little that matches neither the frame before the action nor the one it settles on is
// a flash. Actions: every tab to every other tab, and Settings' pages; every button, tab, chip, switch and link in each tab (the first of
// each kind, so 50 rows' Play count once); and the app pushing news (an account opening, running, closing, a toast).
//
//   node Tests/ui/flash.mjs                       Starry night (the animated sky left out: it moves on its own),
//                                                 Windows' animation effects off (prefers-reduced-motion), English
//   node Tests/ui/flash.mjs --theme dark          any theme id
//   node Tests/ui/flash.mjs --only stats,tabs     these tabs' controls ("tabs": switching tabs and Settings' pages,
//                                                 "push": the app's news)
//   node Tests/ui/flash.mjs --motion              Windows' animation effects on
// Game monitor has six windows in it (one cut off, one joining), as mock.js makes them with ?tiles=6&drop=1.
//
// Each flash is saved as a picture (before | the passing frame | its odd part in magenta | after) in Tests/ui/out/flash,
// and listed in Tests/ui/out/flash.txt. The exit code is how many were found.
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, sleep } from './cdp.mjs';
import { serve } from './serve.mjs';
import { decode, encode } from './png.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const OUT = path.join(HERE, 'out', 'flash');
const args = process.argv.slice(2);
const opt = f => (args.includes(f) ? args[args.indexOf(f) + 1] : null);
const THEME = opt('--theme') || 'starrynight', ONLY = opt('--only')?.split(','), MOTION = args.includes('--motion');
const VIEWS = ['accounts', 'presets', 'news', 'macros', 'monitor', 'stats', 'settings'];
const want = k => !ONLY || ONLY.includes(k);

/** How far apart two colours may be and still count as the same (text edges, the clock). */
const NEAR = 48;
/** The share of the frame that may match neither side (the live dot blinking, a figure ticking). */
const MOST = 0.012;
/** Waits that are Roblox's: lists asked of roblox.com that say "Looking for…" until they come (not counted). */
const WAITS = ['accounts · Browse servers'];
/** How long the page has to paint nothing to count as settled; how long an action is watched at most. */
const QUIET = 300, WATCH = 1500;

fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
const server = await serve(REPO);
const b = await launch({ width: 1280, height: 800 });
if (!MOTION) await b.send('Emulation.setEmulatedMedia', { features: [{ name: 'prefers-reduced-motion', value: 'reduce' }] });

// Every painted frame, small: Edge sends one each time the page changes on screen (none while nothing does).
let frames = [], lastAt = Date.now();
b.on(m => {
  if (m.method !== 'Page.screencastFrame') return;
  b.send('Page.screencastFrameAck', { sessionId: m.params.sessionId }).catch(() => {});
  frames.push(decode(Buffer.from(m.params.data, 'base64')));
  lastAt = Date.now();
});
const settle = async (max = 3000) => {
  const t0 = Date.now();
  await sleep(QUIET);
  while (Date.now() - lastAt < QUIET && Date.now() - t0 < max) await sleep(50);
};
const latest = () => frames[frames.length - 1];

/** Per pixel and channel, the darkest and brightest that before or after have within SLIDE pixels up or down (and one
 *  aside): a page fading and moving between them (app.css .leaving, .entering) stays inside, a flash of something
 *  that is in neither (a bare white border, an empty background) doesn't. */
const SLIDE = 16, spans = new WeakMap();
function span(a, z) {
  const key = spans.get(a); if (key?.z === z) return key;
  const W = a.width, H = a.height, n = W * H * 4;
  const lo0 = new Uint8Array(n), hi0 = new Uint8Array(n), lo = new Uint8Array(n), hi = new Uint8Array(n);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) for (let c = 0; c < 3; c++) {
    let l = 255, h = 0;
    for (let dx = -1; dx <= 1; dx++) { const q = x + dx; if (q < 0 || q >= W) continue; const j = (y * W + q) * 4 + c; l = Math.min(l, a.data[j], z.data[j]); h = Math.max(h, a.data[j], z.data[j]); }
    lo0[(y * W + x) * 4 + c] = l; hi0[(y * W + x) * 4 + c] = h;
  }
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) for (let c = 0; c < 3; c++) {
    let l = 255, h = 0;
    for (let dy = Math.max(0, y - SLIDE); dy <= Math.min(H - 1, y + SLIDE); dy++) { const j = (dy * W + x) * 4 + c; if (lo0[j] < l) l = lo0[j]; if (hi0[j] > h) h = hi0[j]; }
    lo[(y * W + x) * 4 + c] = l; hi[(y * W + x) * 4 + c] = h;
  }
  const r = { z, lo, hi }; spans.set(a, r); return r;
}
/** The share of pixels in f matching none of the pictures before nor the one after, nor in between them (span), and f
 *  with those in magenta. */
function foreign(befores, f, z) {
  let n = 0;
  const out = Buffer.from(f.data);
  const sized = befores.filter(x => x.width === f.width && x.height === f.height);
  const W = f.width, H = f.height;
  // Like a pixel of x at i, or (shift) one pixel beside it: the two pictures of before are drawn apart, and fine
  // detail (a heatmap's cells, text) lands a pixel over between them. The screencast's own frame can also be a tab
  // behind after a quiet spell, which leaves the screenshot alone to say what was there.
  const near = (x, j, i) => Math.max(Math.abs(x.data[j] - f.data[i]), Math.abs(x.data[j + 1] - f.data[i + 1]), Math.abs(x.data[j + 2] - f.data[i + 2])) <= NEAR;
  const like = (x, i, shift) => {
    if (near(x, i, i)) return true;
    if (!shift) return false;
    const px = (i >> 2) % W, py = ((i >> 2) / W) | 0;
    for (let dy = -1; dy <= 1; dy++) for (let dx = -1; dx <= 1; dx++) {
      const qx = px + dx, qy = py + dy;
      if ((dx || dy) && qx >= 0 && qy >= 0 && qx < W && qy < H && near(x, (qy * W + qx) * 4, i)) return true;
    }
    return false;
  };
  const r = sized[0] && z.width === W && z.height === H ? span(sized[0], z) : null;
  const inside = i => { for (let c = 0; c < 3; c++) if (f.data[i + c] < r.lo[i + c] - NEAR || f.data[i + c] > r.hi[i + c] + NEAR) return false; return true; };
  for (let i = 0; i < f.data.length; i += 4) {
    if (!sized.some(x => like(x, i, true)) && !like(z, i, false) && !(r && inside(i))) { n++; out[i] = 255; out[i + 1] = 0; out[i + 2] = 255; }
  }
  return { share: n / (f.width * f.height), image: { width: f.width, height: f.height, data: out } };
}
function sideBySide(list) {
  const w = list[0].width, h = list[0].height, gap = 6, width = list.length * w + (list.length - 1) * gap;
  const data = Buffer.alloc(width * h * 4, 255);
  list.forEach((p, k) => { for (let y = 0; y < h; y++) p.data.copy(data, (y * width + k * (w + gap)) * 4, y * w * 4, (y + 1) * w * 4); });
  return encode({ width, height: h, data });
}

const found = [];
/** Runs one action (a function run in the page) and looks at every frame it paints until the page settles. */
async function watch(where, label, expr) {
  await settle();
  // Two pictures of before: the screencast's last frame (drawn as the others are), and one asked for now, as that frame
  // can be one behind after a long quiet spell. A pixel like either is no flash.
  const before = latest();
  if (!before) return;
  const shot = await b.send('Page.captureScreenshot', { format: 'png', clip: { x: 0, y: 0, width: 1280, height: 800, scale: 0.5 } });
  const now = decode(Buffer.from(shot.data, 'base64'));
  frames = [before];
  const t0 = Date.now();
  const ok = await b.eval(expr).catch(() => false);
  if (ok === false) return;
  await sleep(QUIET);
  while (Date.now() - lastAt < QUIET && Date.now() - t0 < WATCH) await sleep(30);
  const seen = frames.slice(1), after = latest();
  let worst = null;
  for (const f of seen.slice(0, -1)) {
    const r = foreign([before, now], f, after);
    if (r.share > MOST && (!worst || r.share > worst.share)) worst = { ...r, frame: f };
  }
  if (!worst || WAITS.includes(`${where} · ${label}`)) return;
  const n = found.length + 1, file = `${String(n).padStart(3, '0')}-${where}-${label.replace(/[^\p{L}\p{N}]+/gu, '_').slice(0, 40)}.png`;
  fs.writeFileSync(path.join(OUT, file), sideBySide([before, worst.frame, worst.image, after]));
  found.push(`${(worst.share * 100).toFixed(1).padStart(5)} %  ${where} · ${label}  (${seen.length} frames)  out/flash/${file}`);
  console.log(`FLASH  ${where} · ${label}  ${(worst.share * 100).toFixed(1)} % of the window`);
}

async function open(view = 'accounts') {
  await b.goto(`http://127.0.0.1:${server.port}/ui/dev/preview.html?lang=en&theme=${THEME}&tiles=6&drop=1`);
  await sleep(1500);
  // The animated sky changes every frame by itself: left out, the still picture under it stays.
  await b.eval(`(() => { const s = document.createElement('style'); s.textContent = '#live-scene, #live-photo { visibility: hidden !important; }'; document.head.append(s); return 1; })()`);
  await b.eval(`document.querySelector('button.nav[data-view="${view}"]').click(); 1`);
  await settle();
}

await b.send('Page.startScreencast', { format: 'png', maxWidth: 640, maxHeight: 400, everyNthFrame: 1 });
await open();

if (want('tabs')) {
  // Each tab seen once first: a picture loading the first time (an empty box, then the picture) isn't a flash.
  for (const view of VIEWS) { await b.eval(`document.querySelector('button.nav[data-view="${view}"]').click(); 1`); await settle(); }
  for (const from of VIEWS) for (const to of VIEWS) {
    if (from === to) continue;
    await b.eval(`document.querySelector('button.nav[data-view="${from}"]').click(); 1`);
    await watch('tabs', `${from} → ${to}`, `document.querySelector('button.nav[data-view="${to}"]').click(), true`);
  }
}

// Settings' pages, each from the one before it and back.
if (want('tabs')) {
  await b.eval(`document.querySelector('button.nav[data-view="settings"]').click(); 1`);
  const pages = await b.eval(`[...document.querySelectorAll('#view-settings .set-nav-item')].map(e => e.textContent.trim())`);
  const go = page => `(() => { const e = [...document.querySelectorAll('#view-settings .set-nav-item')].find(x => x.textContent.trim() === ${JSON.stringify(page)}); e?.click(); return !!e; })()`;
  for (const page of pages) await settle(), await b.eval(go(page));
  for (let i = 1; i < pages.length; i++) {
    await watch('settings pages', `${pages[i - 1]} → ${pages[i]}`, go(pages[i]));
    await watch('settings pages', `${pages[i]} → ${pages[i - 1]}`, go(pages[i - 1]));
    await b.eval(go(pages[i]));
  }
}

// What can be clicked in a tab (the dock's buttons with the Accounts tab): one of each kind (its classes and words,
// numbers left out; a switch by its box's id), the tab bar and the window's own buttons aside, nothing that types.
const FIND = String(function find(view, k, act) {
  const v = document.querySelector('#view-' + view);
  const key = e => `${e.tagName} ${e.className} ${(e.getAttribute('aria-label') || e.title || e.textContent || e.querySelector('input')?.id || '').replace(/\d+/g, '#').trim().slice(0, 40)}`;
  const all = [...document.querySelectorAll('button, [role="tab"], [role="button"], [role="switch"], [role="checkbox"], a[href], summary, label.switch, .chip')]
    .filter(e => e.offsetParent && !e.disabled && !e.closest('.nav, .titlebar, .win-ctl, .set-nav-item') && (v.contains(e) || (view === 'accounts' && e.closest('.dock'))));
  if (k === null) { const seen = new Set(); return all.map(key).filter(x => !seen.has(x) && seen.add(x)); }
  const e = all.find(x => key(x) === k);
  if (!e) return false;
  if (act) e.click(); else e.scrollIntoView({ block: 'center' });
  return true;
});
const find = (view, k, act) => `(${FIND})(${JSON.stringify(view)}, ${JSON.stringify(k)}, ${act})`;
const healthy = view => b.eval(`!!document.querySelector('button.nav[data-view="${view}"]')?.offsetParent && !document.querySelector('#view-${view}')?.hidden`).catch(() => false);

// Settings page by page (each one's controls show only while it's chosen).
const pagesOf = view => view !== 'settings' ? [null]
  : b.eval(`[...document.querySelectorAll('#view-settings .set-nav-item')].map(e => e.textContent.trim())`);
const toPage = page => page === null ? 1
  : b.eval(`(() => { const e = [...document.querySelectorAll('#view-settings .set-nav-item')].find(x => x.textContent.trim() === ${JSON.stringify(page)}); e?.click(); return 1; })()`);
for (const view of VIEWS) {
  if (!want(view)) continue;
  await open(view);
  for (const page of await pagesOf(view)) {
    await open(view);
    await toPage(page);
    await settle();
    const where = page ? `${view} › ${page}` : view;
    const keys = await b.eval(find(view, null, false));
    console.log(`${where}: ${keys.length} kinds of control`);
    for (const k of keys) {
      // Back to a calm page: menus and dialogs shut, the tab (and page) shown, or the page opened again (a lock screen).
      for (let i = 0; i < 2; i++) await b.key('Escape');
      if (!(await healthy(view))) await open(view);
      await toPage(page);
      if (!(await b.eval(find(view, k, false)))) continue;
      await watch(where, k.split(' ').slice(2).join(' ') || k, find(view, k, true));
    }
  }
}

// The app's news, in every tab: an account opening, running, closing again; a toast; a new piece of news.
if (want('push')) {
  const PUSHES = [
    ['opening', `__mockBridge.call('debugAccount', { n: 2, set: { state: 'launching' } })`],
    ['running', `__mockBridge.call('debugAccount', { n: 2, set: { state: 'running' } })`],
    ['closed', `__mockBridge.call('debugAccount', { n: 2, set: { state: 'idle' } })`],
    ['toast', `__mockBridge.call('debugToast', { text: 'Saved.', kind: 'ok' })`],
    ['news', `__mockBridge.call('debugFeedCard', { title: 'A new event' })`],
  ];
  await open();
  for (const view of VIEWS) {
    await b.eval(`document.querySelector('button.nav[data-view="${view}"]').click(); 1`);
    for (const [label, call] of PUSHES) await watch(`push ${view}`, label, `(${call}, true)`);
  }
}

fs.writeFileSync(path.join(HERE, 'out', 'flash.txt'), found.join('\n') + (found.length ? '\n' : ''));
console.log(`\n${found.length} flash(es)${found.length ? ' (Tests/ui/out/flash.txt):\n' + found.join('\n') : ''}`);
await b.close?.();
process.exit(found.length);
