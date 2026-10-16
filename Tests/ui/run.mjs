// Bloxkeep's interface checks: the real page (RobloxManager/wwwroot) with the fake app behind it (dev/mock.js), opened
// in a headless Edge. Each scenario (scenarios.mjs) opens the page, clicks and types like a person, and the page is then
// written down as text (page-hooks.js snap(): what is shown, how it nests, labels, states, inline styles) together with
// the commands it sent to the app. That text must match the saved copy in baseline/, so a change to how the page is
// built (such as moving it to React) shows every place where the interface came out different.
//
//   node Tests/ui/run.mjs                  every scenario, English and Vietnamese
//   node Tests/ui/run.mjs --only menu,edit scenarios whose name contains one of these
//   node Tests/ui/run.mjs --lang en        one language
//   node Tests/ui/run.mjs --update         save what the page shows now as the expected copy
//   node Tests/ui/run.mjs --shots          also screenshots, in Tests/ui/out/shots, to look at
//   node Tests/ui/run.mjs --layout         also each element's position and size, against a copy kept on this PC only
//                                          (Tests/ui/out/layout: positions differ a little between PCs)
//   node Tests/ui/run.mjs --pixels         also every screenshot pixel by pixel, against copies kept on this PC only
//                                          (Tests/ui/out/pixels; differences drawn in Tests/ui/out/pixel-diff)
//   node Tests/ui/run.mjs --coverage       which parts of app.js the scenarios ran (Tests/ui/out/coverage.txt)
//   node Tests/ui/run.mjs --jank           instead of comparing: where things on screen jump once something has loaded
//                                          (the fake app answers 3x slower), worst first (Tests/ui/out/jank.txt)
// The exit code is the number of scenarios that came out different (or failed).
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { launch, sleep } from './cdp.mjs';
import { serve } from './serve.mjs';
import { decode, compare } from './png.mjs';
import scenarios from './scenarios.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const args = process.argv.slice(2);
const flag = f => args.includes(f);
const opt = f => (args.includes(f) ? args[args.indexOf(f) + 1] : null);
const UPDATE = flag('--update'), SHOTS = flag('--shots'), LAYOUT = flag('--layout'), PIXELS = flag('--pixels'), COVERAGE = flag('--coverage'), JANK = flag('--jank');
const ONLY = opt('--only')?.split(',');
const LANGS = opt('--lang')?.split(',') || ['en', 'vi'];
const SIZE = [1280, 800];
const OUT = path.join(HERE, 'out');
const BASE_DIR = LAYOUT ? path.join(OUT, 'layout') : path.join(HERE, 'baseline');
const dirs = { actual: path.join(OUT, 'actual'), shots: path.join(OUT, 'shots'), pixels: path.join(OUT, 'pixels'), pixelDiff: path.join(OUT, 'pixel-diff') };
for (const d of [BASE_DIR, dirs.actual, ...(SHOTS ? [dirs.shots] : []), ...(PIXELS ? [dirs.pixels, dirs.pixelDiff] : [])]) fs.mkdirSync(d, { recursive: true });

const hooks = (JANK ? 'window.__uiSlow = 3;\n' : '') + fs.readFileSync(path.join(HERE, 'page-hooks.js'), 'utf8');
const server = await serve(REPO);
const base = `http://127.0.0.1:${server.port}`;
const b = await launch({ width: SIZE[0], height: SIZE[1] });
await b.send('Page.addScriptToEvaluateOnNewDocument', { source: hooks });
// Nothing from the internet (the mock's game icons are on Roblox's CDN): the same page offline as online.
await b.send('Network.enable');
await b.send('Network.setBlockedURLs', { urls: ['https://*'] });
const cover = COVERAGE ? await startCoverage() : null;

/** JSON with sorted keys, so the same arguments always read the same. */
const stable = v => JSON.stringify(v, (k, x) => (x && typeof x === 'object' && !Array.isArray(x) ? Object.fromEntries(Object.entries(x).sort()) : x));

