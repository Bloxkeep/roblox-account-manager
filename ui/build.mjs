// Builds ui/src into RobloxManager/wwwroot: ui.js, which the app's page loads before app.js, and one bundle per small
// page (ui-tray.js, ui-panel.js, ui-mini.js, ui-overlay.js, ui-dialog.js, ui-browser.js from ui/src/pages), so a small
// window parses only its own part and not the whole app. Generated files: not committed; RobloxManager.csproj runs this
// before every build (and embeds them), and it is run by hand before the interface checks (Tests/ui/run.mjs).
//   node build.mjs          once
//   node build.mjs --watch  again on every change (with a preview page open)
import * as esbuild from 'esbuild';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
/** The small pages: wwwroot/<page>.html loads ui-<page>.js, built from ui/src/pages/<page>.tsx. */
const PAGES = ['tray', 'panel', 'mini', 'overlay', 'dialog', 'browser'];
const options = {
  entryPoints: {
    ui: path.join(HERE, 'src', 'main.tsx'),
    ...Object.fromEntries(PAGES.map(p => [`ui-${p}`, path.join(HERE, 'src', 'pages', `${p}.tsx`)])),
  },
  outdir: path.join(HERE, '..', 'RobloxManager', 'wwwroot'),
  bundle: true,
  format: 'iife', // each bundle on its own, no shared chunks: a page is one classic script
  target: 'chrome120', // WebView2 is Edge's engine, kept up to date by Windows
  jsx: 'automatic',
  minify: true,
  define: { 'process.env.NODE_ENV': '"production"' },
  legalComments: 'eof', // React's MIT notice stays with its code
  banner: { js: '/* Bloxkeep: built from ui/src by ui/build.mjs. Edit the source, not this file. */' },
  logLevel: 'warning',
};

if (process.argv.includes('--watch')) {
  const ctx = await esbuild.context(options);
  await ctx.watch();
  console.log('Watching ui/src…');
} else {
  await esbuild.build(options);
}
