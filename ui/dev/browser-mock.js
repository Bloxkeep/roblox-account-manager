'use strict';
// Fake app for the top of Bloxkeep's browser windows (browser.html) in a normal browser. ?many=1 twelve tabs, ?blank=1 a
// blank tab someone signed in on (Save to Bloxkeep), ?offline=1 roblox.com unreachable (Retry), ?login=1 the sign-in
// window's bar, ?theme=light a theme, ?lang=vi Vietnamese. The words are the app's (Forms/WebsiteTab.cs), already in the
// app's language. The acts change the fake tabs as the window would; what is clicked shows in the page's calls.
(() => {
  const params = new URLSearchParams(location.search);
  const vi = params.get('lang') === 'vi';
  const L = (en, vn) => (vi ? vn : en);
  // Stand-in avatars (a blocky head on a colour), drawn here: no picture of a real account.
  const avatar = colour => 'data:image/svg+xml,' + encodeURIComponent(
    `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 40 40"><rect width="40" height="40" fill="${colour}"/><rect x="11" y="8" width="18" height="17" rx="3" fill="#f5c542"/><rect x="15" y="14" width="3" height="3" fill="#222"/><rect x="22" y="14" width="3" height="3" fill="#222"/><rect x="8" y="27" width="24" height="13" rx="3" fill="#3b82f6"/></svg>`);
  const colours = ['#7c9cbf', '#9b87c9', '#c98787', '#87c9a1', '#c9b887', '#87b8c9', '#b087c9', '#c99f87', '#8794c9', '#a8c987', '#c987b5', '#87c9c2'];
  const names = ['Main', 'Farm 1', 'Farm 2', 'Trader alt with a long name', 'Fisher', 'Builder', 'Farm 3', 'Farm 4', 'Collector', 'Grinder', 'Farm 5', 'Backup'];
  const count = params.has('many') ? 12 : 3;
  let next = 0;
  let tabs = names.slice(0, count).map((title, i) => ({ id: `t${++next}`, title, avatar: avatar(colours[i]), saved: true }));
  if (params.has('blank')) tabs.push({ id: `t${++next}`, title: 'Roblox', avatar: null, saved: false, user: 'Newbie' });
  let selected = params.has('blank') ? tabs[tabs.length - 1].id : tabs[params.has('many') ? 5 : 0].id;
  const offline = params.has('offline');
  const login = params.has('login');
  const state = () => {
    const tab = tabs.find(x => x.id === selected);
    if (login) return { message: L('Sign in to each account in turn. Press Done when finished.', 'Đăng nhập lần lượt từng tài khoản. Xong hết thì bấm Xong.'), tone: '', retry: false, save: false };
    if (offline) return { message: L("Can't reach roblox.com. Check your internet connection, then press Retry.", 'Không kết nối được roblox.com. Kiểm tra mạng rồi bấm Thử lại.'), tone: 'bad', retry: true, save: false };
    if (tab && !tab.saved && tab.user) return { message: L(`Signed in as ${tab.user}.`, `Đã đăng nhập: ${tab.user}.`), tone: '', retry: false, save: true };
    if (tab && tab.justSaved) return { message: L(`${tab.title} saved.`, `Đã lưu ${tab.title}.`), tone: 'ok', retry: false, save: false };
    return { message: '', tone: '', retry: false, save: false };
  };
  let emit = () => {};
  const send = () => emit('browser', {
    mode: login ? 'login' : 'tabs',
    tabs: login ? [] : tabs.map(x => ({ id: x.id, title: x.title, avatar: x.avatar, saved: x.saved, selected: x.id === selected })),
    back: !login && !offline, forward: false, host: offline ? null : 'www.roblox.com', ...state(),
    theme: params.get('theme') || 'dark', customTheme: null, language: vi ? 'vi' : 'en',
  });
  const acts = {
    select(a) { selected = a.id; },
    close(a) {
      const i = tabs.findIndex(x => x.id === a.id);
      tabs = tabs.filter(x => x.id !== a.id);
      if (selected === a.id && tabs.length) selected = tabs[Math.min(i, tabs.length - 1)].id;
    },
    newTab() { const tab = { id: `t${++next}`, title: 'Roblox', avatar: null, saved: false }; tabs.push(tab); selected = tab.id; },
    save() { const tab = tabs.find(x => x.id === selected); if (tab) { tab.saved = true; tab.justSaved = true; tab.title = tab.user || tab.title; } },
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
