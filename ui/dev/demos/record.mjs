// What's new demos, filmed like a screen recording from ui/dev/preview.html (made-up accounts): the browser's own
// screencast (frames as they are painted, with their times), a real mouse (hover states show) with a cursor drawn
// where it is, real clicks and drags. Each take ends where it starts, so the clip loops.
//   python -m http.server 5178            (from the repository, as .claude/launch.json's ui-preview; PORT=… for another)
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

// The site's takes are 16:9, the shape of its feature cards (which cut a picture to it); What's new's are 16:10.
const W = 1280, H = ['servers', 'presets', 'rejoin'].includes(name) ? 720 : 800, BASE = `http://localhost:${process.env.PORT || 5178}`;
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
  c.style.cssText = 'position:fixed;left:0;top:0;z-index:2147483647;pointer-events:none;transform:translate(-99px,-99px)';
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
    await b.goto(`${BASE}/ui/dev/preview.html?settings=appearance&theme=custom&saved=1&lang=${lang}&nodelay=1`);
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
  /** The themes from photos: one after another, the mouse moving over each so the scene answers. */
  async photos() {
    await b.goto(`${BASE}/ui/dev/preview.html?settings=appearance&theme=miaminight&lang=${lang}&nodelay=1`);
    await sleep(1500);
    await b.eval(`document.querySelector('#s-theme-groups .chip[data-group="live"]').click()`);
    await sleep(500);
    await drawCursor();
    at = await where('.swatch[data-id="miaminight"]');
    await mouse('mouseMoved', at);
    await sleep(1000);
    return async () => {
      for (const id of ['nightharbour', 'lanternriver', 'deepsea', 'snowfall', 'miaminight']) {
        await click(`.swatch[data-id="${id}"]`, 500);
        await sleep(700);
        const here = at;
        await move({ x: W * .86, y: H * .78 }, 800);
        await move(here, 700);
      }
      await sleep(300);
    };
  },
  /** A macro: its picture of the game taken, a step opened and closed, its area dragged on the picture and back. */
  async macros() {
    await b.goto(`http://localhost:5178/ui/dev/preview.html?macroedit=m4&lang=${lang}&nodelay=1`);
    await sleep(1800);
    await b.eval(`document.querySelector('#mc-pic-take').click()`);
    await sleep(900);
    await drawCursor();
    at = { x: 520, y: 260 };
    await mouse('mouseMoved', { x: at.x + 1, y: at.y });
    await mouse('mouseMoved', at);
    const head = n => `.mc-step:nth-child(${n}) .mc-step-head`;
    return async () => {
      await sleep(600);
      await click(head(3), 700);
      await sleep(1800);
      await click(head(3), 500);
      await sleep(700);
      const area = await where('.mc-pic-area');
      const grab = { x: area.x + 20, y: area.y };
      await move(grab, 800);
      await sleep(150);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await move({ x: grab.x + 60, y: grab.y - 120 }, 900, true);
      await sleep(400);
      await move(grab, 800, true);
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(500);
      await move({ x: 520, y: 260 }, 700);
      await sleep(400);
    };
  },
  /** The panel the player sees: its tabs, a switch added from the menu and taken away again. */
  async panel() {
    await b.goto(`http://localhost:5178/ui/dev/preview.html?macros=1&lang=${lang}&nodelay=1`);
    await sleep(1500);
    const vi = lang === 'vi', tx = (en, viText) => vi ? viText : en;
    const [fish, enchant] = [tx('Fishing', 'Câu cá'), tx('Enchant', 'Phù phép')];
    await b.eval(`(async () => {
      const bridge = window.__mockBridge;
      const m4 = ((await bridge.call('getState')).state.settings?.macros || []).find(x => x.id === 'm4');
      if (!m4) return;
      const tabs = ${JSON.stringify([fish, enchant])};
      const options = [
        { ...m4.options[0], tab: tabs[0] }, { ...m4.options[1], tab: tabs[0] },
        { name: 'o3', label: ${JSON.stringify(tx('Sell fish', 'Tự bán cá'))}, kind: 'check', value: true, default: true, tab: tabs[0] },
        { name: 'o4', label: ${JSON.stringify(tx('Bait', 'Mồi'))}, kind: 'choice', value: ${JSON.stringify(tx('Shrimp', 'Tôm'))}, default: ${JSON.stringify(tx('Shrimp', 'Tôm'))},
          choices: ${JSON.stringify(tx('Worm,Shrimp,Squid', 'Giun,Tôm,Mực').split(','))}, tab: tabs[0] },
        { name: 'o5', label: ${JSON.stringify(tx('Keep', 'Giữ lại'))}, kind: 'heading', value: null, default: null, tab: tabs[1] },
        { name: 'o6', label: ${JSON.stringify(tx('Enchants', 'Phù phép'))}, kind: 'list', value: ['Abyssal', 'Lucky'], default: [],
          choices: ['Blessed', 'Abyssal', 'Hasty', 'Lucky', 'Resilient'], tab: tabs[1] },
      ];
      await bridge.call('saveMacro', { macro: { ...m4, options, panel: { ...m4.panel, tabs } } });
    })()`);
    await sleep(400);
    await b.eval(`(() => {
      document.querySelector('.mc-item[data-id="m4"] [data-mc=more]').click();
      setTimeout(() => [...document.querySelectorAll('.menu button')].find(x => x.textContent.trim() === (window.I18N?.['${lang}']?.['macro.edit'] || 'Edit')).click(), 80);
    })()`);
    await sleep(900);
    // The heading a little below the top, so the shorter tab doesn't make the dialog scroll back.
    await b.eval(`(() => {
      const h = document.querySelector('#mc-sees-h');
      h.scrollIntoView({ block: 'start' });
      let box = h.parentElement;
      while (box && box.scrollHeight <= box.clientHeight) box = box.parentElement;
      if (box) box.scrollTop -= 140;
    })()`);
    await sleep(400);
    await drawCursor();
    at = await where('#mc-sees-add');
    at = { x: at.x - 120, y: at.y + 160 };
    await mouse('mouseMoved', { x: at.x + 1, y: at.y });
    await mouse('mouseMoved', at);
    // The tab not shown (the shown one is a box to type its name in).
    const tabAt = () => b.eval(`(() => { const r = document.querySelector('button.mp-tab:not(.add)').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
    return async () => {
      await sleep(700);
      await move(await tabAt(), 700);
      await sleep(120);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(1600);
      await move(await tabAt(), 600);
      await sleep(120);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(900);
      await click('#mc-sees-add', 700);
      await sleep(600);
      const item = await b.eval(`(() => { const t = [...document.querySelectorAll('.menu button')].find(x => x.textContent.includes(${JSON.stringify(tx('Switch', 'Công tắc'))})); const r = t.getBoundingClientRect(); return { x: r.left + 40, y: r.top + r.height / 2 }; })()`);
      await move(item, 500);
      await sleep(100);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(1500);
      const x = await b.eval(`(() => { const rows = document.querySelectorAll('.mp-opts li'); const r = rows[rows.length - 1].querySelector('.mp-acts .icon-btn:last-child').getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
      await move(x, 700);
      await sleep(120);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
      await sleep(600);
      const start = await where('#mc-sees-add');
      await move({ x: start.x - 120, y: start.y + 160 }, 700);
      await sleep(400);
    };
  },
  /** Statistics: 30 days and back, then an account opened to its timeline and closed. */
  async stats() {
    await b.goto(`http://localhost:5178/ui/dev/preview.html?stats=1&lang=${lang}&nodelay=1`);
    await sleep(1800);
    await drawCursor();
    const byText = text => b.eval(`(() => { const t = [...document.querySelectorAll('button')].find(x => x.textContent.trim() === ${JSON.stringify(text)}); const r = t.getBoundingClientRect(); return { x: r.left + r.width / 2, y: r.top + r.height / 2 }; })()`);
    const [days30, days7] = lang === 'vi' ? ['30 ngày', '7 ngày'] : ['30 days', '7 days'];
    const press = async () => {
      await sleep(120);
      await mouse('mousePressed', at, { button: 'left', buttons: 1, clickCount: 1 });
      await mouse('mouseReleased', at, { button: 'left', clickCount: 1 });
    };
    const wheel = async (dy, steps = 8) => {
      for (let i = 0; i < steps; i++) { await b.send('Input.dispatchMouseEvent', { type: 'mouseWheel', x: at.x, y: at.y, deltaX: 0, deltaY: dy / steps }); await sleep(40); }
    };
    const row = `tr.opens`;
    at = { x: 760, y: 330 };
    await mouse('mouseMoved', { x: at.x + 1, y: at.y });
    await mouse('mouseMoved', at);
    return async () => {
      await sleep(600);
      await move(await byText(days30), 700);
      await press();
      await sleep(1600);
      await move(await byText(days7), 500);
      await press();
      await sleep(1000);
      await move({ x: 760, y: 500 }, 600);
      await wheel(300);
      await sleep(500);
      await click(row, 700);
      await sleep(2200);
      await click(row, 500);
      await sleep(500);
      await move({ x: 760, y: 450 }, 400);
      await wheel(-300);
      await sleep(400);
      await move({ x: 760, y: 330 }, 500);
      await sleep(400);
    };
  },
  /** The modes for Roblox: each picked in turn, back to the one in use. */
  async modes() {
    await b.goto(`${BASE}/ui/dev/preview.html?settings=perf&open=s-rc-levels&lang=${lang}&nodelay=1`);
    await sleep(1500);
    await b.eval(`document.querySelector('#s-rc-levels').scrollIntoView({ block: 'center' })`);
    await sleep(300);
    await drawCursor();
    const button = n => `#s-rc-levels button:nth-of-type(${n})`;
    const inUse = await b.eval(`[...document.querySelectorAll('#s-rc-levels button')].findIndex(x => x.classList.contains('on')) + 1`) || 1;
    const others = [1, 2, 3].filter(n => n !== inUse);
    at = await where(button(inUse));
    await mouse('mouseMoved', at);
    return async () => {
      await sleep(900);
      for (const n of [...others, inUse]) {
        await click(button(n), 700);
        await sleep(1300);
      }
    };
  },
  /** The site's "Pick a game, pick a server": a game searched by name, then its servers, sorted, one used. */
  async servers() {
    await b.goto(`${BASE}/ui/dev/preview.html?select=1&lang=${lang}&nodelay=1`);
    await sleep(1500);
    await drawCursor();
    at = await where('#place');
    at = { x: at.x + 80, y: at.y - 90 };
    await mouse('mouseMoved', at);
    return async () => {
      await sleep(600);
      await click('#place', 700);
      await sleep(400);
      for (const ch of 'adopt') {
        await b.send('Input.insertText', { text: ch });
        await sleep(110);
      }
      await sleep(900);
      await click('[data-act=pick-game][data-i="0"]', 650);
      await sleep(900);
      await click('[data-act=servers]', 700);
      await sleep(1300);
      // Down to the public servers, as a wheel does.
      const list = await where('.overlay .modal-body');
      await move({ x: list.x, y: list.y + 40 }, 500);
      for (let i = 0; i < 6; i++) {
        await mouse('mouseWheel', at, { deltaX: 0, deltaY: 100 });
        await sleep(60);
      }
      await sleep(700);
      for (const sort of ['fewest', 'ping']) {
        await click(`.overlay [data-sort="${sort}"]`, 600);
        await sleep(1100);
      }
      await b.eval(`(() => { const u = [...document.querySelectorAll('.overlay .btn.sm')].find(x => !Object.keys(x.dataset).length); u.dataset.demo = 'use'; })()`);
      await click('.overlay [data-demo=use]', 650);
      await sleep(1800);
      await move({ x: at.x - 120, y: at.y - 160 }, 700);
      await sleep(600);
    };
  },
  /** The site's "Presets and schedules": a preset's schedule shown and its days changed and back, another one's turned on and off. */
  async presets() {
    await b.goto(`${BASE}/ui/dev/preview.html?view=presets&lang=${lang}&nodelay=1`);
    await sleep(1500);
    await drawCursor();
    at = await where('.pv-item[data-id="p1"] [data-pv=edit]');
    at = { x: at.x - 160, y: at.y + 140 };
    await mouse('mouseMoved', at);
    return async () => {
      await sleep(600);
      await click('.pv-item[data-id="p1"] [data-pv=edit]', 700);
      await sleep(1000);
      const body = await where('.overlay .modal-body');
      await move({ x: body.x, y: body.y }, 500);
      for (let i = 0; i < 16; i++) {
        await mouse('mouseWheel', at, { deltaX: 0, deltaY: 100 });
        await sleep(55);
      }
      await sleep(900);
      for (const day of [6, 0, 6, 0]) {
        await click(`.overlay [data-day="${day}"]`, 550);
        await sleep(600);
      }
      await sleep(500);
      await click('.overlay .modal-foot [data-i="1"]', 700);
      await sleep(900);
      for (let i = 0; i < 2; i++) {
        await click('.pv-item[data-id="p3"] label.switch', 650);
        await sleep(1300);
      }
      const back = await where('.pv-item[data-id="p1"] [data-pv=edit]');
      await move({ x: back.x - 160, y: back.y + 140 }, 700);
      await sleep(500);
    };
  },
  /** The site's "Stay in game": an account kicked for idling, rejoining by itself, in the game again. */
  async rejoin() {
    await b.goto(`${BASE}/ui/dev/preview.html?lang=${lang}&nodelay=1`);
    await sleep(1500);
    const game = await b.eval(`({ placeId: 2753915549, name: 'Blox Fruits' })`);
    const set = s => b.eval(`window.__mockBridge.call('debugAccount', { n: 2, set: ${JSON.stringify(s)} })`);
    const ago = m => new Date(Date.now() - m * 60000).toISOString();
    const playing = since => ({ state: 'running', rejoin: null, presence: { type: 'ingame', game: game.name, placeId: game.placeId },
      session: { pid: 2222, since, game, connection: 'ingame' } });
    await set(playing(ago(38)));
    await drawCursor();
    at = await where('[data-id="00000000-0000-4000-8000-000000000002"]').catch(() => null) ?? { x: W * .45, y: H * .3 };
    at = { x: at.x + 150, y: at.y + 160 };
    await mouse('mouseMoved', at);
    await sleep(800);
    return async () => {
      await sleep(1500);
      await set({ presence: { type: 'online' }, session: { pid: 2222, since: ago(38), game, connection: 'disconnected', reason: 278 } });
      await sleep(2600);
      await set({ state: 'idle', session: null, rejoin: { at: new Date(Date.now() + 6000).toISOString(), restart: false, game: null } });
      await sleep(2600);
      await set({ state: 'running', rejoin: null, session: { pid: 3333, since: ago(0), game, connection: 'joining' } });
      await sleep(2400);
      await set(playing(ago(0)));
      await sleep(2200);
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
