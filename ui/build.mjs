// Builds ui/src into RobloxManager/wwwroot: ui.js, which the app's page loads before app.js, and one bundle per small
// page (ui-tray.js, ui-panel.js, ui-mini.js, ui-overlay.js, ui-dialog.js, ui-browser.js from ui/src/pages), so a small
// window parses only its own part and not the whole app. A small page gets its own words and stylesheet too:
//   - ui-<page>.js starts with the words of wwwroot/i18n.js its code can ask for (pages/page.tsx loads all of i18n.js
//     if one is ever missing);
//   - ui-<page>.css is app.css without the rules that can't match anything the page draws: a rule is left out only when
//     a class or id it needs is nowhere in the page's bundle, its .html or the app's C# (so a state name the app sends,
//     "recording", still counts). The rules kept stay in app.css's order, so the cascade is the same.
// The app's own page (index.html) still loads all of app.css: the copy the app embeds is obj/ui/app.css, the same
// rules without comments and extra spaces. app.css stays the file to edit; after a change to it, build again before
// looking at a small page (--watch does).
// Generated files: not committed; RobloxManager.csproj runs this before every build (and embeds them), and it is run by
// hand before the interface checks (Tests/ui/run.mjs).
//   node build.mjs          once
//   node build.mjs --watch  again on every change (with a preview page open)
import * as esbuild from 'esbuild';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const APP = path.join(HERE, '..', 'RobloxManager');
const WWW = path.join(APP, 'wwwroot');
/** The small pages: wwwroot/<page>.html loads ui-<page>.js and ui-<page>.css, built from ui/src/pages/<page>.tsx. */
const PAGES = ['tray', 'panel', 'mini', 'overlay', 'dialog', 'browser'];
const BANNER = '/* Bloxkeep: built from ui/src by ui/build.mjs. Edit the source, not this file. */';
const CSS_BANNER = '/* Bloxkeep: built from wwwroot/app.css by ui/build.mjs. Edit app.css, not this file. */';

/** i18n.js's words ({ en: {key: text}, vi: {…} }), as the page gets them. */
function strings() {
  const window = {};
  vm.runInNewContext(fs.readFileSync(path.join(WWW, 'i18n.js'), 'utf8'), { window });
  return window.I18N;
}

/**
 * The words a bundle's code can ask for: each key written in it, each key a `prefix.${…}` or "prefix." + … in it can
 * make, and each one's singular ("…1") and plural twin (tn picks the "1" key; Vietnamese falls back from it to the plural).
 */
