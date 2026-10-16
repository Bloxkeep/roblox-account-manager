// Builds ui/src into RobloxManager/wwwroot/ui.js, which the page loads before app.js (a generated file: not committed;
// RobloxManager.csproj runs this before every build, and Tests/ui/run.mjs before the interface checks).
//   node build.mjs          once
//   node build.mjs --watch  again on every change (with the preview page open)
import * as esbuild from 'esbuild';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const options = {
  entryPoints: [path.join(HERE, 'src', 'main.tsx')],
  outfile: path.join(HERE, '..', 'RobloxManager', 'wwwroot', 'ui.js'),
  bundle: true,
  format: 'iife',
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
