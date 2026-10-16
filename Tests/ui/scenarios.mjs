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
  S('import-vault-same', 'vimport=pick&ivsame=1', async s => { await s.click({ css: '.overlay .modal-foot .btn.primary' }); await s.wait(500); }),
  S('import-vault-wrong', 'vimport=wrong', async s => { await s.wait(2000); }),
  S('import-vault-checks', 'vimport=password', async s => {
    await s.wait(700);
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.snap('no password');
    await s.type('secret', { css: '#iv-pw' });
    await s.click({ css: '#iv-eye' });
    await s.snap('shown');
    await s.click({ css: '#iv-eye' });
  }),
  S('import-vault-lockout', 'vimport=password', async s => {
    await s.wait(700);
    for (let i = 0; i < 3; i++) {
      await s.fill({ css: '#iv-pw' }, 'nope');
      await s.click({ css: '.overlay .modal-foot .btn.primary' });
      await s.wait(900);
    }
    await s.snap('waiting');
    await s.wait(1200);
  }, { langs: ['en'] }),
  S('import-vault-pick', 'vimport=preview', async s => {
    await s.wait(1500);
    await s.click({ key: 'iv.none', within: '.overlay' });
    await s.snap('none');
    await s.click({ css: '#iv-presets' });
    await s.snap('presets only');
    await s.click({ key: 'iv.all', within: '.overlay' });
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(500);
  }),
  S('import-vault-other', 'vimport=preview', async s => {
    await s.wait(1500);
    await s.click({ css: '#iv-other' });
    await s.wait(500);
  }),
  S('import-vault-hidden', 'vimport=preview&hide=1', async s => { await s.wait(1500); }, { langs: ['en'] }),

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
  S('edit-totp', 'edit=1', async s => {
    await s.wait(300);
    await s.click({ css: '.overlay [data-totp=remove]' });
    await s.snap('removed');
    await s.click({ css: '.overlay [data-totp=undo]' });
    await s.snap('undone');
    await s.click({ css: '.overlay [data-totp=remove]' });
    await s.type('abc1');
    await s.wait(500);
    await s.snap('bad key');
    await s.fill({ css: '.overlay [name=authenticator]' }, 'JBSWY3DPEHPK3PXP');
    await s.wait(500);
    await s.snap('good key');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('edit-reveal', 'edit=1', async s => {
    await s.wait(300);
    await s.click({ css: '.overlay [data-reveal=password]' });
    await s.snap('shown');
    await s.click({ css: '.overlay [data-reveal=password]' });
    await s.click({ css: '.overlay [data-totp=copy]' });
  }),
  S('edit-ask-close', 'edit=1', async s => {
    await s.wait(300);
    await s.fill({ css: '.overlay [name=group]' }, 'Farmers');
    await s.key('Escape');
  }),
  S('edit-hidden-names', 'edit=1&hide=1', async s => { await s.wait(300); }, { langs: ['en'] }),
  S('add-cookie-empty', '', async s => {
    await s.click(key('acc.add'));
    await s.click(key('start.cookie'));
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.snap('needs cookie');
    await s.click({ css: '.overlay [data-reveal=cookie]' });
    await s.type('_|WARNING:-x');
    await s.key('Escape');
  }),

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
  S('servers-room', 'select=1', async s => {
    await s.click(key('srv.button'));
    await s.wait(800);
    await s.snap('room for 2');
    await s.click({ css: '.overlay [data-room]' });
    await s.snap('all');
    await s.click({ css: '.overlay [data-random]' });
  }),
  S('servers-friend', 'select=1', async s => {
    await s.click(key('srv.button'));
    await s.wait(800);
    await s.click({ css: '.overlay [data-friend]' });
  }),
  S('servers-no-friends', 'select=1&nofriends=1', async s => { await s.click(key('srv.button')); await s.wait(800); }),
  S('servers-friends-fail', 'select=1&friendsfail=1', async s => { await s.click(key('srv.button')); await s.wait(800); }),
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
  S('palette-keys', '', async s => {
    await s.key('k', ['ctrl']);
    await s.key('ArrowUp');
    await s.snap('wrapped');
    await s.key('PageUp');
    await s.key('PageDown');
    await s.key('Tab');
    await s.snap('paged');
    await s.key('ArrowDown');
  }),
  S('palette-alt', '', async s => {
    await s.key('k', ['ctrl']);
    await s.type('farm 1');
    await s.snap('alts');
    await s.key('Enter', ['ctrl']);
    await s.wait(500);
  }),
  S('palette-mouse', '', async s => {
    await s.key('k', ['ctrl']);
    await s.hover({ css: '.pal-row:nth-child(3)' });
    await s.snap('hovered');
    await s.type('farm 1');
    await s.click({ css: '.pal-row.on .pal-alt[data-alt="1"]' });
  }),
  S('palette-many', 'accounts=40&many=1', async s => { await s.key('k', ['ctrl']); }, { langs: ['en'] }),

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
  S('presets-schedule', 'view=presets', async s => {
    await s.click({ css: 'label:has(.pv-item[data-id="p3"] [data-pv=sched]), .pv-item[data-id="p3"] label.switch' });
    await s.snap('on');
    await s.click({ css: '.pv-item[data-id="p1"] label.switch' });
  }),
  S('presets-close', 'view=presets', async s => { await s.click({ css: '.pv-item[data-id="p1"] [data-pv=close]' }); await s.wait(400); }),
  S('presets-full', 'view=presets&many=1', async s => { await s.click(key('ps.new')); }),
  S('presets-icons', 'view=presets&noicons=1', async s => { await s.wait(600); }),
  S('presets-long', 'view=presets&long=1', null, { langs: ['en'] }),
  S('presets-search-none', 'view=presets', async s => { await s.type('zzzz', key('pv.search')); await s.wait(300); }),
  S('presets-move-top', 'view=presets', async s => { await s.click({ css: '.pv-item[data-id="p2"] [data-move="-1"]' }); }),
  S('presets-drag', 'view=presets', async s => {
    await s.drag({ css: '.pv-item[data-id="p3"] .pv-grip' }, { css: '.pv-item[data-id="p1"]' }, -12);
    await s.snap('dragged up');
    await s.drag({ css: '.pv-item[data-id="p3"] .pv-grip' }, { css: '.pv-item[data-id="p2"]' }, 12);
  }),
  S('preset-editor-save', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '.overlay input.field' }, 'Farm Blox Fruits 2');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-schedule-off', 'preset=1', async s => { await s.wait(300); await s.click({ css: '.overlay #ps-sch-on, .overlay [id^=ps-sch] + .switch, .overlay .switch' }); }),
  S('preset-editor-accounts', 'preset=1', async s => {
    await s.wait(300);
    await s.click({ css: '.overlay .ord-item:nth-child(6) .ord-txt' });
    await s.click({ css: '.overlay .ord-item:nth-child(6) [data-move="-1"]' });
    await s.click({ css: '.overlay .ord-item:nth-child(1) input' });
    await s.snap('changed');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-drag', 'preset=1', async s => {
    await s.wait(300);
    await s.drag({ css: '.overlay .ord-item:nth-child(3) .ord-grip' }, { css: '.overlay .ord-item:nth-child(1)' }, -8);
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-game-search', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '#ps-place' }, 'adopt');
    await s.wait(500);
    await s.snap('results');
    await s.key('ArrowDown');
    await s.key('Enter');
    await s.wait(300);
  }),
  S('preset-editor-game-browse', 'view=presets', async s => {
    await s.click(key('ps.new'));
    await s.wait(300);
    await s.fill({ css: '#ps-place' }, '');
    await s.wait(400);
    await s.snap('lists');
    await s.click({ css: '#ps-pick [data-gtab=popular], #ps-pick [data-gtab]:last-child' });
    await s.click({ css: '#ps-pick .game-row' });
    await s.wait(300);
  }),
  S('preset-editor-game-id', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '#ps-place' }, '920587237');
    await s.wait(700);
  }),
  S('preset-editor-job', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-server' }, 'job');
    await s.snap('job box');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-once', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-sch-mode' }, 'once');
    await s.choose({ css: '#ps-sch-stop' }, 'at');
    await s.snap('once');
    await s.choose({ css: '#ps-sch-mode' }, 'days');
    await s.click({ css: '#ps-sch-days [data-day="6"]' });
    await s.choose({ css: '#ps-sch-stop' }, 'after');
    await s.fill({ css: '#ps-sch-hours' }, '0.3');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(300);
  }),
  S('preset-editor-options', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-profile' }, 'afk');
    await s.choose({ css: '#ps-restart' }, '6');
    await s.choose({ css: '#ps-afk' }, 'off');
    await s.snap('notes');
    await s.choose({ css: '#ps-dc-mode' }, '');
    await s.choose({ css: '#ps-dc-mode' }, 'own');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(300);
  }),
  S('preset-editor-new-discord', 'view=presets', async s => {
    await s.click(key('ps.new'));
    await s.wait(300);
    await s.choose({ css: '#ps-dc-mode' }, 'own');
    await s.snap('asks link');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(300);
  }),
  S('preset-editor-ask-close', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '.overlay input.field' }, 'Something else');
    await s.key('Escape');
    await s.snap('asked');
    await s.key('Escape');
  }),
  S('preset-editor-delete', 'preset=1', async s => {
    await s.wait(300);
    await s.click({ css: '.overlay .modal-foot .btn.danger' });
    await s.snap('asked');
    await s.click({ css: '.overlay:last-of-type .modal-foot .btn.danger' });
    await s.wait(300);
  }),
  S('launch-order-move', 'order=1', async s => {
    await s.click({ css: '.overlay .ord-item:nth-child(2) [data-move="1"]' });
    await s.click({ css: '.overlay .ord-item:nth-child(1) [data-move="-1"]' });
    await s.snap('moved');
    await s.click({ key: 'common.done', within: '.overlay' });
    await s.wait(300);
  }),
  S('launch-order-drag', 'order=1', async s => {
    await s.drag({ css: '.overlay .ord-item:nth-child(4) .ord-grip' }, { css: '.overlay .ord-item:nth-child(1)' }, -8);
    await s.snap('dragged');
    await s.click({ key: 'common.done', within: '.overlay' });
    await s.wait(300);
  }),

  // ── Statistics ──
  S('stats-period', 'stats=1', async s => { await s.click(key('stats.period.7')); await s.snap('7 days'); await s.click(key('stats.period.30')); }),
  S('stats-fail', 'stats=1&statsfail=1', async s => { await s.wait(300); await s.click(key('srv.retry')); await s.wait(300); }),
  S('stats-today', 'stats=1', async s => { await s.click(key('stats.period.1')); await s.wait(300); }),
  S('stats-hidden-names', 'stats=1&hide=1', null, { langs: ['en'] }),
  S('stats-activity-filter', 'stats=1', async s => {
    await s.click(key('alog.title'));
    await s.wait(300);
    await s.choose({ css: '#alog-group' }, 'drops');
    await s.wait(300);
    await s.snap('drops');
    await s.choose({ css: '#alog-account' }, '00000000-0000-4000-8000-000000000001');
    await s.wait(300);
    await s.snap('nothing');
    await s.choose({ css: '#alog-group' }, '');
    await s.wait(300);
  }),
  S('stats-activity-more', 'stats=1&manyactivity=1', async s => {
    await s.click(key('alog.title'));
    await s.wait(300);
    await s.click(key('alog.more'));
    await s.wait(300);
  }, { langs: ['en'] }),
  S('stats-activity-none', 'stats=1&noactivity=1', async s => { await s.click(key('alog.title')); await s.wait(300); }),
  S('stats-activity-fail', 'stats=1&activityfail=1', async s => { await s.click(key('alog.title')); await s.wait(300); await s.click(key('srv.retry')); await s.wait(300); }),
  S('stats-names-flip', 'stats=1', async s => {
    await s.click(key('alog.title'));
    await s.wait(300);
    await s.click({ css: '.nav[data-view=settings]' });
    await s.click({ css: '#view-settings [data-set-page=appearance]' });
    await s.click({ css: 'label:has(#s-hidenames)' });
    await s.click({ css: '.nav[data-view=stats]' });
    await s.wait(400);
  }, { langs: ['en'] }),
  S('palette-export', '', async s => {
    await s.key('k', ['ctrl']);
    await s.type('export');
    await s.key('Enter');
    await s.wait(500);
  }),
  S('stats-activity', 'stats=1', async s => { await s.click(key('alog.title')); await s.wait(400); await s.snap('activity'); await s.click(key('alog.export')); }),

  // ── Settings: what each control does ──
  S('settings-staying-toggles', 'settings=perf', async s => {
    await s.click({ css: 'label:has(#s-antiafk)' });
    await s.click({ css: 'label:has(#s-trim)' });
    await s.choose({ css: '#s-rejoin' }, 'on');
  }),
  S('settings-launch-controls', 'settings=launch', async s => {
    await s.click({ css: 'label:has(#s-rename)' });
    await s.choose({ css: '#s-openas' }, 'tile');
    await s.choose({ css: '#s-winlayout' }, 'small');
    await s.snap('windows');
    await s.choose({ css: '#s-launch' }, 'direct');
    await s.choose({ css: '#s-delay' }, '20');
  }),
  S('settings-launch-multi', 'settings=launch&multioff=1', async s => {
    await s.click({ css: 'label:has(#s-multi)' });
    await s.snap('asked');
    await s.click({ key: 'common.cancel', within: '.modal' });
    await s.snap('declined');
    await s.click({ css: 'label:has(#s-multi)' });
    await s.click({ key: 'set.multiOk', within: '.modal' });
  }),
  S('settings-launch-store', 'settings=launch&store=1&others=1', async s => {
    await s.choose({ css: '#s-launch' }, 'store');
    await s.snap('store');
    await s.choose({ css: '#s-launch' }, 'other:Voidstrap');
  }),
  S('settings-launch-long', 'settings=launch&long=1&notready=1'),
  S('settings-backup-problems', 'settings=backup&backup=1&bkerr=1&weak=1', async s => { await s.click(key('bk.now')); await s.wait(500); }),
  S('settings-backup-use', 'settings=backup', async s => {
    await s.click({ css: '#s-bk-suggest button' });
    await s.snap('used');
    await s.click({ css: 'label:has(#s-bk-on)' });
  }),
  S('settings-backup-pick', 'settings=backup&long=1', async s => { await s.click(key('bk.choose')); }),
  S('settings-roblox-closed', 'settings=roblox&rbxclosed=1', async s => { await s.snap('warned'); await s.click(key('ru.hide')); }),
  S('settings-roblox-changed', 'settings=roblox&rw=changed&rbxold=1'),
  S('settings-roblox-renamed', 'settings=roblox&rw=renamed'),
  S('settings-roblox-worked', 'settings=roblox&rw=worked', null, { langs: ['en'] }),
  S('settings-roblox-unread', 'settings=roblox&rw=none', null, { langs: ['en'] }),
  S('settings-roblox-config', 'settings=roblox', async s => {
    await s.choose({ css: '#s-rc-mode' }, 'all');
    await s.choose({ css: '#s-rc-graphics' }, '5');
    await s.choose({ css: '#s-rc-fps' }, '144');
    await s.choose({ css: '#s-rc-volume' }, '0');
    await s.snap('all');
    await s.choose({ css: '#s-rc-mode' }, 'off');
  }),
  S('settings-theme-keys', 'settings=appearance', async s => {
    await s.click({ css: '#s-themes .swatch.on' });
    await s.key('End');
    await s.snap('last');
    await s.key('Home');
    await s.key('ArrowLeft');
    await s.snap('wrapped');
    await s.click({ css: '#s-themes [data-id=paper]' });
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
  S('settings-discord-nolink', 'settings=discord', async s => { await s.click({ css: 'label:has(#s-dc-on)' }); }),
  S('settings-discord-gone', 'settings=discord&dcgone=1'),
  S('settings-discord-fail', 'settings=discord&discord=1&dcfail=1', async s => { await s.click(key('dc.test')); await s.wait(500); }),
  S('settings-discord-reveal', 'settings=discord&discord=1', async s => {
    await s.click({ css: '#s-dc-reveal' });
    await s.snap('shown');
    await s.click({ css: '#s-dc-reveal' });
  }),
  S('settings-discord-when', 'settings=discord&discord=1', async s => {
    await s.choose({ css: '#s-dc-when' }, 'both');
    await s.snap('both');
    await s.choose({ css: '#s-dc-daily-time' }, '21:30');
    await s.choose({ css: '#s-dc-weekday' }, '3');
    await s.choose({ css: '#s-dc-weekly-time' }, '');
    await s.choose({ css: '#s-dc-days' }, '30');
    await s.click({ css: 'label:has(#s-dc-after)' });
    await s.snap('times');
    await s.choose({ css: '#s-dc-when' }, 'none');
  }),
  S('settings-discord-board', 'settings=discord&discord=1', async s => {
    await s.choose({ css: '#s-dc-minutes' }, '15');
    await s.click({ css: 'label:has(#s-dc-quiet)' });
    await s.choose({ css: '#s-dc-quiet-from' }, '07:00');
    await s.snap('quiet');
    await s.choose({ css: '#s-dc-quiet-to' }, '07:00');
    await s.click({ css: 'label:has(#s-dc-board)' });
  }),
  S('settings-discord-pings', 'settings=discord&discord=1', async s => {
    await s.choose({ css: '[data-ping=summary]' }, 'user');
    await s.snap('asks id');
    await s.type('123');
    await s.key('Enter');
    await s.snap('bad id');
    await s.fill({ css: '[data-ping-id=summary]' }, '123456789012345678');
    await s.key('Enter');
    await s.snap('saved');
    await s.choose({ css: '[data-ping=alerts]' }, 'role');
    await s.click(key('dc.sendAuto'));
    await s.wait(300);
    await s.snap('left empty');
    await s.choose({ css: '[data-ping=alerts]' }, 'here');
  }),
  S('settings-discord-alerts', 'settings=discord&discord=1', async s => {
    await s.click({ css: '#s-dc-kinds summary' });
    await s.click({ css: 'label:has(#s-dc-ev-drop)' });
    await s.choose({ css: '[data-ping=drop]' }, 'everyone');
    await s.snap('one kind');
    await s.click({ css: 'label:has(#s-dc-alerts)' });
    await s.snap('all off');
    await s.click({ css: 'label:has(#s-dc-alerts)' });
  }),
  S('settings-discord-look', 'settings=discord&discord=1', async s => {
    await s.click({ css: '#s-dc-look summary' });
    await s.fill({ css: '#s-dc-name' }, 'Discord farm');
    await s.key('Enter');
    await s.snap('bad name');
    await s.fill({ css: '#s-dc-name' }, '  ');
    await s.key('Enter');
    await s.snap('empty name');
    await s.fill({ css: '#s-dc-avatar' }, 'http://example.com/a.png');
    await s.key('Enter');
    await s.snap('bad avatar');
    await s.choose({ css: '#s-dc-lang' }, 'vi');
    await s.choose({ css: '#s-dc-style' }, 'plain');
    await s.click({ css: 'label:has(#s-dc-hidenames)' });
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
  S('settings-password-checks', 'settings=security', async s => {
    await s.key('Enter', []);
    await s.snap('nothing typed');
    await s.type('demo', key('set.pwCur'));
    await s.type('abc', key('set.pwNew'));
    await s.snap('weak');
    await s.key('Enter');
    await s.snap('short');
    await s.fill(key('set.pwNew'), '      ');
    await s.key('Enter');
    await s.snap('blank');
    await s.fill(key('set.pwNew'), 'demo');
    await s.key('Enter');
    await s.snap('same');
    await s.fill(key('set.pwNew'), 'Longer pass 12!');
    await s.type('Longer pass', key('set.pwNew2'));
    await s.key('Enter');
    await s.snap('mismatch');
    await s.type('x');
  }),
  S('settings-password-kept', 'settings=security', async s => {
    await s.type('Longer pass 12!', key('set.pwNew'));
    await s.click({ css: '#view-settings [data-set-page=appearance]' });
    await s.click({ css: '[data-act=lang][data-lang=vi]' });
    await s.click({ css: '#view-settings [data-set-page=security]' });
    await s.snap('other language');
    await s.click({ css: '.nav[data-view=accounts]' });
    await s.click({ css: '.nav[data-view=settings]' });
  }, { langs: ['en'] }),
  S('settings-backup-now', 'settings=backup&backup=1', async s => { await s.click(key('bk.now')); await s.wait(500); }),
  S('settings-backup-restore', 'settings=backup&backup=1', async s => { await s.click(key('bk.restoreBtn')); }),
  S('settings-check-updates', 'settings=updates', async s => { await s.click({ css: '[data-act=check-updates]' }); await s.wait(800); }),
];
