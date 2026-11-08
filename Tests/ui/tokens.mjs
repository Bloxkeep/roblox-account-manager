// Does app.css keep to Bloxkeep's scales? Font sizes, gaps, corner radii and transition times are meant to come from the
// tokens in its first block (--fs-*, --sp-*, --r-*, --dur-*), so the same thing looks the same everywhere. This reads the
// file (no browser, no build) and lists every value written by hand instead.
//   node Tests/ui/tokens.mjs            the report; exit code 0
//   node Tests/ui/tokens.mjs --strict   fails (exit 1) when a hand-written value appears more often than tokens-baseline.json allows
//   node Tests/ui/tokens.mjs --update   writes the current counts as the new baseline (run it after removing some)
//   node Tests/ui/tokens.mjs --fix      writes the tokens in where a hand-written value equals one exactly, then proves
//                                       the file means the same: every token put back as its value gives the old text
//
// What it leaves alone: 0, percentages, values already using var(), gaps of 1-2px and radii up to 4px (hairlines, meter
// ends, the switch's knob), animation timings (long loops of the picture themes).
import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
const cssPath = join(root, 'RobloxManager', 'wwwroot', 'app.css');
const basePath = join(root, 'Tests', 'ui', 'tokens-baseline.json');
const args = new Set(process.argv.slice(2));

const css = readFileSync(cssPath, 'utf8');

/** The scales, read from the tokens themselves so the two can't disagree. */
function scale(prefix) {
  const map = new Map();
  for (const m of css.matchAll(new RegExp(`--(${prefix}-[a-z0-9]+):\\s*([^;]+);`, 'g'))) if (!map.has(m[2].trim())) map.set(m[2].trim(), m[1]);
  return map; // value → token name
}
const SCALES = { size: scale('fs'), gap: scale('sp'), radius: scale('r'), time: scale('dur') };

/** Comments blanked to spaces of the same length, so every index still points into the real file. */
const plain = css.replace(/\/\*[\s\S]*?\*\//g, c => c.replace(/[^\n]/g, ' '));
const lineAt = i => css.slice(0, i).split('\n').length;

const KINDS = [
  { kind: 'size', props: /^font-size$/, unit: /(?<![\w.-])(\d*\.?\d+px)(?![\w-])/g },
  { kind: 'size', props: /^font$/, unit: /(?<![\w./-])(\d*\.?\d+px)(?=\s*[\s/])/g, first: true },
  { kind: 'gap', props: /^(row-|column-)?gap$/, unit: /(?<![\w.-])(\d*\.?\d+px)(?![\w-])/g, keep: v => parseFloat(v) <= 2 },
  { kind: 'radius', props: /^border(-(top|bottom|start|end)-(left|right|start|end))?-radius$/, unit: /(?<![\w.-])(\d*\.?\d+px)(?![\w-])/g, keep: v => parseFloat(v) <= 4 },
  { kind: 'time', props: /^transition(-duration|-delay)?$/, unit: /(?<![\w.-])(\d*\.?\d+m?s)(?![\w-])/g, keep: v => parseFloat(v) === 0 },
];

/** Every hand-written value: {kind, prop, value, line, start, end (into css)}. */
function scan() {
  const found = [];
  for (const block of plain.matchAll(/\{([^{}]*)\}/g)) {
    const bodyAt = block.index + 1;
    for (const decl of block[1].matchAll(/(^|;)\s*([a-z-]+)\s*:\s*([^;]*)/g)) {
      const prop = decl[2], value = decl[3];
      const valueAt = bodyAt + decl.index + decl[0].length - value.length;
      for (const k of KINDS) {
        if (!k.props.test(prop)) continue;
        const v = value.replace(/url\([^)]*\)|"[^"]*"|'[^']*'/g, s => ' '.repeat(s.length)); // never read inside urls or names
        for (const m of v.matchAll(k.unit)) {
          if (k.keep?.(m[1])) continue;
          found.push({ kind: k.kind, prop, value: m[1], line: lineAt(valueAt + m.index), start: valueAt + m.index, end: valueAt + m.index + m[1].length });
          if (k.first) break; // a font shorthand's first length is its size; the one after the slash is line height
        }
      }
    }
  }
  return found;
}

