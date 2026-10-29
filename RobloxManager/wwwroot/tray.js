'use strict';
// The tray's page (tray.html in TrayWindow.cs's window): draws the quick panel or menu (ui/src/Tray.tsx) from the app's
// "tray" message, and sends back clicks ("act") and its size ("size"), in the same messages as the app's own page.
(() => {
  const { ICONS, LOGO, customVars } = window.BloxkeepShared;
  const DICT = window.I18N;
  let lang = 'en';
  function t(key, vars) {
    let s = (DICT[lang] && DICT[lang][key]) ?? DICT.en[key] ?? key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m));
    return s;
  }
  const tn = (key, n, vars = {}) => t(n === 1 && `${key}1` in DICT[lang] ? `${key}1` : key, { n, ...vars });

  const wv = window.chrome && window.chrome.webview, mock = window.__mockBridge;
  let seq = 0;
  const send = (cmd, args = {}) => {
    if (wv) wv.postMessage({ id: ++seq, cmd, args });
    else if (mock) mock.call(cmd, args);
  };

  const host = { t, tn, get lang() { return lang; }, get locale() { return lang === 'vi' ? 'vi-VN' : 'en-US'; }, logo: LOGO, iconPaths: name => ICONS[name] || '' };
  const ui = window.BloxkeepUI.tray(document.getElementById('tray'), host, {
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h, opened) => send('size', { w, h, opened }),
  });

  /** The app's theme, and a custom one's colours (without its picture: the tray is a plain card). */
  function theme(id, custom) {
    const root = document.documentElement;
    root.dataset.theme = id || 'dark';
    const vars = id === 'custom' && custom ? customVars({ ...custom, picture: null }) : null;
    if (vars) {
      root.dataset.tone = vars.tone;
      Object.entries(vars.vars).forEach(([k, v]) => root.style.setProperty(k, v));
    } else {
      delete root.dataset.tone;
      root.removeAttribute('style');
    }
  }

  function show(d) {
    if (!d) return;
    if (d.language && DICT[d.language]) lang = d.language;
    document.documentElement.lang = lang;
    theme(d.theme, d.customTheme);
    ui.show(d.mode, d.state, d.opened);
  }
  if (wv) wv.addEventListener('message', e => { const m = e.data; if (m && m.type === 'event' && m.name === 'tray') show(m.data); });
  else if (mock) mock.attach((name, data) => { if (name === 'tray') show(data); });
  send('ready');
})();
