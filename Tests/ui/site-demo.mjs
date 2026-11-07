// Checks the website's try-it page (site/build.mjs → site/dist/demo/) in headless Edge, served as GitHub Pages does
// (under /roblox-account-manager/): in English and Vietnamese it opens with no console errors and no request to any
// other host, Play takes an account from opening to playing, the Game monitor shows its tiles, every tab opens and a
// theme can be picked. Screenshots go to Tests/ui/out/site-demo/. Build the site first (cd site && node build.mjs).
//   node Tests/ui/site-demo.mjs
import fs from 'node:fs';
import http from 'node:http';
import path from 'node:path';
import { launch, sleep } from './cdp.mjs';

const ROOT = path.resolve(import.meta.dirname, '..', '..');
const DIST = path.join(ROOT, 'site', 'dist');
const OUT = path.join(import.meta.dirname, 'out', 'site-demo');
const BASE = '/roblox-account-manager/';
if (!fs.existsSync(path.join(DIST, 'demo', 'app', 'index.html'))) throw new Error('Build the site first: cd site && node build.mjs');
fs.mkdirSync(OUT, { recursive: true });

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.svg': 'image/svg+xml',
  '.png': 'image/png', '.jpg': 'image/jpeg', '.webp': 'image/webp', '.xml': 'application/xml', '.txt': 'text/plain' };
const server = http.createServer((req, res) => {
  const url = decodeURIComponent(new URL(req.url, 'http://x').pathname);
  if (!url.startsWith(BASE)) { res.writeHead(404); return res.end(); }
  let file = path.join(DIST, url.slice(BASE.length));
  if (!file.startsWith(DIST)) { res.writeHead(403); return res.end(); }
  if (fs.existsSync(file) && fs.statSync(file).isDirectory()) file = path.join(file, 'index.html');
  if (!fs.existsSync(file)) { res.writeHead(404); return res.end(); }
  res.writeHead(200, { 'content-type': TYPES[path.extname(file)] || 'application/octet-stream' });
  fs.createReadStream(file).pipe(res);
});
await new Promise(r => server.listen(0, '127.0.0.1', r));
const origin = `http://127.0.0.1:${server.address().port}`;

let failed = 0;
const fail = msg => { failed++; console.log(`  FAIL ${msg}`); };
const until = async (b, expr, what, ms = 8000) => {
  for (let t = 0; t < ms; t += 100) { if (await b.eval(expr)) return true; await sleep(100); }
  fail(`timed out waiting for ${what}`);
  return false;
};

