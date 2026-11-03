// P5 (sign-in, quick login, lock, vaults, import): extra fake answers on top of mock.js. Loaded after it by preview.html.
(() => {
  const bridge = window.__mockBridge;
  if (!bridge) return;
  const call = bridge.call.bind(bridge);
  const params = new URLSearchParams(location.search);
  let user = '';
  // ?guardfail=1: the vault list is a number, so the sidebar (which reads it as a list) fails to draw and Guard shows its note.
  // The failure is the point of the scenario: Guard and React report it with console.error, which the runner would count as a bug.
  if (params.has('guardfail')) console.error = () => {};
  const broken = st => params.has('guardfail') && st ? { ...st, vaults: { ...(st.vaults || {}), list: 5 } } : st;
  const attach = bridge.attach.bind(bridge);
  bridge.attach = fn => attach((ev, data) => fn(ev, ev === 'state' ? broken(data) : data));
  bridge.call = async (cmd, a) => {
    if (cmd === 'loginPassword') user = a.username;
    // User "captchaclosed": the puzzle window is closed without solving it (Solve then asks again).
    if (user === 'captchaclosed') {
      if (cmd === 'loginPassword') return { status: 'captcha' };
      if (cmd === 'loginCaptcha') return { status: 'captcha', closed: true };
    }
    const r = await call(cmd, a);
    if (cmd === 'getState' && r?.state) r.state = broken(r.state);
    // ?ivmany=1: the file holds 29 accounts (a list long enough for its search box). Only for looking: it can't be imported.
    if (cmd === 'vaultImportOpen' && params.has('ivmany') && r?.status === 'ok') {
      const more = Array.from({ length: 24 }, (_, i) => ({
        key: r.accounts.length + i, label: `Alt ${i + 1}`, username: `alt_account_${i + 1}`, realm: '', group: i % 3 ? '' : 'Farm',
        hasPassword: i % 2 === 0, cookieValid: i % 4 === 0 ? false : true, avatarUrl: null, exists: false,
      }));
      r.accounts = [...r.accounts, ...more];
    }
    return r;
  };
})();