function wordsFor(code, dict) {
  const keys = new Set(Object.values(dict).flatMap(Object.keys));
  const prefixes = new Set([...code.matchAll(/([A-Za-z][\w.-]*\.)(?:\$\{|["'`]\s*\+)/g)].map(m => m[1]));
  const used = new Set([...keys].filter(k => code.includes(k) || [...prefixes].some(p => k.startsWith(p))));
  for (const k of [...used]) {
    if (keys.has(`${k}1`)) used.add(`${k}1`);
    if (k.endsWith('1') && keys.has(k.slice(0, -1))) used.add(k.slice(0, -1));
  }
  return Object.fromEntries(Object.entries(dict).map(([lang, all]) => [lang, Object.fromEntries(Object.entries(all).filter(([k]) => used.has(k)))]));
}

// ── The stylesheets ──
/** At s[i], a quoted string: the index after it. */
function stringEnd(s, i) {
  let j = i + 1;
  while (j < s.length && s[j] !== s[i]) j += s[j] === '\\' ? 2 : 1;
  return j + 1;
}
/** The text with its comments out and its spaces squeezed (strings as they are): one space where any were, none next to { } ;. */
function squeeze(s) {
  let out = '', gap = false;
  for (let i = 0; i < s.length;) {
    const c = s[i];
    if (s.startsWith('/*', i)) { const e = s.indexOf('*/', i + 2); i = e < 0 ? s.length : e + 2; gap = true; continue; }
    if (/\s/.test(c)) { i++; gap = true; continue; }
    if (gap && out && !'{};'.includes(out.at(-1)) && !'{};'.includes(c)) out += ' ';
    gap = false;
    if (c === '"' || c === "'") { const e = stringEnd(s, i); out += s.slice(i, e); i = e; continue; }
    out += c;
    i++;
  }
  return out;
}
/** A stylesheet's rules in order: { head, body } (a style rule or an at-rule's block, body its raw text) or { head } (a statement). */
function rules(s) {
  const out = [];
  let i = 0;
  while (i < s.length) {
    // The head: up to its { or ; (outside strings, comments and brackets).
    let head = '', depth = 0;
    for (; i < s.length; i++) {
      const c = s[i];
      if (s.startsWith('/*', i)) { const e = s.indexOf('*/', i + 2); i = (e < 0 ? s.length : e + 2) - 1; head += ' '; continue; }
      if (c === '"' || c === "'") { const e = stringEnd(s, i); head += s.slice(i, e); i = e - 1; continue; }
      if (c === '(' || c === '[') depth++;
      else if (c === ')' || c === ']') depth--;
      else if (depth === 0 && (c === '{' || c === ';' || c === '}')) break;
      head += c;
    }
    head = head.trim();
    if (i >= s.length) { if (head) throw new Error(`app.css: unfinished "${head.slice(0, 40)}"`); break; }
    if (s[i] !== '{') { i++; if (head) out.push({ head }); continue; }
    // The body: up to the matching }.
    const start = ++i;
    for (let level = 1; ; i++) {
      if (i >= s.length) throw new Error(`app.css: "${head.slice(0, 40)}" is never closed`);
      const c = s[i];
      if (s.startsWith('/*', i)) { const e = s.indexOf('*/', i + 2); i = (e < 0 ? s.length : e + 2) - 1; continue; }
      if (c === '"' || c === "'") { i = stringEnd(s, i) - 1; continue; }
      if (c === '{') level++;
      else if (c === '}' && --level === 0) break;
    }
    out.push({ head, body: s.slice(start, i) });
    i++;
  }
  return out;
}
/** A selector list's selectors (split at the commas outside brackets). */
function selectorsOf(list) {
  const out = [];
  let depth = 0, cur = '';
  for (const c of list) {
    if (c === '(' || c === '[') depth++;
    else if (c === ')' || c === ']') depth--;
    if (c === ',' && depth === 0) { out.push(cur); cur = ''; } else cur += c;
  }
  return [...out, cur];
}
/** The classes and ids an element must have for the selector to match: those outside :not(…), :is(…), […] and the like. */
function needs(selector) {
  let flat = '', depth = 0;
  for (let i = 0; i < selector.length; i++) {
    const c = selector[i];
    if (c === '"' || c === "'") { i = stringEnd(selector, i) - 1; continue; }
    if (c === '(' || c === '[') depth++;
    else if (c === ')' || c === ']') depth--;
    else if (depth === 0) flat += c;
  }
  return [...flat.matchAll(/[.#](-?[_a-zA-Z][\w-]*)/g)].map(m => m[1]);
}
/** The stylesheet's rules a page can match (possible: whether a class or id can be on the page), squeezed. */
function sheet(list, possible) {
  let out = '';
  for (const r of list) {
    if (r.body === undefined) out += squeeze(r.head) + ';';
    else if (/^@(media|supports|container)\b/.test(r.head)) {
      const inner = sheet(rules(r.body), possible);
      if (inner) out += `${squeeze(r.head)}{${inner}}`;
    } else if (r.head.startsWith('@') || selectorsOf(r.head).some(sel => needs(sel).every(possible))) {
      out += `${squeeze(r.head)}{${squeeze(r.body).trim()}}`; // keyframes and the like whole
    }
  }
  return out;
}
/** Everything the app's C# says, in small letters: a class can come from a value it sends (a state's name). */
function csharp() {
  const walk = dir => fs.readdirSync(dir, { withFileTypes: true }).flatMap(e =>
    e.isDirectory() ? (['bin', 'obj', 'wwwroot'].includes(e.name) ? [] : walk(path.join(dir, e.name))) : e.name.endsWith('.cs') ? [path.join(dir, e.name)] : []);
  return walk(APP).map(f => fs.readFileSync(f, 'utf8')).join('\n').toLowerCase();
}
/** Whether a class or id can be on a page whose code (its bundle and .html) is this: written in it or the C#, or made by
 * a `prefix-${…}`, "prefix-" + …, `${…}-suffix` in it. */
function possibleOn(code, cs) {
  const text = (code + '\n' + cs).toLowerCase();
  const prefixes = [...new Set([...code.matchAll(/([\w-]+-)(?:\$\{|["'`]\s*\+)/g)].map(m => m[1].toLowerCase()))];
  const suffixes = [...new Set([...code.matchAll(/(?:\}|\+\s*["'`])(-[\w-]+)/g)].map(m => m[1].toLowerCase()))];
  return name => {
    const n = name.toLowerCase();
    return text.includes(n) || prefixes.some(p => n.startsWith(p)) || suffixes.some(x => n.endsWith(x));
  };
}

/** Writes what esbuild built, each small page with its words in front (window.I18N, as i18n.js sets it) and its stylesheet. */
const write = {
  name: 'bloxkeep-write',
  setup(build) {
    build.onEnd(result => {
      if (result.errors.length) return;
      const dict = strings();
      const css = fs.readFileSync(path.join(WWW, 'app.css'), 'utf8'), all = rules(css), cs = csharp();
      for (const out of result.outputFiles) {
        let text = out.text;
        const page = /[\\/]ui-([a-z]+)\.js$/.exec(out.path)?.[1];
        if (page) {
          const words = `window.I18N=${JSON.stringify(wordsFor(text, dict))};`;
          text = text.startsWith(BANNER) ? `${BANNER}\n${words}${text.slice(BANNER.length)}` : words + text;
          const possible = possibleOn(text + fs.readFileSync(path.join(WWW, `${page}.html`), 'utf8'), cs);
          fs.writeFileSync(path.join(WWW, `ui-${page}.css`), `${CSS_BANNER}\n${sheet(all, possible)}\n`);
        }
        fs.writeFileSync(out.path, text);
      }
      fs.mkdirSync(path.join(APP, 'obj', 'ui'), { recursive: true });
      fs.writeFileSync(path.join(APP, 'obj', 'ui', 'app.css'), `${CSS_BANNER}\n${sheet(all, () => true)}\n`);
    });
  },
};

const options = {
  entryPoints: {
    ui: path.join(HERE, 'src', 'main.tsx'),
    ...Object.fromEntries(PAGES.map(p => [`ui-${p}`, path.join(HERE, 'src', 'pages', `${p}.tsx`)])),
  },
  outdir: WWW,
  write: false, // the plugin above writes them
  plugins: [write],
  bundle: true,
  format: 'iife', // each bundle on its own, no shared chunks: a page is one classic script
  target: 'chrome120', // WebView2 is Edge's engine, kept up to date by Windows
  jsx: 'automatic',
  minify: true,
  define: { 'process.env.NODE_ENV': '"production"' },
  legalComments: 'eof', // React's MIT notice stays with its code
  banner: { js: BANNER },
  logLevel: 'warning',
};

if (process.argv.includes('--watch')) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  // app.css and i18n.js aren't modules of the bundles: a change to them builds again too.
  let soon = null;
  for (const f of ['app.css', 'i18n.js']) fs.watch(path.join(WWW, f), () => { clearTimeout(soon); soon = setTimeout(() => ctx.rebuild(), 100); });
  console.log('Watching ui/src, app.css and i18n.js…');
} else {
  await esbuild.build(options);
}
