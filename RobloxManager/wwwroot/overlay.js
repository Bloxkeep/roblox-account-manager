'use strict';
// The overlay's page (overlay.html in OverlayWindow.cs's window, over a game or a monitor): draws the macro's dots and
// areas, or the frozen picture to pick on (ui/src/overlay/OverlayView.tsx), from the app's "overlay" event, and sends
// back where things went ("act": dot, area, pick, done, cancel) in parts of the picture, in the app page's own messages.
(() => {
  const { ICONS, LOGO, customVars } = window.BloxkeepShared;
  const DICT = window.I18N;
  let lang = 'en';
  function t(key, vars) {
    const own = DICT[lang] || {};
    let s = own[key] ?? DICT.en[key] ?? key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m));
    return s;
  }
  const tn = (key, n, vars = {}) => t(n === 1 && `${key}1` in DICT[lang] ? `${key}1` : key, { n, ...vars });
  const locale = () => (lang === 'vi' ? 'vi-VN' : 'en-US');

  const wv = window.chrome && window.chrome.webview, mock = window.__mockBridge;
  // In its window the dimming is the window's own (see-through as a whole); a browser preview draws it (app.css).
  if (wv) document.documentElement.classList.add('in-window');
  let seq = 0;
  const send = (cmd, args = {}) => {
    if (wv) wv.postMessage({ id: ++seq, cmd, args });
    else if (mock) mock.call(cmd, args);
  };

  const host = { t, tn, get lang() { return lang; }, get locale() { return locale(); }, logo: LOGO, iconPaths: name => ICONS[name] || '' };
  const ui = window.BloxkeepUI.overlay(document.getElementById('overlay'), host, { act: (what, args = {}) => send('act', { what, ...args }) });

  /** The app's theme for the bar, and a custom one's colours (without its picture). */
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
    document.body.classList.toggle('ov-dim', d.mode === 'edit' || d.mode === 'areas');
    ui.show(d);
  }
  if (wv) wv.addEventListener('message', e => {
    const m = e.data;
    if (m && m.type === 'event' && m.name === 'overlay') show(m.data);
  });
  else if (mock) mock.attach((name, data) => { if (name === 'overlay') show(data); });
  send('ready');
})();
