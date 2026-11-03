'use strict';
// Fake bridge for designing the UI in a normal browser. ?locked=1 shows the lock screen, ?new=1 the first-run screen,
// ?empty=1 an empty vault, ?theme=light (system|light|paper|dim|dark|black) a theme, ?christmas=1 December's logo. Presets: ?view=presets opens their
// tab, ?nopresets=1 has none, ?many=1 the most the app keeps (50), ?noicons=1 no game icons known yet. ?rejoin=1: an
// account waiting minutes for auto-rejoin's next try.
(() => {
  const params = new URLSearchParams(location.search);
  if (params.has('christmas')) window.BloxkeepChristmas = true;
  const cdn = 'https://tr.rbxcdn.com/';
  // Made-up block-head avatars drawn here (no real player's picture or name appears in screenshots).
  const face = (bg, skin, hat) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 150 150"><rect width="150" height="150" fill="${bg}"/><rect x="38" y="44" width="74" height="70" rx="16" fill="${skin}"/>${hat ? `<rect x="32" y="30" width="86" height="26" rx="10" fill="${hat}"/>` : ''}<circle cx="62" cy="76" r="5" fill="#1d1d1f"/><circle cx="88" cy="76" r="5" fill="#1d1d1f"/><path d="M60 94q15 12 30 0" stroke="#1d1d1f" stroke-width="5" fill="none" stroke-linecap="round"/></svg>`);
  const games = {
    2753915549: { name: 'Blox Fruits', creator: 'Gamer Robot Inc', playing: 412873, iconUrl: `${cdn}180DAY-774ec14539b264f85fdb6e8a34dfa344/150/150/Image/Png/noFilter` },
    920587237: { name: '[🎃] Adopt Me!', creator: 'Uplift Games', playing: 186204, iconUrl: `${cdn}180DAY-fd05afa91e29a7895178342064489a36/150/150/Image/Png/noFilter` },
    4924922222: { name: 'Brookhaven 🏡RP', creator: 'Wolfpaq Games', playing: 655120, iconUrl: `${cdn}180DAY-01fa54388131d629ec74c042784454c5/150/150/Image/Png/noFilter` },
  };
  // ?long=1: names as long as they get on Roblox (emoji and brackets, no spaces), to check nothing overflows.
  const LONG = params.has('long');
  // ?noroblox=1: Roblox isn't installed on this PC (no window, no version).
  const NO_ROBLOX = params.has('noroblox');
  if (LONG) {
    games[2753915549].name = '[🎃 HALLOWEEN PT 1] Grand Piece Online';
    games[920587237].name = '[🎃 UPDATE 5] Some Very Long Game Name Simulator 2 — Ultimate Edition';
    games[4924922222].name = 'Brookhaven 🏡RP: Roleplay Hangout, Chill & Make Friends Worldwide (NEW!)';
  }
  // Made-up names and notes in the page's language, so the English screenshots have no Vietnamese in them.
  const tx = (en, vi) => params.get('lang') === 'vi' ? vi : en;
  const minsAgo = m => new Date(Date.now() - m * 60000).toISOString();
  // Like the app: the next minute a preset's schedule starts at (local time), or null.
  const nextRun = s => {
    if (!s?.enabled || !/^\d\d:\d\d$/.test(s.time)) return null;
    const [h, m] = s.time.split(':').map(Number);
    const now = new Date();
    if (s.date) {
      const [y, mo, d] = s.date.split('-').map(Number);
      const at = new Date(y, mo - 1, d, h, m);
      return at >= now ? at.toISOString() : null;
    }
    for (let d = 0; d <= 7; d++) {
      const at = new Date(now.getFullYear(), now.getMonth(), now.getDate() + d, h, m);
      if (s.days.includes(at.getDay()) && at >= now) return at.toISOString();
    }
    return null;
  };
  const id = n => `00000000-0000-4000-8000-00000000000${n}`;
  const g = p => ({ placeId: p, name: games[p].name, iconUrl: games[p].iconUrl });
  const PRESET_SERVERS = ['any', 'most', 'fewest', 'ping', 'random'];
  // Tests/perf: ?coldicons=1 gives Statistics' and the presets' game icons addresses the browser hasn't loaded yet (games
  // not on the accounts page); ?statsms=350 has Statistics answer that much later, as the app does when it asks Roblox first.
  const coldStamp = Date.now();
  const cold = url => url && params.has('coldicons') ? `${url}?cold=${coldStamp}` : url;
  // Like the app: a preset's game icon when the game is known (?noicons=1: none yet, as right after starting).
  const presetIcon = input => {
    const m = /(\d{5,})/.exec(input || '');
    return m && games[m[1]] && !params.has('noicons') ? cold(games[m[1]].iconUrl) : null;
  };
  const MAX_PRESETS = 50;
  const QL_STEPS = (params.get('ql') || '').split(',').filter(Boolean);
  let qlStep = 0, qlStarts = 0, qlFailed = false;

  let accounts = params.has('empty') ? [] : [
    { id: id(1), userId: 5140000001, username: 'sunny_builds', displayName: 'Sunny', alias: tx('Main', 'Tài khoản chính'), label: tx('Main', 'Tài khoản chính'), group: 'Main', note: tx("My main account, don't farm on it", 'Tài khoản chơi chính, đừng dùng farm'),
      hasPassword: true, hasAuthenticator: true, cookieValid: true, robux: 12450, autoRejoin: true, lastUsed: minsAgo(3), avatarUrl: face('#2b3a55', '#f2c94c', '#3b82f6'), state: 'running',
      presence: { type: 'ingame', game: games[2753915549].name, placeId: 2753915549, iconUrl: games[2753915549].iconUrl },
      session: { pid: 1111, since: minsAgo(47), game: g(2753915549), connection: 'ingame', hidden: params.has('winhidden'), fpsLimit: Number(params.get('fps')) || 0, robloxVersion: params.has('oldwin') ? '0.742.0.7421053' : null, oldBuild: params.has('oldwin') } },
    { id: id(2), userId: 5140000002, username: 'pixelfarmer01', displayName: 'pixelfarmer01', alias: 'Farm 1', label: 'Farm 1', group: 'Farm', note: '',
      hasPassword: false, cookieValid: true, robux: 80, lastUsed: minsAgo(2), avatarUrl: face('#1f3b2d', '#f2c94c', '#22c55e'), state: 'running',
      presence: { type: 'online' }, session: { pid: 2222, since: minsAgo(params.has('kicked') ? 12 : 1), game: g(2753915549), connection: params.has('kicked') ? 'disconnected' : params.get('phase') === 'joinFailed' ? 'menu' : 'joining',
        phase: params.get('phase'), queuePosition: params.has('queue') ? Number(params.get('queue')) : null, reason: params.has('kicked') ? (params.get('kicked') === 'unknown' ? null : 278) : null } },
    { id: id(3), userId: 5140000003, username: 'pixelfarmer02', displayName: 'pixelfarmer02', alias: 'Farm 2', label: 'Farm 2', group: 'Farm', note: tx('Expired last week', 'Hết hạn tuần trước'),
      hasPassword: true, cookieValid: false, lastUsed: minsAgo(60 * 24 * 9), avatarUrl: face('#3a2a1f', '#e8b48a', '#f97316'), state: 'idle',
      presence: { type: 'offline', lastOnline: minsAgo(60 * 30) }, session: null },
    { id: id(4), userId: 5140000004, username: 'night_owl_rb', displayName: 'Night Owl', alias: '', label: 'Night Owl', group: 'Farm', note: '',
      hasPassword: false, cookieValid: true, robux: 0, lastUsed: minsAgo(60 * 5), avatarUrl: face('#262a3a', '#d6d9e0', null), state: 'idle',
      presence: { type: 'ingame', game: games[920587237].name, placeId: 920587237, jobId: 'b6f1c2d4-0000-4c5e-9a7b-123456789abc', iconUrl: games[920587237].iconUrl }, session: null },
    { id: id(5), userId: 5140000005, username: 'lananh_2010', displayName: 'Lan Anh', alias: tx('Alt', 'Tài khoản phụ'), label: tx('Alt', 'Tài khoản phụ'), group: '', note: '',
      hasPassword: true, cookieValid: true, robux: 1300, lastUsed: null, avatarUrl: face('#3a2440', '#f5d0b5', '#ec4899'), state: 'idle',
      presence: { type: 'studio', game: 'Place1' }, session: null },
    { id: id(6), userId: 5140000006, username: 'tradepost_vn', displayName: 'Trade Post', alias: 'Trade', label: 'Trade', group: '', note: tx('Trading only', 'Chỉ dùng trade'),
      hasPassword: false, cookieValid: true, banned: true, bannedSince: minsAgo(60 * 20), lastUsed: minsAgo(60 * 48), avatarUrl: face('#2f2a1c', '#c99a6b', '#a3762e'), state: 'idle',
      presence: null, session: null,
      // ?rejoin=1: dropped a few times in a row, so auto-rejoin waits a few minutes before the next try.
      rejoin: params.has('rejoin') ? { at: new Date(Date.now() + 4.5 * 60000).toISOString(), restart: false, game: g(920587237) } : null },
    { id: id(7), userId: 0, username: 'noavatar_user', displayName: 'No Avatar', alias: tx('Testing', 'Thử nghiệm'), label: tx('Testing', 'Thử nghiệm'), group: '', note: '',
      hasPassword: false, cookieValid: null, lastUsed: null, avatarUrl: null, state: 'launching',
      presence: { type: 'offline' }, session: { pid: 0, since: minsAgo(0), game: g(4924922222) } },
  ];
  // ?vng=1: Trade is a Roblox VNG account (the VNG tag on its card and in its list row).
  if (params.has('vng')) accounts.find(a => a.id === id(6)).realm = 'vng';
  // ?fresh=1: accounts Bloxkeep has never opened (just added): none running or opened before, so Getting started shows.
  if (params.has('fresh')) accounts = accounts.map(a => ({ ...a, state: 'idle', session: null, lastUsed: null, presence: a.presence?.type === 'ingame' ? { type: 'offline' } : a.presence }));
  // ?nowindows=1: no Roblox window open (every account idle), for the macro editor's empty picture. ?noroblox=1: Roblox
  // isn't installed, so no window can be open either.
  if (params.has('nowindows') || params.has('noroblox'))
    accounts = accounts.map(a => a.state === 'running' || (a.state === 'launching' && params.has('noroblox')) ? { ...a, state: 'idle', session: null } : a);
  // ?accounts=300: that many accounts (copies of the ones above), for timing the page with a big list.
  if (params.has('accounts')) {
    const want = Math.min(1000, Number(params.get('accounts')) || 0), base = accounts.slice();
    for (let i = accounts.length; i < want; i++) {
      const b = base[i % base.length];
      accounts.push({ ...b, id: '00000000-0000-4000-8000-' + String(1000 + i).padStart(12, '0'), userId: 5140100000 + i, label: `${b.label} ${i}`, alias: `${b.alias} ${i}`, username: `${b.username}_${i}`,
        session: b.session && { ...b.session, pid: 20000 + i } });
    }
  }
  if (LONG) {
    const longs = [['Sunny_The_Builder_Of_Many_Worlds_2010', 'SunnyTheBuilderOfManyWorlds2010', 'Main accounts and everything else'],
      ['pixelfarmer01_with_a_very_long_alias', 'pixelfarmer01_very_long_username', 'Farm farm farm farm farm farm farm'],
      ['MegaLongAccountLabelWithoutAnySpaces', 'MegaLongAccountLabelWithoutAnySp', 'Farm farm farm farm farm farm farm'],
      ['Night Owl The Night Owl Of The Night Owls', 'night_owl_rb_night_owl_rb_nigh', 'Farm farm farm farm farm farm farm']];
    longs.forEach(([label, username, group], i) => Object.assign(accounts[i], { label, alias: label, username, displayName: username, group, note: 'A very long note about this account that goes on and on and on, and on, and on, and on.' }));
    accounts[4].username = 'lananh_2010_lananh_2010_lananh'; accounts[4].label = accounts[4].alias = tx('Lan Anh (alt, for trading and buying items)', 'Lan Anh (tài khoản phụ, dùng để trade và mua đồ)');
    accounts[6].group = tx('AVeryLongTestingGroupNameWithNoBreaksAtAll', 'Nhóm thử nghiệm với tên rất dài không ngắt'); accounts[6].username = 'noavatar_user_noavatar_user';
  }
  // ?robuxwait=1: right after unlocking, the Robux not answered yet (each signed-in card keeps the number's place).
  if (params.has('robuxwait')) accounts.forEach(a => { if (a.cookieValid && !a.banned) { delete a.robux; a.robuxWait = true; } });
  // ?autolock=15: Bloxkeep behind its lock screen after a while without use (the vault open behind it);
  // ?autolockin=1500: the same, coming up 1.5 s after the page opens (as if the PC went unused meanwhile).
  let screenLock = params.has('autolock') ? { minutes: +params.get('autolock') || 15 } : null;
  let locked = params.has('locked') || params.has('new') || !!screenLock;
  // ?lockupdatein=1500: the same update found 1.5 s after the lock screen came up (someone may be typing by then).
  let lockUpdate = params.get('lockupdate');
  let lockUpdateGo = null;
  let language = params.get('lang') || 'en';
  let theme = params.get('theme') || 'dark';
  // Set up yet (?new=1: not). The vaults on this PC (?vaults=1: three, one with a picture; ?vaultpick=1: the picker
  // shows, as at a start that asks), the one offered, one being added.
  let vaultExists = !params.has('new');
  let vaultList = params.has('new') ? [] : params.has('vaults')
    ? [{ id: 'a1b2c3d4', name: 'Main', picture: null }, { id: 'b2c3d4e5', name: 'Farm', picture: 'scene-halong.svg' }, { id: 'c3d4e5f6', name: 'Alt', picture: null }]
    : [{ id: 'a1b2c3d4', name: 'Vault 1', picture: null }];
  let vaultCurrent = vaultList[0]?.id || null, vaultPick = params.has('vaultpick'), vaultAdding = null, askVault = params.has('vaultpick'), vaultAddFromPick = false;
  // ?guardfail=1: the vault list is a number, so the sidebar (which reads it as a list) fails to draw and Guard shows its
  // note. The failure is the point of the scenario: Guard and React report it with console.error, which Tests/ui counts.
  if (params.has('guardfail')) console.error = () => {};
  const vaultsState = () => {
    const cur = vaultAdding || vaultList.find(v => v.id === vaultCurrent);
    return { list: params.has('guardfail') ? 5 : vaultList, current: cur?.id || null, name: cur?.name || null, picture: vaultAdding ? null : cur?.picture || null,
      open: locked ? null : vaultCurrent, pick: vaultPick, adding: !!vaultAdding, ask: askVault };
  };
  // Settings › Appearance › Custom (?theme=custom; ?ctpicture=1 with a picture, a scenery theme's standing in).
  let customTheme = { background: params.has('ctbg') ? '#' + params.get('ctbg') : '#1b2a3a', accent: '#f59e0b', fade: 45, blur: +(params.get('ctblur') || 0),
    picture: params.has('ctpicture') ? 'scene-halong.svg' : null, video: false };
  // Your themes (?saved=1): a few kept by name, one of them the theme in use.
  let savedThemes = params.has('saved') ? [
    { id: 't1', name: 'Night drive', look: { background: '#141a26', accent: '#22c55e', fade: 45, blur: 0, picture: null, video: false } },
    { id: 't2', name: 'Ha Long', look: { background: '#1b2a3a', accent: '#f59e0b', fade: 45, blur: 8, picture: 'scene-halong.svg', video: false } },
    { id: 't3', name: 'Rain loop', look: { background: '#1a2b3a', accent: '#60a5fa', fade: 55, blur: 4, picture: 'scene-rainy.svg', video: true } },
  ] : [];
  let closeToTray = true;
  let hideNames = params.has('hide'); // ?hide=1: Settings › Appearance › Hide account names is on
  // A made-up authenticator code (not a real TOTP): six digits that change every 30 s, as the app's do.
  const fakeCode = () => {
    const step = Math.floor(Date.now() / 30000);
    return { ok: true, code: String((step * 7919 + 271828) % 1000000).padStart(6, '0'), seconds: 30 - Math.floor(Date.now() / 1000) % 30, period: 30 };
  };
  let notifications = !params.has('nonotify'); // ?nonotify=1: the main switch off (the kinds greyed)
  let notifyOff = params.has('nonotify') ? [] : ['ram']; // kinds turned off under it
  let dailySignInCheck = true;
  let betaUpdates = params.has('beta'); // ?beta=1: Settings › Get beta versions on
  let countOnline = !params.has('offline'); // ?offline: Settings › Add me to the online count off
  // ?remote=code|linked|connecting: Settings › Discord › Control from Discord on, waiting for its code to be typed, linked,
  // or not connected yet.
  const remoteCode = () => ({ on: true, connected: true, linked: false, user: null, code: 'K7MX4P', codeUntil: Date.now() + 9.5 * 60_000, pc: 'GAMING-PC' });
  let remote = params.get('remote') === 'linked' ? { on: true, connected: true, linked: true, user: 'Minh', code: null, codeUntil: null, pc: 'GAMING-PC' }
    : params.get('remote') === 'connecting' ? { on: true, connected: false, linked: null, user: null, code: null, codeUntil: null, pc: 'GAMING-PC' }
    : params.get('remote') === 'code' ? remoteCode()
    // ?remote=ask: someone typed this PC's code into /pc link, replacing the link to Minh.
    : params.get('remote') === 'ask' ? { on: true, connected: true, linked: true, user: 'Minh', code: null, codeUntil: null, pc: 'GAMING-PC', ask: { user: '@stranger', replacing: 'Minh', id: '900000000000000001' } }
    : { on: false, connected: false, linked: null, pc: 'GAMING-PC' };
  const skipConfirm = [];
  const lastSignInCheck = minsAgo(60 * 5);
  let miniOpen = params.has('mini'); // ?mini=1: the mini window counts as open
  // ?backup=1 shows a chosen folder with a recent backup; ?bkerr=1 a failed one; ?weak=1 the weak-password line.
  let backup = {
    enabled: true, folder: params.has('backup') ? 'C:\\Users\\Admin\\OneDrive\\Bloxkeep' : null,
    lastBackup: params.has('backup') ? minsAgo(5) : null, error: params.has('bkerr') ? tx("Can't write to this folder.", 'Không ghi được vào thư mục này.') : null,
    suggested: [{ path: 'G:\\My Drive\\Bloxkeep', name: 'Google Drive' }, ...(params.has('backup') ? [] : [{ path: 'C:\\Users\\Admin\\OneDrive\\Bloxkeep', name: 'OneDrive' }])],
    cloud: params.has('backup'), weakPassword: params.has('weak'),
  };
  // Settings › Discord: ?discord=1 has a webhook set up and sending on; ?dcgone=1 one Discord said was deleted;
  // ?dcfail=1 makes "Send test" fail. The link is made up (the page only sees it on Show).
  const DC_ID = '123456789012345678';
  let discord = {
    enabled: params.has('discord'), presence: false, presenceReady: true, hasWebhook: params.has('discord') || params.has('dcgone'), webhookHint: `discord.com/api/webhooks/…${DC_ID.slice(-4)}/••••••`,
    stopped: params.has('dcgone') ? 'gone' : null, username: 'Bloxkeep', avatarUrl: '', language: 'app', style: 'embed', hideNames: false,
    summary: { daily: true, dailyTime: '22:00', weekly: false, weekday: 0, weeklyTime: '21:00', afterSchedule: true, days: 1, parts: ['total', 'accounts', 'games', 'sessions', 'kicks'], accounts: [] },
    board: { enabled: params.has('discord'), minutes: 5 }, events: ['drop', 'schedule', 'signin', 'launch'],
    pings: { drop: `user:${DC_ID}`, signin: 'here' }, quiet: { enabled: true, from: '23:00', to: '07:00' },
    lastSent: params.has('discord') ? minsAgo(4) : null, error: null, queued: 0,
  };
  let unlockedAt = 0; // the loading screen's steps take a moment after unlocking
  const boot = () => {
    const ms = Date.now() - unlockedAt;
    return ms < 1400 ? { stage: 'status', percent: 0.3 } : ms < 2200 ? { stage: 'avatars', percent: 0.8 } : null;
  };
  const themeApplied = () => theme !== 'system' ? theme : matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  let settings = {
    lastPlaceInput: 'https://www.roblox.com/games/2753915549/Blox-Fruits', lastJobId: '', renameWindows: true, windowPictures: true, taskbarGroups: 'roblox', launchVia: 'auto', launchDelaySeconds: 8, autoLockMinutes: 0,
    memoryWarnings: true, trimBackground: false, autoUpdateRoblox: true, robloxUpdateWhen: 'idle', antiAfk: false,
    // The fast cores for the window played; ?cpuone=1: a CPU with one kind of core (only power saving to give).
    splitCores: false, cores: params.has('cpuone') ? { hybrid: false, efficient: 0, logical: 8 } : { hybrid: true, efficient: 16, logical: 32 }, robloxConfig: { mode: params.has('rc') ? 'marked' : 'off', graphics: 1, fps: 30, volume: 0 },
    recentGames: [2753915549, 920587237, 4924922222].map(p => ({ placeId: p, name: games[p].name, iconUrl: games[p].iconUrl })),
    // Saved servers (the game picker's Saved tab): a private server's link, and a game's own code; ?nosaved=1: none.
    savedServers: params.has('nosaved') ? [] : [
      { id: 's1', name: 'VIP farm', placeId: 2753915549, universeId: 0, gameName: games[2753915549].name, link: 'https://www.roblox.com/games/2753915549/Blox-Fruits?privateServerLinkCode=12345678901234567890', code: '', iconUrl: games[2753915549].iconUrl },
      { id: 's2', name: tx('Trade room', 'Phòng trade'), placeId: 920587237, universeId: 0, gameName: games[920587237].name, link: '', code: '4F7K2Q', iconUrl: games[920587237].iconUrl },
    ],
    // Launch presets; ?nopresets=1 starts without any. A deleted account (…09) is skipped when it runs.
    presets: params.has('nopresets') ? [] : [
      // p1 runs by itself on weekdays at 20:00 and closes what it opened three hours later; its windows get anti-AFK,
      // the light AFK setup and a restart every 4 hours (options left out follow the usual settings).
      { id: 'p1', name: 'Farm Blox Fruits', accountIds: [id(2), id(3), id(4), id(9)], placeInput: 'https://www.roblox.com/games/2753915549/Blox-Fruits', gameName: games[2753915549].name, server: 'ping', jobId: '', delaySeconds: 12,
        schedule: { enabled: true, time: '20:00', days: [1, 2, 3, 4, 5], date: '', stop: 'after', stopHours: 3, stopTime: '' },
        antiAfk: true, profile: 'afk', restartHours: 4, arrange: true, discord: 'discord.com/api/webhooks/…5678/••••••', running: params.has('nopresetrun') ? 0 : 3 },
      { id: 'p2', name: 'Adopt Me trade', accountIds: [id(6)], placeInput: '920587237', gameName: games[920587237].name, server: 'any', jobId: '', delaySeconds: null, schedule: null, lastRun: minsAgo(95),
        autoRejoin: false },
      // p3 opens Roblox without a game; its one-off schedule is switched off.
      { id: 'p3', name: 'Morning check-in', accountIds: [id(1), id(2), id(4), id(5), id(6), id(7)], placeInput: '', gameName: '', server: 'any', jobId: '', delaySeconds: 0,
        schedule: { enabled: false, time: '07:30', days: [], date: '2026-12-24', stop: 'at', stopHours: 0, stopTime: '08:00' }, lastRun: minsAgo(60 * 26) },
      // ?many=1: as many presets as the app keeps (50), to check the tab scrolls.
      ...(params.has('many') ? Array.from({ length: 47 }, (_, i) => {
        const place = [2753915549, 920587237, 4924922222][i % 3];
        return { id: `pm${i}`, name: i === 5 ? 'A preset name that is as long as the app allows, sixty chars' : `Farm group ${i + 4}`, accountIds: [id(1 + (i % 7)), id(1 + ((i + 3) % 7))],
          placeInput: String(place), gameName: games[place].name, server: PRESET_SERVERS[i % 5], jobId: '', delaySeconds: i % 4 ? 5 + i % 9 : null,
          schedule: i % 3 === 0 ? { enabled: i % 2 === 0, time: `${String(6 + i % 16).padStart(2, '0')}:00`, days: i % 4 ? [0, 6] : [0, 1, 2, 3, 4, 5, 6], date: '', stop: 'never', stopHours: 0, stopTime: '' } : null };
      }) : []),
    ].map(p => ({ antiAfk: null, autoRejoin: null, profile: '', restartHours: 0, arrange: null, discord: null, running: 0, ...p, nextRun: nextRun(p.schedule), iconUrl: presetIcon(p.placeInput), lastRun: p.lastRun || null })),
    // The order Play opens selected accounts in (the ⋯ menu's "Launch order…"); accounts added later go last.
    launchOrder: [id(1), id(2), id(3), id(4), id(5), id(6), id(7)],
    // Names last looked up with "Join a player…" (kept in the vault).
    recentPlayers: ['builder_friend', 'hidden_friend'],
    followJoinedPlayer: true,
  };
  if (LONG) {
    settings.customLauncher = 'Froststrap-Custom-Launcher-With-A-Long-Name'; settings.launchVia = 'custom';
    backup.folder = 'C:\\Users\\Administrator\\OneDrive - Some Company Name Pty Ltd\\Documents\\Backups\\Bloxkeep Vault Backups'; backup.lastBackup = minsAgo(5); backup.cloud = true;
    backup.suggested = [{ path: 'G:\\My Drive\\Some Long Shared Drive Name\\Bloxkeep', name: 'Google Drive - a.very.long.email.address@example.com' }];
    settings.presets[0].name = 'Farm Blox Fruits overnight with every spare account 🎃🎃'; // 60 characters, the most the app keeps
    settings.presets[1].name = 'Adopt_Me_trade_with_no_spaces_in_the_name_at_all_so_it_cant_wrap';
    settings.presets[0].gameName = games[2753915549].name; settings.presets[1].gameName = games[920587237].name;
  }
  // Macros (Beta): ?macros=1 opens their tab with three made-up macros, their windows' runs in mixed states (running,
  // paused while the PC is used, waiting for Unikey to be turned off, failed, done); ?nomacros=1: none yet; ?macrorec=1:
  // a recording under way. ?macroedit=<m1|m2|m3|m4> opens that macro's form; with it ?macropic=1 takes its window picture,
  // ?macrotry=1 presses Try it, ?macrogame=1 Edit on game (?gamedrag=1: a box is then dragged on the game).
  // macroPicture's picture is drawn here (a made-up game: sky, ground, a block player and a green CLAIM button).
  const MACRO_BTN = 'iVBORw0KGgoAAAANSUhEUgAAACQAAAAQCAIAAADxiUp0AAAAJklEQVR42mMQbbClG2IYAMuUjsbRFI1aNmrZqGWjlg1Fy4ZhqQ8AF4uFH2pOtyIAAAAASUVORK5CYII=';
  const MACRO_SCENE = 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="1280" height="720" viewBox="0 0 1280 720">
    <rect width="1280" height="430" fill="#7ec8f0"/><rect y="430" width="1280" height="290" fill="#5aa04a"/>
    <rect x="160" y="300" width="220" height="130" fill="#b97a4a"/><rect x="190" y="240" width="160" height="70" fill="#a33"/>
    <rect x="600" y="300" width="70" height="70" fill="#f2c94c"/><rect x="590" y="370" width="90" height="100" fill="#3b82f6"/>
    <rect x="600" y="470" width="30" height="80" fill="#1f2937"/><rect x="640" y="470" width="30" height="80" fill="#1f2937"/>
    <rect x="24" y="20" width="44" height="44" rx="8" fill="#000" fill-opacity=".5"/><rect x="1120" y="20" width="136" height="40" rx="8" fill="#000" fill-opacity=".45"/>
    <rect x="1000" y="540" width="180" height="64" rx="10" fill="#15803d"/><rect x="1006" y="546" width="168" height="52" rx="8" fill="#22c55e"/>
    <text x="1090" y="582" font-family="Arial" font-size="28" font-weight="700" fill="#fff" text-anchor="middle">CLAIM</text></svg>`);
  let macroSeq = params.has('p2broken') ? 5 : 4;
  let macros = params.has('nomacros') ? [] : [
    { id: 'm1', name: tx('Anti-AFK jump', 'Nhảy chống AFK'), targets: 'accounts', accountIds: [id(1), id(2)], trigger: { kind: 'interval', everySeconds: 300 }, enabled: true, hotkey: 1,
      steps: [{ kind: 'key', key: 'Space', ms: 60 }], recordedSize: { w: 1280, h: 720 } },
    { id: 'm2', name: tx('Claim daily chest', 'Nhận rương hằng ngày'), targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: 2, onFail: 'skip', maxMinutes: 60,
      steps: [{ kind: 'loop', times: 0 }, { kind: 'waitImage', image: MACRO_BTN, threshold: 0.9, timeoutMs: 600000, gone: false },
        { kind: 'clickImage', image: MACRO_BTN, button: 'left', threshold: 0.9, timeoutMs: 5000, region: { x: 0.7, y: 0.65, w: 0.3, h: 0.3 } }, { kind: 'wait', ms: 2000 }, { kind: 'endLoop' }],
      recordedSize: { w: 1280, h: 720 } },
    { id: 'm3', name: tx('Farm route', 'Đường farm'), targets: 'all', accountIds: [], trigger: { kind: 'join' }, enabled: false, hotkey: null, jitterMs: 60,
      steps: [{ kind: 'down', key: 'W' }, { kind: 'wait', ms: 2000 }, { kind: 'up', key: 'W' }, { kind: 'click', x: 0.62, y: 0.48, button: 'left', double: false },
        { kind: 'if', test: { kind: 'text', text: 'Claim' } }, { kind: 'clickImage', image: MACRO_BTN, button: 'left', threshold: 0.9, timeoutMs: 3000 }, { kind: 'endIf' },
        { kind: 'type', text: 'gg' }],
      recordedSize: { w: 800, h: 600 } },
  ].map(m => ({ jitterMs: 40, jitterPx: 3, maxMinutes: 10, onFail: 'stop', ...m }));
  settings.macros = macros;
  const setMacros = list => { macros = list; settings = { ...settings, macros }; };
  let macroRuns = params.has('macros') && !params.has('nomacros') ? [
    { macroId: 'm1', accountId: id(1), status: 'running', step: 0, loops: 12, startedAt: minsAgo(60) },
    { macroId: 'm1', accountId: id(2), status: 'paused', reason: 'personActive', step: 0, loops: 11, startedAt: minsAgo(55) },
    { macroId: 'm2', accountId: id(1), status: 'paused', reason: 'imeOn', step: 1, startedAt: minsAgo(3) },
    // An ended run keeps the step it stopped on as lastStep (the app's r.Pc once it finished), for "Fix" and the editor's marker.
    { macroId: 'm2', accountId: id(2), status: 'failed', reason: 'notFound', lastStep: 1, startedAt: minsAgo(14), endsAt: minsAgo(4) },
    { macroId: 'm3', accountId: id(1), status: 'done', startedAt: minsAgo(30), endsAt: minsAgo(29) },
  ] : [];
  let macroRecording = params.has('macrorec') ? { accountId: id(1), startsAt: minsAgo(0.05), steps: 7 } : null;
  let macroSweep = null;
  const RECORDED = [{ kind: 'down', key: 'W' }, { kind: 'wait', ms: 1200 }, { kind: 'up', key: 'W' }, { kind: 'click', x: 0.703, y: 0.79, button: 'left', double: false },
    { kind: 'wait', ms: 400 }, { kind: 'key', key: 'E', ms: 80 }];

  // m4 "Cast and reel", made the new way: an area its steps look in (Fish area), two values the player may change on its
  // panel (Hold time, Bar colour) and a fish counter. Its panel is up with ?macros=1.
  if (!params.has('nomacros')) macros.push({ id: 'm4', name: tx('Cast and reel', 'Quăng và kéo cần'), targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: 3,
    jitterMs: 40, jitterPx: 3, maxMinutes: 60, onFail: 'stop', recordedSize: { w: 1280, h: 720 },
    options: [{ name: 'o1', label: tx('Hold time', 'Thời gian giữ'), kind: 'number', value: 600, default: 600, min: 0, max: 10000, unit: 's' },
      { name: 'o2', label: tx('Bar color', 'Màu thanh'), kind: 'colour', value: '#22c55e', default: '#22c55e' }],
    areas: [{ name: 'a1', label: tx('Fish area', 'Vùng cá'), colour: '#22c55e', rect: { x: 0.3, y: 0.74, w: 0.4, h: 0.1 } }],
    panel: { autoShow: true, placeId: 2753915549 },
    steps: [
      { kind: 'loop', times: 0 }, { kind: 'click', x: 0.5, y: 0.6, button: 'left', double: false, holdMs: '$o1' },
      { kind: 'waitColour', x: 0.5, y: 0.5, colour: '$o2', tolerance: 24, timeoutMs: 15000, gone: false, region: '@a1' },
      { kind: 'follow', region: '@a1', target: '$o2', marker: { left: '#ffffff', right: '#ffffff' }, tolerance: 24, button: 'left', mode: 'smooth', kp: 1, kd: 0.4,
        predict: true, fps: 60, untilGoneMs: 1500, timeoutMs: 60000 },
      { kind: 'addVar', name: 'fish', value: 1 }, { kind: 'endLoop' }] });
  // ?p2broken=1: one more macro, with what stops a save on its rows (an If never ended, a click with no point).
  if (params.has('p2broken')) macros.push({ id: 'm5', name: tx('Daily reward', 'Quà hằng ngày'), targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null,
    jitterMs: 40, jitterPx: 3, maxMinutes: 10, onFail: 'stop', options: [], areas: [], panel: { autoShow: false, placeId: null },
    steps: [{ kind: 'key', key: 'E', ms: 60, note: tx('Opens the rewards window', 'Mở cửa sổ phần thưởng') },
      { kind: 'if', test: { kind: 'text', text: 'Claim' } },
      { kind: 'click', button: 'left', double: false },
      { kind: 'addVar', name: 'claims', value: 1 }] });
  setMacros(macros.map(m => ({ options: [], areas: [], panel: { autoShow: false, placeId: null }, ...m })));
  let panelsShown = params.has('macros') && !params.has('nomacros') ? ['m4'] : [];
  const macroVars = params.has('macros') ? { [id(1)]: { fish: 12 }, [id(2)]: { fish: 9 } } : {};
  const TPL_BASE = { targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null, jitterMs: 40, jitterPx: 3, onFail: 'stop',
    options: [], areas: [], panel: { autoShow: false, placeId: null } };
  // The five built-in templates, as MacroTemplates.cs builds them (keys, click, cast, rejoin, blank), with their notes and the
  // fields still to set for the person's game (todo). A new copy each time: the page fills one in.
  const TEMPLATES = () => ({ macros: [
    { ...TPL_BASE, id: '', name: tx('Press a key every few minutes', 'Bấm phím định kỳ'), icon: 'keyboard', maxMinutes: 240,
      about: tx("Presses a key now and then, so the game doesn't take you for away.", 'Thỉnh thoảng bấm một phím để game không coi bạn là vắng mặt.'),
      options: [{ name: 'o1', label: tx('Every', 'Mỗi'), kind: 'number', value: 300000, default: 300000, min: 1000, max: 1080000, unit: 's' },
        { name: 'o2', label: tx('Key', 'Phím'), kind: 'key', value: 'Space', default: 'Space' }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'key', key: '$o2', ms: 60, note: tx('Roblox kicks idle players after 20 minutes; this presses a key before that.', 'Roblox đá người chơi đứng yên sau 20 phút; bước này bấm phím trước lúc đó.') },
        { kind: 'wait', ms: '$o1', note: tx('The player can change how long on the panel', 'Người chơi đổi thời gian này trên bảng') }, { kind: 'endLoop' }] },
    { ...TPL_BASE, id: '', name: tx('Click a button when it shows', 'Bấm nút khi nó hiện'), icon: 'pointer', maxMinutes: 120,
      about: tx('Waits for a button of a color to show up in an area, clicks it, and waits for the next one.', 'Chờ nút có màu đã chọn hiện trong vùng, bấm vào rồi chờ nút tiếp theo.'),
      options: [{ name: 'o1', label: tx('Button color', 'Màu nút'), kind: 'colour', value: '#22c55e', default: '#22c55e' },
        { name: 'o2', label: tx('Then wait', 'Rồi chờ'), kind: 'number', value: 500, default: 500, min: 0, max: 60000, unit: 's' }],
      areas: [{ name: 'a1', label: tx('Button area', 'Vùng nút'), colour: '#22c55e', rect: { x: 0.25, y: 0.25, w: 0.5, h: 0.5 } }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'waitColour', colour: '$o1', tolerance: 24, region: '@a1', timeoutMs: 600000, todo: ['region', 'colour'], note: tx('Move the box to where the button appears', 'Kéo khung tới chỗ nút hiện ra') },
        { kind: 'clickColour', colour: '$o1', tolerance: 24, region: '@a1', minSize: 6, button: 'left', timeoutMs: 2000, note: tx("Pick the button's color on the game", 'Chọn màu nút trên game') },
        { kind: 'addVar', name: 'clicks', value: 1, note: tx('Counts the clicks, shown on the panel', 'Đếm số lần bấm, hiện trên bảng') },
        { kind: 'wait', ms: '$o2' }, { kind: 'endLoop' }] },
    { ...TPL_BASE, id: '', name: tx('Cast and reel', 'Thả câu và kéo'), icon: 'bar', maxMinutes: 240,
      about: tx('Holds the mouse to cast, waits for the bar, then keeps the marker on the target until the bar goes.', 'Giữ chuột để quăng, chờ thanh hiện ra, rồi giữ vạch trên mục tiêu đến khi thanh biến mất.'),
      options: [{ name: 'o1', label: tx('Hold to cast', 'Giữ để quăng'), kind: 'number', value: 800, default: 800, min: 0, max: 10000, unit: 's' },
        { name: 'o2', label: tx('Target color', 'Màu mục tiêu'), kind: 'colour', value: '#22c55e', default: '#22c55e' },
        { name: 'o3', label: tx('Marker color', 'Màu vạch'), kind: 'colour', value: '#ffffff', default: '#ffffff' },
        { name: 'o4', label: tx('Strength', 'Lực'), kind: 'number', value: 1, default: 1, min: 0, max: 10, unit: '' },
        { name: 'o5', label: tx('Wait for the bar', 'Chờ thanh'), kind: 'number', value: 60000, default: 60000, min: 1000, max: 600000, unit: 's' }],
      areas: [{ name: 'a1', label: tx('Bar area', 'Vùng thanh'), colour: '#3b82f6', rect: { x: 0.3, y: 0.75, w: 0.4, h: 0.06 } }],
      steps: [{ kind: 'loop', times: 0 },
        { kind: 'click', x: 0.5, y: 0.5, button: 'left', holdMs: '$o1', todo: ['x'], note: tx('Holds to cast: drag dot 2 to where you cast', 'Giữ để quăng: kéo chấm 2 tới chỗ quăng') },
        { kind: 'follow', region: '@a1', target: '$o2', marker: '$o3', tolerance: 24, button: 'left', mode: 'smooth', kp: '$o4', kd: 0.4, predict: true, fps: 60,
          untilGoneMs: 1000, timeoutMs: '$o5', todo: ['region', 'target', 'marker'], note: tx('Keeps the marker on the target until the bar goes', 'Giữ vạch trên mục tiêu đến khi thanh biến mất') },
        { kind: 'addVar', name: 'catches', value: 1, note: tx('Counts each catch', 'Đếm số lần câu được') },
        { kind: 'wait', ms: 1500, note: tx('A short break before the next cast', 'Nghỉ một chút trước lần quăng sau') }, { kind: 'endLoop' }] },
    { ...TPL_BASE, id: '', name: tx('Rejoin and walk back', 'Vào lại rồi đi về chỗ cũ'), icon: 'rejoin', maxMinutes: 10, targets: 'all', trigger: { kind: 'rejoin' },
      about: tx('After a rejoin, waits for the game to load, walks back and presses E.', 'Sau khi vào lại, chờ game tải, đi về chỗ cũ rồi bấm E.'),
      steps: [{ kind: 'wait', ms: 8000, note: tx('Give the game time to load', 'Chờ game tải xong') },
        { kind: 'key', key: 'W', ms: 2000, note: tx('Walks forward for 2 s: change it to your way back', 'Đi tới 2 giây: đổi theo đường về của bạn') },
        { kind: 'wait', ms: 1000, note: tx('Stops for a moment', 'Dừng một chút') },
        { kind: 'key', key: 'E', ms: 60, note: tx('Uses what is there', 'Dùng thứ ở đó') }] },
    { ...TPL_BASE, id: '', name: tx('New macro', 'Macro mới'), icon: 'blank', maxMinutes: 10, about: tx('Start from nothing and build your own.', 'Bắt đầu từ đầu và tự làm.'), steps: [] },
  ] });
  let tryTimer = 0;
  let multi = { enabled: !params.has('multioff'), armed: !params.has('notready') && !params.has('multioff'), windowed: true, tile: false, trayAutostart: true };
  /**
   * The app's launch warnings (AppController.LaunchAccountsAsync): Multi-Roblox off or not ready with several accounts,
   * low RAM with ?lowram, Windows' commit limit almost used up with ?lowcommit; one answered "Don't ask again" (launch.<kind>) is left out, as the app does.
   */
  function launchAsks(count, force) {
    const vi = language === 'vi', asks = [];
    if (count > 1 && !multi.armed) asks.push(multi.enabled
      ? ['multiNotReady', vi ? 'Multi-Roblox chưa hoạt động: Roblox đã mở từ trước khi bật Multi-Roblox. Cửa sổ Roblox mới có thể đóng cửa sổ cũ.' : "Multi-Roblox isn't working yet: Roblox was already open before it was turned on. A new Roblox window may close the old one."]
      : ['multiOff', vi ? 'Multi-Roblox đang tắt nên mỗi lần mở Roblox mới, cửa sổ cũ sẽ bị đóng.' : 'Multi-Roblox is off, so each new Roblox window closes the previous one.']);
    // ?lowcommit=1: Windows' virtual memory (RAM and paging file together) almost used up, with RAM to spare.
    if (params.has('lowcommit')) asks.push(['memory', vi ? 'RAM vẫn còn trống, nhưng bộ nhớ ảo của Windows sắp đầy nên mở thêm 2 cửa sổ Roblox có thể bị văng. Hãy tắt bớt ứng dụng khác hoặc tăng bộ nhớ ảo.' : "RAM is free, but Windows' virtual memory is almost full, so 2 more Roblox windows may crash as they open. Close other apps or increase virtual memory."]);
    else if (params.has('lowram')) asks.push(['memory', vi ? 'RAM chỉ còn trống 1.2 GB, mà mở thêm 2 cửa sổ Roblox cần khoảng 1.6 GB. Máy có thể bị chậm hoặc Roblox bị văng.' : 'Only 1.2 GB of RAM is free, and 2 more Roblox windows need about 1.6 GB. The PC may slow down or Roblox may crash.']);
    const left = asks.filter(([k]) => !(['multiOff', 'watch', 'memory'].includes(k) && skipConfirm.includes(`launch.${k}`)));
    return left.length && !force ? { needsConfirm: true, message: left.map(x => x[1]).join('\n\n'), asks: left.map(x => x[0]) } : null;
  }
  // Import from another vault file: the made-up file's accounts (display fields only, as the app sends them) and
  // presets. Its master password is "demo"; three wrong ones make each next try wait 10 s, as in the app.
  // ?ivsame=1: the picked file is the vault in use.
  const IV_FILE = [
    { label: 'Farm 3', username: 'pixelfarmer03', realm: '', group: 'Farm', hasPassword: true, cookieValid: true, avatarUrl: face('#1f2f3b', '#f2c94c', '#14b8a6') },
    { label: tx('Storage', 'Kho đồ'), username: 'khodo_vng', realm: 'vng', group: '', hasPassword: false, cookieValid: null, avatarUrl: null },
    { label: 'Farm 2', username: 'pixelfarmer02', realm: '', group: 'Farm', hasPassword: true, cookieValid: true, avatarUrl: face('#3a2a1f', '#e8b48a', '#f97316') },
    { label: 'Main', username: 'sunny_builds', realm: '', group: '', hasPassword: true, cookieValid: true, avatarUrl: face('#2b3a55', '#f2c94c', '#3b82f6') },
    { label: 'Event runner', username: 'event_runner_2026', realm: '', group: 'Event', hasPassword: false, cookieValid: false, avatarUrl: face('#2a1f3a', '#d6d9e0', '#a855f7') },
  ];
  const IV_PRESETS = [{ name: 'Farm Blox Fruits', users: ['pixelfarmer02', 'pixelfarmer03'], placeInput: '2753915549' }, { name: 'Event night', users: ['event_runner_2026'], placeInput: '' }];
  let ivWrong = 0, ivWaitUntil = 0, ivToken = null;
  const ivHere = u => accounts.find(x => x.username.toLowerCase() === u.toLowerCase());
  let rbxClosedHidden = false;
  let robloxUpdating = false; // Settings › Roblox version › Update now pressed
  let multiAtStart = false; // first start › Just use Multi-Roblox
  const rw = ['changed', 'renamed', 'gone', 'worked', 'none'].includes(params.get('rw')) ? params.get('rw') : 'same';
  // Statistics › Activity: made-up events, the same on every call (minutes back from now). ?noactivity=1: none yet;
  // ?activityfail=1: the list can't be loaded; ?manyactivity=1: more than a page. Like the app, each line comes in the
  // page's language.
  const ACTIVITY = params.has('noactivity') ? [] : [
    [4, 'joined', 1, 'ok', 'Joined Blox Fruits.', 'Đã vào Blox Fruits.'],
    [5, 'opened', 1, 'ok', 'Opened Roblox to play Blox Fruits.', 'Đã mở Roblox để chơi Blox Fruits.'],
    [11, 'joined', 2, 'ok', 'Joined Blox Fruits.', 'Đã vào Blox Fruits.'],
    [12, 'rejoined', 2, 'ok', 'Auto-rejoin opened Roblox again to play Blox Fruits.', 'Tự vào lại: đã mở lại Roblox để chơi Blox Fruits.'],
    [12.5, 'closed', 2, 'info', 'Roblox window closed.', 'Đã đóng cửa sổ Roblox.'],
    [13, 'disconnected', 2, 'warn', 'Disconnected from Blox Fruits: kicked for being idle.', 'Bị văng khỏi Blox Fruits: bị kick vì AFK.'],
    [75, 'schedule', null, 'ok', 'Scheduled preset Farm Blox Fruits started: 3 accounts opened.', 'Bộ khởi chạy hẹn giờ Farm Blox Fruits đã chạy: mở 3 tài khoản.'],
    [140, 'left', 4, 'info', 'Left [🎃] Adopt Me!.', 'Đã rời [🎃] Adopt Me!.'],
    [60 * 24 + 30, 'signin', null, 'warn', '1 account needs signing in again: Farm 2', '1 tài khoản cần đăng nhập lại: Farm 2'],
    [60 * 24 + 95, 'closedByRoblox', 6, 'warn', 'Roblox closed this window when another one opened.', 'Roblox đã đóng cửa sổ này khi một cửa sổ khác mở ra.'],
    [60 * 24 + 180, 'robloxUpdated', null, 'info', 'Roblox updated to 0.742.0.7421053.', 'Roblox đã cập nhật lên 0.742.0.7421053.'],
    [60 * 24 * 3, 'scheduleStop', null, 'info', 'Scheduled preset Farm Blox Fruits finished: closing 3 accounts.', 'Bộ khởi chạy hẹn giờ Farm Blox Fruits đã xong: đang đóng 3 tài khoản.'],
    [60 * 24 * 3 + 10, 'disconnected', 4, 'warn', 'Disconnected from Brookhaven 🏡RP: signed in on another device.', 'Bị văng khỏi Brookhaven 🏡RP: đăng nhập ở thiết bị khác.'],
  ];
  // Like the app's ActivityGroup: the kinds the page filters by.
  const activityGroup = k => ['opened', 'rejoined', 'joined', 'left', 'closed'].includes(k) ? 'play' : k === 'signin' ? 'signin'
    : ['robloxUpdated', 'updateReady'].includes(k) ? 'updates' : k.startsWith('schedule') ? 'schedule' : 'drops';
  let emit = () => {};
  const state = () => ({
    multiAtStart, language, theme, themeApplied: themeApplied(), customTheme, savedThemes, closeToTray, hideNames, notifications, notifyOff, dailySignInCheck, lastSignInCheck, skipConfirm, miniOpen, backup, boot: boot(), vaultExists, vaults: vaultsState(), locked, screenLock,
    whatsNew: params.has('whatsnew') ? { from: '1.0.0', releases: [{ version: '1.0.1', date: '2026-10-19', en: ['Faster sign-in check: six accounts at a time.', 'Shows the version on the update button.'], vi: ['Kiểm tra đăng nhập nhanh hơn: 6 tài khoản một lúc.', 'Nút cập nhật ghi rõ phiên bản.'] }] } : null,
    // ?noroblox=1: no Roblox installed, so no version of it (the app's RobloxVersionDto with no client).
    robloxVersion: { installed: NO_ROBLOX ? null : '0.742.0.7421053', latest: params.has('rbxold') ? '0.743.0.7430112' : '0.742.0.7421053', upToDate: NO_ROBLOX ? null : !params.has('rbxold'), direct: false, store: settings.launchVia === 'store',
      updater: settings.launchVia === 'store' || NO_ROBLOX ? null : 'Bloxstrap', updating: robloxUpdating, oldWindows: params.has('oldwin') ? 1 : 0, reopens: params.has('oldwin') ? 1 : 0 },
    // Settings › Roblox updates, newest first: a new build under the same version, an update, the first build seen.
    // ?rbxclosed=1: Roblox closed windows when another one opened (the warning there and on the Multi-Roblox card).
    // ?rw=changed|renamed|gone: what the Roblox watch found in the installed build (default: no change, and the next
    // build checked ahead); ?rw=worked: it changed, but two windows played together on it here since; ?rw=none: not read.
    robloxUpdates: {
      installed: NO_ROBLOX ? null : { version: '0.742.0.7421053', build: 'version-cec3ad5889b447cf', source: 'Bloxstrap' },
      history: [
        { at: minsAgo(60 * 20), version: '0.742.0.7421053', build: 'version-cec3ad5889b447cf', source: 'Bloxstrap', first: false,
          check: rw === 'worked' ? 'changed' : rw === 'none' ? null : rw, worked: rw === 'worked' },
        { at: minsAgo(60 * 50), version: '0.742.0.7421053', build: 'version-5b1e07d2a6c84f13', source: 'Bloxstrap', first: false, check: rw === 'none' ? null : 'same', worked: true },
        { at: minsAgo(60 * 24 * 9), version: '0.741.0.7410829', build: 'version-91f0c3ab72d14e6e', source: 'Bloxstrap', first: true, check: null, worked: false },
      ],
      watch: rw === 'none' ? { readAt: null, version: '0.742.0.7421053', updatesFirst: false, check: null, worked: false, next: null, held: [] } : {
        readAt: minsAgo(4), version: '0.742.0.7421053', updatesFirst: false, check: rw === 'worked' ? 'changed' : rw, worked: rw === 'worked',
        next: rw === 'same' ? { version: '0.743.0.7431134', check: 'same' } : null, held: rw === 'renamed' ? ['ROBLOX_singletonEvent2'] : [],
      },
      closed: params.has('rbxclosed') && !rbxClosedHidden ? { at: minsAgo(12), count: 3, updatedAt: minsAgo(60 * 20) } : null,
    },
    // ?update=quiet: a quiet fix downloaded (WhatsNew.IsQuietFix), installed by itself without the Restart to update button.
    // ?lockupdate=40 (with ?locked=1): an update found before the unlock, 40 % downloaded (ready: downloaded); the lock
    // screen gives it its place (AppController.UpdateOffersAtLock).
    update: lockUpdate !== null
      ? { supported: true, current: '1.0.0', status: lockUpdate === 'ready' ? 'ready' : 'downloading', version: '1.0.1', quiet: false, progress: +lockUpdate || 0, error: null, auto: true, beta: betaUpdates, checkedAt: minsAgo(0) }
      : { supported: true, current: '1.0.0', status: params.has('update') ? 'ready' : 'upToDate', version: params.has('update') ? (params.get('update') === 'quiet' ? '1.0.1-alpha.1' : '1.0.1') : null,
        quiet: params.get('update') === 'quiet', progress: 0, error: null, auto: true, beta: betaUpdates, checkedAt: minsAgo(23) },
    lockUpdate: locked && !screenLock && lockUpdate !== null ? { go: lockUpdateGo } : null,
    countOnline,
    remote,
    installs: NO_ROBLOX ? { handler: null, bloxstrap: false, fishstrap: false, others: [], roblox: false, direct: false, store: false, vng: params.has('vng') }
      : { handler: 'Bloxstrap', bloxstrap: true, fishstrap: false, others: params.has('others') ? ['Voidstrap'] : [], roblox: false, direct: true, store: params.has('store'), vng: params.has('vng') },
    monitors: [{ index: 0, primary: true, width: 1920, height: 1080 }, { index: 1, primary: false, width: 2560, height: 1440 }], dataPath: LONG ? 'C:\\Users\\Administrator\\AppData\\Local\\Packages\\Some.Very.Long.Package.Name_8wekyb3d8bbwe\\LocalCache\\Local\\Bloxkeep' : 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep', paths: { vault: locked ? null : 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep\\vault.rmv', log: 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep\\error.log', app: 'C:\\Users\\Admin\\AppData\\Local\\BloxkeepApp\\current' },
    hotkeys: { on: !params.has('nohotkeys'), taken: params.has('keystaken') ? ['Ctrl+Alt+H'] : [] },
    robloxInstalled: !NO_ROBLOX,
    macroRuns: locked ? [] : macroRuns, macroRecording: locked ? null : macroRecording, macroVars, macroSweep, panelsShown,
    // Like AppController's MacroCanRunDto: while an account plays, the macros on the selected accounts that have a game set.
    macroCanRun: !locked && accounts.some(a => a.state === 'running') ? macros.filter(m => m.targets === 'selected' && m.panel?.placeId).map(m => m.id) : [],
    launching: false, checking: false, multi, settings: locked ? null : settings, accounts: locked ? [] : accounts, discord: locked ? null : discord,
  });
  // Roblox windows of accounts deleted while playing: like the app, deleting doesn't close them, and they still count.
  let orphans = 0;
  const metrics = () => ({
    accounts: accounts.filter(a => a.state === 'running').map((a, i) => ({ id: a.id, ramMb: 780 + i * 140 + Math.round(Math.random() * 30) })),
    robloxCount: accounts.filter(a => a.state === 'running').length + orphans,
    monitorCount: accounts.filter(a => a.state === 'running').length + orphans,
    totalMb: 1750 + Math.round(Math.random() * 40), systemMb: 32 * 1024, usedPct: 63, availableMb: 12 * 1024, cpuPct: 12, gpuPct: 34,
  });
  // The panel's numbers go with every change to the cards, not only on the next 1.5 s tick: a window opened or closed
  // is then counted in the sidebar as soon as its card says so, and Tests/ui doesn't write down one or the other by
  // chance (the app's own check sends both within the same 1.5 s).
  const push = () => { emit('state', state()); if (!locked) emit('metrics', metrics()); };
  let loginUser = '';
  const srvAsked = []; // when each server list was given, for Roblox's few-a-minute limit
  let serverTries = 0; // ?srvfail=1: the failed loads so far
  // ?nodelay=1: answers at once, like the app, for timing the page (Tests/perf); otherwise a little later, as over a bridge.
  const wait = ms => new Promise(r => setTimeout(r, params.has('nodelay') ? 0 : ms));

  window.__mockBridge = {
    attach(fn) {
      emit = fn;
      // The first right away rather than 1.5 s in, so the panel's numbers are there when the page settles (Tests/ui).
      setTimeout(() => !locked && emit('metrics', metrics()), 0);
      // ?settled=1: the app says it has settled (it does a moment after the loading screen), so the tabs not opened yet
      // get ready (Tests/ui settled-ready). Not otherwise: the commands it brings would land in every scenario.
      if (params.has('settled')) setTimeout(() => !locked && emit('settled', null), 300);
      setInterval(() => !locked && emit('metrics', metrics()), 1500);
    },
    async call(cmd, a) {
      await wait(120);
      switch (cmd) {
        case 'getState': return { resume: false, view: null, state: state() };
        case 'debugToast': emit('toast', a); return null; // for the dev tests: __mockBridge.call('debugToast', { text, kind })
        // For the demos (ui/dev/demos): an account's card moved on, e.g. kicked, then rejoining. { n: 2, set: { session, rejoin } }
        case 'debugAccount': { const acc = accounts.find(x => x.id === id(a.n)); if (acc) { Object.assign(acc, a.set); push(); } return null; }
        // A game's page: made-up figures, the accounts that "played" it, the hours (?gpnew=1: updated an hour ago).
        case 'gameDetails': {
          const now = Date.now(), h = n => new Date(now - n * 3600e3).toISOString();
          return {
            placeId: a.placeId, name: 'Blox Fruits', creator: 'Gamer Robot Inc', genre: params.get('lang') === 'vi' ? 'Phiêu lưu' : 'Adventure',
            iconUrl: null, playing: 412873, visits: 48216000000, favorites: 8700000, likes: 92,
            created: '2019-01-16T03:29:19Z', updated: params.has('gpnew') ? h(1) : h(30),
            description: 'Become a master swordsman or a powerful blox fruit user as you train to become the strongest player to ever live.',
            accounts: [
              { id: 'a1', name: 'Farm 1', minutes: 1860, kicks: 2, kickWhy: 'AFK', drops: 1, now: 'ingame' },
              { id: 'a2', name: 'Farm 2', minutes: 1740, kicks: 0, drops: 3, now: 'ingame' },
              { id: 'a3', name: 'Night Owl', minutes: 720, kicks: 1, drops: 0, now: 'banned' },
              { id: 'a4', name: 'Main', minutes: 240, kicks: 0, drops: 0, now: '', lastPlayed: h(2) },
            ],
            hours: params.has('gpempty') ? null : [30, 22, 15, 10, 8, 9, 12, 20, 32, 40, 48, 52, 58, 60, 62, 66, 72, 85, 92, 100, 98, 90, 70, 45].map(v => v * 4200),
          };
        }
        // A custom theme as given (a video's address, say): __mockBridge.call('debugCustomTheme', { picture, video: true })
        case 'debugCustomTheme': customTheme = { ...customTheme, ...a }; theme = 'custom'; push(); return null;
        case 'keepView': case 'pageBusy': return null;
        // Like Roblox: three lists a minute, then "wait" with the seconds left (?srvbusy=1: waiting from the start).
        // ?srvfail=1: the server list fails to load (Roblox down) until Try again is pressed twice.
        case 'servers': {
          if (params.has('srvfail') && serverTries++ < 2) {
            await new Promise(r => setTimeout(r, 300));
            throw new Error(tx("Couldn't reach Roblox.", 'Không kết nối được Roblox.'));
          }
          await wait(400);
          const now = Date.now();
          while (srvAsked.length && now - srvAsked[0] >= 60000) srvAsked.shift();
          if (params.has('srvbusy') || srvAsked.length >= 3)
            return { servers: [], next: null, busy: true, retryIn: params.has('srvbusy') ? 23 : Math.ceil((60000 - (now - srvAsked[0])) / 1000), left: 0 };
          srvAsked.push(now);
          return { servers: Array.from({ length: 12 }, (_, i) => ({ jobId: `00000000-0000-0000-0000-${String(i).padStart(12, '0')}`, playing: 15 - i, maxPlayers: 15, ping: 140 - i * 9, fps: 50 + (i * 7) % 11 })), next: a?.cursor ? null : 'more', busy: false, left: 3 - srvAsked.length };
        }
        case 'seenWhatsNew': return null;
        case 'updateAtLock': lockUpdateGo = !!a.go; push(); return null;
        // The real notes, newest first: the newest three, as after an update over two versions (?wn=1: only the newest).
        case 'whatsNew': return (await fetch('/RobloxManager/whatsnew.json').then(r => r.json())).slice(0, +(params.get('wn') || 3));
        case 'unlock': case 'createVault':
          await wait(500);
          // "sai" or anything starting "wrong" is the wrong master password (the app's own words for it).
          if (a.password === 'sai' || a.password.startsWith('wrong')) throw new Error(language === 'vi' ? 'Sai mật khẩu chủ.' : 'Wrong master password.');
          if (cmd === 'createVault') {
            // The vault set up joins the list under the name typed (the first one: Vault 1).
            const made = vaultAdding || { id: 'a1b2c3d4', name: 'Vault 1', picture: null };
            vaultList = [...vaultList.filter(v => v.id !== made.id), { ...made, name: (a.name || '').trim() || made.name }];
            vaultCurrent = made.id; vaultAdding = null; vaultExists = true;
          }
          locked = false;
          screenLock = null;
          unlockedAt = Date.now();
          setTimeout(push, 1450);
          setTimeout(push, 2250);
          return state();
        case 'lock': locked = true; screenLock = null; push(); return null;
        case 'setLanguage': language = a.language; push(); return null;
        case 'setTheme': theme = a.theme; push(); return null;
        case 'setCustomTheme':
          customTheme = { ...customTheme, background: a.background || customTheme.background, accent: a.accent || customTheme.accent, fade: a.fade ?? customTheme.fade, blur: a.blur ?? customTheme.blur };
          theme = 'custom'; push(); return null;
        case 'pickThemePicture': customTheme = { ...customTheme, picture: 'scene-halong.svg' }; theme = 'custom'; push(); return null;
        case 'clearThemePicture': customTheme = { ...customTheme, picture: null, video: false }; theme = 'custom'; push(); return null;
        case 'saveTheme': {
          const same = savedThemes.find(s => s.name.toLowerCase() === a.name.toLowerCase());
          savedThemes = same ? savedThemes.map(s => s === same ? { ...s, look: { ...customTheme } } : s)
            : [{ id: 't' + Date.now(), name: a.name, look: { ...customTheme } }, ...savedThemes];
          push(); return null;
        }
        case 'useSavedTheme': customTheme = { ...savedThemes.find(s => s.id === a.id).look }; theme = 'custom'; push(); return null;
        case 'deleteSavedTheme': savedThemes = savedThemes.filter(s => s.id !== a.id); push(); return null;
        case 'exportTheme': return { saved: savedThemes.find(s => s.id === a.id).name + '.bloxkeeptheme' };
        case 'importTheme': return null;
        case 'setCloseToTray': closeToTray = a.enabled; push(); return null;
        case 'setHideNames': hideNames = a.enabled; push(); return null;
        // Authenticator codes: account 1 has a key; a typed key is checked like the app does (base32, long enough).
        case 'totpCode': if (!accounts.find(x => x.id === a.id)?.hasAuthenticator) throw new Error('This account has no authenticator key.'); return fakeCode();
        case 'totpCheck': {
          const k = String(a.key || '').replace(/[\s-]/g, '').replace(/=+$/, '').toUpperCase();
          const bad = [...k].find(c => !/[A-Z2-7]/.test(c));
          if (bad) return { ok: false, error: `The key can't contain "${bad}": it only has the letters A–Z and the digits 2–7. Check it for a typo.` };
          if (k.length < 16) return { ok: false, error: 'The key is too short. Check that you copied all of it.' };
          return fakeCode();
        }
        case 'loginTwoStepCode': return fakeCode();
        case 'setNotifications':
          if (a.kind) notifyOff = a.enabled ? notifyOff.filter(k => k !== a.kind) : [...new Set([...notifyOff, a.kind])];
          else notifications = a.enabled;
          push();
          return null;
        case 'setDailySignInCheck': dailySignInCheck = a.enabled; push(); return null;
        case 'setBetaUpdates': betaUpdates = a.enabled; push(); return null;
        case 'setCountOnline': countOnline = a.enabled; push(); return null;
        case 'setRemote': remote = a.enabled ? remoteCode() : { on: false, connected: false, linked: null, pc: 'GAMING-PC' }; push(); return null;
        case 'remoteCode': remote = { ...remoteCode(), code: 'Q2W8RN' }; push(); return null;
        case 'openDiscordApp': return null;
        case 'remoteAllow': remote = { ...remote, ask: null, user: a.allow ? remote.ask?.user : remote.user }; push(); return null;
        case 'skipConfirm': if (!skipConfirm.includes(a.key)) skipConfirm.push(a.key); push(); return null;
        case 'resetSkipConfirm': { const n = skipConfirm.length; skipConfirm.length = 0; push(); return n; }
        case 'toggleMini': miniOpen = !miniOpen; push(); return null;
        case 'window': console.log('window', a.action); return null;
        case 'setBackup':
          if ('enabled' in a) backup = { ...backup, enabled: a.enabled };
          if (a.folder) backup = { ...backup, folder: a.folder, cloud: true, lastBackup: new Date().toISOString(), error: null, suggested: backup.suggested.filter(s => s.path !== a.folder) };
          push();
          return null;
        case 'pickBackupFolder': backup = { ...backup, folder: 'D:\\Backups\\Bloxkeep', cloud: false, lastBackup: new Date().toISOString(), error: null }; push(); return null;
        case 'backupNow': await wait(300); if (params.has('bkerr')) throw new Error(tx("Couldn't back up: Can't write to this folder.", 'Không sao lưu được: Không ghi được vào thư mục này.')); backup = { ...backup, lastBackup: new Date().toISOString() }; push(); return null;
        case 'openBackupFolder': return null;
        case 'openReleaseNotes': return null; // the app opens Roblox's page in the default browser
        case 'openRobloxWatch': return null; // likewise, docs/roblox-watch.md on GitHub
        // Send feedback; ?fbfail=1: the service can't be reached.
        case 'feedbackPreview': return { preview: [language === 'vi' ? (a.kind === 'bug' ? 'Báo lỗi' : 'Góp ý') : (a.kind === 'bug' ? 'Bug report' : 'Idea'),
          `Bloxkeep 1.0.11 · Windows 11 (10.0.26300) · ${language === 'vi' ? 'Tiếng Việt' : 'English'}`, ...(a.contact.trim() ? [`${language === 'vi' ? 'Liên hệ' : 'Contact'}: ${a.contact.trim()}`] : []),
          '', a.text.trim() || '…', '', a.log ? '── error-log.txt ──\n[2026-10-16 02:10:44] System.Net.Http.HttpRequestException: No such host is known. (presence.roblox.com:443)\n   at RobloxManager.RobloxApi.GetPresenceAsync([account id]) in C:\\Users\\[user]\\Bloxkeep\\RobloxApi.cs:line 512'
            : (language === 'vi' ? '(không kèm nhật ký lỗi)' : '(no error log)')].join('\n') };
        case 'sendFeedback': await wait(700); if (params.has('fbfail')) throw new Error(language === 'vi' ? 'Không kết nối được tới dịch vụ nhận góp ý. Kiểm tra mạng rồi thử lại.' : 'Couldn’t reach the feedback service. Check the internet connection and try again.'); return { sent: true };
        case 'openWebsite': console.log('openWebsite', a.id); return null; // the app opens its own roblox.com window for the account
        case 'openWebsiteBlank': console.log('openWebsiteBlank'); return null; // likewise, blank, to sign in on the site
        case 'hideRobloxClosed': rbxClosedHidden = true; push(); return null;
        case 'updateRoblox': robloxUpdating = true; push(); return null;
        case 'updateAndReopen': robloxUpdating = true; push(); return null;
        case 'multiOnly': multiAtStart = !!a.on; push(); return null;
        // Like the app: only the fields sent change; a ping is { kind, value }, an alert switch { kind, enabled }.
        case 'setDiscordPresence': discord = { ...discord, presence: !!a.enabled }; push(); return null;
        case 'setDiscord': {
          if (a.enabled && !discord.hasWebhook) throw new Error('Paste a webhook link first.');
          if (a.ping && !/^(none|here|everyone|(user|role):\d{17,20})$/.test(a.ping.value)) throw new Error('A user ID is 17 to 20 digits.');
          if (a.summaryParts && !a.summaryParts.length) throw new Error('Pick at least one thing for the summary.');
          // The app's own rules for the sender name and picture.
          if (typeof a.username === 'string' && (a.username.length > 80 || /discord|clyde/i.test(a.username))) throw new Error("Discord doesn't allow that name.");
          if (a.avatarUrl && !/^https:[/][/][^ ]+$/i.test(a.avatarUrl)) throw new Error('The picture link must start with https://.');
          const s = { ...discord.summary };
          for (const [k, f] of [['summaryDaily', 'daily'], ['summaryDailyTime', 'dailyTime'], ['summaryWeekly', 'weekly'], ['summaryWeekday', 'weekday'], ['summaryWeeklyTime', 'weeklyTime'],
            ['summaryAfterSchedule', 'afterSchedule'], ['summaryDays', 'days'], ['summaryParts', 'parts'], ['summaryAccounts', 'accounts']]) if (k in a) s[f] = a[k];
          discord = { ...discord, summary: s };
          for (const k of ['enabled', 'username', 'avatarUrl', 'language', 'style', 'hideNames']) if (k in a) discord[k] = k === 'username' ? a[k] || 'Bloxkeep' : a[k];
          if (a.enabled) discord.stopped = null;
          if ('board' in a) discord.board = { ...discord.board, enabled: a.board };
          if ('boardMinutes' in a) discord.board = { ...discord.board, minutes: a.boardMinutes };
          if (a.event) discord.events = a.event.enabled ? [...new Set([...discord.events, a.event.kind])] : discord.events.filter(k => k !== a.event.kind);
          if (a.ping) discord.pings = { ...discord.pings, [a.ping.kind]: a.ping.value };
          if ('quiet' in a) discord.quiet = { ...discord.quiet, enabled: a.quiet };
          if (a.quietFrom) discord.quiet = { ...discord.quiet, from: a.quietFrom };
          if (a.quietTo) discord.quiet = { ...discord.quiet, to: a.quietTo };
          push();
          return null;
        }
        case 'setDiscordWebhook':
          if (a.url && !/^https:\/\/(?:(?:canary|ptb)\.)?discord(?:app)?\.com\/api(?:\/v\d{1,2})?\/webhooks\/\d{17,20}\/[\w-]{50,100}\/?$/.test(a.url)) throw new Error("That isn't a Discord webhook link.");
          discord = { ...discord, hasWebhook: !!a.url, enabled: a.url ? discord.enabled : false, stopped: null, lastSent: null, error: null, webhookHint: a.url ? `discord.com/api/webhooks/…${a.url.match(/webhooks\/(\d+)/)[1].slice(-4)}/••••••` : '' };
          push();
          return null;
        case 'revealDiscordWebhook': return `https://discord.com/api/webhooks/${DC_ID}/${'demo-token-'.repeat(6)}demo`;
        case 'discordTest': case 'discordSummaryNow':
          await wait(500);
          if (params.has('dcfail')) throw new Error(tx("Couldn't reach Discord: ", 'Không kết nối được với Discord: ') + 'No such host is known. (discord.com:443)');
          discord = { ...discord, lastSent: new Date().toISOString(), error: null };
          push();
          return null;
        // Like the app: the picker and a new vault's form go over the open vault, which locks only for another one.
        case 'pickVault': vaultAdding = null; vaultPick = true; push(); return null;
        case 'closeVaultPicker': vaultPick = false; push(); return null;
        case 'selectVault':
          if (locked || a.id !== vaultCurrent) { locked = true; screenLock = null; }
          vaultCurrent = a.id; vaultAdding = null; vaultExists = true; vaultPick = false; push(); return null;
        case 'addVault':
          vaultAddFromPick = vaultPick; vaultPick = false;
          vaultAdding = { id: 'd4e5f6a7', name: `Vault ${vaultList.length + 1}`, picture: null };
          push(); return null;
        case 'cancelAddVault': vaultAdding = null; vaultPick = vaultAddFromPick && vaultList.length > 0; push(); return null;
        case 'removeVault':
          vaultList = vaultList.filter(v => v.id !== a.id);
          if (vaultCurrent === a.id) vaultCurrent = vaultList[0]?.id || null;
          vaultPick = vaultList.length > 1; vaultExists = vaultList.length > 0; push(); return null;
        case 'renameVault': vaultList = vaultList.map(v => v.id === a.id ? { ...v, name: a.name.trim() || v.name } : v); push(); return null;
        case 'pickVaultPicture': vaultList = vaultList.map(v => v.id === a.id ? { ...v, picture: 'scene-aurora.svg' } : v); push(); return null;
        case 'clearVaultPicture': vaultList = vaultList.map(v => v.id === a.id ? { ...v, picture: null } : v); push(); return null;
        case 'setAskVault': askVault = !!a.enabled; push(); return null;
        case 'restoreBackup': await wait(500); vaultExists = true; locked = true; push(); emit('toast', { text: tx('Backup restored. Unlock it with the master password it was saved with.', 'Đã khôi phục bản sao lưu. Mở khóa bằng mật khẩu chủ đã dùng khi sao lưu.'), kind: 'ok' }); return null;
        // Like the app (GameTarget.TryParse): a share link is read when launching; a link with no number is refused.
        case 'resolveGame': {
          if (/share\?code=/i.test(a.input || '')) return { ok: true, kind: 'share' };
          const m = /(\d{5,})/.exec(a.input);
          if (!m) return { ok: false, error: tx('This link has no Place ID.', 'Link không có Place ID.') };
          const info = games[m[1]];
          return { ok: true, kind: /privateServerLinkCode|accessCode/.test(a.input) ? 'private' : 'game', placeId: +m[1], ...(info || {}) };
        }
        case 'setSettings':
          if (a.multiTile) { multi.tile = true; settings.maximizeOnOpen = false; } else if (a.multiTile === false) multi.tile = false;
          if (a.maximizeOnOpen) { settings.maximizeOnOpen = true; multi.tile = false; } else if (a.maximizeOnOpen === false) settings.maximizeOnOpen = false;
          if ('multiInstance' in a) multi = { ...multi, enabled: a.multiInstance, armed: a.multiInstance && !params.has('notready') };
          // The Multi-Roblox dialog's two switches (kept, so reopening it shows them as left).
          if ('multiWindowed' in a) multi = { ...multi, windowed: !!a.multiWindowed };
          if ('multiTile' in a) multi = { ...multi, tile: !!a.multiTile };
          // Like the app: only the fields sent change (robloxConfig included).
          settings = { ...settings, ...a, robloxConfig: { ...settings.robloxConfig, ...(a.robloxConfig || {}) } };
          push();
          return null;
        case 'launch': {
          // Like the app: a banned account isn't opened (said with the result; alone, said at once).
          const shown = acc => (hideNames ? `Account ${accounts.indexOf(acc) + 1}` : acc.label);
          const banned = accounts.filter(y => a.ids.includes(y.id) && y.banned).map(y => `${shown(y)} is banned on Roblox, so it wasn't opened.`);
          const ids = a.ids.filter(x => !accounts.find(y => y.id === x)?.banned);
          if (!ids.length && banned.length) throw new Error(banned.join('\n'));
          { const ask = launchAsks(ids.length, a.force); if (ask) return ask; }
          for (const x of ids) {
            const acc = accounts.find(y => y.id === x);
            if (acc) acc.state = 'launching';
            emit('progress', { text: language === 'vi' ? `Đang mở ${acc ? shown(acc) : ''}…` : `Opening ${acc ? shown(acc) : ''}…` });
            push();
            await wait(900);
          }
          emit('progress', { text: null });
          setTimeout(() => {
            accounts.forEach(x => { if (x.state === 'launching') { x.state = 'running'; x.session = { pid: 9, since: new Date().toISOString(), game: g(2753915549) }; } });
            push();
          }, 1500);
          return { launched: ids.length, errors: banned, cancelled: false };
        }
        // Like the app: name and accounts required, a Job ID for "job", no server choice without a game.
        case 'savePreset': {
          if (!a.name) throw new Error('Give the preset a name.');
          const ids = a.ids.filter(x => accounts.some(y => y.id === x));
          if (!ids.length) throw new Error('Select at least one account.');
          // Like the app: following a player needs no game (it's the one they're in).
          const server = a.server === 'follow' ? 'follow' : a.placeInput ? a.server : 'any';
          if (server === 'job' && !a.jobId) throw new Error("Enter the server's Job ID.");
          const follow = server === 'follow' ? String(a.followPlayer || '').trim().replace(/^@/, '') : '';
          if (server === 'follow' && !follow) throw new Error("Type the player's Roblox username or user ID.");
          const m = /(\d{5,})/.exec(a.placeInput);
          const sc = a.schedule || null;
          if (sc?.enabled) {
            if (!/^\d\d:\d\d$/.test(sc.time)) throw new Error('Enter the start time as HH:mm.');
            if (!sc.date && !sc.days.length) throw new Error('Pick at least one weekday, or a date.');
            if (sc.date && !nextRun(sc)) throw new Error('That date and time have already passed.');
            if (sc.stop === 'after' && !(sc.stopHours > 0)) throw new Error('Enter how many hours to play (up to 168).');
            if (sc.stop === 'at' && !/^\d\d:\d\d$/.test(sc.stopTime)) throw new Error('Enter the stop time as HH:mm.');
          }
          const preset = { id: a.id || `p${Date.now()}`, name: a.name.slice(0, 60), accountIds: ids, placeInput: a.placeInput,
            gameName: m && games[m[1]] ? games[m[1]].name : '', server, jobId: server === 'job' ? a.jobId : '', followPlayer: follow, delaySeconds: a.delaySeconds,
            schedule: sc, nextRun: nextRun(sc), iconUrl: m && games[m[1]] ? games[m[1]].iconUrl : null, lastRun: null,
            // Like the app: no restart without a game (it would have nothing to open again).
            antiAfk: a.antiAfk ?? null, autoRejoin: a.autoRejoin ?? null, profile: a.profile === 'afk' ? 'afk' : '',
            restartHours: (a.placeInput || server === 'follow') && [2, 4, 6, 12].includes(a.restartHours) ? a.restartHours : 0, arrange: a.arrange ?? null, running: 0,
            discord: a.discordWebhook === '' ? null : a.discordWebhook ? `discord.com/api/webhooks/…${(a.discordWebhook.match(/webhooks\/(\d+)/) || [, '0000'])[1].slice(-4)}/••••••`
              : settings.presets.find(x => x.id === a.id)?.discord ?? null };
          if (a.discordWebhook && !/^https:\/\/(?:(?:canary|ptb)\.)?discord(?:app)?\.com\/api(?:\/v\d{1,2})?\/webhooks\/\d{17,20}\/[\w-]{50,100}\/?$/.test(a.discordWebhook)) throw new Error("That isn't a Discord webhook link.");
          const list = settings.presets;
          const i = list.findIndex(p => p.id === preset.id);
          if (i < 0 && list.length >= MAX_PRESETS) throw new Error(`You can save up to ${MAX_PRESETS} presets.`);
          if (i >= 0) list[i] = { ...preset, lastRun: list[i].lastRun }; else list.push(preset);
          push();
          return preset.id;
        }
        case 'deletePreset': settings.presets = settings.presets.filter(p => p.id !== a.id); push(); return null;
        // Like the app: a link must be a private server's; without one a code and a game are needed; the same link updates.
        case 'saveServer': {
          if (a.link && !/privateServerLinkCode=|share\?code=|type=Server/i.test(a.link))
            throw new Error(language === 'vi' ? 'Link này không phải link server riêng. Hãy dán link server riêng, hoặc nhập code mà game yêu cầu.' : 'That link isn’t a private server’s. Paste the private server’s link, or enter the code the game asks for.');
          if (!a.link && !a.code) throw new Error(language === 'vi' ? 'Hãy nhập code mà game yêu cầu, hoặc dán link server riêng vào ô game trước.' : 'Enter the code the game asks for, or put a private server’s link in the game box first.');
          let s = a.link ? settings.savedServers.find(x => x.link === a.link) : null;
          if (!s) { s = { id: `s${Date.now()}` }; settings.savedServers.unshift(s); }
          Object.assign(s, { name: a.name || a.gameName || (language === 'vi' ? 'Server riêng' : 'Private server'), placeId: a.placeId, universeId: a.universeId, gameName: a.gameName, link: a.link, code: a.code, iconUrl: games[a.placeId]?.iconUrl });
          push();
          return s;
        }
        case 'deleteServer': settings.savedServers = settings.savedServers.filter(s => s.id !== a.id); push(); return null;
        case 'copyServerCode': return { code: settings.savedServers.find(s => s.id === a.id)?.code };
        // Like the app: the copy goes right after the original, named "… (copy)" (numbered when taken), its schedule off.
        case 'duplicatePreset': {
          const list = settings.presets, i = list.findIndex(p => p.id === a.id);
          if (i < 0) throw new Error('Preset not found.');
          if (list.length >= MAX_PRESETS) throw new Error(language === 'vi' ? `Chỉ lưu được tối đa ${MAX_PRESETS} bộ khởi chạy.` : `You can save up to ${MAX_PRESETS} presets.`);
          const src = list[i], word = language === 'vi' ? 'bản sao' : 'copy';
          let name = '';
          for (let n = 1; n <= MAX_PRESETS + 1; n++) {
            const suffix = n === 1 ? ` (${word})` : ` (${word} ${n})`;
            name = [...src.name].slice(0, 60 - suffix.length).join('').trimEnd() + suffix;
            if (!list.some(p => p.name.toLowerCase() === name.toLowerCase())) break;
          }
          const schedule = src.schedule ? { ...src.schedule, days: [...src.schedule.days], enabled: false } : null;
          const copy = { ...src, id: `p${Date.now()}`, name, accountIds: [...src.accountIds], schedule, nextRun: null, lastRun: null };
          let at = i + 1; // after the copies already made of it
          while (at < list.length && list[at].name.startsWith(`${[...src.name].slice(0, 40).join('')}`) && list[at].name.includes(`(${word}`)) at++;
          list.splice(at, 0, copy);
          push();
          return copy.id;
        }
        case 'setPresetOrder': {
          const byId = new Map(settings.presets.map(p => [p.id, p]));
          const order = a.ids.filter(x => byId.has(x)).map(x => byId.get(x));
          settings.presets = [...order, ...settings.presets.filter(p => !order.includes(p))];
          push();
          return null;
        }
        case 'setPresetSchedule': {
          const p = settings.presets.find(x => x.id === a.id);
          if (!p) throw new Error('Preset not found.');
          if (!p.schedule) throw new Error('This preset has no schedule. Edit it to add one.');
          const sc = { ...p.schedule, enabled: a.enabled };
          if (sc.enabled && sc.date && !nextRun(sc)) throw new Error(language === 'vi' ? 'Ngày giờ đó đã qua rồi.' : 'That date and time have already passed.');
          p.schedule = sc;
          p.nextRun = nextRun(sc);
          push();
          return null;
        }
        case 'presetIcons': await wait(300); settings.presets.forEach(p => { const m = /(\d{5,})/.exec(p.placeInput); if (m && games[m[1]]) p.iconUrl = games[m[1]].iconUrl; }); push(); return null;
        case 'setLaunchOrder':
          settings.launchOrder = [...a.ids.filter(x => accounts.some(y => y.id === x)), ...settings.launchOrder.filter(x => !a.ids.includes(x))];
          push();
          return null;
        // ?noroom=1: no server has room, so Roblox picks (the app says so in a toast).
        case 'closePreset': {
          const p = settings.presets.find(x => x.id === a.id);
          const n = p?.running || 0;
          if (p) p.running = 0;
          push();
          return n;
        }
        case 'presetDiscordTest':
          await wait(500);
          if (!a.url && !settings.presets.find(x => x.id === a.id)?.discord) throw new Error('Paste a webhook link first.');
          return null;
        case 'runPreset': {
          const p = settings.presets.find(x => x.id === a.id);
          if (!p) throw new Error('Preset not found.');
          const ids = p.accountIds.filter(x => accounts.some(y => y.id === x));
          if (!ids.length) throw new Error('This preset has no accounts left.');
          { const ask = launchAsks(ids.length, a.force); if (ask) return ask; }
          const skipped = ids.filter(x => accounts.find(y => y.id === x).state !== 'idle').map(x => `${accounts.find(y => y.id === x).label}: already running — skipped.`);
          if (!['any', 'job'].includes(p.server) && p.placeInput) {
            emit('progress', { text: language === 'vi' ? 'Đang tìm server…' : 'Finding a server…' });
            await wait(600);
            if (params.has('noroom')) emit('toast', { text: `No server has room for ${ids.length} accounts right now, so Roblox picks one.`, kind: 'warn' });
          }
          const r = await window.__mockBridge.call('launch', { ids: ids.filter(x => accounts.find(y => y.id === x).state === 'idle'), force: true });
          if (r.launched) { p.lastRun = new Date().toISOString(); push(); }
          return { ...r, skipped };
        }
        case 'closeAccounts':
          accounts.forEach(x => { if (a.ids.includes(x.id) && x.state === 'running') { x.state = 'idle'; x.session = null; } });
          push();
          return a.ids.length;
        case 'deleteAccounts':
          orphans += accounts.filter(x => a.ids.includes(x.id) && x.state === 'running').length;
          accounts = accounts.filter(x => !a.ids.includes(x.id)); push(); return a.ids.length;
        case 'checkAccounts': await wait(800); return { ok: 5, bad: 1, failed: 0, banned: accounts.filter(x => x.banned).length };
        case 'refreshPresence': await wait(700); return params.has('refreshbusy') ? { skipped: true } : null; // ?refreshbusy=1: one was already running
        case 'copy': return { secret: !['username', 'pid'].includes(a.field), seconds: a.field === 'totp' ? fakeCode().seconds : null };
        case 'reveal': return a.field === 'password' ? 'matkhau-demo' : '_|WARNING:-DO-NOT-SHARE-THIS.--demo';
        case 'updateAccount': {
          const acc = accounts.find(y => y.id === a.id);
          // Like the app: no alias while names are hidden keeps it; a key is checked ("" removes it).
          if (typeof a.authenticator === 'string' && a.authenticator) {
            const check = await window.__mockBridge.call('totpCheck', { key: a.authenticator });
            if (!check.ok) throw new Error(check.error);
          }
          const alias = typeof a.alias === 'string' ? a.alias : acc.alias;
          Object.assign(acc, { alias, group: a.group, note: a.note, label: alias || acc.username });
          if (typeof a.authenticator === 'string') acc.hasAuthenticator = !!a.authenticator;
          push();
          return true;
        }
        case 'addViaCookie': throw new Error(tx('The cookie is invalid or has expired.', 'Cookie không hợp lệ hoặc đã hết hạn.'));
        case 'addViaBrowser': throw new Error(tx("The preview can't open the sign-in browser.", 'Bản xem thử không mở được trình duyệt đăng nhập.'));
        // Ctrl+Alt+H's windows, or the ones named: hidden or shown again (session.hidden, as the app reports it).
        case 'hideWindows': case 'showWindows':
          accounts.forEach(x => { if (x.session && (!a?.ids?.length || a.ids.includes(x.id))) x.session = { ...x.session, hidden: cmd === 'hideWindows' }; });
          push();
          return null;
        case 'closeAll': return 2;
        case 'friendServers': await wait(600); if (params.has('friendsfail')) throw new Error('HTTP 500'); return { asked: 2, friends: params.has('nofriends') ? [] : [
          { userId: 5140000101, name: LONG ? 'kiwi_runner_kiwi_runner_kiwi' : 'kiwi_runner', displayName: LONG ? 'Kiwi The Fastest Runner In The World' : 'Kiwi', avatarUrl: null, jobId: '00000000-0000-0000-0000-000000000002', placeId: 2753915549, via: hideNames ? 'Account 1' : LONG ? 'Sunny_The_Builder_Of_Many_Worlds_2010' : tx('Main', 'Tài khoản chính') },
          { userId: 5140000102, name: 'mochi_cat', displayName: 'Mochi', avatarUrl: null, jobId: '00000000-0000-0000-0000-000000000002', placeId: 2753915549, via: hideNames ? 'Account 2' : 'Farm 1' }] };
        // ?noprivate=1: no private servers; ?privatefail=1: Roblox didn't answer. One server only Farm 1 can join.
        case 'privateServers': await wait(500); if (params.has('privatefail')) return { asked: 2, failed: true, servers: [] };
          return { asked: 2, failed: false, servers: params.has('noprivate') ? [] : [
            { id: 901, name: tx('Farm server', 'Server farm'), owner: hideNames ? 'Account 1' : 'Sunny', ownerName: 'sunny_builds', accessCode: '11111111-2222-4333-8444-555555555555', playing: 3, maxPlayers: 12,
              via: hideNames ? ['Account 1', 'Account 2'] : [tx('Main', 'Tài khoản chính'), 'Farm 1'], viaIds: [id(4), id(2)] },
            { id: 902, name: tx("Kiwi's base", 'Căn cứ của Kiwi'), owner: 'Kiwi', ownerName: 'kiwi_runner', accessCode: '66666666-7777-4888-8999-000000000000', playing: 12, maxPlayers: 12,
              via: hideNames ? ['Account 2'] : ['Farm 1'], viaIds: [id(2)] }] };
        case 'pickLauncher': settings = { ...settings, customLauncher: 'Froststrap', launchVia: 'custom' }; push(); return null;
        case 'vaultImportPick':
          await wait(300);
          if (params.has('ivsame')) throw new Error(language === 'vi' ? 'Đây chính là dữ liệu Bloxkeep bạn đang dùng.' : "This is the vault you're using.");
          ivToken = null;
          return { name: 'Bloxkeep-backup-3f2a91c0-20261012-221500.rmv', wait: Math.max(0, Math.ceil((ivWaitUntil - Date.now()) / 1000)) };
        case 'vaultImportOpen': {
          await wait(700); // key stretching
          const left = Math.ceil((ivWaitUntil - Date.now()) / 1000);
          if (left > 0) return { status: 'wrong', wait: left, error: language === 'vi' ? `Sai quá nhiều lần. Chờ ${left} giây rồi thử lại.` : `Too many wrong tries. Wait ${left} seconds, then try again.` };
          if (a.password !== 'demo') {
            if (++ivWrong >= 3) ivWaitUntil = Date.now() + 10000;
            return { status: 'wrong', wait: ivWrong >= 3 ? 10 : 0, error: language === 'vi' ? 'Sai mật khẩu chủ của file này.' : 'Wrong master password for this file.' };
          }
          ivWrong = 0;
          ivToken = `t${Date.now()}`;
          return { status: 'ok', token: ivToken, name: 'Bloxkeep-backup-3f2a91c0-20261012-221500.rmv', presets: IV_PRESETS.length,
            accounts: [...IV_FILE.map((x, key) => ({ key, ...x, exists: !!ivHere(x.username) })),
              // ?ivmany=1: 24 more (29 in all), a list long enough for its search box. Only for looking: they can't be imported.
              ...(params.has('ivmany') ? Array.from({ length: 24 }, (_, i) => ({
                key: IV_FILE.length + i, label: `Alt ${i + 1}`, username: `alt_account_${i + 1}`, realm: '', group: i % 3 ? '' : 'Farm',
                hasPassword: i % 2 === 0, cookieValid: i % 4 === 0 ? false : true, avatarUrl: null, exists: false })) : [])] };
        }
        // Like the app: new accounts are added (sign-in checked afterwards); one already here only takes the file's
        // sign-in when its own doesn't work; presets keep their name, or get "(imported)", with schedules off.
        case 'vaultImportApply': {
          if (a.token !== ivToken) throw new Error('The file was closed because it was open too long. Open it again.');
          ivToken = null;
          const added = [];
          let updated = 0, presets = 0;
          for (const key of a.keys) {
            const x = IV_FILE[key], here = ivHere(x.username);
            if (here) {
              if (here.cookieValid !== true) { here.cookieValid = null; updated++; }
              continue;
            }
            const acc = { id: `iv${Date.now()}${key}`, userId: 5140000100 + key, username: x.username, displayName: x.username, alias: x.label, label: x.label, group: x.group, note: '',
              realm: x.realm, hasPassword: x.hasPassword, cookieValid: null, lastUsed: null, avatarUrl: x.avatarUrl, state: 'idle', presence: null, session: null };
            accounts.push(acc);
            added.push(acc.id);
          }
          if (a.presets) {
            for (const p of IV_PRESETS) {
              const ids = p.users.map(ivHere).filter(Boolean).map(x => x.id);
              if (!ids.length || settings.presets.length >= MAX_PRESETS) continue;
              const taken = n => settings.presets.some(y => y.name.toLowerCase() === n.toLowerCase());
              const word = language === 'vi' ? 'đã nhập' : 'imported';
              let name = p.name;
              for (let n = 1; taken(name); n++) name = `${p.name} (${n === 1 ? word : `${word} ${n}`})`;
              settings.presets.push({ id: `p${Date.now()}${presets}`, name, accountIds: ids, placeInput: p.placeInput, gameName: games[p.placeInput]?.name || '', server: 'any', jobId: '',
                delaySeconds: null, schedule: null, nextRun: null, iconUrl: presetIcon(p.placeInput), lastRun: null });
              presets++;
            }
          }
          push();
          return { added: added.length, updated, presets, presetsSkipped: 0, ids: added };
        }
        case 'vaultImportCancel': ivToken = null; return null;
        case 'arrangeWindows': return 2;
        case 'monitorArea': return null; // the live pictures are the app's own; the preview shows the empty area
        case 'setUseRobloxConfig':
          accounts.forEach(x => { if (a.ids.includes(x.id)) x.useRobloxConfig = a.enabled; });
          push();
          return a.ids.length;
        case 'setFpsCap':
          accounts.forEach(x => { if (a.ids.includes(x.id)) x.fpsCap = a.fps; });
          push();
          return a.ids.length;
        case 'setAutoRejoin':
          accounts.forEach(x => { if (a.ids.includes(x.id)) x.autoRejoin = a.enabled; });
          push();
          return a.ids.length;
        // The account card's Cancel on a rejoin waiting its turn (?rejoin=1).
        case 'cancelRejoin': accounts.forEach(x => { if (x.id === a.id) x.rejoin = null; }); push(); return null;
        case 'disableTrayAutostart': multi = { ...multi, trayAutostart: false }; push(); return null;
        // Sign-in: user "2fa" asks for a 2-step code (any 6 digits passes), "sms" a code by text message (email too),
        // "key" has only a security key, "captcha" hits a captcha ("captchaclosed": its window is closed unsolved, so Solve asks again), "x" is a wrong password.
        case 'loginPassword':
          loginUser = a.username;
          await wait(600);
          if (a.username === 'x') throw new Error('Incorrect username or password. Please try again.');
          if (['captcha', 'locked', 'captchaclosed'].includes(a.username)) return { status: 'captcha' };
          // "2fa": the key is saved here, so the code is filled in (Verify is still pressed by hand).
          if (a.username === '2fa') return { status: 'twostep', media: 'authenticator', methods: ['authenticator', 'email'], autofill: true };
          if (a.username === 'sms') return { status: 'twostep', media: 'sms', methods: ['sms', 'email'] };
          // "finish": signed in, but the account lookup failed; "Try again" (loginFinish) adds it.
          if (a.username === 'finish') return { status: 'finish', error: language === 'vi' ? 'Không kết nối được Roblox. Kiểm tra kết nối mạng rồi thử lại.' : "Couldn't reach Roblox. Check your internet connection and try again." };
          if (a.username === 'key') return { status: 'login', error: language === 'vi' ? 'Tài khoản này xác nhận đăng nhập bằng khóa bảo mật, passkey hoặc thiết bị khác, Bloxkeep chưa làm được. Hãy đăng nhập bằng điện thoại hoặc trình duyệt.' : "This account confirms sign-ins with a security key, a passkey or another device, which Bloxkeep can't do. Sign in with your phone or the browser instead." };
          return { status: 'added', added: { id: id(1), label: a.username, merged: false } };
        case 'loginTwoStep':
          await wait(500);
          if (!/^\d{6}$/.test(a.code)) throw new Error('Invalid code.');
          // "000000": too many tries, Verify waits a minute.
          if (a.code === '000000') return { status: 'wait', seconds: 60, error: language === 'vi' ? 'Đăng nhập sai quá nhiều lần. Chờ vài phút rồi thử lại.' : 'Too many sign-in attempts. Wait a few minutes and try again.' };
          return { status: 'added', added: { id: id(2), label: 'Farm 1', merged: false } };
        case 'loginTwoStepSend': await wait(400); return { status: 'sent' };
        case 'loginFinish': await wait(500); return { status: 'added', added: { id: id(4), label: 'finish', merged: false } };
        // Game search with the latencies measured from Vietnam (Oct 2026): search ~480 ms, icons ~330 ms more.
        case 'gameLists': {
          await wait(300);
          const row = id => ({ placeId: +id, universeId: 0, name: games[id].name, playing: games[id].playing, iconUrl: games[id].iconUrl, pinned: false });
          return { recent: settings.recentGames.map(g => row(g.placeId)), pinned: [], favorites: [row(920587237)], popular: Object.keys(games).map(row) };
        }
        case 'searchGames': {
          await wait(480);
          const q = (a.query || '').toLowerCase();
          return Object.entries(games).filter(([, g]) => g.name.toLowerCase().includes(q) || q.length < 3)
            .map(([p, g], i) => ({ placeId: +p, universeId: 1000 + i, name: g.name, playing: g.playing, iconUrl: null, pinned: false }));
        }
        case 'gameIcons': {
          await wait(330);
          return Object.fromEntries(a.universeIds.map((u, i) => [u, Object.values(games)[i % 3].iconUrl]));
        }
        // "locked": Roblox locked the account after the puzzle; the attempt is over (the page goes back to the first step).
        case 'loginCaptcha': if (loginUser === 'captchaclosed') return { status: 'captcha', closed: true }; await wait(800); if (loginUser === 'locked') return { status: 'login', error: language === 'vi' ? 'Roblox đã khóa tài khoản này để bảo vệ nó.' : 'Roblox has locked this account to protect it.' }; return { status: 'added', added: { id: id(3), label: 'captcha', merged: false } };
        // Quick Login: ?ql=linked,adding,added,… walks the code through those answers, one a poll, and stays on the last;
        // ?qlfail=1: the first code can't be had.
        case 'quickLoginStart':
          await wait(500);
          if (params.has('qlfail') && !qlFailed) { qlFailed = true; throw new Error(language === 'vi' ? 'Roblox không đưa mã. Thử lại sau giây lát.' : "Roblox didn't give a code. Try again in a moment."); }
          return { code: 'K7Q2X' + 'WABCDEFGH'[Math.min(8, qlStarts++)], seconds: 120, qr: null }; // K7Q2XW first, then A, B…
        case 'quickLoginPoll': {
          await wait(200);
          const step = QL_STEPS[Math.min(qlStep++, QL_STEPS.length - 1)] || 'waiting';
          if (step === 'error') throw new Error(language === 'vi' ? 'Roblox không trả lời. Thử lại.' : "Roblox didn't answer. Try again.");
          if (step === 'linked') return { status: 'linked', name: 'PhoneUser', picture: null };
          if (step === 'added') {
            const n = accounts.length + 1;
            const acc = { id: `ql${n}`, userId: 5150000000 + n, username: `PhoneUser${n}`, displayName: `PhoneUser${n}`, alias: '', label: `PhoneUser${n}`, group: '', note: '',
              realm: '', hasPassword: false, cookieValid: true, lastUsed: null, avatarUrl: null, state: 'idle', presence: null, session: null };
            accounts.push(acc);
            push();
            return { status: 'added', added: { id: acc.id, label: acc.label, merged: false } };
          }
          if (step === 'waiting') return { status: 'waiting', seconds: 110 };
          return { status: step };
        }
        case 'changePassword': await wait(400); if (a.current !== 'demo') throw new Error(language === 'vi' ? 'Mật khẩu chủ hiện tại không đúng.' : 'The current master password is wrong.'); return true;
        // Join a player: a name with "nobody" isn't a Roblox user; "hidden", "offline", "studio" and "web" are users
        // Roblox won't show a server for; any other name is found in Blox Fruits. Found names are remembered.
        case 'findPlayer': {
          await wait(500);
          const q = String(a.input || '').trim().replace(/^@/, '');
          if (!q) throw new Error(language === 'vi' ? 'Nhập tên đăng nhập Roblox hoặc ID người dùng của người chơi.' : 'Type the player’s Roblox username or user ID.');
          if (!/^[A-Za-z0-9_]{3,20}$/.test(q) && !/^\d{1,19}$/.test(q)) {
            throw new Error(language === 'vi' ? 'Tên đăng nhập Roblox gồm 3 đến 20 chữ cái, chữ số hoặc dấu _ (tên đăng nhập, không phải tên hiển thị). Cũng có thể dùng ID người dùng hoặc link trang cá nhân.'
              : 'A Roblox username is 3 to 20 letters, digits or _ (the username, not the display name). A user ID or a profile link works too.');
          }
          const lower = q.toLowerCase();
          if (lower.includes('nobody')) return { status: 'notFound', name: q };
          const status = lower.includes('hidden') ? 'hidden' : lower.includes('offline') ? 'offline' : lower.includes('studio') ? 'studio' : lower.includes('web') ? 'website' : 'found';
          settings = { ...settings, recentPlayers: [q, ...settings.recentPlayers.filter(n => n.toLowerCase() !== lower)].slice(0, 8) };
          push();
          const user = { userId: 5140000099, name: q, displayName: status === 'found' ? 'Builder Friend' : q };
          return status === 'found'
            ? { status, ...user, placeId: 2753915549, jobId: 'b6f1c2d4-0000-4c5e-9a7b-123456789abc', game: games[2753915549].name }
            : { status, ...user, placeId: null, jobId: null, game: null };
        }
        case 'forgetPlayer': settings = { ...settings, recentPlayers: settings.recentPlayers.filter(n => n.toLowerCase() !== String(a.name).toLowerCase()) }; push(); return null;
        case 'joinServer':
          await wait(300);
          // Like the app: the follow box's choice is remembered for next time.
          if (typeof a.follow === 'boolean') { settings = { ...settings, followJoinedPlayer: a.follow }; push(); }
          { const ask = launchAsks(a.ids.length, a.force); if (ask) return ask; }
          return { launched: a.ids.length, errors: [], cancelled: false };
        case 'activity': {
          await wait(150);
          if (params.has('activityfail')) throw new Error('Couldn’t load the activity.');
          const labelOf = n => { const acc = accounts.find(x => x.id === id(n)); return acc ? acc.label : 'Deleted account'; };
          // ?manyactivity=1: 130 lines, more than a page (100), to show "Show more".
          const rows = params.has('manyactivity') ? Array.from({ length: 130 }, (_, i) => [i * 9 + 1, ...ACTIVITY[i % ACTIVITY.length].slice(1)]) : ACTIVITY;
          const all = rows.map(([mins, kind, n, tone, en, vi]) => ({ at: minsAgo(mins), kind, group: activityGroup(kind), tone,
            accountId: n ? id(n) : null, account: n ? labelOf(n) : null, text: language === 'vi' ? vi : en }));
          const shown = all.filter(e => (!a.account || e.accountId === a.account) && (!a.group || e.group === a.group));
          const named = new Map(all.filter(e => e.accountId).map(e => [e.accountId, e.account]));
          return { total: shown.length, entries: shown.slice(0, a.limit || 100), accounts: [...named].map(([accId, label]) => ({ id: accId, label })) };
        }
        case 'exportActivity': await wait(200); return ACTIVITY.length; // the app shows Windows' Save dialog and writes the file
        // Play-time statistics: made-up stays for three of the accounts and the three games, the same on every call
        // (no randomness) so screenshots repeat. ?nostats=1: nothing recorded yet.
        // Today's stays by account, as seconds from midnight (the cards' day strips): three accounts, one dropped at 3 am.
        case 'todayStrips': {
          const now = new Date(), since = new Date(now); since.setHours(0, 0, 0, 0);
          const upTo = Math.round((now - since) / 1000), cap = x => Math.min(upTo, x);
          const rows = Object.fromEntries([[1, [[0, 3 * 3600 + 600, 1], [3 * 3600 + 1500, upTo, 0]]], [2, [[1800, cap(9 * 3600), 0], [cap(11 * 3600), upTo, 0]]],
            [4, [[cap(7 * 3600), cap(8 * 3600 + 1200), 1]]]].map(([n, stays]) => [id(n), stays.map(([a, b, c]) => [cap(a), cap(b), c]).filter(([a, b]) => b > a)]));
          return { since: since.toISOString(), now: now.toISOString(), rows };
        }
        case 'playStats': {
          await wait(200);
          if (params.has('statsms')) await new Promise(r => setTimeout(r, Number(params.get('statsms'))));
          if (params.has('statsfail')) throw new Error('Couldn’t load the statistics.'); // ?statsfail=1: every request fails
          const days = [7, 30].includes(a.days) ? a.days : 1, hourly = days === 1, now = new Date();
          const start = new Date(now);
          start.setHours(0, 0, 0, 0);
          if (!hourly) start.setDate(start.getDate() - (days - 1));
          const noise = i => ((i * 7919 + days * 104729) % 1000) / 1000;
          const buckets = Array.from({ length: hourly ? 24 : days }, (_, i) => {
            const at = new Date(start);
            if (hourly) at.setHours(i); else at.setDate(start.getDate() + i);
            const seconds = at > now || params.has('nostats') ? 0 : Math.round(noise(i) * (hourly ? 3000 : 12000) + (i % 3 === 0 ? (hourly ? 600 : 3600) : 0));
            return { start: at.toISOString(), seconds };
          });
          const total = buckets.reduce((s, b) => s + b.seconds, 0), count = Math.round(total / 2700) + (total ? 2 : 0);
          const share = p => Math.round(total * p);
          const result = { days, hourly, since: start.toISOString(), recorded: !params.has('nostats'), totalSeconds: total, sessions: count, kicks: Math.round(total / 20000),
            // The period before, as far in: more play, fewer drops. ?noprev=1: the records don't reach back that far.
            previous: params.has('noprev') || !total ? null : { totalSeconds: Math.round(total / 1.2), kicks: Math.round(total / 20000) + 2 },
            icons: Object.fromEntries([2753915549, 920587237, 4924922222].map(p => ['u' + p, cold(games[p].iconUrl)])),
            accounts: [[1, .45, 1, 2753915549], [2, .33, 2, 2753915549], [4, .2, 0, 4924922222], [9, .02, 0, 920587237]].map(([n, p, k, g]) => { const acc = accounts.find(x => x.id === id(n)); return { id: id(n), label: acc?.label, deleted: acc ? undefined : true, username: acc?.username, avatarUrl: acc?.avatarUrl, seconds: share(p), sessions: Math.max(1, Math.round(count * p)), kicks: k,
              topGame: { key: 'u' + g, name: games[g].name, iconUrl: cold(games[g].iconUrl) },
              // Its play per hour or day, and its run of full days (the main farm account six, one other two).
              buckets: buckets.map((b, i) => Math.round(b.seconds * p * (.5 + noise(i * 13 + n)))), streak: n === 1 ? 6 : n === 2 ? 2 : 0 }; }),
            games: [[2753915549, .55], [920587237, .3], [4924922222, .15]].map(([p, s]) => ({ key: 'u' + p, placeId: p, name: games[p].name, iconUrl: cold(games[p].iconUrl), seconds: share(s), sessions: Math.max(1, Math.round(count * s)) })),
            ends: (() => { const e = new Date(start); e.setDate(e.getDate() + days); return e.toISOString(); })(),
            // Stays as shares of the time so far: [from, to, game, how it ended].
            timeline: !total ? [] : [[1, [[0, .34, 2753915549, 'idle'], [.38, .72, 2753915549], [.76, 1, 920587237]]], [2, [[.05, .3, 2753915549, 'other'], [.44, .62, 920587237, 'idle'], [.68, .93, 2753915549]]],
              [4, [[.2, .55, 4924922222], [.6, .8, 2753915549]]], [9, [[.1, .13, 920587237]]]].map(([n, stays]) => ({ id: id(n), segments: stays.map(([from, to, p, reason]) => {
                const when = k => new Date(start.getTime() + (now - start) * k).toISOString();
                return { from: when(from), to: when(to), game: 'u' + p, name: games[p].name, reason: reason || null };
              }) })),
            losses: !total ? [] : [{ kind: 'idle', times: 2, seconds: share(.14), accounts: [id(1), id(2)] }, { kind: 'other', times: 1, seconds: share(.05), accounts: [id(2)] }],
            buckets,
            // 7 and 30 days: windows by hour, more at night (left farming), one night the PC was off, none after now.
            heat: hourly ? undefined : Array.from({ length: days }, (_, i) => Array.from({ length: 24 }, (_, h) => {
              const at = new Date(start); at.setDate(start.getDate() + i); at.setHours(h);
              if (at > now || params.has('nostats') || (i === days - 4 && h >= 1 && h < 7)) return 0;
              return Math.round(((h < 8 ? 6 : h >= 20 ? 4 : 2) + noise(i * 24 + h) * 3) * 100) / 100;
            })),
            presets: [{ id: 'p1', name: 'Farm Blox Fruits', iconUrl: cold(games[2753915549].iconUrl), seconds: share(.62), sessions: Math.max(1, Math.round(count * .6)), kicks: 1, accounts: 3 },
              { id: 'p2', name: 'Adopt Me trade', iconUrl: cold(games[920587237].iconUrl), seconds: share(.25), sessions: 3, kicks: 0, accounts: 1 },
              { id: '00000000-0000-0000-0000-000000000000', name: '', iconUrl: null, seconds: share(.13), sessions: 2, kicks: 0, accounts: 2 }],
            longest: total ? { seconds: Math.round(total * .3), account: id(1), label: accounts.find(x => x.id === id(1))?.label, game: games[2753915549].name } : null };
          // One game only (picked on the games' map): its box, the accounts that mostly play it, its stays.
          result.gamesSeconds = result.totalSeconds;
          if (a.game) Object.assign(result, { totalSeconds: result.games.find(g => g.key === a.game)?.seconds || 0,
            accounts: result.accounts.filter(x => x.topGame?.key === a.game),
            timeline: result.timeline.map(r => ({ ...r, segments: r.segments.filter(x => x.game === a.game) })).filter(r => r.segments.length) });
          return result;
        }
        // Macros (Beta), as the app answers (AppController's macro commands); nothing is ever pressed or clicked here.
        case 'saveMacro': {
          const m = a.macro;
          if (!String(m.name || '').trim()) throw new Error(language === 'vi' ? 'Hãy đặt tên cho macro.' : 'Give the macro a name.');
          if (m.hotkey && macros.some(o => o.id !== m.id && o.hotkey === m.hotkey)) throw new Error(language === 'vi' ? `Ctrl+Alt+${m.hotkey} đã dùng cho macro khác.` : `Ctrl+Alt+${m.hotkey} is already another macro's.`);
          const saved = { ...m, name: m.name.trim(), id: m.id || `m${++macroSeq}` };
          setMacros(macros.some(o => o.id === saved.id) ? macros.map(o => o.id === saved.id ? saved : o) : [...macros, saved]);
          push();
          return saved;
        }
        case 'deleteMacro': setMacros(macros.filter(m => m.id !== a.id)); macroRuns = macroRuns.filter(r => r.macroId !== a.id); push(); return null;
        case 'duplicateMacro': {
          const i = macros.findIndex(m => m.id === a.id);
          const copy = { ...macros[i], id: `m${++macroSeq}`, name: `${macros[i].name} ${language === 'vi' ? '(bản sao)' : '(copy)'}`.slice(0, 60), hotkey: null,
            panel: { ...(macros[i].panel || { placeId: null }), autoShow: false } };
          setMacros([...macros.slice(0, i + 1), copy, ...macros.slice(i + 1)]);
          push();
          return copy;
        }
        case 'moveMacro': {
          const m = macros.find(x => x.id === a.id), rest = macros.filter(x => x.id !== a.id);
          rest.splice(Math.max(0, Math.min(rest.length, a.to)), 0, m);
          setMacros(rest);
          push();
          return null;
        }
        case 'runMacro': {
          const m = macros.find(x => x.id === a.id);
          if (a.accountIds && !a.accountIds.length) throw new Error(language === 'vi' ? 'Chưa chọn tài khoản nào.' : 'No accounts chosen.');
          const want = a.accountIds || (m.targets === 'accounts' ? m.accountIds : m.targets === 'all' ? accounts.map(x => x.id) : [id(2), id(4)]);
          const ids = want.filter(x => accounts.find(acc => acc.id === x)?.state === 'running');
          const busy = new Set(macroRuns.filter(r => r.macroId === m.id && ['waiting', 'running', 'paused'].includes(r.status)).map(r => r.accountId));
          const fresh = ids.filter(x => !busy.has(x));
          macroRuns = [...macroRuns.filter(r => r.macroId !== m.id || !fresh.includes(r.accountId)),
            ...fresh.map((x, k) => ({ macroId: m.id, accountId: x, status: k ? 'waiting' : 'running', reason: k ? 'busy' : undefined, step: k ? undefined : 0, startedAt: minsAgo(0) }))];
          macroSweep = fresh.length ? { seconds: 2 * fresh.length } : null;
          push();
          return { queued: fresh.length };
        }
        case 'stopMacros':
          // {id, accountId, trial: true}: only Try it's run on that window (the page asks this; see the report).
          if (a.trial) clearTimeout(tryTimer);
          macroRuns = macroRuns.map(r => (!a.id || r.macroId === a.id) && (!a.accountId || r.accountId === a.accountId) && (!a.trial || r.trial)
            && ['waiting', 'running', 'paused'].includes(r.status) ? { ...r, status: 'stopped', reason: 'byUser', step: undefined, lastStep: r.step, endsAt: minsAgo(0) } : r);
          if (!a.id) macroRecording = null;
          push();
          return null;
        case 'macroPicture':
          await wait(200);
          if (accounts.find(x => x.id === a.accountId)?.state !== 'running') throw new Error(language === 'vi' ? 'Tài khoản này chưa mở cửa sổ Roblox.' : 'This account has no Roblox window open.');
          return { image: MACRO_SCENE, w: 1280, h: 720 };
        // Try: a picture is found where the CLAIM button is; text "claim" is read there; a colour always matches.
        case 'macroTest': {
          await wait(250);
          const tst = a.test || {};
          if (tst.kind === 'image') return { ok: true, found: { x: 0.781, y: 0.75, w: 0.141, h: 0.089 }, score: 0.96 };
          if (tst.kind === 'text') return String(tst.text || '').trim().toLowerCase() === 'claim' ? { ok: true, found: { x: 0.8, y: 0.77, w: 0.1, h: 0.05 }, text: 'CLAIM' } : { ok: false, text: 'CLAIM' };
          if (tst.kind === 'colour') return { ok: true };
          return { ok: false };
        }
        // Recording: the countdown, then steps as they come; Stop (or F8) hands the steps to the page.
        case 'recordMacro':
          if (accounts.find(x => x.id === a.accountId)?.state !== 'running') throw new Error(language === 'vi' ? 'Tài khoản này chưa mở cửa sổ Roblox.' : 'This account has no Roblox window open.');
          macroRecording = { accountId: a.accountId, startsAt: new Date(Date.now() + 3000).toISOString(), steps: 0 };
          push();
          return null;
        case 'stopRecording':
          if (macroRecording) setTimeout(() => emit('macroRecorded', { steps: RECORDED, w: 1280, h: 720 }), 50);
          macroRecording = null;
          push();
          return null;
        case 'importMacro':
          await wait(200);
          return { id: '', name: tx('Fishing loop', 'Câu cá tự động'), targets: 'selected', accountIds: [], trigger: { kind: 'manual' }, enabled: true, hotkey: null,
            jitterMs: 40, jitterPx: 3, maxMinutes: 30, onFail: 'stop', steps: [{ kind: 'loop', times: 20 }, { kind: 'click', x: 0.5, y: 0.6, button: 'left', double: false },
              { kind: 'waitColour', x: 0.5, y: 0.4, colour: '#ffffff', tolerance: 24, timeoutMs: 20000, gone: false }, { kind: 'click', x: 0.5, y: 0.6, button: 'left', double: false },
              { kind: 'endLoop' }] };
        case 'exportMacro': await wait(150); return true;
        case 'macroTemplates': await wait(50); return TEMPLATES();
        // The panel beside the game, and a value the player changed there.
        case 'showPanel': if (!panelsShown.includes(a.id)) panelsShown = [...panelsShown, a.id]; push(); return null;
        case 'hidePanel': panelsShown = panelsShown.filter(x => x !== a.id); push(); return null;
        case 'setOption':
          setMacros(macros.map(m => m.id !== a.id ? m : { ...m, options: (m.options || []).map(o => o.name === a.name ? { ...o, value: a.value } : o) }));
          push();
          return null;
        // Edit on game: the app draws the dots and boxes on the window; ?gamedrag=1 drags Fish area there a moment later.
        case 'editOnGame':
          if (a.accountId && params.has('gamedrag')) setTimeout(() => emit('onGameEdit', { area: 'a1', rect: { x: 0.25, y: 0.7, w: 0.5, h: 0.12 } }), 300);
          return null;
        // Pick on game: the next click on the window (here the bar's green, low in the middle); null if Esc.
        case 'pickOnGame': await wait(300); return { x: 0.52, y: 0.8, colour: '#22c55e' };
        // Try it: the draft once on one window, as a trial run (in macroRuns with trial: true, not the list's); the steps
        // go by a few a second. What it looks for is found, except the bar (follow), which isn't on screen until a fish
        // bites, and a picture not cut yet; an empty value stops it there. The watch sends the window meanwhile, its last
        // frame saying how it ended.
        case 'macroTryRun': {
          clearTimeout(tryTimer);
          const m = a.macro, macroId = m.id || 'mtry', acc = a.accountId;
          if (macroRuns.some(r => !r.trial && r.macroId === macroId && r.accountId === acc && ['waiting', 'running', 'paused'].includes(r.status)))
            throw new Error(tx("It's running on this account already. Stop it first.", 'Macro đang chạy trên tài khoản này. Hãy dừng trước.'));
          const empty = v => typeof v === 'string' && v.startsWith('$') && (m.options || []).find(o => '$' + o.name === v)?.value === '';
          const runOf = () => macroRuns.find(r => r.trial && r.macroId === macroId && r.accountId === acc && r.status === 'running');
          macroRuns = [...macroRuns.filter(r => !(r.trial && r.macroId === macroId && r.accountId === acc)),
            { macroId, accountId: acc, status: 'running', step: 0, trial: true, startedAt: new Date().toISOString() }];
          const set = p => { macroRuns = macroRuns.map(r => r === runOf() ? { ...r, ...p } : r); push(); };
          const frame = (note, boxes = []) => emit('macroSeen', { accountId: acc, image: MACRO_SCENE, w: 1280, h: 720, boxes, numbers: [], note });
          const next = i => {
            if (!runOf()) return;
            const st = m.steps[i];
            if (!st) { set({ status: 'done', step: undefined, lastStep: i ? i - 1 : undefined, endsAt: new Date().toISOString() }); return frame(tx('Tried: it ran to the end.', 'Đã thử: chạy hết.')); }
            set({ step: i });
            if (Object.values(st).some(empty)) {
              set({ status: 'failed', reason: 'missingValue', step: undefined, lastStep: i, endsAt: new Date().toISOString() });
              return frame(tx('Tried: it stopped (a setting or area is missing).', 'Đã thử: dừng (thiếu cài đặt hoặc vùng).'));
            }
            if (st.kind === 'follow' || st.image === '') {
              set({ status: 'failed', reason: 'notFound', step: undefined, lastStep: i, endsAt: new Date().toISOString() });
              return frame(tx('Tried: it stopped (what it looks for wasn\'t there).', 'Đã thử: dừng (không thấy thứ cần tìm).'));
            }
            frame('', ['waitImage', 'clickImage', 'waitColour', 'clickColour', 'waitText', 'if'].includes(st.kind) ? [{ x: 0.781, y: 0.75, w: 0.141, h: 0.089, label: tx('found', 'thấy') }] : []);
            tryTimer = setTimeout(() => next(i + 1), 200);
          };
          push();
          tryTimer = setTimeout(() => next(0), 200);
          return { queued: 1, macroId };
        }
        // The editor's live view of a window (macroSeen about 5 times a second); accountId null stops it.
        case 'macroWatch': return null;
        default: return null;
      }
    },
  };
  // Scripted views for headless screenshots: ?settings=1, ?stats=1, ?monitor=1, ?select=1, ?menu=1, ?edit=1
  window.addEventListener('load', () => setTimeout(() => {
    const click = sel => document.querySelector(sel)?.click();
    // ?settings=<page or group> shows that page (for a group, the page it is on); ?open=a,b scrolls to the last of those
    // groups; with ?backup=1, the Backup page.
    if (params.has('settings')) {
      click('[data-view=settings]');
      const want = [params.get('settings'), params.has('backup') ? 'backup' : '', ...(params.get('open') || '').split(',')].filter(v => v && v !== '1');
      const pageOf = id => document.querySelector(`[data-set-page="${id}"]`) ? id : document.querySelector(`#sec-${id}`)?.closest('.set-page')?.dataset.page;
      const page = want.map(pageOf).find(Boolean);
      if (page) click(`[data-set-page="${page}"]`);
      const group = [...want].reverse().find(id => id !== page && document.querySelector(`#sec-${id}`));
      if (group) document.querySelector(`#sec-${group}`).scrollIntoView({ block: 'start' });
    }
    // ?vimport=pick|password|wrong|preview walks the import from a vault file to that step (Settings › Backup's button).
    if (params.has('vimport')) {
      const stepTo = params.get('vimport');
      click('[data-view=settings]');
      click('[data-set-page="backup"]');
      click('[data-act=bk-import]');
      const press = () => document.querySelector('.overlay .modal-foot .btn.primary')?.click();
      const type = v => { const box = document.querySelector('#iv-pw'); if (box) box.value = v; };
      if (stepTo !== 'pick') setTimeout(press, 200);
      if (stepTo === 'wrong') setTimeout(() => { type('nope'); press(); }, 900);
      if (stepTo === 'preview') setTimeout(() => { type('demo'); press(); }, 900);
    }
    if (params.has('lockupdatein')) setTimeout(() => { lockUpdate = '30'; push(); }, +params.get('lockupdatein') || 1500);
    if (params.has('autolockin')) setTimeout(() => { screenLock = { minutes: settings.autoLockMinutes || 15 }; locked = true; push(); }, +params.get('autolockin') || 1500);
    if (params.has('stats')) click('[data-view=stats]');
    if (['macros', 'nomacros', 'macrorec', 'macroedit'].some(k => params.has(k))) click('[data-view=macros]');
    if (params.has('macroedit')) setTimeout(() => {
      click(`.mc-item[data-id="${params.get('macroedit')}"] [data-mc=more]`);
      setTimeout(() => {
        [...document.querySelectorAll('.menu button')].find(b => b.textContent.trim() === (window.I18N?.[language]?.['macro.edit'] || 'Edit'))?.click();
        if (params.has('macropic')) setTimeout(() => click('#mc-pic-take'), 300);
        if (params.has('macrotry')) setTimeout(() => click('#mc-try'), 300);
        if (params.has('macrogame')) setTimeout(() => click('#mc-ongame'), 300);
      }, 50);
    }, 100);
    if (params.has('monitor')) click('[data-view=monitor]');
    if (params.has('select') || params.has('menu')) { click('.card[data-id$="2"]'); click('.card[data-id$="4"]'); }
    if (params.has('menu')) click('.card[data-id$="4"] [data-act=card-menu]');
    if (params.has('edit')) click('.card[data-id$="1"] [data-act=edit]');
    // ?order=1 opens the launch-order dialog; ?view=presets (or ?presets=1) the Presets tab, through the dock menu's
    // "Edit presets…"; ?preset=1 the form of the scheduled preset (p1) from that tab.
    const menuItem = text => [...document.querySelectorAll('.menu button')].find(b => b.textContent.includes(text))?.click();
    // The item by its words in the page's language (i18n.js), so a reworded label doesn't stop the view opening.
    const words = key => (window.I18N?.[language]?.[key] || window.I18N?.en?.[key] || key).replace(/…$/, '');
    if (params.has('order')) { click('[data-act=tools-menu]'); menuItem(words('m.launchOrder')); }
    if (params.get('view') === 'presets' || params.has('presets') || params.has('preset')) {
      click('[data-act=preset-menu]');
      menuItem(words('ps.manage'));
      if (params.has('preset')) setTimeout(() => click('.pv-item[data-id="p1"] [data-pv=edit]'), 100);
    }
  }, 600));
})();

