'use strict';
// The glue of the app's small pages that are a window's whole face or top: dialog.html (Forms/Dialog.cs, the message
// box) and browser.html (the top of Forms/WebsiteForm.cs and BrowserLoginForm.cs). The page's <body data-page> names
// the part (window.BloxkeepUI.dialog or .browser) and the app's event it draws ("dialog", "browser"); it sends back
// clicks ("act"), its size ("size") and a drag of its head ("drag"), in the same messages as the app's own page.
(() => {
  const { ICONS, LOGO, customVars } = window.BloxkeepShared;
  const DICT = window.I18N;
  const page = document.body.dataset.page;
  let lang = 'en';
  // A "1" key the language lacks (Vietnamese has no plural) is its plural key in that language, never English.
  function t(key, vars) {
    const own = DICT[lang] || {};
    let s = own[key] ?? (lang !== 'en' && key.endsWith('1') ? own[key.slice(0, -1)] : undefined) ?? DICT.en[key] ?? key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m));
    return s;
  }
  const tn = (key, n, vars = {}) => t(n === 1 && `${key}1` in DICT[lang] ? `${key}1` : key, { n, ...vars });

  const wv = window.chrome && window.chrome.webview, mock = window.__mockBridge;
  // In its window Windows draws the corners and the hairline (a browser preview draws the card's own).
  if (wv) document.documentElement.classList.add('in-window');
  let seq = 0;
  const send = (cmd, args = {}) => {
    if (wv) wv.postMessage({ id: ++seq, cmd, args });
    else if (mock) mock.call(cmd, args);
  };

  const host = { t, tn, get lang() { return lang; }, get locale() { return lang === 'vi' ? 'vi-VN' : 'en-US'; }, logo: LOGO, iconPaths: name => ICONS[name] || '' };
  const ui = window.BloxkeepUI[page](document.getElementById('root'), host, {
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h) => send('size', { w, h }),
    drag: () => send('drag'),
  });

  /** The app's theme, and a custom one's colours (without its picture: these pages are plain). */
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

  let shownTheme = null;
  function show(d) {
    if (!d) return;
    if (d.language && DICT[d.language]) lang = d.language;
    document.documentElement.lang = lang;
    const look = JSON.stringify([d.theme, d.customTheme]);
    if (look !== shownTheme) { shownTheme = look; theme(d.theme, d.customTheme); }
    ui.show(d);
  }
  if (wv) wv.addEventListener('message', e => {
    const m = e.data;
    if (m && m.type === 'event' && m.name === page) show(m.data);
  });
  else if (mock) mock.attach((name, data) => { if (name === page) show(data); });
  send('ready');
})();
