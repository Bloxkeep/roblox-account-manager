// The try-it page's app (build.mjs puts it in site/dist/demo/app/; /demo/ and /vi/demo/ show it in a frame): Bloxkeep's
// real interface from RobloxManager/wwwroot on the fake bridge ui/dev/mock.js, with ui/dev/demo.js keeping it offline
// and its accounts made up. ui.js is built, not committed: run `npm ci && npm run build` in ui/ first.
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';

/** Copies the app's files into <out>/demo/app/ and writes its page. */
export function buildDemo(root, out) {
  const www = path.join(root, 'RobloxManager', 'wwwroot');
  const dev = path.join(root, 'ui', 'dev');
  const dir = path.join(out, 'demo', 'app');
  if (!fs.existsSync(path.join(www, 'ui.js')))
    throw new Error('RobloxManager/wwwroot/ui.js is missing: build the interface first (cd ui && npm ci && npm run build).');
  fs.mkdirSync(dir, { recursive: true });

  // The main window's files: not the other windows' pages and bundles, not the What's new pictures (below).
  for (const f of fs.readdirSync(www, { withFileTypes: true }))
    if (f.isFile() && !/\.html$|^ui-.+\.(js|css)$/.test(f.name))
      fs.copyFileSync(path.join(www, f.name), path.join(dir, f.name));
  for (const f of ['mock.js', 'demo.js', 'wiki-sample.json'])
    fs.copyFileSync(path.join(dev, f), path.join(dir, f));
  // Real pictures (site/demo-pics.mjs): copied to pics/, their list read by demo.js from demo-pics.js.
  const pics = path.join(root, 'site', 'demo-pics');
  if (fs.existsSync(path.join(pics, 'map.json'))) {
    fs.mkdirSync(path.join(dir, 'pics'), { recursive: true });
    for (const f of fs.readdirSync(pics)) if (f !== 'map.json') fs.copyFileSync(path.join(pics, f), path.join(dir, 'pics', f));
    fs.writeFileSync(path.join(dir, 'demo-pics.js'), `window.__demoPics = ${fs.readFileSync(path.join(pics, 'map.json'), 'utf8')};`);
  } else fs.writeFileSync(path.join(dir, 'demo-pics.js'), '');
  // The Mini window's page: its own bundle, on the main page's demo bridge (demo.js's window.__demoMini).
  for (const f of ['ui-mini.js', 'ui-mini.css'])
    fs.copyFileSync(path.join(www, f), path.join(dir, f));
  fs.writeFileSync(path.join(dir, 'demo-mini.js'), 'window.__mockBridge = window.parent.__demoMini;\n');

  // What's new: the three newest versions, as the mock shows them, with their pictures.
  const news = JSON.parse(fs.readFileSync(path.join(root, 'RobloxManager', 'whatsnew.json'), 'utf8')).slice(0, 3);
  fs.writeFileSync(path.join(dir, 'whatsnew.json'), JSON.stringify(news));
  const pictures = new Set(JSON.stringify(news).match(/whatsnew\/[^"]+/g) || []);
  for (const p of pictures)
    for (const lang of ['en', 'vi']) {
      const name = p.replace('{lang}', lang), from = path.join(www, name);
      if (!fs.existsSync(from)) continue;
      fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true });
      fs.copyFileSync(from, path.join(dir, name));
    }

  // Each script's address changes with it, so a browser never mixes this build's files with the last one's.
  const v = f => createHash('sha256').update(fs.readFileSync(path.join(dir, f))).digest('hex').slice(0, 10);
  const script = f => `<script src="${f}?v=${v(f)}"></script>`;
  fs.writeFileSync(path.join(dir, 'mini.html'), `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self'; connect-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'">
<meta name="robots" content="noindex">
<title>Bloxkeep</title>
<link rel="stylesheet" href="ui-mini.css?v=${v('ui-mini.css')}">
</head>
<body class="panel-page mini-page">
<div id="mini"></div>
${['demo-mini.js', 'ui-mini.js'].map(script).join('\n')}
</body>
</html>
`);
  fs.writeFileSync(path.join(dir, 'index.html'), `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<!-- Only its own files: nothing is fetched from Roblox or anywhere else (ui/dev/demo.js swaps outside pictures). -->
<meta http-equiv="Content-Security-Policy" content="default-src 'self'; img-src 'self' data: blob:; media-src 'self' data: blob:; style-src 'self' 'unsafe-inline'; script-src 'self' 'unsafe-inline'; connect-src 'self'; font-src 'self' data:; worker-src 'self' blob:; object-src 'none'; base-uri 'none'; form-action 'none'">
<meta name="robots" content="noindex">
<title>Bloxkeep demo</title>
<link rel="stylesheet" href="app.css?v=${v('app.css')}">
</head>
<body>
<div id="app"><div class="splash"><img src="logo.svg" alt=""></div></div>
<div id="layer"></div>
<div id="toasts"></div>
<script>window.__mockQuery = 'nodelay=1&lang=' + (new URLSearchParams(location.search).get('lang') === 'vi' ? 'vi' : 'en');</script>
${['demo-pics.js', 'mock.js', 'demo.js', 'shared.js', 'i18n.js', 'ui.js', 'app.js', 'livescene.js'].map(script).join('\n')}
</body>
</html>
`);
}
