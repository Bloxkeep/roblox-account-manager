// Bloxkeep's interface checks: the real page (RobloxManager/wwwroot) with the fake app behind it (dev/mock.js), opened
// in a headless Edge. Each scenario (scenarios.mjs) opens the page, clicks and types like a person, and the page is then
// written down as text (page-hooks.js snap(): what is shown, how it nests, labels and states) together with the commands
// it sent to the app. That text must match the saved copy in baseline/, so a change to how the page is built (such as
// moving it to React) shows every place where the interface came out different.
//
//   node Tests/ui/run.mjs                  every scenario, English and Vietnamese
//   node Tests/ui/run.mjs --only menu,edit scenarios whose name contains one of these
//   node Tests/ui/run.mjs --lang en        one language
//   node Tests/ui/run.mjs --update         save what the page shows now as the expected copy
//   node Tests/ui/run.mjs --shots          also screenshots, in Tests/ui/out/shots, to look at
//   node Tests/ui/run.mjs --layout         also each element's position and size, against a copy kept on this PC only
//                                          (Tests/ui/out/layout: positions differ a little between PCs)
// The exit code is the number of scenarios that came out different (or failed).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, sleep } from './cdp.mjs';
import { serve } from './serve.mjs';
import scenarios from './scenarios.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const args = process.argv.slice(2);
const flag = f => args.includes(f);
const opt = f => (args.includes(f) ? args[args.indexOf(f) + 1] : null);
const UPDATE = flag('--update'), SHOTS = flag('--shots'), LAYOUT = flag('--layout');
const ONLY = opt('--only')?.split(',');
const LANGS = opt('--lang')?.split(',') || ['en', 'vi'];
const BASE_DIR = LAYOUT ? path.join(HERE, 'out', 'layout') : path.join(HERE, 'baseline');
const OUT = path.join(HERE, 'out');
fs.mkdirSync(BASE_DIR, { recursive: true });
fs.mkdirSync(path.join(OUT, 'actual'), { recursive: true });
if (SHOTS) fs.mkdirSync(path.join(OUT, 'shots'), { recursive: true });

const hooks = fs.readFileSync(path.join(HERE, 'page-hooks.js'), 'utf8');
const server = await serve(REPO);
const base = `http://127.0.0.1:${server.port}`;
const b = await launch({ width: 1280, height: 800 });
await b.send('Page.addScriptToEvaluateOnNewDocument', { source: hooks });
// Nothing from the internet (the mock's game icons are on Roblox's CDN): the same page offline as online.
await b.send('Network.enable');
await b.send('Network.setBlockedURLs', { urls: ['https://*'] });

/** JSON with sorted keys, so the same arguments always read the same. */
const stable = v => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort()) : x));

let failed = 0;
const started = Date.now();
const list = scenarios.filter(s => !ONLY || ONLY.some(o => s.name.includes(o)));
for (const sc of list) {
  for (const lang of LANGS.filter(l => !sc.langs || sc.langs.includes(l))) {
    const id = `${sc.name}.${lang}`;
    const parts = [];
    let callsSeen = 0;
    const record = async label => {
      const settled = await b.eval('__ui.settle()');
      const snap = await b.eval(`__ui.snap(${JSON.stringify({ layout: LAYOUT })})`);
      const calls = await b.eval('__ui.calls');
      const fresh = calls.slice(callsSeen).map(c => `  ${c.cmd} ${stable(c.args)}`);
      callsSeen = calls.length;
      parts.push(`== ${label}${settled ? '' : ' (still changing)'}\n${snap}\n-- commands\n${fresh.join('\n') || '  (none)'}`);
      if (SHOTS) await b.shot(path.join(OUT, 'shots', `${id}${label === 'end' ? '' : '-' + label}.png`));
    };
    const s = {
      lang,
      /** Clicks the element like a mouse would (its centre), then waits for the page to settle. */
      async click(spec) {
        const p = await b.eval(`__ui.point(${JSON.stringify(spec)})`);
        await b.click(p.x, p.y);
        await b.eval('__ui.settle()');
      },
      async key(k, mods) { await b.key(k, mods); await b.eval('__ui.settle()'); },
      /** Types into the focused element (or the one given first). */
      async type(text, spec) {
        if (spec) await s.click(spec);
        await b.type(text);
        await b.eval('__ui.settle()');
      },
      async wait(ms) { await sleep(ms); await b.eval('__ui.settle()'); },
      eval: expr => b.eval(expr),
      snap: label => record(label),
    };
    try {
      b.problems.length = 0;
      await b.send('Storage.clearDataForOrigin', { origin: base, storageTypes: 'all' });
      await b.goto(`${base}/dev/preview.html?lang=${lang}${sc.query ? '&' + sc.query : ''}`);
      await b.eval(`__ui.lang = ${JSON.stringify(lang)}`);
      await sleep(900); // the mock's own scripted views (?settings=…, ?menu=1…) start 600 ms after load
      await b.eval('__ui.settle()');
      if (sc.run) await sc.run(s);
      await record('end');
      if (b.problems.length) parts.push(`== page errors\n${b.problems.join('\n')}`);
    } catch (e) {
      parts.push(`== FAILED\n${e.message}`);
    }
    const text = parts.join('\n') + '\n';
    const file = path.join(BASE_DIR, `${id}.txt`);
    fs.writeFileSync(path.join(OUT, 'actual', `${id}.txt`), text);
    const broke = text.includes('\n== FAILED\n') || text.includes('\n== page errors\n');
    if (UPDATE && !broke) {
      fs.writeFileSync(file, text);
      console.log(`  saved  ${id}`);
    } else if (!fs.existsSync(file)) {
      failed++;
      console.log(`  NEW    ${id}: no saved copy (run with --update)${broke ? ' — and it failed:\n' + problems(text) : ''}`);
    } else {
      const want = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n'); // whatever line ends a checkout gave it
      if (want === text && !broke) console.log(`  PASS   ${id}`);
      else {
        failed++;
        console.log(broke ? `  FAIL   ${id}\n${problems(text)}` : `  DIFF   ${id}\n${diff(want, text)}`);
      }
    }
  }
}
await b.close();
server.close();
console.log(failed ? `${failed} scenario(s) differ — see Tests/ui/out/actual` : `All ${list.length} scenarios match (${Math.round((Date.now() - started) / 1000)} s).`);
process.exit(failed);

/** A run's failure and page errors, indented. */
function problems(text) {
  return ('\n' + text).split('\n== ').filter(p => /^(FAILED|page errors)/.test(p))
    .map(p => p.split('\n').slice(0, 6).map(l => `         ${l}`).join('\n')).join('\n');
}

/** The first differing lines, with a little context. */
function diff(a, b) {
  const x = a.split('\n'), y = b.split('\n');
  let i = 0;
  while (i < x.length && i < y.length && x[i] === y[i]) i++;
  let ex = x.length - 1, ey = y.length - 1;
  while (ex > i && ey > i && x[ex] === y[ey]) { ex--; ey--; }
  const from = Math.max(0, i - 3);
  const cut = (lines, a0, a1) => lines.slice(a0, Math.min(a1 + 1, a0 + 25));
  return [...x.slice(from, i).map(l => `         ${l}`), ...cut(x, i, ex).map(l => `       - ${l}`), ...cut(y, i, ey).map(l => `       + ${l}`)].join('\n');
}
