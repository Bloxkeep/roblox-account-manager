// Re-makes the pictures of Bloxkeep's own interface in docs/screenshots (English) and docs/screenshots/vi (Vietnamese)
// from ui/dev/preview.html: the real page on the made-up accounts of ui/dev/mock.js (with real pictures, ui/dev/real-pics.js), in a headless Edge (nothing opens
// on screen). Each picture keeps the size and framing it had; the photos of real Roblox, the installer's pictures and
// the Discord messages are not made here, nor hero.png, games.png and macros.png (from the site's demo app).
//   cd ui && node build.mjs                      first, so the page is today's
//   node docs/screenshots/shoot.mjs              every picture, both languages
//   node docs/screenshots/shoot.mjs --only lock,stats --lang en
//   node docs/screenshots/shoot.mjs --out Tests/ui/out/docshots   somewhere else, to compare first
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const REPO = path.resolve(HERE, '..', '..');
const { launch, sleep } = await import(pathToFileURL(path.join(REPO, 'Tests/ui/cdp.mjs')).href);
const { serve } = await import(pathToFileURL(path.join(REPO, 'Tests/ui/serve.mjs')).href);
const args = process.argv.slice(2);
const opt = f => (args.includes(f) ? args[args.indexOf(f) + 1] : null);
const ONLY = opt('--only')?.split(',');
const LANGS = opt('--lang')?.split(',') || ['en', 'vi'];
const OUT = opt('--out') ? path.resolve(opt('--out')) : HERE;
// The same clock (16 October 2026, 12:00) and numbers on every run, and __ui.find() for elements by their words.
const hooks = fs.readFileSync(path.join(REPO, 'Tests/ui/page-hooks.js'), 'utf8');
// Real game pictures and Roblox avatars (site/demo-pics) instead of the mock's drawn ones, with made-up logins, as on
// the website's try-it page.
const realPics = `window.__demoPics = ${fs.readFileSync(path.join(REPO, 'site/demo-pics/map.json'), 'utf8')};
${fs.readFileSync(path.join(REPO, 'ui/dev/real-pics.js'), 'utf8')}`;

const BIG = [1280, 800, 1.5]; // 1920 × 1200: the README's large pictures
const SMALL = [1320, 840, 1]; // 1320 × 840: the guide's pictures

/** The Main account's card (and its row in the list). */
const MAIN = '.card[data-id="00000000-0000-4000-8000-000000000001"]';

const JOBS = [
  { file: 'accounts-list.png', size: BIG, steps: async s => { await s.click({ key: 'layout.list' }); } },
  { file: 'presets.png', size: BIG, q: 'view=presets' },
  { file: 'stats.png', size: BIG, q: 'stats=1' },
  { file: 'lock.png', size: BIG, q: 'locked=1' },
  { file: 'theme-photos.jpg', size: BIG, q: 'theme=lanternriver&select=1', wait: 2500 },

  { file: '2-restart-to-update.png', size: SMALL, q: 'update=1' },
  { file: '3-settings-updates.png', size: [1320, 1200, 1], q: 'settings=updates&update=1' },
  { file: '6-whats-new.png', size: SMALL, q: 'settings=updates', steps: async s => { await s.click({ css: '[data-act=whats-new]' }); } },
  { file: 'account-menu.png', size: SMALL, q: 'menu=1' },
  { file: 'add-account.png', size: SMALL, steps: async s => { await s.click({ key: 'acc.add' }); } },
  { file: 'arrange-layouts.png', size: SMALL, q: 'select=1', steps: async s => { await s.click({ css: '[data-act=dock-more]' }); } },
  { file: 'before-you-open.png', size: SMALL, q: 'select=1&multioff=1', steps: async s => { await s.click({ css: '[data-act=launch]' }); } },
  {
    file: 'card-rejoin.png', size: SMALL, q: 'rejoin=1&kicked=1',
    steps: async s => { await s.eval(`document.querySelector('[data-act="cancel-rejoin"]')?.closest('[data-id]')?.scrollIntoView({ block: 'center' })`); },
  },
  { file: 'game-picker.png', size: SMALL, steps: async s => { await s.click({ css: '#place' }); } },
  { file: 'palette.png', size: SMALL, q: 'select=1', steps: async s => { await s.key('k', ['ctrl']); } },
  { file: 'servers.png', size: SMALL, q: 'select=1', steps: async s => { await s.click({ css: '[data-act=servers]' }); } },
  // The form scrolled to what the preset does to the Roblox windows it opens (that part and Discord's unfolded).
  {
    file: 'preset-editor.png', size: SMALL, q: 'preset=1',
    steps: async s => {
      await s.eval(`(() => {
        document.activeElement?.blur?.();
        for (const id of ['ps-opts', 'ps-dc']) document.getElementById(id)?.setAttribute('open', '');
        document.getElementById('ps-opts')?.scrollIntoView({ block: 'start' });
      })()`);
    },
  },
  { file: 'multi-help.png', size: SMALL, steps: async s => { await s.click({ css: '[data-act=multi-help]' }); } },
  // The Main account's menu (Hide Roblox window in it) beside its card, at 1.4× as before.
  {
    file: 'hide-window.png', size: [1280, 800, 1.4],
    steps: async s => { await s.click({ css: `${MAIN} [data-act=card-menu]` }); },
    clip: async s => {
      const card = await s.rect(MAIN), menu = await s.rect('.menu');
      return { x: card.x - 4, y: Math.min(card.y, menu.y) - 8, width: 570, height: 668 };
    },
  },
  // Settings › Discord › Control from Discord with a code to type into /pc link.
  {
    file: 'discord-control.png', size: BIG, q: 'settings=discord&discord=1&remote=code',
    steps: async s => { await s.eval(`document.querySelector('#sec-dc-remote').scrollIntoView({ block: 'center' })`); },
    clip: s => s.rect('#sec-dc-remote .set-card'),
  },
  // Six themes side by side, each the accounts page with two accounts selected, named under it.
  { file: 'themes.png', themes: ['dark', 'city', 'aurora', 'sakura', 'paper', 'terminal'] },
  { file: 'roblox-updates.png', size: SMALL, q: 'settings=roblox&rw=changed' },
  { file: 'settings-backup.png', size: [1320, 1080, 1], q: 'settings=backup&backup=1' },
  { file: 'settings-staying.png', size: SMALL, q: 'settings=perf' },
  { file: 'settings.png', size: SMALL, q: 'settings=launch' },
  { file: 'discord.png', size: SMALL, q: 'settings=discord&discord=1' },
  { file: 'lock-update.png', size: SMALL, q: 'locked=1&lockupdate=40' },
];