let failed = 0;
const started = Date.now();
const jank = [];
const list = scenarios.filter(s => !ONLY || ONLY.some(o => s.name.includes(o)));
for (const sc of list) {
  for (const lang of LANGS.filter(l => !sc.langs || sc.langs.includes(l))) {
    const id = `${sc.name}.${lang}`;
    const parts = [], pixelNotes = [];
    let callsSeen = 0;
    const record = async label => {
      const settled = await b.eval('__ui.settle()');
      const snap = await b.eval(`__ui.snap(${JSON.stringify({ layout: LAYOUT })})`);
      const calls = await b.eval('__ui.calls');
      // Sorted: commands sent together (the page asking for several things as it opens) arrive in any order.
      const fresh = calls.slice(callsSeen).map(c => `  ${c.cmd} ${stable(c.args)}`).sort();
      callsSeen = calls.length;
      parts.push(`== ${label}${settled ? '' : ' (still changing)'}\n${snap}\n-- commands\n${fresh.join('\n') || '  (none)'}`);
      const name = `${id}${label === 'end' ? '' : '-' + label}.png`;
      if (SHOTS || PIXELS) {
        const png = Buffer.from((await b.send('Page.captureScreenshot', { format: 'png' })).data, 'base64');
        if (SHOTS) fs.writeFileSync(path.join(dirs.shots, name), png);
        if (PIXELS) {
          const saved = path.join(dirs.pixels, name);
          if (UPDATE) fs.writeFileSync(saved, png);
          else if (!fs.existsSync(saved)) pixelNotes.push(`${name}: no saved screenshot (run with --pixels --update)`);
          else {
            const r = compare(decode(fs.readFileSync(saved)), decode(png));
            if (r.count) {
              pixelNotes.push(`${name}: ${r.count === Infinity ? 'a different size' : `${r.count} pixels differ`}`);
              if (r.image) fs.writeFileSync(path.join(dirs.pixelDiff, name), r.image);
            }
          }
        }
      }
    };
    const s = {
      lang,
      /** Clicks the element like a mouse would (its centre), then waits for the page to settle. */
      async click(spec, opts = {}) {
        const p = await s.point(spec);
        await b.click(p.x, p.y, opts);
        await b.eval('__ui.settle()');
      },
      /** Drags one element onto another with the mouse; dy moves the drop point down from the target's centre. */
      async drag(from, to, dy = 0) {
        const a = await s.point(from), z = await s.point(to);
        await b.drag(a.x, a.y, z.x, z.y + dy);
        await b.eval('__ui.settle()');
      },
      /** Where the element is, waiting up to 2 s for it to show (a view still being drawn), as a person would. */
      async point(spec) {
        for (let tries = 0; ; tries++) {
          try { return await b.eval(`__ui.point(${JSON.stringify(spec)})`); } catch (e) {
            if (tries >= 20) throw e;
            await sleep(100);
          }
        }
      },
      async key(k, mods) { await b.key(k, mods); await b.eval('__ui.settle()'); },
      /** Types into the focused element (or the one given first). */
      async type(text, spec) {
        if (spec) await s.click(spec);
        await b.type(text);
        await b.eval('__ui.settle()');
      },
      /** Sets a text box's whole text, as selecting all of it and typing would. */
      async fill(spec, text) {
        await s.click(spec);
        await b.key('a', ['ctrl']);
        await b.key('Backspace');
        if (text) await b.type(text);
        await b.eval('__ui.settle()');
      },
      /** Picks an option of a <select> (Edge's own drop-down list isn't part of the page), as choosing it would. */
      async choose(spec, value) {
        await b.eval(`(() => { const el = __ui.find(${JSON.stringify(spec)}); el.focus(); el.value = ${JSON.stringify(value)};
          el.dispatchEvent(new Event('input', { bubbles: true })); el.dispatchEvent(new Event('change', { bubbles: true })); })()`);
        await b.eval('__ui.settle()');
      },
      /** Moves the mouse onto an element (hover). */
      async hover(spec) {
        const p = await s.point(spec);
        await b.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: p.x, y: p.y });
        await b.eval('__ui.settle()');
      },
      async wait(ms) { await sleep(ms); await b.eval('__ui.settle()'); },
      /** An app event, as the app would send it (dev/mock.js passes it to the page). */
      async emit(name, data) { await b.eval(`__ui.emit(${JSON.stringify(name)}, ${JSON.stringify(data)})`); await b.eval('__ui.settle()'); },
      eval: expr => b.eval(expr),
      snap: label => record(label),
    };
    try {
      b.problems.length = 0;
      const [w, h] = sc.size || SIZE;
      await b.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: 1, mobile: false });
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
    if (cover) await cover.take();
    if (JANK) {
      // Shifts not right after a click or key: something loaded and moved what was on screen.
      const late = (await b.eval('__ui.shifts')).filter(x => !x.recent && x.value >= 0.001);
      if (late.length) jank.push({ id, score: late.reduce((n, x) => n + x.value, 0), late });
      console.log(`  ${late.length ? 'JANK  ' : 'calm  '} ${id}${late.length ? ` ${late.reduce((n, x) => n + x.value, 0).toFixed(3)}` : ''}`);
      continue;
    }
    const text = parts.join('\n') + '\n';
    const file = path.join(BASE_DIR, `${id}.txt`);
    fs.writeFileSync(path.join(dirs.actual, `${id}.txt`), text);
    const broke = /(^|\n)== (FAILED|page errors)\n/.test(text); // a step that fails first leaves it at the very start
    let ok;
    if (UPDATE && !broke) {
      fs.writeFileSync(file, text);
      console.log(`  saved  ${id}`);
      ok = true;
    } else if (!fs.existsSync(file)) {
      console.log(`  NEW    ${id}: no saved copy (run with --update)${broke ? ' — and it failed:\n' + problems(text) : ''}`);
    } else {
      const want = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n'); // whatever line ends a checkout gave it
      ok = want === text && !broke;
      if (ok && !pixelNotes.length) console.log(`  PASS   ${id}`);
      else if (!ok) console.log(broke ? `  FAIL   ${id}\n${problems(text)}` : `  DIFF   ${id}\n${diff(want, text)}`);
    }
    if (pixelNotes.length) { ok = false; console.log(`  PIXELS ${id}\n${pixelNotes.map(n => `         ${n}`).join('\n')}`); }
    if (!ok) failed++;
  }
}
await b.close();
server.close();
if (cover) cover.report();
if (JANK) {
  jank.sort((x, y) => y.score - x.score);
  const lines = jank.map(j => [`${j.score.toFixed(3)}  ${j.id}`, ...j.late.slice(0, 6).map(x => `         ${x.at} ms  ${x.value.toFixed(3)}  ${x.sources.map(s => `${s.node} y${s.from[0]}→${s.to[0]} h${s.from[1]}→${s.to[1]}`).join(', ')}`)].join('\n'));
  fs.writeFileSync(path.join(OUT, 'jank.txt'), lines.join('\n') + '\n');
  console.log(`\n${jank.length} scenario(s) where something jumps after loading (Tests/ui/out/jank.txt):\n${lines.slice(0, 15).join('\n')}`);
  process.exit(0);
}
console.log(failed ? `${failed} scenario(s) differ — see Tests/ui/out` : `All ${list.length} scenarios match (${Math.round((Date.now() - started) / 1000)} s).`);
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