// The game monitor's tiles (folded in from mock-p7.js).
// The game monitor (?monitor=1&tiles=N): N Roblox windows, sent as the app sends them ("monitor") once the page reports
// the tab's area, each with a stand-in picture where the app would lay the live one (the browser can't show windows).
// ?tiles=N&mintile=1 the second one minimised, &long=1 long names and games, &hide=1 names hidden, &others=1 the last one
// not opened by Bloxkeep, &same=1 all in one game (no lanes), &drop=1 one cut off and one joining. The tiles' acts (monitorAct) change the list as the app would.
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
  /** A game icon: a coloured square with a block on it. */
  const icon = (bg, fg) => 'data:image/svg+xml,' + encodeURIComponent(`<svg xmlns="http://www.w3.org/2000/svg" width="64" height="64"><rect width="64" height="64" fill="${bg}"/><rect x="18" y="18" width="28" height="28" rx="4" fill="${fg}"/></svg>`);
  const ICONS = [icon('#2f6fd6', '#f2c94c'), icon('#7a3fd1', '#7ee0a1'), icon('#0f766e', '#f4b56a')];
  const PICTURES = [scene('#7ec8f0', '#5aa04a', '#b97a4a', '#9aa3ad'), scene('#f4b56a', '#c9a15b', '#7a5230', '#5b6b7a'),
    scene('#1c2a4a', '#2f4f3a', '#45607d', '#6b5b8c'), scene('#a8d8e8', '#e8eef2', '#7a8b99', '#c45c5c')];
  const NAMES = long ? ['MainAccountWithAVeryLongUsername', 'FarmAlt_number_two_2024', 'Trader'] : ['MainAccount', 'FarmAlt02', 'Trader'];
  const GAMES = long ? ['Blox Fruits [Second Sea] Update 24 with the Dragon rework', 'Pet Simulator 99 Trading Plaza', 'Fisch'] : ['Blox Fruits', 'Pet Simulator 99', 'Fisch'];
  let tiles = Array.from({ length: count }, (_, i) => {
    const other = params.has('others') && i === count - 1;
    return {
      id: String(1050000 + i * 2468), pid: 14280 + i * 412,
      name: other ? 'Roblox' : hide ? (vi ? `Tài khoản ${i + 1}` : `Account ${i + 1}`) : i < 3 ? NAMES[i] : `Farm${String(i).padStart(2, '0')}`,
      game: params.has('same') ? GAMES[0] : i === 1 && !params.has('mintile') ? (vi ? 'Chưa vào game' : 'No game') : GAMES[i % 3],
      ramMb: [1240, 812, 960][i % 3] + i * 10, ours: !other, minimized: params.has('mintile') && i === 1, picture: PICTURES[i % PICTURES.length],
      // &drop=1: the fourth window cut off, the seventh still joining; the rest of a game's share two servers.
      status: i === 1 && !params.has('mintile') ? 'menu' : params.has('drop') && i === 3 ? 'disconnected' : params.has('drop') && i === 6 ? 'joining' : 'ingame',
      jobId: i === 1 || params.has('drop') && (i === 3 || i === 6) ? null : `job-${i % 3}-${i < 5 ? 'a' : 'b'}`, iconUrl: i === 1 && !params.has('mintile') ? null : ICONS[i % 3],
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
