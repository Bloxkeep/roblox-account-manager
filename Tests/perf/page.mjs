// Bloxkeep's page, timed: the real page (RobloxManager/wwwroot) with the fake app behind it (dev/mock.js) in a headless
// Edge, as Tests/ui runs it. For each number of accounts:
//   open      a fresh Edge opens the page until the accounts are drawn (ms after the navigation started), with Chrome's
//             own counters for that open (script, style, layout time; the page's JS heap)
//   redraw    the main-thread time one state from the app costs (a small change, as most are): twenty times, the median
//   tabs      the Presets and Statistics tabs, opened a few seconds in (after the app said it has settled), until their
//             game icons are drawn: icons the browser hasn't loaded yet (from Roblox's CDN: needs the internet), and
//             Statistics answering 350 ms later, as the app does when it asks Roblox for them first (measured 0.32-1.35 s)
//   node Tests/perf/page.mjs                       7 and 300 accounts, 5 opens each
//   node Tests/perf/page.mjs --accounts 50 --runs 3 --json Tests/perf/out/before.json
//   --root <checkout>   another checkout's page (an older commit, its ui.js built), with this one's dev/mock.js copied in
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch } from '../ui/cdp.mjs';
import { serve } from '../ui/serve.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const args = process.argv.slice(2);
const opt = f => (args.includes(f) ? args[args.indexOf(f) + 1] : null);
const COUNTS = (opt('--accounts') || '7,300').split(',').map(Number);
const RUNS = Number(opt('--runs')) || 5;
const median = xs => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const r1 = x => Math.round(x * 10) / 10;

const server = await serve(path.resolve(opt('--root') || REPO));
const base = `http://127.0.0.1:${server.port}`;
const results = {};

for (const n of COUNTS) {
  const url = `${base}/dev/preview.html?lang=en&nodelay=1&settled=1&coldicons=1&statsms=350&accounts=${n}`;
  const opens = [], script = [], style = [], layout = [], heap = [];
  const redraws = [], tabs = { Presets: [], Statistics: [] };
  for (let run = 0; run < RUNS; run++) {
    const b = await launch({ width: 1280, height: 800 });
    try {
      await b.send('Performance.enable');
      await b.goto(url);
      // The accounts on screen: as many cards (or rows) as there are accounts, then one frame drawn.
      const shown = await b.eval(`new Promise(r => { const want = ${n};
        const check = () => document.querySelectorAll('#grid .card, #grid .row, #grid [role=option]').length >= want
          ? requestAnimationFrame(() => r(performance.now())) : setTimeout(check, 5); check(); })`);
      opens.push(shown);
      const m = Object.fromEntries((await b.send('Performance.getMetrics')).metrics.map(x => [x.name, x.value]));
      script.push(m.ScriptDuration * 1000);
      style.push(m.RecalcStyleDuration * 1000);
      layout.push(m.LayoutDuration * 1000);
      heap.push(m.JSHeapUsedSize / 1048576);
      await new Promise(res => setTimeout(res, 3000)); // the first ticks done, and the app has settled
      if (run === 0) {
        // Main-thread time a state costs the page (script, style, layout), from Chrome's counters around each push; the
        // fake app's own answer delay (120 ms) is not in it.
        const busy = async () => { const m = Object.fromEntries((await b.send('Performance.getMetrics')).metrics.map(x => [x.name, x.value]));
          return (m.ScriptDuration + m.RecalcStyleDuration + m.LayoutDuration) * 1000; };
        for (let i = 0; i < 20; i++) {
          await b.eval(`window.__mockBridge.call('getState')`); // the fake app's delay, outside the timed part
          const before = await busy();
          await b.eval(`(async () => { await window.__mockBridge.call('setSettings', { autoLockMinutes: ${i % 2 ? 5 : 0} });
            await new Promise(r => requestAnimationFrame(() => requestAnimationFrame(r))); })()`);
          redraws.push((await busy()) - before);
        }
      }
      for (const tab of ['Presets', 'Statistics']) {
        tabs[tab].push(await b.eval(`new Promise(r => {
            const nav = [...document.querySelectorAll('button.nav')].find(x => x.title === ${JSON.stringify(tab)});
            const icons = ${JSON.stringify(tab === 'Presets' ? '#view-presets img.pv-icon' : '#view-stats .sgame img')};
            const t = performance.now(); nav.click();
            // Every game icon there, loaded, then the frame that shows them.
            const check = () => { const imgs = [...document.querySelectorAll(icons)];
              if (imgs.length && imgs.every(i => i.complete && i.naturalWidth)) requestAnimationFrame(() => r({ ms: performance.now() - t, icons: imgs.length }));
              else if (performance.now() - t > 8000) r({ ms: null, icons: imgs.length }); else setTimeout(check, 5); };
            check(); })`));
      }
    } finally {
      await b.close?.();
      b.kill?.();
    }
  }
  results[n] = {
    openMs: r1(median(opens)), scriptMs: r1(median(script)), styleMs: r1(median(style)), layoutMs: r1(median(layout)), heapMb: r1(median(heap)),
    redrawMs: r1(median(redraws)), redrawWorstMs: r1(Math.max(...redraws)),
    tabs: Object.fromEntries(Object.entries(tabs).map(([tab, xs]) => [tab, {
      ms: r1(median(xs.map(x => x.ms ?? Infinity))), worstMs: r1(Math.max(...xs.map(x => x.ms ?? Infinity))), icons: xs[0]?.icons }])),
  };
  console.log(`${n} accounts:`, JSON.stringify(results[n]));
}
server.close();
if (opt('--json')) {
  fs.mkdirSync(path.dirname(path.resolve(opt('--json'))), { recursive: true });
  fs.writeFileSync(opt('--json'), JSON.stringify(results, null, 2));
}
process.exit(0);
