// What's new demos, filmed like a screen recording from ui/dev/preview.html (made-up accounts): the browser's own
// screencast (frames as they are painted, with their times), a real mouse (hover states show) with a cursor drawn
// where it is, real clicks and drags. Each take ends where it starts, so the clip loops.
//   python -m http.server 5178            (from the repository, as .claude/launch.json's ui-preview)
//   node ui/dev/demos/record.mjs themes vi <frames folder>
//   python ui/dev/demos/encode.py <frames folder> RobloxManager/wwwroot/whatsnew/<version>-themes.vi.webp
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
const [name, lang = 'vi', out] = process.argv.slice(2);
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..');
const { launch, sleep } = await import(pathToFileURL(path.join(repo, 'Tests/ui/cdp.mjs')).href);
fs.mkdirSync(out, { recursive: true });
for (const f of fs.readdirSync(out)) fs.rmSync(path.join(out, f));

const W = 1280, H = 800;
const b = await launch({ width: W, height: H, scale: 1.5, locale: lang === 'vi' ? 'vi-VN' : 'en-US' });
let at = { x: W / 2, y: H * .7 };
const mouse = (type, p, extra = {}) => b.send('Input.dispatchMouseEvent', { type, x: p.x, y: p.y, ...extra });
const ease = t => t < .5 ? 4 * t * t * t : 1 - (-2 * t + 2) ** 3 / 2;
/** Moves the mouse there over ms, in a slight arc as a hand does; held: with the button down (a drag). */
async function move(to, ms = 650, held = false) {
  const from = at, t0 = Date.now();
  for (;;) {
    const k = Math.min(1, (Date.now() - t0) / ms), e = ease(k);
    const lift = Math.sin(Math.PI * k) * Math.min(40, Math.hypot(to.x - from.x, to.y - from.y) * .08);
    await mouse('mouseMoved', { x: from.x + (to.x - from.x) * e, y: from.y + (to.y - from.y) * e - lift }, held ? { button: 'left', buttons: 1 } : {});
    if (k >= 1) break;
    await sleep(12);
  }
  at = to;
}
const where = css => b.eval(`(() => { const r = document.querySelector(${JSON.stringify(css)}).getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2, left: r.left, width: r.width }; })()`);
async function click(css, ms) {
  await move(await where(css), ms);
  await sleep(120);
  await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
  await sleep(70);
  await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
}
/** The cursor the screenshots don't have: drawn where the mouse is, with a ring on each press. */
const drawCursor = () => b.eval(`(() => {
  const c = document.createElement('div');
  c.id = 'demo-cursor';
  c.innerHTML = '<svg width="24" height="24" viewBox="0 0 24 24"><path d="M4 2l14 9.5-6.2 1.3 3.6 7.2-2.9 1.4-3.6-7.2L4 19z" fill="#fff" stroke="#111" stroke-width="1.4" stroke-linejoin="round"/></svg><i></i>';
  c.style.cssText = 'position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none';
  const s = document.createElement('style');
  s.textContent = '#demo-cursor i{position:absolute;left:-13px;top:-13px;width:26px;height:26px;border-radius:50%;border:2px solid rgba(255,255,255,.9);opacity:0}#demo-cursor.tap i{animation:tap .4s ease-out}@keyframes tap{0%{opacity:.9;transform:scale(.3)}100%{opacity:0;transform:scale(1.3)}}';
  document.head.append(s);
  document.documentElement.append(c);
  addEventListener('mousemove', e => { c.style.transform = 'translate(' + e.clientX + 'px,' + e.clientY + 'px)'; }, true);
  addEventListener('mousedown', () => { c.classList.remove('tap'); void c.offsetWidth; c.classList.add('tap'); }, true);
})()`);

const takes = {
  /** Your themes: a kept theme picked (the window behind a blurred picture), its blur dragged, the plain one again. */
  async themes() {
    await b.goto(`http://localhost:5178/ui/dev/preview.html?settings=appearance&theme=custom&saved=1&lang=${lang}&nodelay=1`);
    await sleep(1500);
    await b.eval(`window.__mockBridge.call('useSavedTheme', { id: 't1' })`);
    await sleep(600);
    await b.eval(`document.querySelector('#s-saved-themes').scrollIntoView({ block: 'center' })`);
    await drawCursor();
    at = await where('.ct-theme[data-id="t1"] .ct-use');
    at = { x: at.x + 60, y: at.y - 70 };
    await mouse('mouseMoved', at);
    return async () => {
      await sleep(700);
      await click('.ct-theme[data-id="t2"] .ct-use', 650);
      await sleep(1300);
      const s = await where('#s-ct-blur');
      const knob = v => ({ x: s.left + s.width * v / 30, y: s.y });
      await move(knob(8), 650);
      await sleep(150);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await move(knob(24), 1000, true);
      await sleep(400);
      await move(knob(4), 900, true);
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(800);
      await click('.ct-theme[data-id="t1"] .ct-use', 750);
      await sleep(700);
      // Back where the take began (the rows for the picture are gone, so the list moved up again).
      const back = await where('.ct-theme[data-id="t1"] .ct-use');
      await move({ x: back.x + 60, y: back.y - 70 }, 600);
      await sleep(500);
    };
  },
};

const take = await takes[name]();
const frames = [];
b.on(m => {
  if (m.method !== 'Page.screencastFrame') return;
  const file = `f${String(frames.length).padStart(4, '0')}.jpg`;
  fs.writeFileSync(path.join(out, file), Buffer.from(m.params.data, 'base64'));
  frames.push({ file, t: m.params.metadata.timestamp });
  b.send('Page.screencastFrameAck', { sessionId: m.params.sessionId }).catch(() => {});
});
await b.send('Page.startScreencast', { format: 'jpeg', quality: 95, maxWidth: W * 1.5, maxHeight: H * 1.5, everyNthFrame: 1 });
await take();
await b.send('Page.stopScreencast');
const end = Date.now() / 1000;
await sleep(300);
fs.writeFileSync(path.join(out, 'frames.json'), JSON.stringify({ frames, end }));
await b.close();
console.log(frames.length, 'frames');
