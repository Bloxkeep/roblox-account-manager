'use strict';
// Fake app for a macro panel's page (panel.html) in a normal browser. ?state=running|paused|stopped|noGame|recording
// (else ready), ?many=1 three accounts running, ?pill=1 folded, ?tabs=1 three tabs, ?long=1 long names and labels,
// ?editing=1 areas being moved, ?changed=1 a value changed (Reset all on), ?theme=light a theme, ?lang=vi Vietnamese.
// The acts change the fake macro as the app would (nudge, typed, set, run, stop, reset); what is clicked shows in the
// page's calls (Tests/ui/page-hooks.js).
(() => {
  const params = new URLSearchParams(location.search);
  const vi = params.get('lang') === 'vi';
  const long = params.has('long');
  let state = params.get('state') || 'ready';
  let pill = params.has('pill');
  let editing = params.has('editing');
  let undoable = false;
  const tabs = params.has('tabs') ? ['Main', 'Fishing', long ? 'Selling at the merchant' : 'Sell'] : [];
  const fmt = (n, digits = 2) => n.toLocaleString(vi ? 'vi-VN' : 'en-US', { maximumFractionDigits: digits, useGrouping: false });
  const sec = vi ? ' giây' : ' s';
  const opts = [
    { name: 'hold', label: long ? 'Hold the mouse button down to cast for' : 'Hold to cast', kind: 'number', tab: 'Main', value: 800, def: 800, min: 100, max: 30000, step: 100, scale: 1000, suffix: sec },
    { name: 'key', label: long ? 'Key that casts the fishing rod' : 'Cast key', kind: 'key', tab: 'Main', value: 'E', def: 'E' },
    { name: 'spot', label: 'Cast spot', kind: 'point', tab: 'Main', value: { x: 0.52, y: 0.48 }, def: { x: 0.52, y: 0.48 } },
    { name: 'jump', label: long ? 'Jump when the character gets stuck on a wall' : 'Jump when stuck', kind: 'check', tab: 'Main', value: 1, def: 1 },
    { name: 'bait', label: 'Bait', kind: 'choice', tab: 'Fishing', value: long ? 'Golden worm from the Second Sea shop' : 'Worm', def: 'Worm',
      choices: ['Worm', 'Shrimp', long ? 'Golden worm from the Second Sea shop' : 'Golden worm', 'Squid', 'Glow bait', 'Magnet', 'Leech', 'Minnow', 'Crab', 'Sardine', 'Bagel', 'Insect'] },
    { name: 'keep', label: long ? 'Fish to keep in the backpack' : 'Fish to keep', kind: 'list', tab: 'Fishing', value: ['Salmon', 'Tuna'], def: ['Salmon', 'Tuna'],
      choices: ['Salmon', 'Tuna', 'Carp', 'Pike', 'Eel'] },
    { name: 'colour', label: 'Button color', kind: 'colour', tab: 'Fishing', value: '#22c55e', def: '#22c55e' },
    { name: 'adv', label: long ? 'Advanced settings for selling' : 'Advanced', kind: 'heading', tab: tabs[2] || null, value: null },
    { name: 'clicks', label: 'Clicks per reel', kind: 'number', tab: tabs[2] || null, value: 12, def: 12, min: 1, max: 50, step: 1, scale: 1, suffix: '' },
    { name: 'sellAt', label: long ? 'Sell everything once the bag is this full' : 'Sell when bag is', kind: 'number', tab: tabs[2] || null, value: 90, def: 90, min: 10, max: 100, step: 5, scale: 1, suffix: '%' },
  ];
  if (params.has('changed')) opts[0].value = 1200;
  const shown = o => o.kind === 'number' ? fmt(o.value / o.scale) + o.suffix
    : o.kind === 'list' ? `${o.value.length}/${o.choices.length}` : o.kind === 'check' ? (o.value ? 'On' : 'Off')
    : o.kind === 'point' ? `${Math.round(o.value.x * 100)} %, ${Math.round(o.value.y * 100)} %` : String(o.value ?? '');
  const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
  const names = long ? ['Main account with a long name', 'Farm 1', 'Farm 2'] : ['Main', 'Farm 1', 'Farm 2'];
  const startedAt = new Date(Date.now() - (12 * 60 + 44) * 1000).toISOString();
  const now = () => {
    const runs = state === 'running' ? names.slice(0, params.has('many') ? 3 : 1).map((name, i) => ({ name, status: i === 2 ? 'waiting' : 'running', reason: null }))
      : state === 'paused' ? [{ name: names[0], status: 'paused', reason: vi ? 'bạn đang dùng máy' : "you're using the PC" }] : [];
    return {
      state, runs, counters: state === 'running' || state === 'paused' || state === 'stopped' ? { fish: 37, sold: 4 } : {},
      startedAt: runs.length ? startedAt : null,
      lastError: state === 'stopped' ? { text: vi ? 'không thấy thứ cần tìm' : "what it looks for wasn't there", step: 6 } : null,
      editingAreas: editing, hotkeysOn: true, hasGame: state !== 'noGame', hasWindow: state !== 'noGame',
      recording: state === 'recording' ? { account: names[0] } : null, undoable,
      changed: opts.some(o => o.kind !== 'heading' && !same(o.value, o.def)),
    };
  };
  let emit = () => {};
  const send = () => emit('panel', {
    macro: {
      id: '6f1c2e9a-0d3b-4f7e-9a51-2c8d4b7e1f00', name: long ? 'Farm route for Blox Fruits second sea, every island' : 'Farm route',
      hotkey: 3, tabs, areasCount: 2,
      counters: [{ name: 'fish', label: long ? 'Fish caught this session' : 'fish' }, { name: 'sold', label: 'sold' }],
      options: opts.map(o => ({
        name: o.name, label: o.label, kind: o.kind, tab: o.tab, value: o.value, choices: o.choices || null, shown: shown(o),
        min: o.min ?? null, max: o.max ?? null, step: o.step ?? null, scale: o.scale ?? null, suffix: o.suffix ?? null,
        minShown: o.kind === 'number' ? fmt(o.min / o.scale) + o.suffix : null, maxShown: o.kind === 'number' ? fmt(o.max / o.scale) + o.suffix : null,
      })),
    },
    now: now(), theme: params.get('theme') || 'dark', customTheme: null, language: vi ? 'vi' : 'en', pill,
  });
  const error = (name, text) => emit('panelError', { name, text });
  const find = name => opts.find(o => o.name === name);
  const acts = {
    run() {
      if (state === 'recording') return error(null, vi ? 'Đang ghi, macro chờ.' : 'Runs wait while recording.');
      if (state === 'noGame') return error(null, vi ? 'Farm route: không có cửa sổ Roblox để chạy.' : 'Farm route: no Roblox window to run it in.');
      state = 'running';
    },
    stop() { state = 'ready'; },
    set(a) { const o = find(a.name); if (o) o.value = a.key ?? a.value; },
    nudge(a) {
      const o = find(a.name);
      const v = o.value + a.by * o.step;
      if (v > o.max) error(o.name, (vi ? 'Tối đa ' : 'Max is ') + fmt(o.max / o.scale) + o.suffix);
      if (v < o.min) error(o.name, (vi ? 'Tối thiểu ' : 'Min is ') + fmt(o.min / o.scale) + o.suffix);
      o.value = Math.min(o.max, Math.max(o.min, v));
    },
    typed(a) {
      const o = find(a.name);
      const n = Number(String(a.text).replace(/[^\d.,-]/g, '').replace(',', '.'));
      if (!String(a.text).trim() || Number.isNaN(n)) return error(o.name, vi ? 'Đây không phải số.' : "That isn't a number.");
      const v = Math.round(n * o.scale);
      if (v > o.max) error(o.name, (vi ? 'Tối đa ' : 'Max is ') + fmt(o.max / o.scale) + o.suffix);
      o.value = Math.min(o.max, Math.max(o.min, v));
    },
    reset() { opts.forEach(o => { o.value = o.def; }); undoable = true; setTimeout(() => { undoable = false; send(); }, 6000); },
    undoReset() { undoable = false; },
    areas() { editing = !editing; },
  };
  window.__mockBridge = {
    attach(fn) { emit = fn; },
    async call(cmd, args) {
      if (cmd === 'ready') setTimeout(send, 0);
      if (cmd === 'size') pill = !!args.pill;
      if (cmd === 'act' && acts[args.what]) { acts[args.what](args); setTimeout(send, 0); }
      return null;
    },
  };
})();
