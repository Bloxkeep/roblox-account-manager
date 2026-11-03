'use strict';
// P3's additions to the fake bridge (mock.js), loaded after it; P8 folds them in. ?srvfail=1: the server list fails to
// load (Roblox down) until Try again is pressed twice. A link with no number in it is refused in the page's language.
(() => {
  const params = new URLSearchParams(location.search);
  const bridge = window.__mockBridge;
  if (!bridge) return;
  const base = bridge.call.bind(bridge);
  const vi = () => params.get('lang') === 'vi';
  let serverTries = 0;
  bridge.call = async (cmd, a) => {
    if (cmd === 'resolveGame' && !/\d{5,}/.test(a?.input || '') && !/share\?code=/i.test(a?.input || '')) {
      await new Promise(r => setTimeout(r, 120));
      return { ok: false, error: vi() ? 'Link không có Place ID.' : 'This link has no Place ID.' };
    }
    if (cmd === 'servers' && params.has('srvfail') && serverTries++ < 2) {
      await new Promise(r => setTimeout(r, 300));
      throw new Error(vi() ? 'Không kết nối được Roblox.' : "Couldn't reach Roblox.");
    }
    return base(cmd, a);
  };
})();
