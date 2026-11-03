'use strict';
// P7's additions to the fake app (ui/dev/mock.js), loaded after it by preview.html.
// The game monitor (?monitor=1&tiles=N): N Roblox windows, sent as the app sends them ("monitor") once the page reports
// the tab's area, each with a stand-in picture where the app would lay the live one (the browser can't show windows).
// ?tiles=N&mintile=1 the second one minimised, &long=1 long names and games, &hide=1 names hidden, &others=1 the last one
// not opened by Bloxkeep. The tiles' acts (monitorAct) change the list as the app would.
(() => {
  const base = window.__mockBridge;
  const params = new URLSearchParams(location.search);
  if (!base || !params.has('tiles')) return;
  const vi = params.get('lang') === 'vi';
  const long = params.has('long'), hide = params.has('hide');
  const count = Math.max(0, Math.min(24, Number(params.get('tiles')) || 0));
  /** A blocky game scene in the colours given: what a Roblox window might show. */
  const scene = (sky, ground, a, b) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
    <rect width="1280" height="440" fill="${sky}"/><rect y="440" width="1280" height="280" fill="${ground}"/>
    <rect x="140" y="300" width="240" height="140" fill="${a}"/><rect x="170" y="240" width="180" height="70" fill="#8c2f2f"/>
    <rect x="820" y="250" width="300" height="190" fill="${b}"/><rect x="900" y="330" width="60" height="110" fill="#2b2b2b"/>
    <rect x="600" y="310" width="64" height="64" fill="#f2c94c"/><rect x="590" y="374" width="84" height="96" fill="#3b82f6"/>
    <rect x="598" y="470" width="30" height="78" fill="#1f2937"/><rect x="636" y="470" width="30" height="78" fill="#1f2937"/>
    <rect x="24" y="20" width="44" height="44" rx="8" fill="#000" fill-opacity=".45"/><rect x="1104" y="20" width="152" height="40" rx="8" fill="#000" fill-opacity=".4"/></svg>`);
  const PICTURES = [scene('#7ec8f0', '#5aa04a', '#b97a4a', '#9aa3ad'), scene('#f4b56a', '#c9a15b', '#7a5230', '#5b6b7a'),
    scene('#1c2a4a', '#2f4f3a', '#45607d', '#6b5b8c'), scene('#a8d8e8', '#e8eef2', '#7a8b99', '#c45c5c')];
  const NAMES = long ? ['MainAccountWithAVeryLongUsername', 'FarmAlt_number_two_2024', 'Trader'] : ['MainAccount', 'FarmAlt02', 'Trader'];
  const GAMES = long ? ['Blox Fruits [Second Sea] Update 24 with the Dragon rework', 'Pet Simulator 99 Trading Plaza', 'Fisch'] : ['Blox Fruits', 'Pet Simulator 99', 'Fisch'];
  let tiles = Array.from({ length: count }, (_, i) => {
    const other = params.has('others') && i === count - 1;
    return {
      id: String(1050000 + i * 2468), pid: 14280 + i * 412,
      name: other ? 'Roblox' : hide ? (vi ? `Tài khoản ${i + 1}` : `Account ${i + 1}`) : i < 3 ? NAMES[i] : `Farm${String(i).padStart(2, '0')}`,
      game: i === 1 && !params.has('mintile') ? (vi ? 'Chưa vào game' : 'No game') : GAMES[i % 3],
      ramMb: [1240, 812, 960][i % 3] + i * 10, ours: !other, minimized: params.has('mintile') && i === 1, picture: PICTURES[i % PICTURES.length],
    };
  });
  let emit = () => {};
  let shown = false;
  const send = () => emit('monitor', { tiles });
  const attach = base.attach, call = base.call;
  // The heading and the sidebar count these windows.
  base.attach = fn => { emit = fn; attach.call(base, (name, d) => fn(name, name === 'metrics' && d ? { ...d, monitorCount: tiles.length, robloxCount: tiles.length } : d)); };
  base.call = async (cmd, a) => {
    if (cmd === 'monitorArea') {
      // Sent once the tab shows, as the app's first refresh does; later changes come from the acts.
      if (!a?.hidden && !shown) { shown = true; setTimeout(send, 60); }
      if (a?.hidden) shown = false;
      window.__monitorArea = a; // what the page reported last, for the checks
      return null;
    }
    if (cmd === 'monitorAct') {
      const tile = tiles.find(x => x.id === a.id);
      if (!tile) return null;
      if (a.what === 'copyPid') return tile.pid;
      if (a.what === 'minimize' || a.what === 'restore') tile.minimized = a.what === 'minimize';
      if (a.what === 'close') tiles = tiles.filter(x => x !== tile);
      tiles = tiles.map(x => ({ ...x }));
      setTimeout(send, 80);
      return null;
    }
    return call.call(base, cmd, a);
  };
})();
