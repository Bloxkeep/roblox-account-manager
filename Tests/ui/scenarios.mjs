// What the interface checks look at. Each: a name, the preview's URL options (ui/dev/mock.js lists them: ?menu=1, ?settings=
// <page>…) and steps that click and type like a person. Elements are found by their words ({ key: an i18n key }, in
// whichever language the run is in) or by a CSS selector when they have no words. The page is written down at the end,
// and at each s.snap('label').
const S = (name, query, run, extra = {}) => ({ name, query, run, ...extra });
const key = k => ({ key: k });
/** Signs in with the password form (the mock answers by username: x, 2fa, sms, captcha, locked, finish, key). */
const signIn = async (s, user, pass = 'hunter2') => {
  await s.click(key('acc.add'));
  await s.fill({ css: '#pw-user' }, user);
  await s.fill({ css: '#pw-pass' }, pass);
  await s.click({ css: '#pw-go' });
  await s.wait(900);
};

export default [
  // ── Accounts ──
  S('accounts-cards', ''),
  S('accounts-list', '', async s => { await s.click(key('layout.list')); }),
  // A VNG account in the list: its tag whole, and every row's columns still in line.
  S('accounts-list-vng', 'vng=1', async s => { await s.click(key('layout.list')); }),
  S('accounts-select', 'select=1'),
  S('account-menu', 'menu=1'),
  S('settings-rc-own-fps', '', async s => { await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' }); await s.click(key('m.fps')); await s.click({ text: '20 FPS', within: '.menu' }); await s.click({ css: '[data-view="settings"]' }); await s.click({ css: '#view-settings [data-set-page="perf"]' }); await s.wait(200); }),
  S('account-fps-cap', '', async s => { await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' }); await s.click(key('m.fps')); await s.snap('list'); await s.click({ text: '20 FPS', within: '.menu' }); await s.snap('set'); await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' }); }),
  S('account-edit', 'edit=1'),
  S('accounts-empty', 'empty=1'),
  // Getting started: accounts never opened yet. The game step ticks once a game is in the box; Play is the step left.
  S('start-steps', 'fresh=1', async s => {
    await s.fill(key('dock.place'), 'https://www.roblox.com/games/2753915549/Blox-Fruits');
    await s.wait(800);
  }),
  S('start-steps-list', 'fresh=1', async s => { await s.click(key('layout.list')); }),
  S('accounts-kicked', 'kicked=1'),
  S('accounts-kicked-unknown', 'kicked=unknown'),
  // On the way in, as the client's log and Roblox's join answer say it.
  S('accounts-waiting', 'phase=waiting'),
  S('accounts-full', 'phase=full&queue=3'),
  S('accounts-teleporting', 'phase=teleporting'),
  S('accounts-join-failed', 'phase=joinFailed'),
  // A window hidden while its game runs: the eye before its PID shows it again; the account menu hides or shows.
  S('accounts-window-hidden', 'winhidden=1', async s => { await s.click({ css: '[data-act="show-window"]' }); }),
  // A window opened with Bloxkeep's FPS limit: its card says so.
  S('accounts-fps-limit', 'fps=20'),
  // A window left on an older Roblox: said on its card and in the list; a click goes to where it can be updated.
  S('accounts-old-build', 'oldwin=1&rbxold=1'),
  S('accounts-old-build-list', 'oldwin=1&rbxold=1', async s => { await s.click({ css: '.seg button[aria-pressed="false"]' }); }),
  S('accounts-window-hide-menu', '', async s => { await s.click({ css: '.card[data-id="00000000-0000-4000-8000-000000000001"] [data-act="card-menu"]' }); await s.snap('menu'); await s.click(key('m.hideWindow')); }),
  // Roblox holds the join for its bot check: Verify is the card's and the row's main button.
  S('accounts-challenged', 'phase=challenged', async s => { await s.click({ css: '[data-act="bot-check"]' }); }),
  S('accounts-challenged-list', 'phase=challenged', async s => { await s.click(key('layout.list')); }),
  S('accounts-full-list', 'phase=full&queue=3', async s => { await s.click(key('layout.list')); }),
  // The card waiting for auto-rejoin's next try is low in the list: brought into view, as a person would scroll to it.
  S('accounts-rejoin-wait', 'rejoin=1', async s => {
    await s.eval(`document.querySelector('[data-act="cancel-rejoin"]')?.closest('[data-id]')?.scrollIntoView({ block: 'center' })`);
    await s.wait(200);
  }),
  S('accounts-long-names', 'long=1'),
  S('accounts-many', 'accounts=40'),
  // Right after unlocking: each signed-in account's Robux not answered yet, its place kept (cards, then rows).
  S('accounts-robux-wait', 'robuxwait=1'),
  S('accounts-robux-wait-list', 'robuxwait=1', async s => { await s.click(key('layout.list')); }),
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
  S('play-low-commit', 'select=1&lowcommit=1', async s => { await s.click({ key: 'dock.play', within: '.dock' }); await s.wait(500); }),
  S('first-run', 'new=1'),
  S('locked', 'locked=1'),

  // ── Game ──
  // The game box opens the list (the game picture beside it shows only in wider windows than this one).
  S('game-picker', '', async s => { await s.click(key('dock.place')); }),
  S('game-picker-thumb', '', async s => { await s.click(key('dock.browseTip')); }, { size: [1440, 860] }),
  S('launch-order', 'order=1'),

  // ── Side panel ──
  S('multi-roblox-off', 'multioff=1'),
  S('multi-roblox-dialog', '', async s => { await s.click({ css: '.multi, [data-act=multi]' }); }),
  S('no-roblox', 'noroblox=1'),
  // The low-RAM warning comes with a launch: two accounts picked, Play.
  S('low-ram', 'select=1&lowram=1', async s => { await s.click({ key: 'dock.play', within: '.dock' }); await s.wait(500); }),
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

  // ── Macros (Beta) ── Nothing here presses or clicks anything outside the page: the mock answers every macro command.
  // A line per macro in words (running, paused while the PC is used, Vietnamese typing on, stopped, done), Panel, Run, ⋮.
  S('macros', 'macros=1'),
  S('macros-none', 'nomacros=1'),
  S('macros-narrow', 'macros=1', null, { size: [860, 640] }),
  S('macros-new-menu', 'macros=1', async s => { await s.click({ css: '#mc-new' }); }),
  S('macros-more-menu', 'macros=1', async s => { await s.click({ css: '.mc-item[data-id="m4"] [data-mc=more]' }); }),
  // A template opens as a new macro's form; saved, it is in the list.
  S('macros-template', 'nomacros=1', async s => {
    await s.click({ css: '.mc-tile[data-start=template]' });
    await s.snap('menu');
    await s.click({ css: '.menu > button:nth-of-type(1)' });
    await s.snap('form');
    await s.click(key('macro.save'));
    await s.wait(300);
  }),
  // A blank macro saved at once: its name and steps are asked for, each problem a link to its box.
  S('macros-empty-save', 'nomacros=1', async s => {
    await s.click({ css: '.mc-tile[data-start=blank]' });
    await s.click(key('macro.save'));
  }),
  S('macros-run', 'macros=1', async s => { await s.click({ css: '.mc-item[data-id="m3"] [data-mc=run]' }); await s.wait(300); }, { langs: ['en'] }),
  S('macros-stop-all', 'macros=1', async s => { await s.click({ css: '.mc-stop-all' }); await s.wait(200); }, { langs: ['en'] }),
  S('macros-move', 'macros=1', async s => {
    await s.click({ css: '.mc-item[data-id="m1"] [data-mc=more]' });
    await s.click({ key: 'lo.down', within: '.menu' });
  }, { langs: ['en'] }),
  S('macros-panel', 'macros=1', async s => { await s.click({ css: '.mc-item[data-id="m1"] [data-mc=panel]' }); await s.wait(200); }, { langs: ['en'] }),
  S('macros-recording', 'macros=1&macrorec=1'),
  // A failed run says so on its line and marks its icon; Fix, first in its menu, opens the form on the step that stopped.
  S('macros-fix', 'macros=1', async s => {
    await s.click({ css: '.mc-item[data-id="m2"] [data-mc=more]' });
    await s.click({ key: 'macro.fix', within: '.menu' });
    await s.wait(500);
  }, { langs: ['en'] }),
  // The editor: the steps as sentences with their blocks indented, the game with the dots and boxes, the picture's
  // aspect note (recorded on 800×600), and "Player sees".
  S('macro-editor', 'macroedit=m3&macropic=1', async s => { await s.wait(700); }),
  S('macro-editor-narrow', 'macroedit=m2&macropic=1', async s => { await s.wait(700); }, { size: [860, 640] }),
  S('macro-editor-panel', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.eval(`document.querySelector('.mc-sees')?.scrollIntoView({ block: 'start' })`);
    await s.wait(200);
  }),
  S('macro-add-step', 'macroedit=m1', async s => {
    await s.click({ css: '#mc-add' });
    await s.snap('menu');
    await s.click({ key: 'macro.kind.if', within: '.menu' });
  }),
  // Add step: a search box over every kind (in its group); words find a kind by what it does ("delay" finds Wait).
  S('macro-more-steps', 'macroedit=m1', async s => {
    await s.click({ css: '#mc-add' });
    await s.type('delay');
  }, { langs: ['en'] }),
  // If stuck: added last, it turns on starting again (through it) and the stuck watch.
  S('macro-if-stuck', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '#mc-when' });
    await s.click({ css: '#mc-add' });
    await s.click({ key: 'macro.kind.recover', within: '.menu' });
  }),
  // The panel's own settings: a tab, a heading, a switch and a choice added there (an If can read the switch).
  S('macro-panel-tabs', 'macroedit=m4', async s => {
    await s.wait(500);
    for (const key of ['macro.tab.add', 'macro.opt.add.heading', 'macro.opt.add.check', 'macro.opt.add.choice', 'macro.opt.add.list']) {
      await s.click({ css: '#mc-sees-add' });
      await s.click({ key, within: '.menu' });
    }
    await s.eval(`document.querySelector('.mc-sees')?.scrollIntoView({ block: 'start' })`);
    await s.wait(200);
  }),
  // A text step reads any word ticked in a list; a colour step clicks each spot once, as it shows.
  S('macro-list-each', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '#mc-sees-add' });
    await s.click({ key: 'macro.opt.add.list', within: '.menu' });
    const pick = (id, value) => s.eval(`(() => { const el = document.querySelector('.mc-step.open select[id$="${id}"]');
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, ${JSON.stringify(value)} || el.options[1].value);
      el.dispatchEvent(new Event('change', { bubbles: true })); })()`);
    await s.click({ css: '#mc-add' });
    await s.click({ key: 'macro.kind.waitText', within: '.menu' });
    await pick('-words', '');
    await s.wait(200);
    await s.snap('reads a list');
    await s.click({ css: '#mc-add' });
    await s.click({ key: 'macro.kind.clickColour', within: '.menu' });
    await pick('-each', 'each');
    await s.wait(200);
  }),
  // An open step: its sentence with the values in place, and "Player can change" (ticked: on the panel; unticked: gone).
  S('macro-step-open', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-step:nth-child(2) .mc-step-head' });
    await s.snap('open');
    await s.click({ css: '.mc-ticks input' });
  }),
  // Areas: a new one, picked by its name in a step's "where".
  S('macro-areas', 'macroedit=m4', async s => { await s.wait(500); await s.click({ css: '#mc-area-add' }); }, { langs: ['en'] }),
  // Pick on game: a colour clicked on the window (the mock answers the bar's green).
  S('macro-pick', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-step:nth-child(3) .mc-step-head' });
    await s.click({ css: '.mc-step.open .mc-pick' });
    await s.wait(500);
  }, { langs: ['en'] }),
  // Try it: a line per step; the bar isn't there yet (no bite), so it stops at step 4 and says what to do.
  S('macro-try', 'macroedit=m4&macrotry=1', async s => { await s.wait(2000); }),
  // Edit on game: the window shows the dots and boxes; a box dragged there moves here too.
  S('macro-on-game', 'macroedit=m4&macrogame=1&gamedrag=1', async s => { await s.wait(1200); }),
  // The window picture: a step's picture cut from it.
  S('macro-picture', 'macroedit=m2&macropic=1', async s => {
    await s.wait(700);
    await s.click({ css: '.mc-step-head.k-clickImage' });
    await s.click({ css: '.mc-step.open .mc-cut' });
    await s.snap('cutting');
  }, { langs: ['en'] }),
  // Recording from the form: the bar while it runs; stopped, the new steps are added after the old ones on request.
  S('macro-record', 'macroedit=m1', async s => {
    await s.click({ css: '#mc-record' });
    await s.snap('recording');
    await s.click({ key: 'macro.rec.stop', within: '.mc-form' });
    await s.wait(200);
    await s.snap('choice');
    await s.click(key('macro.rec.append'));
  }, { langs: ['en'] }),
  S('macro-card-menu', '', async s => {
    await s.click({ css: '.card[data-id$="1"] [data-act=card-menu]' });
    await s.click(key('macro.runOn'));
  }),
  S('macro-palette', '', async s => { await s.key('k', ['ctrl']); await s.type('macro'); }, { langs: ['en'] }),
  S('macro-palette-panel', 'macros=1', async s => { await s.key('k', ['ctrl']); await s.type('panel'); }, { langs: ['en'] }),
  // A template with a to-do list on top: what is left to set for the game before it can work.
  S('macro-template-todo', 'nomacros=1', async s => {
    await s.click({ css: '.mc-tile[data-start=template]' });
    await s.click({ css: '.menu > button:nth-of-type(3)' });
    await s.wait(300);
  }),

  // ── Game monitor, statistics ──
  S('monitor', 'monitor=1'),
  S('stats', 'stats=1'),
  S('stats-none', 'stats=1&nostats=1'),
  // The app has settled after the loading screen: Statistics' figures and the presets' icons are asked for before
  // either tab is opened.
  S('settled-ready', 'settled=1', async s => { await s.wait(600); }),

  // ── Settings: every page ──
  ...['launch', 'roblox', 'perf', 'notify', 'discord', 'appearance', 'security', 'backup', 'updates'].map(p => S(`settings-${p}`, `settings=${p}`)),
  S('settings-discord-on', 'settings=discord&discord=1'),
  // Time boxes have Bloxkeep's own list: its button opens it and closes it again, a pick goes into the box (and is
  // saved), Escape closes it.
  S('settings-time-pick', 'settings=discord&discord=1', async s => {
    await s.click({ css: '#s-dc-daily-time + .tf-btn' });
    await s.snap('open');
    await s.click({ css: '#s-dc-daily-time + .tf-btn' });
    await s.snap('closed');
    await s.click({ css: '#s-dc-daily-time + .tf-btn' });
    await s.click({ css: '.tf-pop [data-col="m"] button:nth-child(31)' });
    await s.snap('picked');
    await s.key('Escape');
    // From the keyboard: the keys go on inside it, and Enter on an hour picks it, the list staying open.
    await s.click({ css: '#s-dc-daily-time' });
    await s.key('F4');
    await s.wait(100);
    const inside = await s.eval(`!!document.activeElement.closest('.tf-pop')`);
    if (!inside) throw new Error('opened from the keyboard, the focus stayed outside the list');
    await s.eval(`document.querySelector('.tf-pop [data-col="h"] button:nth-child(4)').focus()`);
    await s.key('Enter');
    const after = await s.eval(`[!!document.querySelector('.tf-pop'), document.querySelector('#s-dc-daily-time').value]`);
    if (!after[0] || Number(after[1].slice(0, 2)) % 12 !== 3) throw new Error('Enter on an hour: ' + JSON.stringify(after));
    // Something else scrolling behind it leaves it open.
    await s.eval(`(() => { const d = document.createElement('div'); d.style.cssText = 'position:fixed;left:0;top:0;width:50px;height:50px;overflow:auto'; d.innerHTML = '<div style="height:500px"></div>'; document.body.append(d); d.scrollTop = 40; window.__scroller = d; })()`);
    await s.wait(100);
    if (!await s.eval(`!!document.querySelector('.tf-pop')`)) throw new Error('closed by an unrelated scroll');
    await s.eval(`window.__scroller.remove()`);
    await s.key('Escape');
  }),
  // The custom theme: picked from its swatch (its colours and picture rows show), a picture chosen (the page behind
  // glass), and one with a picture already, on a light background too.
  S('settings-custom-theme', 'settings=appearance', async s => {
    await s.click({ css: '#s-theme-groups [data-group="custom"]' }); await s.click({ css: '.swatch[data-id="custom"]' });
    await s.wait(500);
    await s.snap('picked');
    await s.click({ key: 'ct.pick' });
    await s.wait(500);
  }),
  S('settings-custom-picture', 'settings=appearance&theme=custom&ctpicture=1'),
  S('settings-custom-light', 'settings=appearance&theme=custom&ctbg=f1e9da'),
  S('accounts-custom-theme', 'theme=custom&ctpicture=1'),
  // The colour boxes open Bloxkeep's own picker: a ready colour picked, a code typed, its button closing it again.
  S('settings-color-pick', 'settings=appearance&theme=custom', async s => {
    await s.click({ css: '#s-ct-accent' });
    await s.snap('open');
    await s.click({ css: '.cf-ready [aria-label="#3b82f6"]' });
    await s.fill({ css: '.cf-hex' }, '#ec4899');
    await s.wait(400);
    await s.snap('typed');
    await s.click({ css: '#s-ct-accent' });
    await s.wait(400);
  }),
  // Blurred behind the page; your themes (the one in use marked), one saved by name, one put in use, one deleted.
  S('accounts-custom-blur', 'theme=custom&ctpicture=1&ctblur=12'),
  S('settings-custom-saved', 'settings=appearance&theme=custom&ctpicture=1&ctblur=8&saved=1', async s => {
    await s.hover({ css: '#s-saved-themes' });
  }),
  S('settings-custom-save', 'settings=appearance&theme=custom&saved=1', async s => {
    await s.type('Ocean', { css: '#s-ct-name' });
    await s.click({ css: '#s-ct-save' });
    await s.wait(400);
  }),
  S('settings-saved-use', 'settings=appearance&theme=custom&saved=1', async s => {
    await s.click({ css: '.ct-theme[data-id="t2"] .ct-use' });
    await s.wait(500);
    await s.snap('used');
    await s.click({ css: '.ct-theme[data-id="t1"] .ct-del' });
    await s.snap('asked');
    await s.click({ css: '.ct-theme[data-id="t1"] .btn.danger' });
    await s.wait(300);
  }),
  S('settings-saved-none', 'settings=appearance'),
  S('settings-backup-on', 'settings=backup&backup=1'),
  S('settings-update-ready', 'settings=updates&update=1'),
  S('settings-update-quiet', 'settings=updates&update=quiet'),
  S('settings-updates-beta', 'settings=updates', async s => { await s.click({ css: 'label.switch:has(#s-betaupd)' }); }),
  S('settings-updates-online', 'settings=updates', async s => { await s.click({ css: 'label.switch:has(#s-online)' }); }),
  // Bloxkeep's Discord server: the row in Help & feedback (opens the bot's /discord).
  S('settings-help-discord', 'settings=updates', async s => { await s.click({ css: '#sec-feedback [data-act="open-discord"]' }); }),
  S('settings-updates-discord', 'settings=updates', async s => { await s.click(key('dc.open')); }),
  S('settings-search', 'settings=1', async s => { await s.type('rejoin', key('set.search')); }),
  S('whats-new', 'settings=updates', async s => { await s.click(key('wn.link')); }),
  // A game's page from its name under the game box; fresh from an update; before Bloxkeep has seen its quiet hours.
  S('game-page', '', async s => { await s.click({ css: '.dock-info .gname-link' }); await s.wait(400); }),
  S('game-page-updated', 'gpnew=1', async s => { await s.click({ css: '.dock-info .gname-link' }); await s.wait(400); }),
  S('game-page-no-hours', 'gpempty=1', async s => { await s.click({ css: '.dock-info .gname-link' }); await s.wait(400); }),
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
  // Play on a card: it opens, and the focus follows (the card, once Play has turned into Close).
  S('card-play-focus', '', async s => {
    await s.click({ key: 'card.play', within: '.card[data-id$="5"]' });
    await s.wait(1500);
  }),
  S('accounts-group-pick', '', async s => {
    await s.click({ css: '#group-btn' });
    await s.snap('menu');
    await s.click({ text: 'Farm', within: '.menu' });
    await s.snap('farm');
    await s.click(key('chip.ingame'));
  }),
  // A sort with a card focused: the cards move and the focus stays on it.
  S('accounts-sort-status', '', async s => {
    await s.click({ css: '.card[data-id$="5"]' });
    await s.click({ css: '#sort-btn' });
    await s.click({ key: 'sort.status', within: '.menu' });
    await s.snap('by status');
    await s.click({ css: '#sort-btn' });
    await s.click({ key: 'sort.recent', within: '.menu' });
  }),
  S('accounts-search-none', 'select=1', async s => {
    await s.type('zzz', key('acc.search'));
    await s.wait(300);
    await s.snap('nothing');
    await s.click(key('nores.clear'));
  }),
  S('accounts-layout-switch', '', async s => {
    await s.click(key('layout.list'));
    await s.snap('list');
    await s.click(key('layout.cards'));
  }),
  // The list is one Tab stop: back from the game box, the chosen card's buttons, then the card.
  S('cards-tab-stop', 'select=1', async s => {
    await s.click(key('dock.place'));
    await s.key('Tab', ['shift']);
    await s.snap('back from the game box');
    await s.key('Tab', ['shift']);
    await s.key('Tab', ['shift']);
    await s.key('Tab', ['shift']);
  }),
  S('cards-double-click', '', async s => { await s.click({ css: '.card[data-id$="4"] .name' }, { double: true }); await s.wait(1500); }),
  S('dock-recent-pick', '', async s => { await s.click({ css: '.recent-pick' }); await s.wait(800); }),
  S('game-picker-keys', '', async s => {
    await s.fill(key('dock.place'), '');
    await s.wait(600);
    await s.snap('lists');
    await s.key('ArrowDown');
    await s.key('ArrowDown');
    await s.snap('second');
    await s.key('Enter');
    await s.wait(800);
  }),
  S('game-picker-tabs', '', async s => {
    await s.click(key('dock.place'));
    await s.wait(600);
    await s.click({ key: 'picker.popular', within: '#picker' });
    await s.snap('popular');
    await s.key('Escape');
  }),
  // A game already in the box: a click opens the list anyway, with the box's text selected, so typing a name replaces
  // the link and searches instead of being added to it.
  S('game-box-with-game', '', async s => {
    await s.click(key('dock.place'));
    await s.wait(600);
    await s.snap('tabs');
    await s.type('adopt');
    await s.wait(800);
  }),
  // Saved servers: the Saved tab, picking one with a code (the game goes in the box, the code is copied), and saving a
  // private server's link, then a game with a code.
  S('saved-servers', '', async s => {
    await s.click(key('dock.place'));
    await s.wait(600);
    await s.click({ key: 'picker.saved', within: '#picker' });
    await s.snap('tab');
    await s.click({ css: '.game-row.saved[data-i="1"]' });
    await s.wait(800);
  }),
  S('save-server-link', '', async s => {
    await s.fill(key('dock.place'), 'https://www.roblox.com/games/2753915549/Blox-Fruits?privateServerLinkCode=99999999999999999999');
    await s.wait(900);
    await s.click(key('sv.save'));
    await s.wait(300);
    await s.snap('dialog');
    await s.click({ key: 'sv.saveBtn', within: '.modal' });
    await s.wait(400);
  }),
  S('save-server-code', '', async s => {
    await s.click(key('sv.save'));
    await s.wait(300);
    await s.click({ key: 'sv.saveBtn', within: '.modal' });
    await s.snap('no code');
    await s.fill({ css: '#sv-code' }, 'ZX81');
    await s.click({ key: 'sv.saveBtn', within: '.modal' });
    await s.wait(400);
  }),
  S('game-bad-link', '', async s => { await s.fill(key('dock.place'), 'https://www.roblox.com/games/abc'); await s.wait(800); }),
  S('game-box-keys', '', async s => {
    await s.click({ css: '.card[data-id$="4"]' });
    await s.key('/');
    await s.snap('slash');
    await s.key('Escape');
    await s.key('l', ['ctrl']);
    await s.snap('ctrl+l');
    await s.key('f', ['ctrl']);
  }),

  // ── An account's menu: each item ──
  // Copy username is in Copy's own menu, offered on one account (not with several selected).
  S('menu-copy-user', '', async s => {
    await s.click({ css: '.card[data-id$="4"] [data-act=card-menu]' });
    await s.click(key('m.copy'));
    await s.click(key('m.copyUser'));
  }),
  S('menu-check', 'menu=1', async s => { await s.click(key('m.check')); await s.wait(1000); }),
  S('menu-auto-rejoin', 'menu=1', async s => { await s.click(key('m.autoRejoin')); }),
  S('menu-close', 'menu=1', async s => { await s.click(key('m.close')); }),
  S('menu-delete', 'menu=1', async s => { await s.click({ key: 'm.deleteN', partial: true }); await s.snap('asked'); await s.key('Escape'); }),
  // Website and Join this server are about one account, so not offered with several selected.
  S('menu-website', '', async s => {
    await s.click({ css: '.card[data-id$="4"] [data-act=card-menu]' });
    await s.click(key('m.website'));
  }),
  // Join this server: only on an account whose server is known (in game, here from Roblox's presence), which fills the
  // game box with that server.
  S('menu-follow', '', async s => {
    await s.click({ css: '.card[data-id$="4"] [data-act=card-menu]' });
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
  // The password sign-in's other turns (ui/dev/mock.js answers by username: x, 2fa, sms, captcha, locked, finish, key).
  S('pw-empty', '', async s => { await s.click(key('acc.add')); await s.click({ css: '#pw-go' }); }),
  S('pw-realm-keeps', '', async s => {
    await s.click(key('acc.add'));
    await s.fill({ css: '#pw-user' }, 'typedname');
    await s.fill({ css: '#pw-pass' }, 'typedpass');
    await s.click({ css: '.overlay [data-realm=vng]' });
    await s.snap('vng');
    await s.click({ css: '#pw-eye' });
    await s.snap('shown');
    await s.click({ css: '.overlay [data-realm=""]' });
  }),
  S('pw-autofill', '', async s => {
    await signIn(s, '2fa');
    await s.snap('filled');
    await s.click({ css: '.overlay [data-m=email]' });
    await s.wait(600);
    await s.snap('email');
    await s.click({ css: '.overlay [data-m=authenticator]' });
    await s.wait(400);
    await s.snap('back');
    await s.click({ css: '#pw-verify' });
    await s.wait(700);
  }),
  S('pw-code-checks', '', async s => {
    await signIn(s, 'sms');
    await s.type('12 3-45');
    await s.snap('digits only');
    await s.key('Enter');
    await s.wait(700);
    await s.snap('invalid');
    await s.fill({ css: '#pw-otp' }, '000000');
    await s.key('Enter');
    await s.wait(700);
    await s.snap('too many');
    await s.click({ css: '.overlay [data-back]' });
  }),
  S('pw-locked', '', async s => { await signIn(s, 'locked'); await s.click({ css: '#pw-solve' }); await s.wait(1000); }),
  S('pw-captcha-solved', '', async s => { await signIn(s, 'captcha'); await s.click({ css: '#pw-solve' }); await s.wait(1000); }, { langs: ['en'] }),
  S('pw-finish', '', async s => { await signIn(s, 'finish'); await s.snap('try again'); await s.click({ css: '#pw-finish' }); await s.wait(800); }),
  S('pw-key', '', async s => { await signIn(s, 'key'); }),
  S('pw-two-added', '', async s => {
    await signIn(s, 'firstfarm');
    await s.wait(4000); // the first "Added" toast gone (it would be close to its end either way)
    await s.fill({ css: '#pw-user' }, 'secondfarm');
    await s.fill({ css: '#pw-pass' }, 'hunter2');
    await s.click({ css: '#pw-go' });
    await s.wait(900);
    await s.snap('two added');
    await s.key('Escape');
    await s.wait(400);
  }, { langs: ['en'] }),
  S('pw-ask-close', '', async s => { await s.click(key('acc.add')); await s.fill({ css: '#pw-user' }, 'half'); await s.key('Escape'); }),
  ...['cookie', 'vault', 'phone'].map(way => S(`pw-way-${way}`, '', async s => { await s.click(key('acc.add')); await s.click({ css: `.overlay [data-x2=${way}]` }); await s.wait(900); }, { langs: ['en'] })),
  S('add-phone', '', async s => { await s.click(key('acc.add')); await s.click({ key: 'start.phone', partial: true }); await s.wait(800); }, { langs: ['en'] }),
  // Quick Login (the phone): the mock answers the polls (every 2.5 s) with ?ql=…, one each, and stays on the last.
  ...[['ql-linked', 'linked'], ['ql-adding', 'adding']].map(([name, ql]) => S(name, `ql=${ql}`, async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(3000);
  }, { langs: ['en'] })),
  S('ql-added', 'ql=added,waiting', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(3000);
    await s.snap('added');
    await s.click({ key: 'common.done', within: '.overlay' });
    await s.wait(300);
  }),
  S('ql-cancelled', 'ql=cancelled,waiting', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(3500);
    await s.snap('declined');
    await s.click({ css: '.overlay [data-new]' });
    await s.wait(900);
  }),
  S('ql-expired', 'ql=expired', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(6000);
  }),
  S('ql-error', 'ql=error', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(3500);
  }, { langs: ['en'] }),
  S('ql-no-code', 'qlfail=1', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(900);
    await s.snap('no code');
    await s.click({ css: '.overlay [data-new]' });
    await s.wait(900);
  }),
  S('ql-vng', '', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(900);
    await s.click({ css: '.overlay [data-realm=vng]' });
    await s.wait(900);
  }),
  // On the Roblox VNG tab, the code after an account came in is a VNG one too (the renewal asked for a Roblox code while
  // the VNG tab stayed picked; review, 2026-10-30): both quickLoginStart commands say realm "vng".
  // The tab is switched before the first poll (2.5 s): s.click would wait for the polls to settle first.
  S('ql-vng-renew', 'ql=added,waiting', async s => {
    await s.click(key('acc.add'));
    await s.eval(`(async () => {
      __ui.find({ key: 'start.phone', partial: true }).click();
      await new Promise(r => setTimeout(r, 900));
      document.querySelector('.overlay [data-realm=vng]').click();
    })()`);
    await s.wait(3800);
  }, { langs: ['en'] }),
  S('ql-again', '', async s => {
    await s.click(key('chip.invalid'));
    await s.click({ css: '[data-act=relogin]' });
    await s.wait(900);
  }, { langs: ['en'] }),
  S('add-browser', '', async s => { await s.click(key('acc.add')); await s.click({ css: '.overlay [data-realm=""]' }); await s.click(key('pw.withBrowser')); await s.wait(500); }),
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
  // The group box offers the other groups in Bloxkeep's own list (it was the browser's): typed letters narrow it, the
  // arrows and Enter take one (not Save), Escape closes the list and leaves the form open.
  S('edit-group-suggest', 'edit=1', async s => {
    await s.wait(300);
    await s.fill({ css: '.overlay [name=group]' }, '');
    await s.snap('all groups');
    await s.type('ar');
    await s.key('ArrowDown');
    await s.key('Enter');
    await s.snap('picked');
    await s.fill({ css: '.overlay [name=group]' }, 'a');
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
    await s.click(key('dock.place'));
    await s.click({ css: '[data-act=pin-game]' });
  }),
  S('game-private-link', '', async s => { await s.fill(key('dock.place'), 'https://www.roblox.com/share?code=abc123&type=Server'); await s.wait(600); }),
  // Your private servers: one both selected accounts can join, one only Farm 1 can (said on its row, in the warning
  // colour). Picking one puts it in the game box as a private server (roblox://placeId=…&accessCode=…).
  S('servers-private', 'select=1', async s => {
    await s.click(key('dock.servers'));
    await s.wait(900);
    await s.snap('list');
    await s.click({ css: '[data-private="0"]' });
    await s.wait(700);
  }),
  S('servers-private-none', 'noprivate=1', async s => { await s.click(key('dock.servers')); await s.wait(900); }),
  S('servers-private-fail', 'privatefail=1', async s => { await s.click(key('dock.servers')); await s.wait(900); }),
  S('servers', '', async s => {
    await s.click(key('dock.servers'));
    await s.wait(600);
    await s.snap('list');
    await s.click(key('srv.sort.fewest'));
    await s.snap('fewest');
    await s.click(key('srv.more'));
    await s.wait(600);
  }),
  // Roblox's few-lists-a-minute limit: the wait counts down where the list was, and the window keeps its size.
  S('servers-busy', 'srvbusy=1', async s => { await s.click(key('dock.servers')); await s.wait(600); }),
  S('servers-sort-switching', '', async s => {
    await s.click(key('dock.servers'));
    await s.wait(600);
    for (const k of ['srv.sort.fewest', 'srv.sort.ping', 'srv.sort.fps', 'srv.sort.most']) await s.click(key(k));
    await s.snap('three lists used');
    await s.click(key('srv.sort.fewest'));
    await s.wait(600);
  }),
  S('servers-use', '', async s => {
    await s.click(key('dock.servers'));
    await s.wait(600);
    await s.click({ key: 'srv.pick', within: '.overlay' });
  }),
  S('servers-room', 'select=1', async s => {
    await s.click(key('dock.servers'));
    await s.wait(800);
    await s.snap('room for 2');
    await s.click({ css: '.overlay [data-room]' });
    await s.snap('all');
    await s.click({ css: '.overlay [data-random]' });
  }),
  S('servers-friend', 'select=1', async s => {
    await s.click(key('dock.servers'));
    await s.wait(800);
    await s.click({ css: '.overlay [data-friend]' });
  }),
  S('servers-no-friends', 'select=1&nofriends=1', async s => { await s.click(key('dock.servers')); await s.wait(800); }),
  S('servers-friends-fail', 'select=1&friendsfail=1', async s => { await s.click(key('dock.servers')); await s.wait(800); }),
  S('join-player', 'select=1', async s => {
    await s.click({ key: 'jp.button' });
    await s.snap('dialog');
    await s.type('builderman');
    await s.key('Enter');
    await s.wait(1000);
  }),
  // What the lookup says when it can't join: the name in bold (hidden, offline, nobody by that name), a name Roblox
  // can't have, and a lookup that fails.
  S('join-player-answers', 'select=1', async s => {
    await s.click({ key: 'jp.button' });
    await s.type('hiddenfriend');
    await s.key('Enter');
    await s.wait(800);
    await s.snap('hidden');
    await s.fill({ css: '#jp-name' }, 'offlinebuddy');
    await s.key('Enter');
    await s.wait(800);
    await s.snap('offline');
    await s.fill({ css: '#jp-name' }, 'nobody_here');
    await s.key('Enter');
    await s.wait(800);
    await s.snap('not found');
    await s.fill({ css: '#jp-name' }, 'not a name!');
    await s.key('Enter');
    await s.wait(800);
  }),
  S('launch-ask-multi-off', 'multioff=1&select=1', async s => { await s.click({ key: 'dock.play', within: '.dock' }); await s.wait(500); }),
  S('dock-more', 'select=1', async s => { await s.click({ key: 'dock.more', within: '.dock' }); }),
  // A second click on a menu's button closes the menu (it used to close and open it again at once).
  S('menu-click-twice', 'select=1', async s => {
    await s.click({ key: 'dock.more', within: '.dock' });
    await s.snap('open');
    await s.click({ key: 'dock.more', within: '.dock' });
  }),
  S('card-menu-click-twice', '', async s => {
    await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' });
    await s.snap('open');
    await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' });
  }),
  // The Windows menu's layouts as pictures: pointing at 4 × 4 lights the cells the open windows would take; picking 2 × 2
  // keeps it and arranges them.
  S('arrange-windows', '', async s => {
    await s.click({ key: 'dock.more', within: '.dock' });
    await s.hover({ css: '.menu [data-layout="4x4"]' });
    await s.snap('pointed');
    await s.click({ css: '.menu [data-layout="2x2"]' });
  }),
  S('open-app-only', 'select=1', async s => { await s.click({ key: 'dock.more', within: '.dock' }); await s.click(key('dock.openAppTip')); await s.wait(800); }),
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
  // The tray icon's quick panel and menu (ui/src/Tray.tsx), on their own page with a fake app (ui/dev/tray-mock.js).
  S('tray-panel', '', null, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-panel-locked', 'locked=1', null, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-panel-many', 'many=1&multi=notReady', null, { page: 'tray-preview.html', size: [420, 640] }),
  S('tray-panel-play', '', async s => { await s.click(key('tray.play')); }, { page: 'tray-preview.html', size: [420, 640] }),
  S('tray-panel-close-all', '', async s => { await s.click(key('tray.closeAll')); await s.snap('asked'); await s.click({ css: '.tray-link' }); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-panel-multi', '', async s => { await s.click({ css: '.tray-multi' }); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-menu', 'mode=menu', null, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-menu-play', 'mode=menu', async s => { await s.click(key('tray.playAs')); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-menu-macros', 'mode=menu&macros=1', async s => { await s.click(key('tray.macros')); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-menu-multi', 'mode=menu', async s => { await s.click({ css: '[data-item=multi]' }); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('tray-menu-close', 'mode=menu', async s => { await s.click(key('tray.closeAllRoblox')); await s.click({ css: '.tray-item.danger' }); }, { page: 'tray-preview.html', size: [420, 520] }),
  S('unlock-ok', 'locked=1', async s => { await s.type('demo', key('lock.pw')); await s.key('Enter'); await s.wait(2600); }),
  // An update found before the unlock: the lock screen gives it its place (downloading, then installing), and its link
  // gives the form back. Found while a password is being typed, the form stays.
  S('lock-update', 'locked=1&lockupdate=40', async s => { await s.wait(300); }),
  S('lock-update-ready', 'locked=1&lockupdate=ready', async s => { await s.wait(300); }),
  S('lock-update-later', 'locked=1&lockupdate=40', async s => { await s.wait(300); await s.click(key('lock.upd.later')); await s.wait(300); }),
  S('lock-update-typed', 'locked=1&lockupdatein=5000', async s => { await s.type('dem', key('lock.pw')); await s.wait(5600); }),
  // Several vaults: the picker (Steam-like) at a start that asks; picking one asks for its password; adding one asks for
  // a name and a new master password and opens it; Switch vault from the sidebar; one taken off the list.
  S('vault-picker', 'locked=1&vaults=1&vaultpick=1'),
  S('vault-pick-one', 'locked=1&vaults=1&vaultpick=1', async s => { await s.click({ css: '.vp-tile[data-id="b2c3d4e5"]' }); await s.wait(400); }),
  S('vault-add', 'locked=1&vaults=1&vaultpick=1', async s => {
    await s.click({ css: '.vp-add' });
    await s.wait(400);
    await s.snap('new');
    await s.fill({ css: '#vault-name' }, 'Trade');
    await s.type('a long new password', { css: '#pw1' });
    await s.type('a long new password', { css: '#pw2' });
    await s.key('Enter');
    await s.wait(2600);
  }),
  S('vault-new-from-lock', 'locked=1', async s => { await s.click(key('vault.new')); await s.wait(400); }),
  S('vault-switch-sidebar', 'vaults=1', async s => { await s.click({ css: '.vault-btn' }); await s.wait(400); }),
  // Switch vault over the open vault: Back returns to it as it was, no password asked.
  S('vault-switch-back', 'vaults=1', async s => {
    await s.click({ css: '.vault-btn' });
    await s.wait(400);
    await s.click({ key: 'vault.back' });
    await s.wait(400);
  }),
  S('vault-remove', 'locked=1&vaults=1&vaultpick=1', async s => {
    await s.click({ css: '.vp-remove[data-id="c3d4e5f6"]' });
    await s.wait(300);
    await s.snap('asked');
    await s.click({ key: 'vault.removeOk', within: '.modal' });
    await s.wait(400);
  }),
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
  // Multi-Roblox only, for someone who signs in to Roblox in their own browser: asked like the sidebar's switch, then
  // the first screen says it is on, and it turns off again from there.
  S('first-run-multi-only', 'new=1', async s => {
    await s.click(key('lock.multiOnly'));
    await s.snap('asked');
    await s.click(key('set.multiOk'));
    await s.snap('on');
    await s.click(key('lock.multiOnlyOff'));
  }),
  S('unlock-checks', 'locked=1', async s => {
    await s.key('Enter');
    await s.snap('nothing typed');
    await s.type('x', key('lock.pw'));
    await s.snap('typing again');
    await s.click({ css: '#pw-eye' });
    await s.snap('shown');
    await s.click({ css: '#pw-eye' });
  }),
  S('first-run-checks', 'new=1', async s => {
    await s.key('Enter');
    await s.snap('too short');
    await s.type('      ', key('lock.pw'));
    await s.type('      ', key('lock.pw2'));
    await s.key('Enter');
    await s.snap('blank');
    await s.fill(key('lock.pw'), 'Abcdef12');
    await s.fill(key('lock.pw2'), 'Abcdef13');
    await s.key('Enter');
    await s.snap('mismatch');
    await s.click({ css: '#pw-eye' });
    await s.snap('shown');
    await s.fill(key('lock.pw'), '');
  }),
  // Another language while typing: the screen is drawn again in it, what was typed and its strength kept.
  S('first-run-language', 'new=1', async s => {
    await s.type('Abcdef12!x', key('lock.pw'));
    await s.type('Abc', key('lock.pw2'));
    await s.click({ css: '[data-act=lang][data-lang=vi]' });
  }, { langs: ['en'] }),
  S('unlock-language', 'locked=1', async s => {
    await s.type('half typed', key('lock.pw'));
    await s.click({ css: '[data-act=lang][data-lang=vi]' });
  }, { langs: ['en'] }),

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
  // "Don't ask again" ticked, then a quick double click on another preset's Delete: one deletePreset, not two (the second
  // asked for a preset already gone and showed an error; review, 2026-10-30).
  S('presets-delete-twice', 'view=presets', async s => {
    await s.click({ css: '.pv-item[data-id="p2"] [data-pv=del]' });
    await s.click({ css: '.overlay #dont-ask' });
    await s.click({ css: '.overlay .btn.danger' });
    await s.wait(300);
    await s.eval(`(() => { const b = document.querySelector('.pv-item[data-id="p1"] [data-pv=del]'); b.click(); b.click(); })()`);
    await s.wait(300);
  }),
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
  // Rejoin server: a dropped window rejoins another account's server; its note shows, and Save sends it.
  S('preset-editor-rejoin-with', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-rejoin-with' }, 'any');
    await s.snap('with another account');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  S('preset-editor-job', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-server' }, 'job');
    await s.snap('job box');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.wait(400);
  }),
  // A drop-down's list in the page's colours, under its box; clicked again, it closes.
  S('select-open', 'preset=1', async s => {
    await s.wait(300);
    await s.click({ css: '#ps-sch-stop' });
    await s.snap('open');
    await s.click({ css: '#ps-sch-stop' });
  }),
  // The date box's own calendar: a day picked goes into the box; the time box's list closes on Escape, the form stays.
  S('preset-date-pick', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-sch-mode' }, 'once');
    await s.click({ css: '#ps-sch-date + .tf-btn' });
    await s.snap('calendar');
    await s.click({ css: '.tf-pop .tf-cal-head [aria-label]:last-child' });
    await s.click({ css: '.tf-pop .tf-cal-grid button:nth-of-type(15)' });
    await s.snap('picked');
    await s.click({ css: '#ps-sch-time + .tf-btn' });
    await s.snap('time');
    await s.key('Escape');
  }),
  // The calendar stays by its button: the form scrolled under it, and a month of six weeks (taller than the one it
  // opened on) still inside the window. It used to stay where it was first put (review, 2026-10-30).
  S('preset-date-follows', 'preset=1', async s => {
    const by = () => s.eval(`(() => {
      const b = document.querySelector('#ps-sch-date + .tf-btn').getBoundingClientRect(), p = document.querySelector('.tf-pop').getBoundingClientRect();
      const gap = Math.min(Math.abs(p.top - b.bottom - 6), Math.abs(b.top - 6 - p.bottom));
      return gap <= 1 && p.top >= 8 && p.bottom <= innerHeight - 8 ? 'by its button' : 'off: gap ' + gap + ', ' + Math.round(p.top) + '–' + Math.round(p.bottom) + ' of ' + innerHeight;
    })()`);
    await s.wait(300);
    await s.choose({ css: '#ps-sch-mode' }, 'once');
    await s.click({ css: '#ps-sch-date + .tf-btn' });
    await s.eval(`document.querySelector('.overlay .modal-body').scrollTop -= 40`);
    await s.wait(200);
    const scrolled = await by();
    // August 2026 has six weeks however the week starts; the fixed date is in October.
    await s.click({ css: '.tf-pop .tf-cal-head [aria-label]:first-child' });
    await s.click({ css: '.tf-pop .tf-cal-head [aria-label]:first-child' });
    await s.wait(200);
    const taller = await by();
    if (scrolled !== 'by its button' || taller !== 'by its button') throw new Error(`calendar after scrolling: ${scrolled}; six weeks: ${taller}`);
    await s.snap('six weeks');
    await s.key('Escape');
  }),
  // The player to follow: the names looked up before, in Bloxkeep's own list (it was the browser's): a click opens it and
  // a second one closes it, arrows and Enter take one, Escape closes the list and not the form.
  S('preset-follow-suggest', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-server' }, 'follow');
    await s.click({ css: '#ps-follow' });
    await s.snap('open');
    await s.click({ css: '#ps-follow' });
    await s.snap('closed');
    await s.type('friend', { css: '#ps-follow' });
    await s.key('ArrowDown');
    await s.key('ArrowDown');
    await s.snap('second');
    await s.key('Enter');
    await s.snap('picked');
    await s.fill({ css: '#ps-follow' }, 'b');
    await s.key('Escape');
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
    await s.click({ css: '#ps-dc > summary' }); // Discord is folded until something in it is set
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
  // 7 days: an account's card opens its own timeline; with no records from before, no "vs" line.
  S('stats-account-open', 'stats=1', async s => { await s.click(key('stats.period.7')); await s.wait(300); await s.click({ css: '.sa-card.opens' }); await s.wait(200); }),
  // A box on the games' map: the page counts that game only, the map keeps every game to pick another.
  S('stats-game-pick', 'stats=1', async s => { await s.click(key('stats.period.30')); await s.wait(300); await s.click({ css: '.gm button:nth-child(2)' }); await s.wait(400); }),
  S('stats-no-before', 'stats=1&noprev=1', async s => { await s.click(key('stats.period.7')); await s.wait(300); }, { langs: ['en'] }),
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
    await s.choose({ css: '#s-ramlimit' }, '1024');
    await s.click({ css: 'label:has(#s-rejoin)' });
  }),
  S('settings-launch-controls', 'settings=launch', async s => {
    await s.click({ css: 'label:has(#s-rename)' });
    await s.choose({ css: '#s-openas' }, 'tile');
    await s.click({ css: '#s-winlayout [data-layout="3x3"]' });
    await s.snap('windows');
    // A size of one's own: its two boxes, saved once left.
    await s.click({ css: '#s-winlayout [data-layout="custom"]' });
    await s.fill({ css: '#s-winw' }, '320');
    await s.fill({ css: '#s-winh' }, '240');
    await s.click({ css: '#s-winw' });
    await s.snap('own size');
    // Or one of the common sizes.
    await s.choose({ css: '#s-winsize' }, '1280x720');
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
  // Roblox VNG on this PC: its own option, chosen and saved.
  S('settings-launch-vng', 'settings=launch&vng=1', async s => { await s.choose({ css: '#s-launch' }, 'vng'); }),
  S('settings-launch-long', 'settings=launch&long=1&notready=1'),
  // Hotkeys: on, one taken by another app said under it; turned off.
  S('settings-hotkeys', 'settings=updates&keystaken=1', async s => { await s.snap('one taken'); await s.click({ css: '#s-hotkeys + span, label:has(#s-hotkeys)' }); }),
  S('settings-backup-problems', 'settings=backup&backup=1&bkerr=1&weak=1', async s => { await s.click(key('bk.now')); await s.wait(500); }),
  S('settings-backup-use', 'settings=backup', async s => {
    await s.click({ css: '#s-bk-suggest button' });
    await s.snap('used');
    await s.click({ css: 'label:has(#s-bk-on)' });
  }),
  S('settings-backup-pick', 'settings=backup&long=1', async s => { await s.click(key('bk.choose')); }),
  S('settings-roblox-closed', 'settings=roblox&rbxclosed=1', async s => { await s.snap('warned'); await s.click(key('ru.hide')); }),
  S('settings-roblox-changed', 'settings=roblox&rw=changed&rbxold=1'),
  // Update automatically: right away (closing every window first), then off hides the choice.
  S('settings-roblox-when', 'settings=roblox', async s => { await s.choose({ css: '#s-rbx-when' }, 'now'); await s.snap('right away'); await s.click({ css: 'label:has(#s-rbx-auto)' }); }),
  // Taskbar buttons: one group per preset, and Roblox's own icon kept.
  S('settings-taskbar', 'settings=launch', async s => { await s.choose({ css: '#s-taskbar' }, 'preset'); await s.click({ css: 'label:has(#s-pictures)' }); }),
  // Roblox out of date, Bloxstrap keeping it: Update now asks first while Roblox windows are open, then waits on Bloxstrap.
  S('settings-roblox-update', 'settings=roblox&rbxold=1', async s => { await s.click(key('rbx.update')); await s.snap('asked'); await s.click(key('rbx.updateOk')); }),
  // The sidebar's out-of-date version line leads to Settings › Roblox version.
  S('sidebar-roblox-old', 'rbxold=1', async s => { await s.click({ css: '#sys-ver .link' }); }),
  S('settings-roblox-renamed', 'settings=roblox&rw=renamed'),
  S('settings-roblox-worked', 'settings=roblox&rw=worked', null, { langs: ['en'] }),
  S('settings-roblox-unread', 'settings=roblox&rw=none', null, { langs: ['en'] }),
  S('settings-roblox-config', 'settings=perf', async s => {
    await s.click({ css: '#s-rc-levels [data-level="own"]' });
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
  S('settings-rc-levels', 'settings=perf', async s => { await s.click({ css: '#s-rc-levels [data-level="light"]' }); await s.snap('light'); await s.click({ css: '#s-rc-levels [data-level="medium"]' }); await s.snap('medium'); await s.click({ css: '#s-rc-levels [data-level="own"]' }); await s.choose({ css: '#s-rc-fps' }, '60'); await s.snap('own'); await s.click({ css: '#s-rc-levels [data-level="off"]' }); }),
  S('settings-theme-groups', 'settings=appearance', async s => { await s.click({ css: '#s-theme-groups [data-group="art"]' }); await s.snap('art'); await s.click({ css: '.swatch[data-id="starrynight"]' }); }),
  S('settings-roblox-pick', 'settings=perf', async s => { await s.click({ css: '#s-rc-levels [data-level="own"]' }); await s.choose({ css: '#s-rc-mode' }, 'marked'); await s.snap('marked'); await s.click(key('rc.pick')); }),
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
  S('settings-discord-preview', 'settings=discord&discord=1', async s => {
    await s.click({ css: '#s-dc-preview-kinds [data-kind="board"]' }); await s.snap('board');
  }),
  S('settings-discord-nolink', 'settings=discord', async s => { await s.click({ css: 'label:has(#s-dc-on)' }); }),
  S('settings-discord-gone', 'settings=discord&dcgone=1'),
  // Control from Discord: turned on, its code to type into /pc link (and a new one); linked; not connected yet.
  S('settings-discord-remote', 'settings=discord', async s => {
    await s.click({ css: 'label:has(#s-dc-remote)' });
    await s.wait(300);
    await s.snap('code');
    await s.click(key('dc.remoteNewCode'));
    await s.wait(300);
  }),
  S('settings-discord-remote-linked', 'settings=discord&remote=linked'),
  // Someone typed the code into /pc link: asked wherever the person is, Don't allow first; answered, the box goes.
  S('remote-ask', 'remote=ask', async s => { await s.snap('asked'); await s.click(key('dc.remoteDeny')); }),
  S('remote-ask-closed', 'remote=ask', async s => { await s.key('Escape'); }),
  S('settings-discord-remote-connecting', 'settings=discord&remote=connecting'),
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
  // ── Locking by itself after a while without use (the lock screen, Bloxkeep open behind it) ──
  // The fast cores for the window played: on, and on a CPU with one kind of core (its own words).
  S('settings-cores', 'settings=perf', async s => { await s.click({ css: '#s-cores' }); }),
  S('settings-cores-one', 'settings=perf&cpuone=1', null, { langs: ['en'] }),
  S('settings-autolock', 'settings=security', async s => {
    await s.choose({ css: '#s-autolock' }, '15');
    await s.snap('15 minutes');
    await s.choose({ css: '#s-autolock' }, '0');
  }),
  S('autolock-screen', 'autolock=15'),
  // Locked by itself with several vaults: only the password; Restore a backup, New vault and Switch vault aren't offered
  // (the app refuses them until the unlock, without a word on this screen; review, 2026-10-30).
  S('autolock-vaults', 'autolock=15&vaults=1'),
  S('autolock-hour', 'autolock=60', null, { langs: ['en'] }),
  S('autolock-language', 'autolock=15', async s => { await s.click({ css: '[data-act=lang][data-lang=vi]' }); }, { langs: ['en'] }),
  S('autolock-wrong', 'autolock=15', async s => { await s.type('wrong-try', key('lock.pw')); await s.key('Enter'); await s.wait(800); }),
  S('autolock-while-editing', 'edit=1&autolockin=1500', async s => {
    await s.wait(2000);
    await s.snap('locked');
    await s.type('demo', key('lock.pw'));
    await s.key('Enter');
    await s.wait(2600);
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

  // ── P1a panel ──
  // A macro's panel beside the game (panel.html, ui/src/panel): each state, the lists, a key, long words, a light theme.
  S('panel-ready', '', null, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-running', 'state=running&many=1', null, { page: 'panel-preview.html', size: [400, 640] }),
  S('panel-paused', 'state=paused', null, { page: 'panel-preview.html', size: [400, 640] }),
  S('panel-stopped', 'state=stopped&changed=1', null, { page: 'panel-preview.html', size: [400, 640] }),
  S('panel-no-game', 'state=noGame', null, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-recording', 'state=recording', async s => { await s.click({ css: '.pn-run' }); }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-editing', 'editing=1', null, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-pill', 'pill=1&state=running', null, { page: 'panel-preview.html', size: [400, 120] }),
  S('panel-fold', '', async s => { await s.click({ css: '.pn-title .pn-icon' }); }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-pick-one', '', async s => { await s.click({ css: '.k-choice .pn-select' }); }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-pick-one-chosen', '', async s => {
    await s.click({ css: '.k-choice .pn-select' }); await s.key('ArrowDown'); await s.key('ArrowDown'); await s.key('Enter');
  }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-pick-one-small', 'tabs=1', async s => {
    await s.click({ css: '.pn-tabs button:nth-child(2)' }); await s.click({ css: '.k-choice .pn-select' });
  }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-pick-several', '', async s => {
    await s.click({ css: '.k-list .pn-select' }); await s.click({ css: '#pn-m-4' }); await s.snap('ticked');
    await s.click({ css: '.k-list .pn-select' });
  }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-key-capture', '', async s => {
    await s.click({ css: '.k-key .btn' }); await s.snap('waiting'); await s.key('q');
  }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-number', '', async s => {
    await s.click({ css: '.k-number .pn-step:last-child' }); await s.wait(400);
    await s.fill({ css: '.k-number .pn-num' }, '99'); await s.key('Enter'); await s.wait(200);
  }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-menu', '', async s => { await s.click({ css: '.pn-name' }, { button: 'right' }); }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-tab-switch', 'tabs=1&state=running', async s => { await s.click({ css: '.pn-tabs button:nth-child(3)' }); }, { page: 'panel-preview.html', size: [400, 620] }),
  S('panel-long-labels', 'long=1&state=running&many=1', null, { page: 'panel-preview.html', size: [400, 720] }),
  S('panel-light', 'theme=light&state=running', null, { page: 'panel-preview.html', size: [400, 640] }),
  S('panel-retro', 'theme=retro&state=stopped&tabs=1', async s => { await s.click({ css: '.pn-tabs button:nth-child(2)' }); await s.click({ css: '.k-choice .pn-select' }); },
    { page: 'panel-preview.html', size: [400, 620], langs: ['en'] }),
  S('panel-macos', 'theme=macos&state=running', null, { page: 'panel-preview.html', size: [400, 640], langs: ['en'] }),
  S('panel-picture', 'theme=fuji&state=paused', async s => { await s.click({ css: '.k-list .pn-select' }); }, { page: 'panel-preview.html', size: [400, 640], langs: ['en'] }),
  // ── end P1a ──

  // ── P1b panel design ──
  // ── end P1b ──

  // ── P2 macro editor ──
  // The step list as blocks: a Repeat folded to one line (its steps counted), unfolded again by its chevron.
  S('macro-block-collapse', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-step[data-i="0"] .mc-fold' });
    await s.snap('folded');
    await s.click({ css: '.mc-step[data-i="0"] .mc-fold' });
  }),
  // The "+" between two rows: Add step's list for that place, searched ("cho" finds Wait in both languages), Enter adds it.
  S('macro-insert-between', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.hover({ css: '.mc-step[data-i="3"]' });
    await s.click({ css: '.mc-step[data-i="3"] .mc-ins' });
    await s.snap('list');
    await s.type('cho');
    await s.snap('found');
    await s.key('Enter');
  }),
  // What stops a save, on its own row with its fix: an If with no end (Add End if), a click with no point (Pick on game);
  // the foot counts them. One click on the fix ends the If.
  S('macro-problem-fix', 'macroedit=m5&p2broken=1', async s => {
    await s.wait(500);
    await s.snap('problems');
    await s.click({ css: '.mc-step[data-i="1"] .mc-step-probs .btn' });
    await s.wait(200);
  }),
  // A note on a step: why it is there, under its sentence.
  S('macro-note', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-step[data-i="1"] [data-st=more]' });
    await s.click({ key: 'macro.note.add', within: '.menu' });
    await s.type('Hold long enough to cast far');
    await s.key('Enter');
  }),
  // From a template: cards saying what each does and what it needs from the game; one opens with its notes.
  S('macro-templates-cards', 'nomacros=1', async s => {
    await s.click({ css: '.mc-tile[data-start=template]' });
    await s.snap('cards');
    await s.click({ css: '.mc-tpl-pop > button:nth-of-type(2)' });
    await s.wait(300);
  }),
  // A counter by the name the panel shows: New counter… in the step's list, a name and what the panel calls it.
  S('macro-counter-label', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-step[data-i="4"] .mc-step-head' });
    await s.eval(`(() => { const el = document.querySelector('.mc-step.open select[id$="-name"]');
      Object.getOwnPropertyDescriptor(HTMLSelectElement.prototype, 'value').set.call(el, '\\u0000new');
      el.dispatchEvent(new Event('change', { bubbles: true })); })()`);
    await s.wait(100);
    await s.type('gold');
    await s.key('Tab');
    await s.type(s.lang === 'vi' ? 'Vàng' : 'Gold coins');
    await s.snap('naming');
    await s.key('Enter');
  }),
  // What a step still uses can't go: an area a step looks in says so, with links to those steps.
  S('macro-area-in-use', 'macroedit=m4', async s => {
    await s.wait(500);
    await s.click({ css: '.mc-areas li button.icon-btn' });
  }, { langs: ['en'] }),
  // The editor in a light theme and another (macOS): blocks, rails, notes, problems.
  S('macro-editor-light', 'macroedit=m5&p2broken=1&theme=light', async s => { await s.wait(500); }),
  S('macro-editor-macos', 'macroedit=m4&theme=macos', async s => { await s.wait(500); await s.click({ css: '.mc-step[data-i="2"] .mc-step-head' }); }),
  // With no Roblox window: the picture says where to open one, with the way there.
  S('macro-editor-nowindow', 'macroedit=m4&nowindows=1', async s => { await s.wait(500); }),
  // ── end P2 ──

  // ── P3 accounts ──
  // The list in the light theme, and the servers dialog in another (Paper).
  S('p3-light-list', 'theme=light&winhidden=1', async s => { await s.click(key('layout.list')); }, { size: [1440, 800] }),
  S('p3-paper-servers', 'theme=paper&select=1', async s => { await s.click(key('dock.servers')); await s.wait(900); }, { langs: ['en'] }),
  // The server list didn't come: said where it would be, with Try again.
  S('p3-servers-fail', 'srvfail=1', async s => { await s.click(key('dock.servers')); await s.wait(900); }),
  // One account's menu: Copy opens its own small menu.
  S('p3-card-menu-copy', '', async s => {
    await s.click({ css: '.card[data-id$="2"] [data-act=card-menu]' });
    await s.snap('menu');
    await s.click(key('m.copy'));
  }),
  // A saved server's ✕ asks "Delete?" in its place first.
  S('p3-saved-server-delete', '', async s => {
    await s.click(key('dock.place'));
    await s.wait(600);
    await s.click({ key: 'picker.saved', within: '#picker' });
    await s.click({ css: '.game-row.saved[data-i="0"] [data-act="arm-delete-server"]' });
  }),
  // A narrow window: the filters that don't fit are in "+n", whose menu lists them.
  S('p3-chips-more', '', async s => { await s.click({ css: '[data-act="chip-more"]' }); }, { size: [860, 640] }),
  // A private server picked from Servers: the game box shows its game and "Private server".
  S('p3-private-face', 'select=1', async s => {
    await s.click(key('dock.servers'));
    await s.wait(900);
    await s.click({ css: '[data-private="0"]' });
    await s.wait(700);
  }),
  // ── end P3 ──

  // ── P4 presets ──
  // The server choices in two groups; "each account in its own server" says so under the row.
  S('preset-server-groups', 'preset=1', async s => {
    await s.wait(300);
    await s.click({ css: '#ps-server' });
    await s.snap('open');
    await s.click({ css: '#ps-server' });
    await s.choose({ css: '#ps-server' }, 'eachMost');
    await s.snap('each');
  }),
  // Without a game the choices that need one are off and the row says why; a player to follow needs none.
  S('preset-no-game', 'view=presets', async s => {
    await s.click(key('ps.new'));
    await s.wait(300);
    await s.fill({ css: '#ps-place' }, '');
    await s.snap('empty');
    await s.click({ css: '#ps-server' });
    await s.snap('open');
    await s.click({ css: '#ps-server' });
    await s.choose({ css: '#ps-server' }, 'follow');
    await s.snap('follow');
  }),
  // The time box is a text box: "9pm" is read as 21:00 and written the way the app writes times; a text that is no time is
  // said under the box (and again when Save is refused).
  S('preset-time-typed', 'preset=1', async s => {
    await s.wait(300);
    await s.fill({ css: '#ps-sch-time' }, '9pm');
    await s.key('Tab');
    await s.snap('9pm');
    await s.fill({ css: '#ps-sch-time' }, '25:00');
    await s.key('Tab');
    await s.snap('bad');
    await s.click({ css: '.overlay .modal-foot .btn.primary' });
    await s.snap('refused');
  }),
  // The same for the date, in the order the language writes it; a day that doesn't exist is no date.
  S('preset-date-typed', 'preset=1', async s => {
    await s.wait(300);
    await s.choose({ css: '#ps-sch-mode' }, 'once');
    await s.fill({ css: '#ps-sch-date' }, s.lang === 'vi' ? '20/12/2026' : '12/20/2026');
    await s.key('Tab');
    await s.snap('typed');
    await s.fill({ css: '#ps-sch-date' }, s.lang === 'vi' ? '31/02/2026' : '02/31/2026');
    await s.key('Tab');
    await s.snap('bad');
  }),
  // A search that finds nothing offers to clear itself.
  S('presets-search-clear', 'view=presets', async s => {
    await s.type('zzzz', key('pv.search'));
    await s.snap('none');
    await s.click({ css: '[data-pv="clear-search"]' });
  }),
  // ── end P4 ──

  // ── P5 sign-in ──
  // The second refused master password says where to go next; the lock screen on a light and on a picture theme.
  S('lock-forgot', 'locked=1', async s => {
    await s.fill(key('lock.pw'), 'wrong-one');
    await s.key('Enter');
    await s.wait(800);
    await s.snap('first try');
    await s.fill(key('lock.pw'), 'wrong-two');
    await s.key('Enter');
    await s.wait(800);
  }),
  S('lock-theme-light', 'locked=1&theme=light', async s => { await s.fill(key('lock.pw'), 'wrong-one'); await s.key('Enter'); await s.wait(800); }),
  S('lock-theme-scene', 'locked=1&theme=halong', async s => { await s.fill(key('lock.pw'), 'wrong-one'); await s.key('Enter'); await s.wait(800); }),
  S('first-run-theme-light', 'new=1&theme=light', async s => { await s.fill(key('lock.pw'), 'Abcdef12'); await s.fill(key('lock.pw2'), 'Abcdef13'); await s.key('Enter'); await s.wait(300); }),
  S('first-run-theme-scene', 'new=1&theme=halong', async s => { await s.fill(key('lock.pw'), 'Abcdef12'); await s.fill(key('lock.pw2'), 'Abcdef13'); await s.key('Enter'); await s.wait(300); }),
  S('vault-picker-light', 'locked=1&vaults=1&vaultpick=1&theme=light'),
  // The sign-in: only the name typed (the password box is marked and takes the focus); the puzzle window closed unsolved;
  // Done after an account came in.
  S('pw-empty-password', '', async s => { await s.click(key('acc.add')); await s.fill({ css: '#pw-user' }, 'someone'); await s.click({ css: '#pw-go' }); }),
  S('pw-captcha-closed', '', async s => { await signIn(s, 'captchaclosed'); await s.click({ css: '#pw-solve' }); await s.wait(600); }),
  S('pw-added-done', '', async s => {
    await signIn(s, 'firstfarm');
    await s.snap('added');
    await s.click({ css: '[data-x2=done]' });
    await s.wait(400);
  }),
  // Import: a long list gets a search (All new and None act on what it shows); Cancel after Bloxkeep file in the sign-in
  // comes back to the sign-in.
  S('import-vault-many', 'vimport=preview&ivmany=1', async s => {
    await s.wait(1500);
    await s.snap('long list');
    await s.fill({ css: '#iv-search' }, 'alt_account_1');
    await s.snap('searched');
    await s.click({ css: '#iv-none' });
    await s.snap('none of those');
    await s.click({ css: '#iv-all' });
  }),
  S('import-vault-cancel-back', '', async s => {
    await s.click(key('acc.add'));
    await s.click({ css: '.overlay [data-x2=vault]' });
    await s.wait(500);
    await s.click({ key: 'common.cancel', within: '.overlay' });
    await s.wait(500);
  }, { langs: ['en'] }),
  // A part of the page that fails to draw shows a short note with Try again (the sidebar, here).
  S('guard-failed', 'guardfail=1', async s => { await s.wait(300); }),
  // The dialogs on a light and a picture theme: the success banner with Done, a dimmed Quick Login code, the vault picker.
  S('pw-added-light', 'theme=light', async s => { await signIn(s, 'firstfarm'); }),
  S('ql-dead-scene', 'theme=halong&ql=cancelled,waiting', async s => {
    await s.click(key('acc.add'));
    await s.click({ key: 'start.phone', partial: true });
    await s.wait(3500);
  }),
  S('vault-picker-scene', 'locked=1&vaults=1&vaultpick=1&theme=halong'),
  // ── end P5 ──

  // ── P6 settings ──
  // Play time by day: 30 days on the dark base, 7 and 30 on light, and a 30-day bar under the pointer (its time and date only).
  S('stats-days-30', 'stats=1', async s => { await s.click(key('stats.period.30')); await s.wait(300); }),
  S('stats-days-light', 'stats=1&theme=light', async s => { await s.click(key('stats.period.7')); await s.wait(300); await s.snap('7 days'); await s.click(key('stats.period.30')); await s.wait(300); }),
  S('stats-days-hover', 'stats=1', async s => { await s.click(key('stats.period.30')); await s.wait(300); await s.hover({ css: '#stats-days .day:nth-child(20)' }); await s.wait(200); }),
  S('stats-days-narrow', 'stats=1', async s => { await s.click(key('stats.period.7')); await s.wait(300); await s.snap('7 days'); await s.click(key('stats.period.30')); await s.wait(300); }, { size: [980, 700] }),
  // Settings on a picture theme: the page list on its own panel. The game monitor with no Roblox window open.
  S('settings-picture-nav', 'settings=perf&theme=halong'),
  S('monitor-empty', 'monitor=1&empty=1'),
  // ── end P6 ──

  // ── P7 native ──
  // The mini window (mini.html, ui/src/mini): the running accounts, empty, locked, names hidden, long words, Close all asked.
  S('mini', '', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-empty', 'empty=1&others=2', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-locked', 'locked=1', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-names-hidden', 'hidden=1', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-long', 'long=1&many=1', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-close', '', async s => { await s.click({ css: '.mini-row:first-child .btn.ghost' }); }, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-close-all', '', async s => { await s.click({ css: '.mini-foot .btn.danger' }); await s.snap('asked'); await s.click({ css: '.mini-ask .btn.danger' }); },
    { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-light', 'theme=light', null, { page: 'mini-preview.html', size: [340, 420] }),
  S('mini-retro', 'theme=retro', async s => { await s.click({ css: '.mini-foot .btn.danger' }); }, { page: 'mini-preview.html', size: [340, 420], langs: ['en'] }),
  // The game monitor's tiles (ui/src/monitor), stand-in pictures where the app lays the live ones (ui/dev/mock-p7.js).
  S('monitor-tiles', 'monitor=1&tiles=3', async s => { await s.point({ css: '.mon-tile' }); }),
  S('monitor-one', 'monitor=1&tiles=1', async s => { await s.point({ css: '.mon-tile' }); }),
  S('monitor-many', 'monitor=1&tiles=9&long=1', async s => { await s.point({ css: '.mon-tile' }); }),
  S('monitor-names-hidden', 'monitor=1&tiles=3&hide=1&others=1', async s => { await s.point({ css: '.mon-tile' }); }),
  S('monitor-minimized', 'monitor=1&tiles=3&same=1', async s => { await s.click({ css: '.mon-tile:nth-child(2) .mon-min' }); await s.wait(300); }),
  // The menu withdraws only the picture it lies over: the other two keep theirs.
  S('monitor-menu', 'monitor=1&tiles=3&same=1', async s => {
    await s.click({ css: '.mon-tile:first-child .mon-pic' }, { button: 'right' });
    await s.wait(200);
    const left = await s.eval('window.__monitorArea?.pictures?.map(p => p.id).join(",")');
    if (!(await s.eval('!!document.querySelector(".menu")')) || left.split(',').length !== 2) throw new Error(`pictures left with the menu open: ${left}`);
  }),
  S('monitor-lanes', 'monitor=1&tiles=9&drop=1', async s => { await s.point({ css: '.mon-lane' }); }),
  S('monitor-lane-folded', 'monitor=1&tiles=9&drop=1', async s => { await s.click({ css: '.mon-lane:first-child .mon-fold' }); await s.wait(300); }),
  S('monitor-lane-solo', 'monitor=1&tiles=9&drop=1', async s => { await s.click({ css: '.mon-lane:nth-child(2) .mon-lane-acts .icon-btn:nth-child(2)' }); await s.wait(300); }),
  S('monitor-lane-close-ask', 'monitor=1&tiles=9&drop=1', async s => { await s.click({ css: '.mon-lane:first-child .mon-kill' }); await s.wait(200); }),
  S('monitor-none', 'monitor=1&tiles=0', async s => { await s.point({ css: '.mon-empty' }); }),
  S('monitor-light', 'monitor=1&tiles=3&theme=light', async s => { await s.point({ css: '.mon-tile' }); }),
  S('monitor-retro', 'monitor=1&tiles=4&mintile=1&theme=retro', async s => { await s.point({ css: '.mon-tile' }); }, { langs: ['en'] }),
  // The overlay over a game and the colour picker (overlay.html, ui/src/overlay), over a stand-in game (ui/dev/overlay-mock.js).
  ...(() => {
    const OV = { page: 'overlay-preview.html', size: [960, 540] };
    /** A press on an element's centre, moved by dx, dy and let go, as pointer events (the page's own drags). */
    const press = (s, sel, dx, dy) => s.eval(`(async () => {
      const el = document.querySelector(${JSON.stringify(sel)}), r = el.getBoundingClientRect(), x = r.left + r.width / 2, y = r.top + r.height / 2;
      const o = (cx, cy, buttons) => ({ bubbles: true, pointerId: 1, isPrimary: true, button: 0, buttons, clientX: cx, clientY: cy });
      const frame = () => new Promise(f => requestAnimationFrame(() => f()));
      el.dispatchEvent(new PointerEvent('pointerdown', o(x, y, 1))); await frame();
      el.dispatchEvent(new PointerEvent('pointermove', o(x + ${dx} / 2, y + ${dy} / 2, 1))); await frame();
      el.dispatchEvent(new PointerEvent('pointermove', o(x + ${dx}, y + ${dy}, 1))); await frame();
      el.dispatchEvent(new PointerEvent('pointerup', o(x + ${dx}, y + ${dy}, 0))); await frame();
    })()`);
    const sent = async (s, what) => {
      const acts = await s.eval('JSON.stringify(window.__acts)');
      if (!JSON.parse(acts).some(a => a.what === what)) throw new Error(`no "${what}" sent: ${acts}`);
    };
    return [
      S('overlay-edit', '', async s => { await s.point({ css: '.ov-dot' }); }, OV),
      // A dot and a box's corner dragged: the app hears where they went, in parts of the game.
      S('overlay-edit-drag', '', async s => {
        await s.point({ css: '.ov-dot' });
        await press(s, '.ov-dot[data-dot="0"]', 120, -60);
        await press(s, '.ov-area[data-area="1"] .ov-grip.br', 80, 40);
        await s.wait(100);
        await sent(s, 'dot');
        await sent(s, 'area');
        await s.key('Escape');
        await sent(s, 'done');
      }, OV),
      S('overlay-new-long', 'new=1&long=1', async s => { await s.point({ css: '.ov-dot' }); }, OV),
      S('overlay-areas', 'mode=areas', async s => { await s.point({ css: '.ov-area' }); }, OV),
      S('overlay-pick', 'mode=pick', async s => {
        await s.point({ css: '.ov-picture' });
        await s.hover({ css: '.ov' });
        await s.wait(150);
        await s.snap('magnifier');
        await s.click({ css: '.ov' });
        await sent(s, 'pick');
      }, OV),
      S('overlay-screen', 'mode=screen', async s => { await s.hover({ css: '.ov' }); await s.wait(150); await s.key('Escape'); await sent(s, 'cancel'); }, OV),
      S('overlay-light', 'theme=light', async s => { await s.point({ css: '.ov-dot' }); }, OV),
      S('overlay-retro', 'mode=areas&theme=retro', async s => { await s.point({ css: '.ov-area' }); }, { ...OV, langs: ['en'] }),
      S('overlay-small', 'mode=edit', async s => { await s.point({ css: '.ov-dot' }); }, { ...OV, size: [480, 300] }),
    ];
  })(),
  // ── end P7 ──

  // ── P7b browser ──
  // The app's message box (dialog.html, ui/src/dialog) and the top of Bloxkeep's browser windows (browser.html, ui/src/browser).
  S('dialog-ask', 'case=ask', null, { page: 'dialog-preview.html', size: [520, 340] }),
  S('dialog-ask-light', 'case=ask&theme=light', async s => { await s.click({ css: '.dlg-remember input' }); }, { page: 'dialog-preview.html', size: [520, 340] }),
  S('dialog-ask-retro', 'case=confirm&theme=retro', null, { page: 'dialog-preview.html', size: [520, 300], langs: ['en'] }),
  S('dialog-choices', 'case=choices', null, { page: 'dialog-preview.html', size: [520, 480] }),
  S('dialog-choices-light', 'case=choices&theme=light', null, { page: 'dialog-preview.html', size: [520, 480], langs: ['en'] }),
  S('dialog-signout', 'case=signout', null, { page: 'dialog-preview.html', size: [520, 340] }),
  S('dialog-error-long', 'case=long', null, { page: 'dialog-preview.html', size: [520, 340] }),
  S('dialog-picture', 'case=choices&theme=fuji', null, { page: 'dialog-preview.html', size: [520, 480], langs: ['en'] }),
  S('browser-chrome', '', null, { page: 'browser-preview.html', size: [1100, 260] }),
  S('browser-chrome-blank', 'blank=1', null, { page: 'browser-preview.html', size: [1100, 260] }),
  S('browser-chrome-saved', 'blank=1', async s => { await s.click({ css: '.bw-bar .btn.primary' }); }, { page: 'browser-preview.html', size: [1100, 260] }),
  S('browser-chrome-offline', 'offline=1', null, { page: 'browser-preview.html', size: [1100, 260] }),
  S('browser-chrome-many-tabs', 'many=1', null, { page: 'browser-preview.html', size: [980, 260] }),
  S('browser-chrome-many-narrow', 'many=1', null, { page: 'browser-preview.html', size: [640, 260], langs: ['en'] }),
  S('browser-chrome-close', '', async s => { await s.hover({ css: '.bw-tab:nth-child(2)' }); await s.snap('hover'); await s.click({ css: '.bw-tab:nth-child(2) .bw-x' }); },
    { page: 'browser-preview.html', size: [1100, 260], langs: ['en'] }),
  S('browser-chrome-light', 'theme=light&blank=1', null, { page: 'browser-preview.html', size: [1100, 260] }),
  S('browser-chrome-retro', 'theme=retro', null, { page: 'browser-preview.html', size: [1100, 260], langs: ['en'] }),
  S('browser-chrome-picture', 'theme=fuji&many=1', null, { page: 'browser-preview.html', size: [980, 260], langs: ['en'] }),
  S('browser-login', 'login=1', null, { page: 'browser-preview.html', size: [1000, 260] }),
  // ── end P7b ──

  // ── P8 ──
  // ── end P8 ──
];
