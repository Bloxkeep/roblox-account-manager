// P5 (sign-in, quick login, lock, vaults, import): extra fake answers on top of mock.js. Loaded after it by preview.html.
(() => {
  const bridge = window.__mockBridge;
  if (!bridge) return;
  const call = bridge.call.bind(bridge);
  let user = '';
  bridge.call = async (cmd, a) => {
    if (cmd === 'loginPassword') user = a.username;
    // User "captchaclosed": the puzzle window is closed without solving it (Solve then asks again).
    if (user === 'captchaclosed') {
      if (cmd === 'loginPassword') return { status: 'captcha' };
      if (cmd === 'loginCaptcha') return { status: 'captcha', closed: true };
    }
    return call(cmd, a);
  };
})();
