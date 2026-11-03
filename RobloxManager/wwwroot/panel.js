'use strict';
// A macro panel's page (panel.html in PanelWindow.cs's window): draws the panel (ui/src/panel/PanelView.tsx) from the
// app's "panel" message, says a refused change from "panelError", and sends back clicks ("act"), its size ("size") and a
// drag of its title row ("drag"), in the same messages as the app's own page.
(() => {
  const { ICONS, LOGO, customVars } = window.BloxkeepShared;
  const DICT = window.I18N;
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
  const ui = window.BloxkeepUI.panel(document.getElementById('panel'), host, {
    act: (what, args = {}) => send('act', { what, ...args }),
    size: (w, h, pill) => send('size', { w, h, pill }),
    drag: () => send('drag'),
  });

  /** The app's theme, and a custom one's colours (without its picture: the panel is a plain card). */
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
  const error = e => { if (e) ui.error(e); };
  if (wv) wv.addEventListener('message', e => {
    const m = e.data;
    if (!m || m.type !== 'event') return;
    if (m.name === 'panel') show(m.data);
    else if (m.name === 'panelError') error(m.data);
  });
  else if (mock) mock.attach((name, data) => { if (name === 'panel') show(data); else if (name === 'panelError') error(data); });
  send('ready');
})();