for (const lang of ['en', 'vi']) {
  console.log(`${lang}:`);
  const b = await launch({ width: 1440, height: 900, locale: lang === 'vi' ? 'vi-VN' : 'en-US' });
  const outside = [], logged = [], seen = [];
  b.on(m => {
    if (m.method === 'Network.requestWillBeSent') {
      const u = m.params.request.url;
      seen.push(u);
      if (!u.startsWith(origin) && !/^(data|blob|about):/.test(u)) outside.push(u);
    }
    if (m.method === 'Log.entryAdded' && m.params.entry.level === 'error') logged.push(`${m.params.entry.source}: ${m.params.entry.text} ${m.params.entry.url || ''}`);
  });
  await b.send('Network.enable');
  await b.send('Log.enable');
  await b.goto(`${origin}${BASE}${lang === 'vi' ? 'vi/' : ''}demo/`);
  const F = 'document.querySelector(".demo-frame iframe").contentDocument';
  await until(b, `!!${F}?.querySelector('.card[data-id]')`, 'the account cards');
  await sleep(3000); // the second account gets into its game
  await b.shot(path.join(OUT, `${lang}-accounts.png`));

  // Play on an account that isn't open: opening, then playing.
  const card = `${F}.querySelector('.card[data-id$="4"]')`;
  await b.eval(`${card}.querySelector('[data-act=play-one]').click()`);
  await until(b, `!!${card}.querySelector('.activity.launching')`, 'the card to show opening', 3000);
  await until(b, `!${card}.querySelector('[data-act=play-one]')`, 'the card to show playing', 8000);
  await b.shot(path.join(OUT, `${lang}-played.png`));

  // Every tab, then the Game monitor's tiles.
  const views = await b.eval(`[...${F}.querySelectorAll('[data-act=view][data-view]')].map(e => e.dataset.view)`);
  for (const v of views) {
    await b.eval(`${F}.querySelector('[data-view="${v}"]').click()`);
    await sleep(900);
    await b.shot(path.join(OUT, `${lang}-view-${v}.png`));
  }
  // The Games tab's parts, where the pictures from outside would be: news (Discord's emoji), the price list, a wiki
  // (cut from the demo: it reads the live wiki).
  for (const [game, parts] of [['blox-fruits', ['news', 'prices', 'wiki']], ['deepwoken', []]]) {
    await b.eval(`${F}.querySelector('[data-view=news]').click()`);
    await sleep(500);
    await b.eval(`${F}.querySelector('#gm-all')?.click()`);
    await sleep(400);
    if (!await b.eval(`!!${F}.querySelector('#view-news .gm-card[data-game="${game}"]')`)) { fail(`no card for ${game}`); continue; }
    await b.eval(`${F}.querySelector('#view-news .gm-card[data-game="${game}"]').click()`);
    await sleep(600);
    if (!parts.length) await b.shot(path.join(OUT, `${lang}-game-${game}.png`));
    for (const part of parts) {
      if (!await b.eval(`!!${F}.querySelector('#view-news .gm-parts [data-part="${part}"]')`)) { fail(`no ${part} part for ${game}`); continue; }
      await b.eval(`${F}.querySelector('#view-news .gm-parts [data-part="${part}"]').click()`);
      await sleep(1200);
      await b.shot(path.join(OUT, `${lang}-game-${game}-${part}.png`));
    }
  }
  await b.eval(`${F}.querySelector('[data-view=monitor]').click()`);
  if (await until(b, `${F}.querySelectorAll('.mon-tile').length >= 3`, 'three monitor tiles')) {
    await sleep(500);
    await b.shot(path.join(OUT, `${lang}-monitor.png`));
  }

  // A theme from Settings › Appearance: a scenery one.
  await b.eval(`${F}.querySelector('[data-view=settings]').click()`);
  await sleep(500);
  await b.eval(`${F}.querySelector('[data-set-page=appearance]')?.click()`);
  await sleep(400);
  const groups = await b.eval(`[...${F}.querySelectorAll('#s-theme-groups [data-group]')].map(e => e.dataset.group)`);
  await b.eval(`${F}.querySelector('#s-theme-groups [data-group="${groups[1]}"]').click()`);
  await sleep(300);
  const pick = await b.eval(`${F}.querySelectorAll('#s-themes .swatch')[2].dataset.id`);
  await b.eval(`${F}.querySelector('#s-themes .swatch[data-id="${pick}"]').click()`);
  await until(b, `${F}.documentElement.dataset.theme === '${pick}' || ${F}.body.dataset.theme === '${pick}'`, `theme ${pick}`, 4000);
  await sleep(1200);
  await b.shot(path.join(OUT, `${lang}-theme.png`));
  await b.eval(`${F}.querySelector('[data-view=accounts]').click()`);
  await sleep(800);
  await b.shot(path.join(OUT, `${lang}-theme-accounts.png`));

  // The checks hear the frame: its own requests are seen, and an error it logs is caught (then dropped).
  if (!seen.some(u => u.includes('/demo/app/app.js'))) fail("the frame's requests weren't seen");
  await b.eval(`document.querySelector('.demo-frame iframe').contentWindow.console.error('site-demo probe')`);
  await sleep(200);
  const probe = b.problems.findIndex(p => p.includes('site-demo probe'));
  if (probe < 0) fail("an error in the frame wasn't heard"); else b.problems.splice(probe, 1);
  for (const p of [...b.problems, ...logged]) fail(p);
  for (const u of [...new Set(outside)]) fail(`request to another host: ${u}`);
  console.log(`  tabs: ${views.join(', ')}; theme: ${pick}`);
  await b.close();
}
server.close();
console.log(failed ? `${failed} failed` : 'all good');
process.exit(failed ? 1 : 0);
