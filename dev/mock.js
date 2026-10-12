'use strict';
// Fake bridge for designing the UI in a normal browser. ?locked=1 shows the lock screen, ?new=1 the first-run screen,
// ?empty=1 an empty vault.
(() => {
  const params = new URLSearchParams(location.search);
  const cdn = 'https://tr.rbxcdn.com/';
  const av = h => `${cdn}30DAY-AvatarHeadshot-${h}-Png/150/150/AvatarHeadshot/Png/noFilter`;
  const games = {
    2753915549: { name: 'Blox Fruits', creator: 'Gamer Robot Inc', playing: 412873, iconUrl: `${cdn}180DAY-774ec14539b264f85fdb6e8a34dfa344/150/150/Image/Png/noFilter` },
    920587237: { name: '[🎃] Adopt Me!', creator: 'Uplift Games', playing: 186204, iconUrl: `${cdn}180DAY-fd05afa91e29a7895178342064489a36/150/150/Image/Png/noFilter` },
    4924922222: { name: 'Brookhaven 🏡RP', creator: 'Wolfpaq Games', playing: 655120, iconUrl: `${cdn}180DAY-01fa54388131d629ec74c042784454c5/150/150/Image/Png/noFilter` },
  };
  const minsAgo = m => new Date(Date.now() - m * 60000).toISOString();
  const id = n => `00000000-0000-4000-8000-00000000000${n}`;
  const g = p => ({ placeId: p, name: games[p].name, iconUrl: games[p].iconUrl });

  let accounts = params.has('empty') ? [] : [
    { id: id(1), userId: 1, username: 'Roblox', displayName: 'Roblox', alias: 'Acc chính', label: 'Acc chính', group: 'Main', note: 'Tài khoản chơi chính, đừng dùng farm',
      hasPassword: true, cookieValid: true, autoRejoin: true, lastUsed: minsAgo(3), avatarUrl: av('310966282D3529E36976BF6B07B1DC90'), state: 'running',
      presence: { type: 'ingame', game: games[2753915549].name, placeId: 2753915549, iconUrl: games[2753915549].iconUrl },
      session: { pid: 1111, since: minsAgo(47), game: g(2753915549), connection: 'ingame' } },
    { id: id(2), userId: 156, username: 'builderman', displayName: 'builderman', alias: 'Farm 1', label: 'Farm 1', group: 'Farm', note: '',
      hasPassword: false, cookieValid: true, lastUsed: minsAgo(2), avatarUrl: av('635A2AA0A87D473B6136800B5B86F761'), state: 'running',
      presence: { type: 'online' }, session: { pid: 2222, since: minsAgo(params.has('kicked') ? 12 : 1), game: g(2753915549), connection: params.has('kicked') ? 'disconnected' : 'joining', reason: params.has('kicked') ? 278 : null } },
    { id: id(3), userId: 261, username: 'Shedletsky', displayName: 'Shedletsky', alias: 'Farm 2', label: 'Farm 2', group: 'Farm', note: 'Hết hạn tuần trước',
      hasPassword: true, cookieValid: false, lastUsed: minsAgo(60 * 24 * 9), avatarUrl: av('3B3B81F4602EE5424673376A726F427E'), state: 'idle',
      presence: { type: 'offline', lastOnline: minsAgo(60 * 30) }, session: null },
    { id: id(4), userId: 2, username: 'John Doe', displayName: 'John Doe', alias: '', label: 'John Doe', group: 'Farm', note: '',
      hasPassword: false, cookieValid: true, lastUsed: minsAgo(60 * 5), avatarUrl: av('9A3C11044D5172166CEFC75C64ECD14E'), state: 'idle',
      presence: { type: 'ingame', game: games[920587237].name, placeId: 920587237, jobId: 'b6f1c2d4-0000-4c5e-9a7b-123456789abc', iconUrl: games[920587237].iconUrl }, session: null },
    { id: id(5), userId: 3, username: 'Jane Doe', displayName: 'Jane', alias: 'Acc phụ', label: 'Acc phụ', group: '', note: '',
      hasPassword: true, cookieValid: true, lastUsed: null, avatarUrl: av('5D3112D29FE230F31350D3346E66000E'), state: 'idle',
      presence: { type: 'studio', game: 'Place1' }, session: null },
    { id: id(6), userId: 16, username: 'erik.cassel', displayName: 'Erik', alias: 'Trade', label: 'Trade', group: '', note: 'Chỉ dùng trade',
      hasPassword: false, cookieValid: true, lastUsed: minsAgo(60 * 48), avatarUrl: av('4DFD3CAFBB2B81962134E5DF0E068619'), state: 'idle',
      presence: null, session: null },
    { id: id(7), userId: 0, username: 'noavatar_user', displayName: 'No Avatar', alias: 'Thử nghiệm', label: 'Thử nghiệm', group: '', note: '',
      hasPassword: false, cookieValid: null, lastUsed: null, avatarUrl: null, state: 'launching',
      presence: { type: 'offline' }, session: { pid: 0, since: minsAgo(0), game: g(4924922222) } },
  ];
  let locked = params.has('locked') || params.has('new');
  let language = params.get('lang') || 'en';
  let settings = {
    lastPlaceInput: 'https://www.roblox.com/games/2753915549/Blox-Fruits', lastJobId: '', renameWindows: true, launchVia: 'auto', launchDelaySeconds: 8,
    memoryWarnings: true, trimBackground: false, antiAfk: false, robloxConfig: { mode: params.has('rc') ? 'marked' : 'off', graphics: 1, fps: 30, volume: 0 },
    recentGames: [2753915549, 920587237, 4924922222].map(p => ({ placeId: p, name: games[p].name, iconUrl: games[p].iconUrl })),
  };
  let multi = { enabled: true, armed: !params.has('notready'), trayAutostart: true };
  let emit = () => {};
  const state = () => ({
    language, vaultExists: !params.has('new'), locked,
    whatsNew: params.has('whatsnew') ? { from: '1.0.0', releases: [{ version: '1.0.1', date: '2026-10-19', en: ['Faster sign-in check: six accounts at a time.', 'Shows the version on the update button.'], vi: ['Kiểm tra đăng nhập nhanh hơn: 6 tài khoản một lúc.', 'Nút cập nhật ghi rõ phiên bản.'] }] } : null,
    robloxVersion: { installed: '0.742.0.7421053', latest: params.has('rbxold') ? '0.743.0.7430112' : '0.742.0.7421053', upToDate: !params.has('rbxold'), direct: false },
    update: { supported: true, current: '1.0.0', status: params.has('update') ? 'ready' : 'upToDate', version: params.has('update') ? '1.0.1' : null, progress: 0, error: null, auto: true },
    installs: { handler: 'Bloxstrap', bloxstrap: true, fishstrap: false, roblox: false, direct: true, store: params.has('store') },
    monitors: [{ index: 0, primary: true, width: 1920, height: 1080 }, { index: 1, primary: false, width: 2560, height: 1440 }], dataPath: 'C:\\Users\\Admin\\AppData\\Local\\RobloxManager', robloxInstalled: true,
    launching: false, checking: false, multi, settings: locked ? null : settings, accounts: locked ? [] : accounts,
  });
  const push = () => emit('state', state());
  const wait = ms => new Promise(r => setTimeout(r, ms));
  const metrics = () => ({
    accounts: accounts.filter(a => a.state === 'running').map((a, i) => ({ id: a.id, ramMb: 780 + i * 140 + Math.round(Math.random() * 30) })),
    robloxCount: accounts.filter(a => a.state === 'running').length,
    totalMb: 1750 + Math.round(Math.random() * 40), systemMb: 32 * 1024, usedPct: 63, availableMb: 12 * 1024,
  });

  window.__mockBridge = {
    attach(fn) {
      emit = fn;
      setInterval(() => !locked && emit('metrics', metrics()), 1500);
    },
    async call(cmd, a) {
      await wait(120);
      switch (cmd) {
        case 'getState': return { resume: false, view: null, state: state() };
        case 'keepView': return null;
        case 'servers': await wait(400); return { servers: Array.from({ length: 12 }, (_, i) => ({ jobId: `00000000-0000-0000-0000-${String(i).padStart(12, '0')}`, playing: 15 - i, maxPlayers: 15, ping: 60 + i * 9, fps: 60 })), next: a?.cursor ? null : 'more' };
        case 'seenWhatsNew': return null;
        case 'whatsNew': return [{ version: '1.0.0', date: '2026-10-12', en: ['First release.'], vi: ['Bản đầu tiên.'] }];
        case 'unlock': case 'createVault':
          await wait(500);
          if (a.password === 'sai') throw new Error('Sai mật khẩu chủ.');
          locked = false;
          return state();
        case 'lock': locked = true; push(); return null;
        case 'setLanguage': language = a.language; push(); return null;
        case 'resolveGame': {
          const m = /(\d{5,})/.exec(a.input);
          if (!m) return { ok: false, error: 'Không đọc được Place ID từ link này.' };
          const info = games[m[1]];
          return { ok: true, kind: /privateServerLinkCode/.test(a.input) ? 'private' : 'game', placeId: +m[1], ...(info || {}) };
        }
        case 'setSettings':
          if ('multiInstance' in a) multi = { ...multi, enabled: a.multiInstance };
          settings = { ...settings, ...a };
          push();
          return null;
        case 'launch': {
          if (a.ids.length > 1 && !multi.armed && !a.force) return { needsConfirm: true, message: 'Multi-Roblox chưa sẵn sàng vì đã có Roblox chạy trước.\n\nVẫn tiếp tục mở?' };
          for (const x of a.ids) {
            const acc = accounts.find(y => y.id === x);
            if (acc) acc.state = 'launching';
            emit('progress', { text: `Đang mở ${acc?.label}…` });
            push();
            await wait(900);
          }
          emit('progress', { text: null });
          setTimeout(() => {
            accounts.forEach(x => { if (x.state === 'launching') { x.state = 'running'; x.session = { pid: 9, since: new Date().toISOString(), game: g(2753915549) }; } });
            push();
          }, 1500);
          return { launched: a.ids.length, errors: [], cancelled: false };
        }
        case 'closeAccounts':
          accounts.forEach(x => { if (a.ids.includes(x.id) && x.state === 'running') { x.state = 'idle'; x.session = null; } });
          push();
          return a.ids.length;
        case 'deleteAccounts': accounts = accounts.filter(x => !a.ids.includes(x.id)); push(); return a.ids.length;
        case 'checkAccounts': await wait(800); return { ok: 5, bad: 1, failed: 0 };
        case 'refreshPresence': await wait(700); return null;
        case 'copy': return { secret: a.field !== 'username' };
        case 'reveal': return a.field === 'password' ? 'matkhau-demo' : '_|WARNING:-DO-NOT-SHARE-THIS.--demo';
        case 'updateAccount': {
          const acc = accounts.find(y => y.id === a.id);
          Object.assign(acc, { alias: a.alias, group: a.group, note: a.note, label: a.alias || acc.username });
          push();
          return true;
        }
        case 'addViaCookie': throw new Error('Cookie không hợp lệ hoặc đã hết hạn.');
        case 'addViaBrowser': throw new Error('Bản xem thử không mở được trình duyệt đăng nhập.');
        case 'closeAll': return 2;
        case 'importRam': return { added: 3, updated: 1, skipped: 0, total: 4 };
        case 'arrangeWindows': return 2;
        case 'setAutoRejoin':
          accounts.forEach(x => { if (a.ids.includes(x.id)) x.autoRejoin = a.enabled; });
          push();
          return a.ids.length;
        case 'disableTrayAutostart': multi = { ...multi, trayAutostart: false }; push(); return null;
        // Sign-in: user "2fa" asks for a 2-step code (any 6 digits passes), "captcha" hits a captcha, "x" is a wrong password.
        case 'loginPassword':
          await wait(600);
          if (a.username === 'x') throw new Error('Incorrect username or password. Please try again.');
          if (a.username === 'captcha') return { status: 'captcha' };
          if (a.username === '2fa') return { status: 'twostep' };
          return { status: 'added', added: { id: id(1), label: a.username, merged: false } };
        case 'loginTwoStep':
          await wait(500);
          if (!/^\d{6}$/.test(a.code)) throw new Error('Invalid code.');
          return { status: 'added', added: { id: id(2), label: 'Farm 1', merged: false } };
        case 'loginCaptcha': await wait(800); return { status: 'added', added: { id: id(3), label: 'captcha', merged: false } };
        case 'quickLoginStart': await wait(500); return { code: 'K7Q2XW', seconds: 120, qr: null };
        case 'quickLoginPoll': await wait(200); return { status: 'waiting' };
        case 'changePassword': await wait(400); if (a.current !== 'demo') throw new Error('Mật khẩu chủ hiện tại không đúng.'); return true;
        default: return null;
      }
    },
  };
  // Scripted views for headless screenshots: ?settings=1, ?select=1, ?menu=1, ?edit=1
  window.addEventListener('load', () => setTimeout(() => {
    const click = sel => document.querySelector(sel)?.click();
    if (params.has('settings')) click('[data-view=settings]');
    if (params.has('select') || params.has('menu')) { click('.card[data-id$="2"]'); click('.card[data-id$="4"]'); }
    if (params.has('menu')) click('.card[data-id$="4"] [data-act=card-menu]');
    if (params.has('edit')) click('.card[data-id$="1"] [data-act=edit]');
  }, 600));
})();
