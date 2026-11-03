'use strict';
// P6's additions to the fake app (loaded after mock.js; P8 folds them in): Statistics' account that is no longer in the
// list comes back marked deleted, as the app marks it (AppController.Stats).
(() => {
  const bridge = window.__mockBridge;
  if (!bridge) return;
  const call = bridge.call.bind(bridge);
  bridge.call = async (cmd, args) => {
    const r = await call(cmd, args);
    if (cmd === 'playStats' && r?.accounts) for (const a of r.accounts) if (!a.label) a.deleted = true;
    return r;
  };
})();