/**
 * Which characters of app.js ran in any scenario (V8's block coverage, from each page before the next one loads), and
 * the functions that never did, biggest first: where a scenario is missing.
 */
async function startCoverage() {
  const file = path.join(REPO, 'RobloxManager', 'wwwroot', 'app.js');
  const src = fs.readFileSync(file, 'utf8');
  const ran = new Uint8Array(src.length);
  const fns = new Map(); // start offset → { name, end, called }
  await b.send('Profiler.enable');
  await b.send('Profiler.startPreciseCoverage', { callCount: true, detailed: true });
  return {
    async take() {
      const { result } = await b.send('Profiler.takePreciseCoverage');
      const app = result.find(r => r.url.endsWith('/wwwroot/app.js'));
      if (!app) return;
      const count = new Int32Array(src.length).fill(-1);
      // Ranges nest: each function's outer range, then blocks inside it that ran a different number of times.
      for (const fn of app.functions) {
        for (const r of fn.ranges) count.fill(r.count, r.startOffset, Math.min(r.endOffset, src.length));
        const whole = fn.ranges[0];
        const f = fns.get(whole.startOffset) || { name: fn.functionName, end: whole.endOffset, called: false };
        f.called ||= whole.count > 0;
        fns.set(whole.startOffset, f);
      }
      for (let i = 0; i < src.length; i++) if (count[i] > 0) ran[i] = 1;
    },
    report() {
      const code = i => !/\s/.test(src[i]);
      let total = 0, hit = 0;
      for (let i = 0; i < src.length; i++) if (code(i)) { total++; hit += ran[i]; }
      // Functions never called, biggest first, leaving out those inside another one never called.
      const lineOf = i => src.slice(0, i).split('\n').length;
      const never = [...fns].filter(([, f]) => !f.called).map(([start, f]) => ({ ...f, start })).sort((x, y) => x.start - y.start);
      const outer = never.filter(f => !never.some(o => o !== f && o.start <= f.start && o.end >= f.end));
      const misses = outer.map(f => ({ name: f.name || '(anonymous)', line: lineOf(f.start), size: f.end - f.start })).filter(f => f.size >= 150);
      misses.sort((x, y) => y.size - x.size);
      const lines = [`app.js: ${(100 * hit / total).toFixed(1)}% of its code ran in these scenarios (${fns.size} functions, ${never.length} never called)`, '',
        'Biggest functions never called (name, line, size):',
        ...misses.map(f => `  ${f.name.slice(0, 34).padEnd(34)} line ${String(f.line).padStart(5)}  ${String(f.size).padStart(6)} chars`)];
      fs.writeFileSync(path.join(OUT, 'coverage.txt'), lines.join('\n') + '\n');
      console.log(lines.slice(0, 33).join('\n') + '\n  (all in Tests/ui/out/coverage.txt)');
    },
  };
}