const server = await serve(REPO);
const BASE = `http://127.0.0.1:${server.port}/ui/dev/preview.html`;
const jobs = JOBS.filter(j => !ONLY || ONLY.some(o => j.file.includes(o)));

for (const lang of LANGS) {
  const b = await launch({ width: 1320, height: 1200, locale: lang === 'vi' ? 'vi-VN' : 'en-US' });
  await b.send('Page.addScriptToEvaluateOnNewDocument', { source: hooks });
  await b.send('Page.addScriptToEvaluateOnNewDocument', { source: realPics });
  const s = {
    eval: e => b.eval(e),
    settle: () => b.eval('__ui.settle(300, 4000)'),
    async point(spec) {
      for (let tries = 0; ; tries++) {
        try { return await b.eval(`__ui.point(${JSON.stringify(spec)})`); } catch (e) {
          if (tries >= 20) throw e;
          await sleep(100);
        }
      }
    },
    async click(spec) { const p = await s.point(spec); await b.click(p.x, p.y); await s.settle(); },
    async hover(spec) { const p = await s.point(spec); await b.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: p.x, y: p.y }); },
    async key(k, mods) { await b.key(k, mods); await s.settle(); },
    rect: css => b.eval(`(() => { const r = document.querySelector(${JSON.stringify(css)}).getBoundingClientRect(); return { x: r.x, y: r.y, width: r.width, height: r.height }; })()`),
  };
  /** The page at that size with these options, steps done, settled; returns the picture's bytes. */
  async function take(job, q0, size) {
    const [w, h, scale] = size;
    await b.send('Emulation.setDeviceMetricsOverride', { width: w, height: h, deviceScaleFactor: scale, mobile: false });
    const q = new URLSearchParams(q0 || '');
    if (!q.has('theme')) q.set('theme', 'dark');
    q.set('lang', lang);
    await b.goto(`${BASE}?${q}`);
    await b.eval(`__ui.lang = ${JSON.stringify(lang)}`);
    await sleep(1200);
    await s.settle();
    // The mouse off the page's buttons, so nothing shows its hover look by chance.
    await b.send('Input.dispatchMouseEvent', { type: 'mouseMoved', x: w - 2, y: 2 });
    if (job.steps) await job.steps(s, lang);
    await sleep(job.wait || 500);
    await s.settle();
    const clip = job.clip ? { ...(await job.clip(s)), scale: 1 } : undefined;
    const jpg = job.file.endsWith('.jpg');
    const shot = await b.send('Page.captureScreenshot', { format: jpg ? 'jpeg' : 'png', ...(jpg ? { quality: 88 } : {}), ...(clip ? { clip } : {}) });
    return shot.data;
  }
  /** Theme pictures (1320 × 840, two accounts selected) laid out three to a row, 510 px wide, named under each. */
  async function themeSheet(job) {
    const tiles = [];
    for (const id of job.themes) {
      const data = await take({ file: 'x.png' }, `select=1&theme=${id}`, SMALL);
      tiles.push({ src: `data:image/png;base64,${data}`, name: await b.eval(`window.I18N?.[${JSON.stringify(lang)}]?.['theme.${id}'] || ${JSON.stringify(id)}`) });
    }
    await b.send('Emulation.setDeviceMetricsOverride', { width: 1600, height: 760, deviceScaleFactor: 1, mobile: false });
    await b.goto('about:blank');
    await b.eval(`(() => {
      document.body.innerHTML = ${JSON.stringify(tiles.map(t => `<figure><img src="${t.src}"><figcaption>${t.name}</figcaption></figure>`).join(''))};
      const st = document.createElement('style');
      st.textContent = 'html,body{margin:0;background:#111318}body{display:grid;grid-template-columns:repeat(3,510px);gap:0 18px;padding:16px;font:600 19px/1 "Segoe UI",sans-serif;color:#eef0f3}figure{margin:0;height:376px}img{display:block;width:510px;height:324px;border-radius:6px;outline:1px solid rgba(255,255,255,.08)}figcaption{margin-top:14px;padding-left:2px}';
      document.head.append(st);
      return Promise.all([...document.images].map(i => i.decode()));
    })()`);
    await sleep(200);
    return (await b.send('Page.captureScreenshot', { format: 'png' })).data;
  }
  for (const job of jobs) {
    let data;
    try {
      data = job.themes ? await themeSheet(job) : await take(job, job.q, job.size);
    } catch (e) {
      console.log(`${lang}/${job.file}: ${e.message}`);
      continue;
    }
    const file = lang === 'en' ? path.join(OUT, job.file) : path.join(OUT, 'vi', job.file);
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, Buffer.from(data, 'base64'));
    console.log(path.relative(REPO, file));
  }
  if (b.problems.length) console.log(b.problems.slice(0, 10).join('\n'));
  await b.close();
}
server.close();
process.exit(0);
