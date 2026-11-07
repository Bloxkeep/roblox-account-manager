'use strict';
// The website's try-it page (site/build.mjs copies it to /demo/app/ with mock.js and the app's files): the real
// interface on the fake bridge, made fit to put online. Loaded right after mock.js, it wraps window.__mockBridge as the
// monitor's ?tiles wrapper does:
// - Nothing leaves the page: the mock's own files come from beside it, any other fetch fails, and pictures Roblox's,
//   Fandom's, Discord's or a price site's CDN would serve are swapped for drawn stand-ins (Discord's custom emoji
//   dropped). The Wiki part's live calls (the whole wiki, finding a wiki) say they aren't in the demo.
// - Logins are ones Roblox can't have (a hyphen isn't allowed in a Roblox username), so none is a real person's.
// - Play goes from opening to playing (the mock's own), and the Game monitor shows one tile per open account, with a
//   stand-in picture; minimise, restore and close act on them.
// The page sets window.__mockQuery (only lang) before mock.js, so the address's dev knobs do nothing here.
(() => {
  const base = window.__mockBridge;
  if (!base) return;
  const vi = new URLSearchParams(window.__mockQuery || '').get('lang') === 'vi';
  const tx = (en, v) => vi ? v : en;

  // The mock's files, from beside the page; nothing from anywhere else.
  const realFetch = window.fetch.bind(window);
  const LOCAL = { '/ui/dev/wiki-sample.json': 'wiki-sample.json', '/RobloxManager/whatsnew.json': 'whatsnew.json' };
  window.fetch = (input, init) => {
    const url = typeof input === 'string' ? input : input.url;
    if (LOCAL[url]) return realFetch(LOCAL[url], init);
    if (new URL(url, document.baseURI).origin !== location.origin) return Promise.reject(new TypeError('The demo stays offline.'));
    return realFetch(input, init);
  };

  // Pictures from outside, swapped for a solid block in a colour picked from their address (the same each time).
  const CDN = /https?:\/\/(?:(?:[a-z0-9-]+\.)*(?:rbxcdn\.com|wikia\.nocookie\.net|postimg\.cc|discordapp\.(?:com|net))\/|bloxfruitsvalues\.com\/api\/)[^\s"'<>)\]]*/gi;
  const COLOURS = [['#2f6fd6', '#f2c94c'], ['#7a3fd1', '#7ee0a1'], ['#0f766e', '#f4b56a'], ['#b45309', '#fde68a'], ['#be123c', '#fbcfe8'],
    ['#334155', '#93c5fd'], ['#15803d', '#d9f99d'], ['#9333ea', '#fcd34d'], ['#0369a1', '#a5f3fc'], ['#a16207', '#e0e7ff']];
  // A blocky game scene in the colours given: what a Roblox window or a game's picture might show.
  const scene = (sky, ground, a, b) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
    <rect width="1280" height="440" fill="${sky}"/><rect y="440" width="1280" height="280" fill="${ground}"/>
    <rect x="140" y="300" width="240" height="140" fill="${a}"/><rect x="170" y="240" width="180" height="70" fill="#8c2f2f"/>
    <rect x="820" y="250" width="300" height="190" fill="${b}"/><rect x="900" y="330" width="60" height="110" fill="#2b2b2b"/>
    <rect x="600" y="310" width="64" height="64" fill="#f2c94c"/><rect x="590" y="374" width="84" height="96" fill="#3b82f6"/>
    <rect x="598" y="470" width="30" height="78" fill="#1f2937"/><rect x="636" y="470" width="30" height="78" fill="#1f2937"/></svg>`);
  const PICTURES = [scene('#7ec8f0', '#5aa04a', '#b97a4a', '#9aa3ad'), scene('#f4b56a', '#c9a15b', '#7a5230', '#5b6b7a'),
    scene('#1c2a4a', '#2f4f3a', '#45607d', '#6b5b8c'), scene('#a8d8e8', '#e8eef2', '#7a8b99', '#c45c5c')];
  const drawn = new Map();
  const standIn = url => {
    let p = drawn.get(url);
    if (!p) {
      let h = 0;
      for (const c of url) h = (h * 31 + c.codePointAt(0)) >>> 0;
      const [bg, fg] = COLOURS[h % COLOURS.length];
      // A game's wide picture (Roblox's 768×432) is a scene; anything else a block on a colour.
      p = /\/768\/432\//.test(url) ? PICTURES[h % PICTURES.length]
        : 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 160 160" preserveAspectRatio="xMidYMid slice"><rect x="-200" y="-200" width="560" height="560" fill="${bg}"/><rect x="50" y="50" width="60" height="60" rx="8" fill="${fg}"/></svg>`);
      drawn.set(url, p);
    }
    return p;
  };
  const EMOJI = /<a?:[A-Za-z0-9_~]+:\d+>/g;
  const clean = v => {
    if (typeof v === 'string') return v.includes('://') || v.includes('<') ? v.replace(CDN, standIn).replace(EMOJI, '') : v;
    if (Array.isArray(v)) return v.map(clean);
    if (v && typeof v === 'object' && Object.getPrototypeOf(v) === Object.prototype) {
      const o = {};
      for (const k in v) o[k] = clean(v[k]);
      return o;
    }
    return v;
  };

  const attach = base.attach, call = base.call;

  // The accounts' logins: made up, and not possible on Roblox.
  const LOGINS = [['demo-sunny'], ['demo-farm-01', 'Farmer One'], ['demo-farm-02', 'Farmer Two'], ['demo-night-owl'], ['demo-lan'], ['demo-trade'], ['demo-tester']];
  const ready = (async () => {
    await Promise.all(LOGINS.map(([username, displayName], i) =>
      call.call(base, 'debugAccount', { n: i + 1, set: displayName ? { username, displayName } : { username } })));
    // The one the mock leaves opening for ever is ready to play instead.
    await call.call(base, 'debugAccount', { n: 7, set: { state: 'idle', session: null } });
  })();

  // The Game monitor: a tile for every account that is playing.
  let emit = () => {}, last = null, shown = false;
  const minimized = new Set();
  const tiles = () => {
    if (!last?.accounts) return [];
    return last.accounts.filter(a => a.state === 'running' && a.session).map(a => {
      const n = parseInt(a.id.slice(-4), 16) || 0;
      const id = String(1050000 + n * 2468);
      const status = a.session.connection || 'ingame';
      return { id, pid: a.session.pid, accountId: a.id,
        name: last.hideNames ? tx(`Account ${n}`, `Tài khoản ${n}`) : a.label || a.displayName,
        game: a.session.game?.name || tx('No game', 'Chưa vào game'), ramMb: 760 + n * 97, ours: true, minimized: minimized.has(id),
        picture: PICTURES[n % PICTURES.length], status, jobId: status === 'ingame' ? `job-${a.session.game?.placeId || 0}` : null,
        iconUrl: a.session.game?.iconUrl ? standIn(a.session.game.iconUrl) : null };
    });
  };
  const send = () => { if (shown) emit('monitor', { tiles: tiles() }); };

  base.attach = fn => {
    emit = (name, d) => fn(name, clean(d));
    attach.call(base, (name, d) => {
      if (name === 'state' && d) { last = d; setTimeout(send, 60); settle(d.accounts); }
      emit(name, d);
    });
    // The second account's window gets into its game a moment after the page opens.
    setTimeout(async () => {
      await ready;
      const two = last?.accounts?.find(a => a.id.endsWith('2'));
      if (two?.session) base.call('debugAccount', { n: 2, set: { session: { ...two.session, connection: 'ingame' } } });
    }, 2500);
  };
  // The mock opens every account into its game at once with a stand-in PID: here it joins first, then plays, with its
  // own PID, and Roblox says it's in that game (not where it was before).
  const settled = new Set();
  const settle = list => {
    for (const a of list || []) {
      if (a.state !== 'running') { settled.delete(a.id); continue; }
      if (a.session?.pid !== 9 || settled.has(a.id)) continue;
      settled.add(a.id);
      const n = parseInt(a.id.slice(-4), 16) || 0, game = a.session.game;
      const session = { ...a.session, pid: 14280 + n * 412, connection: 'joining' };
      const presence = { type: 'ingame', game: game?.name, placeId: game?.placeId, iconUrl: game?.iconUrl };
      setTimeout(() => call.call(base, 'debugAccount', { n, set: { session, presence } }), 0);
      setTimeout(() => call.call(base, 'debugAccount', { n, set: { session: { ...session, connection: 'ingame' } } }), 2500);
    }
  };
  const OFFLINE = new Set(['wikiApi', 'wikiFind', 'wikiAbout']);
  base.call = async (cmd, a) => {
    await ready;
    if (OFFLINE.has(cmd))
      throw new Error(tx("The demo doesn't go online. In Bloxkeep this opens the game's wiki.", 'Bản thử không vào mạng. Trong Bloxkeep, phần này mở wiki của game.'));
    if (cmd === 'monitorArea') {
      if (!a?.hidden && !shown) { shown = true; setTimeout(send, 60); }
      if (a?.hidden) shown = false;
      return null;
    }
    if (cmd === 'monitorAct') {
      const tile = tiles().find(x => x.id === a.id);
      if (!tile) return null;
      if (a.what === 'copyPid') return tile.pid;
      if (a.what === 'minimize') minimized.add(tile.id);
      if (a.what === 'restore') minimized.delete(tile.id);
      if (a.what === 'close') return call.call(base, 'closeAccounts', { ids: [tile.accountId] });
      setTimeout(send, 80);
      return null;
    }
    return clean(await call.call(base, cmd, a));
  };
})();
