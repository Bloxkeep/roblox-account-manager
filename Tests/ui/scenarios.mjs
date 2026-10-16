// What the interface checks look at. Each: a name, the preview's URL options (dev/mock.js lists them: ?menu=1, ?settings=
// <page>…) and steps that click and type like a person. Elements are found by their words ({ key: an i18n key }, in
// whichever language the run is in) or by a CSS selector when they have no words. The page is written down at the end,
// and at each s.snap('label').
const S = (name, query, run, extra = {}) => ({ name, query, run, ...extra });
const key = k => ({ key: k });

export default [
  // ── Accounts ──
  S('accounts-cards', ''),
  S('accounts-list', '', async s => { await s.click(key('layout.list')); }),
  S('accounts-select', 'select=1'),
  S('account-menu', 'menu=1'),
  S('account-edit', 'edit=1'),
  S('accounts-empty', 'empty=1'),
  S('accounts-kicked', 'kicked=1'),
  S('accounts-rejoin-wait', 'rejoin=1'),
  S('accounts-long-names', 'long=1'),
  S('accounts-many', 'accounts=40'),
  S('accounts-search', '', async s => { await s.type('farm', key('acc.search')); }),
  S('accounts-filter-playing', '', async s => { await s.click(key('chip.ingame')); }),
  S('accounts-filter-signin', '', async s => { await s.click(key('chip.invalid')); }),
  S('accounts-sort-menu', '', async s => { await s.click({ css: '#sort-btn' }); }),
  S('add-account', '', async s => { await s.click(key('acc.add')); }),
  S('add-account-more', '', async s => { await s.click(key('acc.addMoreTip')); }),
  S('play-two', 'select=1', async s => {
    await s.click({ key: 'dock.play', within: '.dock' });
    await s.wait(1500);
  }),
  S('first-run', 'new=1'),
  S('locked', 'locked=1'),

  // ── Game ──
  S('game-picker', '', async s => { await s.click(key('dock.browseTip')); }),
  S('launch-order', 'order=1'),

  // ── Side panel ──
  S('multi-roblox-off', 'multioff=1'),
  S('multi-roblox-dialog', '', async s => { await s.click({ css: '.multi, [data-act=multi]' }); }),
  S('no-roblox', 'noroblox=1'),
  S('low-ram', 'lowram=1'),
  S('lock-ask', '', async s => { await s.click(key('side.lock')); }),

  // ── Quick search (Ctrl+K) ──
  S('palette', '', async s => {
    await s.key('k', ['ctrl']);
    await s.snap('open');
    await s.type('farm');
    await s.snap('typed');
    await s.key('Escape');
  }),

  // ── Presets ──
  S('presets', 'view=presets'),
  S('presets-none', 'view=presets&nopresets=1'),
  S('preset-editor', 'preset=1'),

  // ── Game monitor, statistics ──
  S('monitor', 'monitor=1'),
  S('stats', 'stats=1'),
  S('stats-none', 'stats=1&nostats=1'),

  // ── Settings: every page ──
  ...['launch', 'roblox', 'perf', 'notify', 'discord', 'appearance', 'security', 'backup', 'updates'].map(p => S(`settings-${p}`, `settings=${p}`)),
  S('settings-discord-on', 'settings=discord&discord=1'),
  S('settings-backup-on', 'settings=backup&backup=1'),
  S('settings-update-ready', 'settings=updates&update=1'),
  S('settings-search', 'settings=1', async s => { await s.type('rejoin', key('set.search')); }),
  S('whats-new', 'settings=updates', async s => { await s.click(key('wn.link')); }),
  S('feedback', 'settings=updates', async s => { await s.click(key('fb.open')); }),
  S('import-vault', 'vimport=pick'),
  S('import-vault-preview', 'vimport=preview', async s => { await s.wait(1500); }),

  // ── Themes ──
  S('theme-light', 'theme=light'),
  ...['paper', 'dim', 'black', 'system'].map(th => S(`theme-${th}`, `theme=${th}`, null, { langs: ['en'] })),

  // ── Window sizes (the stylesheet changes the layout at 1180, 1080, 900 and 720 px) ──
  S('narrow-1100', '', null, { size: [1100, 720] }),
  S('narrow-1000', 'select=1', null, { size: [1000, 700] }),
  S('narrow-860', '', null, { size: [860, 640] }),
  S('narrow-settings', 'settings=perf', null, { size: [860, 640] }),
  S('wide-1600', '', null, { size: [1600, 900], langs: ['en'] }),

  // ── Keyboard and mouse on the account list ──
  S('cards-arrow-keys', '', async s => {
    await s.click({ css: '.card[data-id$="1"]' });
    await s.key('ArrowRight');
    await s.key('ArrowDown');
    await s.key(' ');
  }),
  S('cards-shift-click', '', async s => {
    await s.click({ css: '.card[data-id$="1"]' });
    await s.click({ css: '.card[data-id$="4"]' }, { modifiers: ['shift'] });
    await s.snap('range');
    await s.click({ css: '.card[data-id$="2"]' }, { modifiers: ['ctrl'] });
  }),
  S('cards-select-all-keys', '', async s => { await s.click({ css: '.card[data-id$="1"]' }); await s.key('a', ['ctrl']); }),
  S('cards-escape', 'select=1', async s => { await s.key('Escape'); }),
  S('card-right-click', '', async s => { await s.click({ css: '.card[data-id$="2"]' }, { button: 'right' }); }),
  S('card-f2', '', async s => { await s.click({ css: '.card[data-id$="3"]' }); await s.key('F2'); }),
  S('card-delete-key', 'select=1', async s => {
    await s.key('Delete');
    await s.snap('asked');
    await s.click({ css: '.overlay .btn.danger' });
  }),
  S('card-close-one', '', async s => { await s.click({ key: 'card.close', within: '.card[data-id$="1"]' }); }),
  S('card-relogin', '', async s => { await s.click({ key: 'card.relogin', within: '.card[data-id$="3"]' }); await s.wait(800); }, { langs: ['en'] }),
  S('list-view-keys', '', async s => {
    await s.click(key('layout.list'));
    await s.click({ css: '.row[data-id$="1"], [data-id$="1"]' });
    await s.key('ArrowDown');
    await s.key('ArrowDown', ['shift']);
  }),

  // ── An account's menu: each item ──
  S('menu-copy-user', 'menu=1', async s => { await s.click(key('m.copyUser')); }),
  S('menu-check', 'menu=1', async s => { await s.click(key('m.check')); await s.wait(1000); }),
  S('menu-auto-rejoin', 'menu=1', async s => { await s.click(key('m.autoRejoin')); }),
  S('menu-close', 'menu=1', async s => { await s.click(key('m.close')); }),
  S('menu-delete', 'menu=1', async s => { await s.click({ key: 'm.deleteN', partial: true }); await s.snap('asked'); await s.key('Escape'); }),
  S('menu-website', 'menu=1', async s => { await s.click(key('m.website')); }),
  S('menu-follow', '', async s => {
    await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' });
    await s.click(key('m.follow'));
  }),

  // ── Adding an account ──
  S('add-password-ok', '', async s => {
    await s.click(key('acc.add'));
    await s.fill({ css: '#pw-user' }, 'newfarm');
    await s.fill({ css: '#pw-pass' }, 'hunter2');
    await s.click({ css: '#pw-go' });
    await s.wait(800);
  }),
  S('add-password-wrong', '', async s => {
    await s.click(key('acc.add'));
    await s.fill({ css: '#pw-user' }, 'x');
    await s.fill({ css: '#pw-pass' }, 'nope');
    await s.key('Enter');
    await s.wait(800);
  }),
  S('add-password-2step', '', async s => {
    await s.click(key('acc.add'));
    await s.fill({ css: '#pw-user' }, 'sms');
    await s.fill({ css: '#pw-pass' }, 'hunter2');
    await s.click({ css: '#pw-go' });
    await s.wait(800);
    await s.snap('code asked');
    await s.type('123456');
    await s.key('Enter');
    await s.wait(800);
  }),
  S('add-password-captcha', '', async s => {
    await s.click(key('acc.add'));
    await s.fill({ css: '#pw-user' }, 'captcha');
    await s.fill({ css: '#pw-pass' }, 'hunter2');
    await s.click({ css: '#pw-go' });
    await s.wait(800);
  }, { langs: ['en'] }),
  S('add-phone', '', async s => { await s.click(key('acc.add')); await s.click({ key: 'start.phone', partial: true }); await s.wait(800); }, { langs: ['en'] }),
  S('add-browser', '', async s => { await s.click(key('acc.add')); await s.click(key('ql.browser')); await s.wait(500); }),
  S('add-cookie', '', async s => {
    await s.click(key('acc.add'));
    await s.click(key('start.cookie'));
    await s.snap('form');
    await s.type('_|WARNING:-DO-NOT-SHARE-THIS.--fake');
    await s.key('Enter');
    await s.wait(500);
  }),
  S('add-vng-tab', '', async s => { await s.click(key('acc.add')); await s.click({ text: 'Roblox VNG', css2: '[role=tab]' }); }),

  // ── Editing an account ──
  S('edit-save', 'edit=1', async s => {
    await s.fill({ css: '.overlay input.field' }, 'Main account');
    await s.key('Enter');
    await s.wait(400);
  }),
  S('edit-escape', 'edit=1', async s => { await s.key('Escape'); }),

  // ── The game box, the picker, servers ──
  S('game-type-search', '', async s => {
    await s.fill(key('dock.place'), 'adopt');
    await s.wait(800);
    await s.snap('suggestions');
    await s.key('ArrowDown');
    await s.key('Enter');
    await s.wait(500);
  }),
  S('game-picker-pin', '', async s => {
    await s.click(key('dock.browseTip'));
    await s.click({ css: '[data-act=pin-game]' });
  }),
  S('game-private-link', '', async s => { await s.fill(key('dock.place'), 'https://www.roblox.com/share?code=abc123&type=Server'); await s.wait(600); }),
  S('servers', '', async s => {
    await s.click(key('srv.button'));
    await s.wait(600);
    await s.snap('list');
    await s.click(key('srv.fewest'));
    await s.snap('fewest');
    await s.click(key('srv.more'));
    await s.wait(600);
  }),
  // Roblox's few-lists-a-minute limit: the wait counts down where the list was, and the window keeps its size.
  S('servers-busy', 'srvbusy=1', async s => { await s.click(key('srv.button')); await s.wait(600); }),
  S('servers-sort-switching', '', async s => {
    await s.click(key('srv.button'));
    await s.wait(600);
    for (const k of ['srv.sort.fewest', 'srv.sort.ping', 'srv.sort.fps', 'srv.sort.most']) await s.click(key(k));
    await s.snap('three lists used');
    await s.click(key('srv.sort.fewest'));
    await s.wait(600);
  }),
  S('servers-use', '', async s => {
    await s.click(key('srv.button'));
    await s.wait(600);
    await s.click({ key: 'srv.pick', within: '.overlay' });
  }),
  S('join-player', 'select=1', async s => {
    await s.click({ key: 'jp.button' });
    await s.snap('dialog');
    await s.type('builderman');
    await s.key('Enter');
    await s.wait(1000);
  }),
  S('launch-ask-multi-off', 'multioff=1&select=1', async s => { await s.click({ key: 'dock.play', within: '.dock' }); await s.wait(500); }),
  S('arrange-windows', '', async s => { await s.click(key('dock.arrangeTip')); }),
  S('open-app-only', 'select=1', async s => { await s.click(key('dock.openAppTip')); await s.wait(800); }),
  S('tools-menu', '', async s => { await s.click({ css: '[data-act=tools-menu]' }); }),
  S('preset-menu', '', async s => { await s.click({ css: '[data-act=preset-menu]' }); }),
  S('save-as-preset', 'select=1', async s => {
    await s.click({ css: '[data-act=preset-menu]' });
    await s.click(key('ps.save'));
    await s.snap('form');
    await s.type('Evening farm');
    await s.key('Enter');
    await s.wait(500);
  }),

  // ── Side panel ──
  S('close-all', '', async s => {
    await s.click(key('sys.closeAll'));
    await s.snap('asked');
    await s.click({ css: '.overlay .btn.danger' });
  }),
  S('multi-switch-off', '', async s => { await s.click({ css: '#multi-card .switch' }); await s.snap('asked'); }),
  S('mini-window', '', async s => { await s.click(key('sys.mini')); }),
  S('lock-now', '', async s => {
    await s.click(key('side.lock'));
    await s.click({ css: '.overlay .btn.primary' });
    await s.wait(500);
  }),

  // ── Lock screen and first run ──
  S('unlock-wrong', 'locked=1', async s => { await s.type('wrong-one', key('lock.pw')); await s.key('Enter'); await s.wait(800); }),
  S('unlock-ok', 'locked=1', async s => { await s.type('demo', key('lock.pw')); await s.key('Enter'); await s.wait(2600); }),
  S('first-run-create', 'new=1', async s => {
    await s.type('short', key('lock.pw'));
    await s.snap('weak');
    await s.fill(key('lock.pw'), 'a much longer master password');
    await s.type('a much longer master password', key('lock.pw2'));
    await s.snap('strong');
    await s.key('Enter');
    await s.wait(2600);
  }),
  S('first-run-restore', 'new=1', async s => { await s.click(key('lock.restore')); await s.wait(800); }),

  // ── Quick search ──
  S('palette-go-settings', '', async s => {
    await s.key('k', ['ctrl']);
    await s.type('discord');
    await s.key('Enter');
  }),
  S('palette-account', '', async s => {
    await s.key('k', ['ctrl']);
    await s.type('farm 2');
    await s.key('ArrowDown');
    await s.key('Enter');
  }),
  S('palette-nothing', '', async s => { await s.key('k', ['ctrl']); await s.type('zzzzqqq'); }),

  // ── Presets ──
  S('presets-run', 'view=presets', async s => { await s.click({ text: 'Run: Adopt Me trade', css2: 'button' }); await s.wait(800); }, { langs: ['en'] }),
  S('presets-duplicate', 'view=presets', async s => { await s.click({ css: '.pv-item[data-id="p2"] [data-pv=dup]' }); await s.wait(400); }),
  S('presets-delete', 'view=presets', async s => {
    await s.click({ css: '.pv-item[data-id="p2"] [data-pv=del], .pv-item[data-id="p2"] [data-pv=delete]' });
    await s.snap('asked');
    await s.click({ css: '.overlay .btn.danger' });
  }),
  S('presets-move', 'view=presets', async s => { await s.click({ css: '.pv-item[data-id="p1"] [data-move="1"]' }); }),
  S('presets-search', 'view=presets', async s => { await s.type('adopt', key('pv.search')); }),
  S('presets-new', 'view=presets', async s => { await s.click(key('ps.new')); }),
  S('preset-editor-save', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '.overlay input.field' }, 'Farm Blox Fruits 2');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-schedule-off', 'preset=1', async s => { await s.wait(300); await s.click({ css: '.overlay #ps-sch-on, .overlay [id^=ps-sch] + .switch, .overlay .switch' }); }),

  // ── Statistics ──
  S('stats-period', 'stats=1', async s => { await s.click(key('stats.period.7')); await s.snap('7 days'); await s.click(key('stats.period.30')); }),
  S('stats-activity', 'stats=1', async s => { await s.click(key('alog.title')); await s.wait(400); await s.snap('activity'); await s.click(key('alog.export')); }),

  // ── Settings: what each control does ──
  S('settings-staying-toggles', 'settings=perf', async s => {
    await s.click({ css: 'label:has(#s-antiafk)' });
    await s.click({ css: 'label:has(#s-trim)' });
    await s.choose({ css: '#s-rejoin' }, 'on');
  }),
  S('settings-theme-pick', 'settings=appearance', async s => {
    await s.click({ css: '#s-themes [role=radio]:nth-child(3)' });
    await s.snap('picked');
    await s.key('ArrowRight');
  }),
  S('settings-language', 'settings=appearance', async s => { await s.click({ css: '[data-act=lang][data-lang=vi]' }); }, { langs: ['en'] }),
  S('settings-hide-names', 'settings=appearance', async s => { await s.click({ css: 'label:has(#s-hidenames), #s-hidenames' }); }),
  S('settings-roblox-pick', 'settings=roblox', async s => { await s.choose({ css: '#s-rc-mode' }, 'marked'); await s.snap('marked'); await s.click(key('rc.pick')); }),
  S('settings-launcher', 'settings=launch', async s => { await s.click(key('launch.pickBtn')); }),
  S('settings-discord-paste', 'settings=discord', async s => {
    await s.click(key('dc.paste'));
    await s.snap('dialog');
    await s.type('https://discord.com/api/webhooks/123/fake-token');
    await s.key('Enter');
    await s.wait(500);
  }),
  S('settings-discord-actions', 'settings=discord&discord=1', async s => {
    await s.click(key('dc.test'));
    await s.wait(500);
    await s.snap('tested');
    await s.click(key('dc.remove'));
    await s.snap('remove asked');
    await s.key('Escape');
  }),
  S('settings-discord-pickers', 'settings=discord&discord=1', async s => {
    await s.click({ css: '[data-dc=accounts]' });
    await s.snap('accounts');
    await s.key('Escape');
    await s.click({ css: '[data-dc=parts]' });
  }),
  S('settings-password', 'settings=security', async s => {
    await s.type('wrong', key('set.pwCur'));
    await s.type('new master password', key('set.pwNew'));
    await s.type('new master password', key('set.pwNew2'));
    await s.key('Enter');
    await s.wait(600);
    await s.snap('wrong current');
    await s.fill(key('set.pwCur'), 'demo');
    await s.key('Enter');
    await s.wait(600);
  }),
  S('settings-backup-now', 'settings=backup&backup=1', async s => { await s.click(key('bk.now')); await s.wait(500); }),
  S('settings-backup-restore', 'settings=backup&backup=1', async s => { await s.click(key('bk.restoreBtn')); }),
  S('settings-check-updates', 'settings=updates', async s => { await s.click({ css: '[data-act=check-updates]' }); await s.wait(800); }),
];
