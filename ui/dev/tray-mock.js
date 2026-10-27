'use strict';
// Fake app for the tray's page (tray.html) in a normal browser: ?mode=menu the right-click menu (else the quick panel),
// ?locked=1 Bloxkeep locked, ?multi=off|notReady, ?many=1 more Roblox windows than the panel lists, ?none=1 none,
// ?theme=light a theme, ?lang=vi Vietnamese. What is clicked shows in the page's calls (Tests/ui/page-hooks.js).
(() => {
  const params = new URLSearchParams(location.search);
  const face = (bg, skin) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 150 150"><rect width="150" height="150" fill="${bg}"/><rect x="38" y="34" width="74" height="74" rx="14" fill="${skin}"/><circle cx="62" cy="66" r="6" fill="#222"/><circle cx="88" cy="66" r="6" fill="#222"/><path d="M60 86q15 12 30 0" stroke="#222" stroke-width="5" fill="none" stroke-linecap="round"/></svg>`);
  const names = ['Main', 'Farm 1', 'Farm 2', 'Alt', 'Trader', 'Builder', 'Night Owl', 'Test'];
  const games = ['Blox Fruits', 'Adopt Me!', 'Brookhaven 🏡RP'];
  const locked = params.has('locked');
  let multi = params.get('multi') || 'ready';
  let running = params.has('none') ? 0 : params.has('many') ? 8 : 2;
  const accounts = names.map((name, i) => ({ id: `a${i}`, name, running: i < running, avatar: i % 3 === 2 ? null : face(['#2b6cb0', '#2f855a', '#975a16'][i % 3], '#f6c94c') }));
  const state = () => ({
    open: !locked, multi, multiOnly: false, robloxCount: running, ramMb: running * 1450, miniOpen: params.has('mini'),
    presets: locked ? [] : [{ id: 'p1', name: 'Evening farm' }, { id: 'p2', name: 'Trading' }],
    accounts: locked ? [] : accounts.map((a, i) => ({ ...a, running: i < running })),
    clients: locked ? [] : accounts.slice(0, running).map((a, i) => ({ id: a.id, name: a.name, game: games[i % 3], pid: 4320 + i, ramMb: 1300 + i * 110, avatar: a.avatar })),
  });
  let emit = () => {};
  const send = () => emit('tray', {
    mode: params.get('mode') === 'menu' ? 'menu' : 'panel', opened: 1, state: state(),
    theme: params.get('theme') || 'dark', customTheme: null, language: params.get('lang') === 'vi' ? 'vi' : 'en',
  });
  window.__mockBridge = {
    attach(fn) { emit = fn; },
    async call(cmd, args) {
      if (cmd === 'ready') setTimeout(send, 0);
      if (cmd === 'act') {
        if (args.what === 'multi') multi = args.on ? 'off' : 'ready';
        if (args.what === 'closeAll') running = 0;
        if (args.what === 'closeClient') running = Math.max(0, running - 1);
        setTimeout(send, 0);
      }
      return null;
    },
  };
})();
