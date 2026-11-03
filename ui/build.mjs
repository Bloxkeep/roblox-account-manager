// Builds ui/src into RobloxManager/wwwroot: ui.js, which the app's page loads before app.js, and one bundle per small
// page (ui-tray.js, ui-panel.js, ui-mini.js, ui-overlay.js, ui-dialog.js, ui-browser.js from ui/src/pages), so a small
// window parses only its own part and not the whole app. A small page's bundle carries its own words too: the ones of
// wwwroot/i18n.js its code can ask for (pages/page.tsx loads all of i18n.js if one is ever missing). Generated files:
// not committed; RobloxManager.csproj runs this before every build (and embeds them), and it is run by hand before the
// interface checks (Tests/ui/run.mjs).
//   node build.mjs          once
//   node build.mjs --watch  again on every change (with a preview page open)
import * as esbuild from 'esbuild';
import fs from 'node:fs';
import path from 'node:path';
import vm from 'node:vm';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const WWW = path.join(HERE, '..', 'RobloxManager', 'wwwroot');
/** The small pages: wwwroot/<page>.html loads ui-<page>.js, built from ui/src/pages/<page>.tsx. */
const PAGES = ['tray', 'panel', 'mini', 'overlay', 'dialog', 'browser'];
const BANNER = '/* Bloxkeep: built from ui/src by ui/build.mjs. Edit the source, not this file. */';

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

/** Writes what esbuild built, each small page's words put in front of its code (window.I18N, as i18n.js sets it). */
const write = {
  name: 'bloxkeep-write',
  setup(build) {
    build.onEnd(result => {
      if (result.errors.length) return;
      const dict = strings();
      for (const out of result.outputFiles) {
        let text = out.text;
        if (/[\\/]ui-[a-z]+\.js$/.test(out.path)) {
          const words = `window.I18N=${JSON.stringify(wordsFor(text, dict))};`;
          text = text.startsWith(BANNER) ? `${BANNER}\n${words}${text.slice(BANNER.length)}` : words + text;
        }
        fs.writeFileSync(out.path, text);
      }
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
  console.log('Watching ui/src… (words changed in i18n.js reach the small pages on the next change or build)');
} else {
  await esbuild.build(options);
}