/** The same value can be written .15s or 150ms, 13px or 13.0px: compare as numbers in one unit. */
const norm = v => v.endsWith('ms') ? `${+parseFloat(v) / 1000}s` : v.endsWith('s') ? `${+parseFloat(v)}s` : `${+parseFloat(v)}px`;
function tokenFor(kind, value) {
  for (const [v, name] of SCALES[kind]) if (norm(v) === norm(value)) return name;
  return null;
}

const found = scan();
const key = f => `${f.kind} ${norm(f.value)}`;

if (args.has('--fix')) {
  let out = '', at = 0, n = 0;
  for (const f of [...found].sort((a, b) => a.start - b.start)) {
    const name = tokenFor(f.kind, f.value);
    if (!name) continue;
    out += css.slice(at, f.start) + `var(--${name})`; at = f.end; n++;
  }
  out += css.slice(at);
  // Proof: each token this run wrote, put back as the value it replaced (checked equal to the token's own value), gives
  // the old file byte for byte; and the fixed file, scanned again, has no value left that a token equals.
  if (rebuild(out) !== css) { console.error('Stopped: putting the tokens back does not give the old file. Nothing written.'); process.exit(2); }
  writeFileSync(cssPath, out);
  console.log(`Wrote ${n} tokens where a hand-written value equalled one. Putting them back gives the old file exactly.`);
  process.exit(0);
}

/** The fixed text with each var(--token) this run wrote turned back into the literal it replaced; null if the two texts
 * differ anywhere else, or a token's value isn't the literal it stands for. */
function rebuild(fixed) {
  let i = 0, j = 0, result = '';
  const edits = found.filter(f => tokenFor(f.kind, f.value)).sort((a, b) => a.start - b.start);
  for (const f of edits) {
    const same = css.slice(i, f.start);
    if (fixed.slice(j, j + same.length) !== same) return null;
    result += same; i = f.start; j += same.length;
    const token = `var(--${tokenFor(f.kind, f.value)})`;
    if (fixed.slice(j, j + token.length) !== token) return null;
    const value = [...SCALES[f.kind]].find(([, n]) => n === tokenFor(f.kind, f.value))[0];
    if (norm(value) !== norm(f.value)) return null;
    result += f.value; i = f.end; j += token.length;
  }
  result += fixed.slice(j);
  return result;
}

// The report: counts by kind and value, with the token to use where one fits.
const counts = {};
for (const f of found) counts[key(f)] = (counts[key(f)] || 0) + 1;
const byKind = {};
for (const f of found) (byKind[f.kind] ??= new Map()).set(norm(f.value), [...((byKind[f.kind].get(norm(f.value))) || []), f.line]);
const names = { size: 'Font sizes', gap: 'Gaps', radius: 'Corner radii', time: 'Transition times' };
for (const kind of Object.keys(names)) {
  const rows = [...(byKind[kind] || new Map())].sort((a, b) => b[1].length - a[1].length);
  console.log(`\n${names[kind]}: ${rows.reduce((n, r) => n + r[1].length, 0)} written by hand, ${rows.length} different values`);
  for (const [value, lines] of rows) {
    const t = tokenFor(kind, value);
    console.log(`  ${value.padEnd(8)} ×${String(lines.length).padEnd(4)} ${t ? `→ var(--${t})` : 'off the scale'}  (line ${lines.slice(0, 6).join(', ')}${lines.length > 6 ? ', …' : ''})`);
  }
}

if (args.has('--update')) {
  writeFileSync(basePath, JSON.stringify(Object.fromEntries(Object.entries(counts).sort()), null, 1) + '\n');
  console.log(`\nBaseline written: ${found.length} hand-written values.`);
}
if (args.has('--strict')) {
  const base = JSON.parse(readFileSync(basePath, 'utf8'));
  const worse = Object.entries(counts).filter(([k, n]) => n > (base[k] || 0));
  if (worse.length) {
    console.log(`\nMore hand-written values than the baseline allows (use a token from the first block of app.css):`);
    for (const [k, n] of worse) console.log(`  ${k}: ${n} (baseline ${base[k] || 0})`);
    process.exit(1);
  }
  const better = Object.entries(base).filter(([k, n]) => (counts[k] || 0) < n);
  if (better.length) console.log(`\nFewer than the baseline (${better.map(([k]) => k).join('; ')}): run with --update to lock that in.`);
}
