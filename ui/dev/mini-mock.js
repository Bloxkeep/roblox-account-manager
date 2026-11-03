'use strict';
// Fake app for the mini window's page (mini.html) in a normal browser. ?empty=1 no accounts running (?others=2 with two
// Roblox not opened by Bloxkeep), ?locked=1 locked, ?hidden=1 names hidden ("Account 1"…), ?long=1 long names and
// games, ?many=1 nine accounts, ?noask=1 "Close all" without asking, ?theme=light a theme, ?lang=vi Vietnamese.
// The acts change the fake list as the app would (close, closeAll); what is clicked shows in the page's calls.
(() => {
  const params = new URLSearchParams(location.search);
  const vi = params.get('lang') === 'vi';
  const long = params.has('long'), hidden = params.has('hidden');
  const games = long ? ['Blox Fruits [Second Sea] Update 24 with the Dragon rework', 'Pet Simulator 99 Trading Plaza', 'Fisch'] : ['Blox Fruits', 'Pet Simulator 99', 'Fisch'];
  const names = long ? ['MainAccountWithAVeryLongUsername', 'FarmAlt_number_two_2024', 'Trader'] : ['MainAccount', 'FarmAlt02', 'Trader'];
  const count = params.has('empty') ? 0 : params.has('many') ? 9 : 3;
  let clients = Array.from({ length: count }, (_, i) => ({
    id: `00000000-0000-4000-8000-00000000000${i + 1}`,
    name: hidden ? (vi ? `Tài khoản ${i + 1}` : `Account ${i + 1}`) : i < 3 ? names[i] : `Farm${String(i).padStart(2, '0')}`,
    game: i === 1 ? (vi ? 'Chưa vào game' : 'No game') : i === 2 && !params.has('many') ? (vi ? 'Mất kết nối' : 'Disconnected') : games[i % 3],
    pid: 14280 + i * 412, ramMb: [1240, 812, 960][i % 3] + i * 10,
    state: i === 1 ? 'menu' : i === 2 && !params.has('many') ? 'disconnected' : 'game',
  }));
  const locked = params.has('locked');
  let ask = !params.has('noask');
  let emit = () => {};
  const send = () => emit('mini', {
    unlocked: !locked, clients: locked ? [] : clients, robloxCount: locked ? 0 : clients.length + (Number(params.get('others')) || 0),
    askCloseAll: ask, theme: params.get('theme') || 'dark', customTheme: null, language: vi ? 'vi' : 'en',
  });
  const acts = {
    close(a) { setTimeout(() => { clients = clients.filter(c => c.id !== a.id); send(); }, 600); },
    closeAll(a) { if (a.remember) ask = false; clients = []; },
  };
  window.__mockBridge = {
    attach(fn) { emit = fn; },
    async call(cmd, args) {
      if (cmd === 'ready') setTimeout(send, 0);
      if (cmd === 'act' && acts[args.what]) { acts[args.what](args); setTimeout(send, 0); }
      return null;
    },
  };
})();
