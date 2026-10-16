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
];
