'use strict';
// Real pictures for the documentation's screenshots and demos (docs/screenshots/shoot.mjs, ui/dev/demos/record.mjs),
// as the website's try-it page has them (ui/dev/demo.js): the games' icons from site/demo-pics/ instead of Roblox's CDN,
// a real Roblox avatar for each of the mock's first six accounts, and logins Roblox can't have (a hyphen isn't allowed
// in a Roblox username), so no real person's name sits beside a real face. Run before the page's own scripts (CDP's
// Page.addScriptToEvaluateOnNewDocument), after window.__demoPics is set to site/demo-pics/map.json; it wraps the fake
// app (window.__mockBridge) so what the page gets is already swapped.
(() => {
  const PICS = window.__demoPics;
  if (!PICS) return;
  const DIR = '/site/demo-pics/';
  const LOGINS = [['demo-sunny'], ['demo-farm-01', 'Farmer One'], ['demo-farm-02', 'Farmer Two'], ['demo-night-owl'], ['demo-lan'], ['demo-trade'], ['demo-tester']];
  const swaps = new Map(); // a mock face (data: address) or login → its stand-in
  // The outside addresses the site's demo saves pictures for (ui/dev/demo.js); one not saved is left as it is.
  const CDN = /https?:\/\/(?:(?:[a-z0-9-]+\.)*(?:rbxcdn\.com|wikia\.nocookie\.net|postimg\.cc|discordapp\.(?:com|net))\/|bloxfruitsvalues\.com\/api\/)[^\s"'<>)\]]*/gi;
  const swapUrls = s => s.replace(CDN, u => (PICS.map[u] ? DIR + PICS.map[u] : u));
  const clean = v => {
    if (typeof v === 'string') return swaps.has(v) ? swaps.get(v) : v.includes('://') ? swapUrls(v) : v;
    if (Array.isArray(v)) return v.map(clean);
    if (v && typeof v === 'object' && Object.getPrototypeOf(v) === Object.prototype) {
      const o = {};
      for (const k in v) o[k] = clean(v[k]);
      return o;
    }
    return v;
  };
  /** Learns each of the mock's accounts' stand-ins from the list (the same every time: by the account's number). */
  const learn = accounts => {
    for (const a of accounts || []) {
      const n = parseInt(String(a.id).slice(-4), 16);
      if (!(n >= 1 && n <= LOGINS.length)) continue;
      const [login, display] = LOGINS[n - 1];
      if (a.avatarUrl?.startsWith('data:') && PICS.avatars[n - 1]) swaps.set(a.avatarUrl, DIR + PICS.avatars[n - 1]);
      if (a.username && a.username !== login) {
        if (display || a.displayName === a.username) swaps.set(`dn:${a.username}`, display || login);
        swaps.set(a.username, login);
      }
    }
  };
  /** The accounts' display names too (where the mock's is the login, or a made-up one is given), then every picture. */
  const withNames = st => {
    if (!st || !Array.isArray(st.accounts)) return st;
    learn(st.accounts);
    return { ...st, accounts: st.accounts.map(a => swaps.has(`dn:${a.username}`) ? { ...a, displayName: swaps.get(`dn:${a.username}`) } : a) };
  };
  // The app's state comes as the "state" event, and inside getState's answer ({ resume, view, state }).
  const fix = d => clean(d?.state?.accounts ? { ...d, state: withNames(d.state) } : withNames(d));
  const wrap = v => {
    if (!v || v.__realPics) return;
    v.__realPics = true;
    const attach = v.attach.bind(v), call = v.call.bind(v);
    v.attach = fn => attach((name, d) => fn(name, fix(d)));
    v.call = async (cmd, a) => fix(await call(cmd, a));
  };
  const was = Object.getOwnPropertyDescriptor(window, '__mockBridge');
  let bridge;
  Object.defineProperty(window, '__mockBridge', {
    configurable: true,
    get: () => (was?.get ? was.get() : bridge),
    set(v) { wrap(v); if (was?.set) was.set.call(window, v); else bridge = v; },
  });
})();
