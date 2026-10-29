'use strict';
// Fake bridge for designing the UI in a normal browser. ?locked=1 shows the lock screen, ?new=1 the first-run screen,
// ?empty=1 an empty vault, ?theme=light (system|light|paper|dim|dark|black) a theme. Presets: ?view=presets opens their
// tab, ?nopresets=1 has none, ?many=1 the most the app keeps (50), ?noicons=1 no game icons known yet. ?rejoin=1: an
// account waiting minutes for auto-rejoin's next try.
(() => {
  const params = new URLSearchParams(location.search);
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
      session: { pid: 1111, since: minsAgo(47), game: g(2753915549), connection: 'ingame', hidden: params.has('winhidden') } },
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
  const vaultsState = () => {
    const cur = vaultAdding || vaultList.find(v => v.id === vaultCurrent);
    return { list: vaultList, current: cur?.id || null, name: cur?.name || null, picture: vaultAdding ? null : cur?.picture || null,
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
    : params.get('remote') === 'ask' ? { on: true, connected: true, linked: true, user: 'Minh', code: null, codeUntil: null, pc: 'GAMING-PC', ask: { user: 'Stranger', replacing: 'Minh' } }
    : { on: false, connected: false, linked: null, pc: 'GAMING-PC' };
  const skipConfirm = [];
  const lastSignInCheck = minsAgo(60 * 5);
  let miniOpen = params.has('mini'); // ?mini=1: the mini window counts as open
  // ?backup=1 shows a chosen folder with a recent backup; ?bkerr=1 a failed one; ?weak=1 the weak-password line.
  let backup = {
    enabled: true, folder: params.has('backup') ? 'C:\\Users\\Admin\\OneDrive\\Bloxkeep' : null,
    lastBackup: params.has('backup') ? minsAgo(5) : null, error: params.has('bkerr') ? 'Access to the path is denied.' : null,
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
    memoryWarnings: true, trimBackground: false, antiAfk: false,
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
    robloxVersion: { installed: '0.742.0.7421053', latest: params.has('rbxold') ? '0.743.0.7430112' : '0.742.0.7421053', upToDate: !params.has('rbxold'), direct: false, store: settings.launchVia === 'store',
      updater: settings.launchVia === 'store' ? null : 'Bloxstrap', updating: robloxUpdating },
    // Settings › Roblox updates, newest first: a new build under the same version, an update, the first build seen.
    // ?rbxclosed=1: Roblox closed windows when another one opened (the warning there and on the Multi-Roblox card).
    // ?rw=changed|renamed|gone: what the Roblox watch found in the installed build (default: no change, and the next
    // build checked ahead); ?rw=worked: it changed, but two windows played together on it here since; ?rw=none: not read.
    robloxUpdates: {
      installed: { version: '0.742.0.7421053', build: 'version-cec3ad5889b447cf', source: 'Bloxstrap' },
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
    installs: { handler: 'Bloxstrap', bloxstrap: true, fishstrap: false, others: params.has('others') ? ['Voidstrap'] : [], roblox: false, direct: true, store: params.has('store'), vng: params.has('vng') },
    monitors: [{ index: 0, primary: true, width: 1920, height: 1080 }, { index: 1, primary: false, width: 2560, height: 1440 }], dataPath: LONG ? 'C:\\Users\\Administrator\\AppData\\Local\\Packages\\Some.Very.Long.Package.Name_8wekyb3d8bbwe\\LocalCache\\Local\\Bloxkeep' : 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep', paths: { vault: locked ? null : 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep\\vault.rmv', log: 'C:\\Users\\Admin\\AppData\\Local\\Bloxkeep\\error.log', app: 'C:\\Users\\Admin\\AppData\\Local\\BloxkeepApp\\current' },
    hotkeys: { on: !params.has('nohotkeys'), taken: params.has('keystaken') ? ['Ctrl+Alt+H'] : [] },
    robloxInstalled: !params.has('noroblox'),
    launching: false, checking: false, multi, settings: locked ? null : settings, accounts: locked ? [] : accounts, discord: locked ? null : discord,
  });
  // Roblox windows of accounts deleted while playing: like the app, deleting doesn't close them, and they still count.
  let orphans = 0;
  const metrics = () => ({
    accounts: accounts.filter(a => a.state === 'running').map((a, i) => ({ id: a.id, ramMb: 780 + i * 140 + Math.round(Math.random() * 30) })),
    robloxCount: accounts.filter(a => a.state === 'running').length + orphans,
    monitorCount: accounts.filter(a => a.state === 'running').length + orphans,
    totalMb: 1750 + Math.round(Math.random() * 40), systemMb: 32 * 1024, usedPct: 63, availableMb: 12 * 1024,
  });
  // The panel's numbers go with every change to the cards, not only on the next 1.5 s tick: a window opened or closed
  // is then counted in the sidebar as soon as its card says so, and Tests/ui doesn't write down one or the other by
  // chance (the app's own check sends both within the same 1.5 s).
  const push = () => { emit('state', state()); if (!locked) emit('metrics', metrics()); };
  let loginUser = '';
  const srvAsked = []; // when each server list was given, for Roblox's few-a-minute limit
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
        case 'servers': {
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
        case 'backupNow': await wait(300); if (params.has('bkerr')) throw new Error("Couldn't back up: Access to the path is denied."); backup = { ...backup, lastBackup: new Date().toISOString() }; push(); return null;
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
          if (params.has('dcfail')) throw new Error("Couldn't reach Discord: No such host is known. (discord.com:443)");
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
        case 'restoreBackup': await wait(500); vaultExists = true; locked = true; push(); emit('toast', { text: 'Backup restored. Unlock it with the master password it was saved with.', kind: 'ok' }); return null;
        case 'resolveGame': {
          const m = /(\d{5,})/.exec(a.input);
          if (!m) return { ok: false, error: 'Không đọc được Place ID từ link này.' };
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
        case 'addViaCookie': throw new Error('Cookie không hợp lệ hoặc đã hết hạn.');
        case 'addViaBrowser': throw new Error('Bản xem thử không mở được trình duyệt đăng nhập.');
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
            accounts: IV_FILE.map((x, key) => ({ key, ...x, exists: !!ivHere(x.username) })) };
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
        case 'setAutoRejoin':
          accounts.forEach(x => { if (a.ids.includes(x.id)) x.autoRejoin = a.enabled; });
          push();
          return a.ids.length;
        // The account card's Cancel on a rejoin waiting its turn (?rejoin=1).
        case 'cancelRejoin': accounts.forEach(x => { if (x.id === a.id) x.rejoin = null; }); push(); return null;
        case 'disableTrayAutostart': multi = { ...multi, trayAutostart: false }; push(); return null;
        // Sign-in: user "2fa" asks for a 2-step code (any 6 digits passes), "sms" a code by text message (email too),
        // "key" has only a security key, "captcha" hits a captcha, "x" is a wrong password.
        case 'loginPassword':
          await wait(600);
          if (a.username === 'x') throw new Error('Incorrect username or password. Please try again.');
          if (a.username === 'captcha' || a.username === 'locked') { loginUser = a.username; return { status: 'captcha' }; }
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
        case 'loginCaptcha': await wait(800); if (loginUser === 'locked') return { status: 'login', error: language === 'vi' ? 'Roblox đã khóa tài khoản này để bảo vệ nó.' : 'Roblox has locked this account to protect it.' }; return { status: 'added', added: { id: id(3), label: 'captcha', merged: false } };
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
          return { days, hourly, since: start.toISOString(), recorded: !params.has('nostats'), totalSeconds: total, sessions: count, kicks: Math.round(total / 20000),
            accounts: [[1, .45, 1], [2, .33, 2], [4, .2, 0], [9, .02, 0]].map(([n, p, k]) => { const acc = accounts.find(x => x.id === id(n)); return { id: id(n), label: acc?.label, username: acc?.username, avatarUrl: acc?.avatarUrl, seconds: share(p), sessions: Math.max(1, Math.round(count * p)), kicks: k }; }),
            games: [[2753915549, .55], [920587237, .3], [4924922222, .15]].map(([p, s]) => ({ placeId: p, name: games[p].name, iconUrl: cold(games[p].iconUrl), seconds: share(s), sessions: Math.max(1, Math.round(count * s)) })),
            buckets,
            presets: [{ id: 'p1', name: 'Farm Blox Fruits', iconUrl: cold(games[2753915549].iconUrl), seconds: share(.62), sessions: Math.max(1, Math.round(count * .6)), kicks: 1, accounts: 3 },
              { id: 'p2', name: 'Adopt Me trade', iconUrl: cold(games[920587237].iconUrl), seconds: share(.25), sessions: 3, kicks: 0, accounts: 1 },
              { id: '00000000-0000-0000-0000-000000000000', name: '', iconUrl: null, seconds: share(.13), sessions: 2, kicks: 0, accounts: 2 }],
            longest: total ? { seconds: Math.round(total * .3), account: id(1), label: accounts.find(x => x.id === id(1))?.label, game: games[2753915549].name } : null };
        }
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
