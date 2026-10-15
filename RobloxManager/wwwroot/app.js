'use strict';
(() => {
  const $ = (sel, root = document) => root.querySelector(sel);
  const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));

  const ICONS = {
    plus: '<path d="M12 5v14M5 12h14"/>',
    search: '<circle cx="11" cy="11" r="7"/><path d="m20 20-3.5-3.5"/>',
    play: '<path d="M8 5.5v13a1 1 0 0 0 1.5.86l10.2-6.5a1 1 0 0 0 0-1.72L9.5 4.64A1 1 0 0 0 8 5.5z" fill="currentColor" stroke="none"/>',
    stop: '<rect x="6.5" y="6.5" width="11" height="11" rx="2" fill="currentColor" stroke="none"/>',
    x: '<path d="M18 6 6 18M6 6l12 12"/>',
    users: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M22 21v-2a4 4 0 0 0-3-3.87M16 3.13a4 4 0 0 1 0 7.75"/>',
    sliders: '<path d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6"/>',
    lock: '<rect x="4" y="11" width="16" height="10" rx="2.5"/><path d="M8 11V7a4 4 0 0 1 8 0v4"/>',
    key: '<circle cx="7.5" cy="15.5" r="4.5"/><path d="m21 2-9.6 9.6M15.5 7.5l3 3L22 7l-3-3"/>',
    edit: '<path d="M17 3a2.85 2.85 0 1 1 4 4L7.5 20.5 2 22l1.5-5.5z"/>',
    trash: '<path d="M3 6h18M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6M8 6V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/>',
    copy: '<rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>',
    check: '<path d="M20 6 9 17l-5-5"/>',
    refresh: '<path d="M21 12a9 9 0 1 1-3-6.7L21 8"/><path d="M21 3v5h-5"/>',
    more: '<circle cx="12" cy="5" r="1.2" fill="currentColor"/><circle cx="12" cy="12" r="1.2" fill="currentColor"/><circle cx="12" cy="19" r="1.2" fill="currentColor"/>',
    eye: '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"/><circle cx="12" cy="12" r="3"/>',
    eyeOff: '<path d="M9.9 4.2A10.4 10.4 0 0 1 12 4c6.5 0 10 8 10 8a17.6 17.6 0 0 1-2.1 3.2M6.6 6.6C3.6 8.6 2 12 2 12s3.5 7 10 7a9.7 9.7 0 0 0 5.4-1.6M14.1 14.1a3 3 0 1 1-4.2-4.2M2 2l20 20"/>',
    folder: '<path d="M4 20h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.7-.9l-.8-1.2A2 2 0 0 0 7.9 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2z"/>',
    shield: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>',
    shieldCheck: '<path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/>',
    alert: '<path d="M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0z"/><path d="M12 9v4M12 17h.01"/>',
    info: '<circle cx="12" cy="12" r="10"/><path d="M12 16v-4M12 8h.01"/>',
    layers: '<path d="m12 2 10 5-10 5L2 7z"/><path d="m2 17 10 5 10-5M2 12l10 5 10-5"/>',
    external: '<path d="M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/>',
    chevronDown: '<path d="m6 9 6 6 6-6"/>',
    chevronRight: '<path d="m9 6 6 6-6 6"/>',
    clock: '<circle cx="12" cy="12" r="9"/><path d="M12 7v5l3 2"/>',
    history: '<path d="M3 12a9 9 0 1 0 3-6.7L3 8"/><path d="M3 3v5h5M12 7v5l3 2"/>',
    userGo: '<circle cx="9" cy="7" r="4"/><path d="M2 21v-1a6 6 0 0 1 9.5-4.9"/><path d="M15 18h7M19 15l3 3-3 3"/>',
    gamepad: '<path d="M6 12h4M8 10v4M15 13h.01M18 11h.01"/><rect x="2" y="6" width="20" height="12" rx="6"/>',
    power: '<path d="M12 2v10M18.4 6.6a9 9 0 1 1-12.8 0"/>',
    server: '<rect x="2" y="3" width="20" height="7" rx="2"/><rect x="2" y="14" width="20" height="7" rx="2"/><path d="M6 6.5h.01M6 17.5h.01"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>',
    userPlus: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M19 8v6M22 11h-6"/>',
    monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>',
    cookie: '<path d="M12 2a10 10 0 1 0 10 10 4 4 0 0 1-5-5 4 4 0 0 1-5-5"/><path d="M8.5 8.5h.01M16 15.5h.01M12 12h.01M11 17h.01M7 14h.01"/>',
    chip: '<rect x="5" y="5" width="14" height="14" rx="2"/><path d="M9 9h6v6H9zM9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
    palette: '<circle cx="13.5" cy="6.5" r="1"/><circle cx="17.5" cy="10.5" r="1"/><circle cx="8.5" cy="7.5" r="1"/><circle cx="6.5" cy="12.5" r="1"/><path d="M12 2a10 10 0 0 0 0 20c.9 0 1.6-.7 1.6-1.7 0-.4-.2-.8-.4-1.1-.3-.3-.4-.7-.4-1.1 0-.9.7-1.7 1.7-1.7h2c3 0 5.5-2.5 5.5-5.5C22 6 17.5 2 12 2z"/>',
    moon: '<path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"/>',
    code: '<path d="m16 18 6-6-6-6M8 6l-6 6 6 6"/>',
    sort: '<path d="M3 6h13M3 12h9M3 18h5M17 10v10M14 17l3 3 3-3"/>',
    languages: '<path d="m5 8 6 6M4 14l6-6 2-3M2 5h12M7 2h1M22 22l-5-10-5 10M14 18h6"/>',
    user: '<circle cx="12" cy="8" r="4"/><path d="M4 21a8 8 0 0 1 16 0"/>',
    qr: '<rect x="3" y="3" width="7" height="7" rx="1"/><rect x="14" y="3" width="7" height="7" rx="1"/><rect x="3" y="14" width="7" height="7" rx="1"/><path d="M14 14h3v3h-3zM21 14v.01M14 21h.01M17 21h4v-4"/>',
    logIn: '<path d="M15 3h4a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2h-4M10 17l5-5-5-5M15 12H3"/>',
    star: '<path d="m12 2.5 2.9 5.9 6.6 1-4.8 4.6 1.1 6.5L12 17.4l-5.8 3.1 1.1-6.5-4.8-4.6 6.6-1z"/>',
    starFill: '<path d="m12 2.5 2.9 5.9 6.6 1-4.8 4.6 1.1 6.5L12 17.4l-5.8 3.1 1.1-6.5-4.8-4.6 6.6-1z" fill="currentColor"/>',
    repeat: '<path d="m17 2 4 4-4 4"/><path d="M3 11v-1a4 4 0 0 1 4-4h14"/><path d="m7 22-4-4 4-4"/><path d="M21 13v1a4 4 0 0 1-4 4H3"/>',
    rows: '<path d="M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01"/>',
    grid: '<rect x="3" y="3" width="7" height="7" rx="1.5"/><rect x="14" y="3" width="7" height="7" rx="1.5"/><rect x="3" y="14" width="7" height="7" rx="1.5"/><rect x="14" y="14" width="7" height="7" rx="1.5"/>',
    download: '<path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4M7 10l5 5 5-5M12 15V3"/>',
    gauge: '<path d="m12 14 4-4"/><path d="M3.34 19a10 10 0 1 1 17.32 0"/>',
    cloud: '<path d="M17.5 19H9a7 7 0 1 1 6.71-9h1.79a4.5 4.5 0 1 1 0 9z"/>',
    bookmark: '<path d="M19 21 12 16l-7 5V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2z"/>',
    tag: '<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><circle cx="7.5" cy="7.5" r="1.5"/>',
    chevronUp: '<path d="m6 15 6-6 6 6"/>',
    grip: '<circle cx="9" cy="6" r="1.3" fill="currentColor"/><circle cx="15" cy="6" r="1.3" fill="currentColor"/><circle cx="9" cy="12" r="1.3" fill="currentColor"/><circle cx="15" cy="12" r="1.3" fill="currentColor"/><circle cx="9" cy="18" r="1.3" fill="currentColor"/><circle cx="15" cy="18" r="1.3" fill="currentColor"/>',
    calendar: '<rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/>',
    screens: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M12 3v14M2 10h20M8 21h8M12 17v4"/>',
    bell: '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>',
    message: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
  };
  const icon = (name, cls = '') => `<svg class="i ${cls}" viewBox="0 0 24 24" aria-hidden="true">${ICONS[name] || ''}</svg>`;
  const LOGO = 'logo.svg';

  /* ───────────────────────── language ───────────────────────── */
  const DICT = window.I18N;
  let lang = 'en';
  /** Translates a key; {name} placeholders are replaced with already-escaped values. */
  function t(key, vars) {
    let s = (DICT[lang] && DICT[lang][key]) ?? DICT.en[key] ?? key;
    if (vars) s = s.replace(/\{(\w+)\}/g, (m, k) => (k in vars ? vars[k] : m));
    return s;
  }
  /**
   * A count in a sentence: the key's "1" form for exactly one when the language has it ("Opened 1 account."), else the
   * key itself (Vietnamese has no plural, so it needs no "1" forms).
   */
  const tn = (key, n, vars = {}) => t(n === 1 && `${key}1` in DICT[lang] ? `${key}1` : key, { n, ...vars });
  document.documentElement.lang = lang; // the page's language from the start, not only after a change
  const locale = () => (lang === 'vi' ? 'vi-VN' : 'en-US');
  const langSwitch = () => `<div class="lang" role="group" aria-label="Language">${['en', 'vi'].map(l =>
    `<button data-act="lang" data-lang="${l}" class="${l === lang ? 'on' : ''}" aria-pressed="${l === lang}" title="${esc(DICT[l]['lang.name'])}">${l.toUpperCase()}</button>`).join('')}</div>`;

  /* ───────────────────────── bridge ─────────────────────────
     The page never talks to the network: it sends commands to the app and receives replies/events. */
  const bridge = (() => {
    const wv = window.chrome && window.chrome.webview;
    const mock = window.__mockBridge;
    const waiting = new Map();
    const listeners = {};
    // Not from 0: a page recreated mid-command must not take a reply meant for the previous page's call.
    let seq = Math.floor(Math.random() * 1e9);
    const emit = (name, data) => (listeners[name] || []).forEach(fn => {
      try { fn(data); } catch (e) { console.error(e); }
    });
    if (wv) {
      wv.addEventListener('message', e => {
        const m = e.data;
        if (!m) return;
        if (m.type === 'reply') {
          const p = waiting.get(m.id);
          if (!p) return;
          waiting.delete(m.id);
          if (m.ok) p.resolve(m.result); else p.reject(new Error(m.error || t('common.unknownError')));
        } else if (m.type === 'event') {
          emit(m.name, m.data);
        }
      });
    } else if (mock) {
      mock.attach(emit);
    }
    return {
      call(cmd, args = {}) {
        if (!wv) return mock ? mock.call(cmd, args) : Promise.reject(new Error(t('common.noBridge')));
        return new Promise((resolve, reject) => {
          const id = ++seq;
          waiting.set(id, { resolve, reject });
          wv.postMessage({ id, cmd, args });
        });
      },
      on(name, fn) { (listeners[name] ||= []).push(fn); },
    };
  })();

  /** Calls a command; failures show a toast and resolve to undefined so callers never see a rejection. */
  async function call(cmd, args, { silent = false } = {}) {
    try {
      return await bridge.call(cmd, args);
    } catch (e) {
      if (!silent) toast(e.message, 'error');
      return undefined;
    }
  }

  /* ───────────────────────── state ───────────────────────── */
  const store = {
    get(key, fallback) { try { return localStorage.getItem(key) ?? fallback; } catch { return fallback; } },
    set(key, value) { try { localStorage.setItem(key, value); } catch { /* storage unavailable */ } },
  };
  const S = {
    app: null,            // last state pushed by the app
    shell: false,         // main shell rendered
    lockMode: null,       // 'create' | 'unlock' while the lock screen is shown
    view: 'accounts',
    selected: new Set(),
    anchor: null,
    filter: 'all',
    group: '',            // account group narrowing the list (its own menu next to Sort; '' = every group)
    layout: store.get('rm.layout', 'cards') === 'list' ? 'list' : 'cards',
    sort: ['name', 'status', 'recent', 'order'].includes(store.get('rm.sort')) ? store.get('rm.sort') : 'name',
    query: '',
    progress: null,
    game: null,
    gameSeq: 0,
    introShown: false,
    metrics: { accounts: [], robloxCount: 0, totalMb: 0, systemMb: 0 },
  };
  const accounts = () => (S.app && S.app.accounts) || [];
  const byId = id => accounts().find(a => a.id === id);
  const isLocal = a => a.state === 'running';
  /** Auto-rejoin for the account's client: a preset that opened it may turn it on or off for that client. */
  const rejoinOn = a => a.session?.autoRejoin ?? a.autoRejoin;
  /** A window open here whose account is no longer in a server (kicked / disconnected), or opened without a game. */
  const isStranded = a => isLocal(a) && ['disconnected', 'menu', 'signedout'].includes(a.session?.connection);
  const inGame = a => (isLocal(a) && !isStranded(a)) || a.presence?.type === 'ingame';
  const isOnline = a => isLocal(a) || ['online', 'ingame', 'studio'].includes(a.presence?.type);

  /**
   * Settings › Appearance › Hide account names (screenshots, streams): each account shows as "Account 1", "Account 2"…
   * (its place in the list, as the app numbers them in toasts and window titles) and the @username lines go. The real
   * names stay in a.real for the search box and the name sort only; copying a username asks the app, which has it.
   */
  const hidingNames = () => !!S.app?.hideNames;
  function maskNames(st) {
    if (!st.accounts) return;
    // Copies: what the bridge handed over stays as it came (the preview's fake bridge hands over its own objects).
    st.accounts = st.accounts.map((a, i) => {
      const real = a.real || { label: a.label, username: a.username, displayName: a.displayName, alias: a.alias };
      return { ...a, real, ...(st.hideNames ? { label: t('hide.account', { n: i + 1 }), username: '', displayName: '', alias: '' } : real) };
    });
  }
  /** "@username", or nothing while names are hidden. */
  const atName = a => (a.username ? `@${esc(a.username)}` : '');

  /* ───────────────────────── formatting ───────────────────────── */
  function ago(iso) {
    if (!iso) return '';
    const s = (Date.now() - new Date(iso).getTime()) / 1000;
    if (s < 60) return t('time.now');
    if (s < 3600) return t('time.min', { n: Math.floor(s / 60) });
    if (s < 86400) return t('time.hour', { n: Math.floor(s / 3600) });
    if (s < 86400 * 2) return t('time.day1');
    if (s < 86400 * 30) return t('time.day', { n: Math.floor(s / 86400) });
    return new Date(iso).toLocaleDateString(locale());
  }
  function duration(iso) {
    const m = Math.max(0, Math.floor((Date.now() - new Date(iso).getTime()) / 60000));
    return m < 60 ? t('dur.min', { m }) : t('dur.hour', { h: Math.floor(m / 60), m: m % 60 });
  }
  /**
   * One Intl formatter per language and options, made once: making one per number or date was most of the time a push
   * spent on the preset rows and the RAM figures. ('n' numbers, 'd' dates and times.)
   */
  const INTL = new Map();
  const intl = (kind, opts = {}) => {
    const key = `${kind}|${locale()}|${JSON.stringify(opts)}`;
    let f = INTL.get(key);
    if (!f) INTL.set(key, f = kind === 'n' ? new Intl.NumberFormat(locale(), opts) : new Intl.DateTimeFormat(locale(), opts));
    return f;
  };
  const fmtNum = n => intl('n').format(n);
  /** "HH:mm" as the app's language writes a time: "10:00 PM" in English, "22:00" (24-hour) in Vietnamese. */
  const fmtClock = hhmm => {
    const [h, m] = String(hhmm || '').split(':').map(Number);
    if (!Number.isInteger(h) || !Number.isInteger(m)) return hhmm || '';
    return intl('d', lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric', minute: '2-digit' }).format(new Date(2000, 0, 1, h, m));
  };
  const fmtMb = mb => mb >= 1024 ? `${intl('n', { maximumFractionDigits: 1 }).format(mb / 1024)} GB` : `${Math.round(mb)} MB`;
  const hue = s => { let h = 7; for (const c of String(s)) h = (h * 31 + c.charCodeAt(0)) >>> 0; return h % 360; };
  const initials = s => (String(s).match(/[\p{L}\p{N}]/gu) || ['?']).slice(0, 2).join('').toUpperCase();
  const avatarInner = a => a.avatarUrl
    ? `<img src="${esc(a.avatarUrl)}" alt="" draggable="false" loading="lazy" decoding="async" data-ini="${esc(initials(a.label || a.username))}" style="--h:${hue(a.username || a.label)}">`
    : `<span class="initials" style="--h:${hue(a.username || a.label)}">${esc(initials(a.label || a.username))}</span>`;

  /* ───────────────────────── toasts ───────────────────────── */
  /** @param action optional { label, run } shown as a button; the toast then stays a little longer. */
  function toast(text, kind = 'info', action = null) {
    const box = $('#toasts');
    if (!box || !text) return;
    // Long messages stay long enough to be read (about 60 ms a character).
    const life = Math.min(Math.max(kind === 'error' ? 6500 : action ? 9000 : 3800, text.length * 60), 15000);
    // The same message again while it still shows (a double click, a repeated event): the one toast stays longer
    // instead of a second copy stacking under it.
    const same = !action && [...box.children].find(x => !x.classList.contains('out') && x.dataset.text === text && x.classList.contains(kind));
    if (same) {
      clearTimeout(same._timer);
      // Held while pointed at or focused: leaving it starts the countdown, as for any toast.
      if (!same.matches(':hover') && !same.contains(document.activeElement)) same._timer = setTimeout(same._dismiss, life);
      same.classList.remove('again');
      void same.offsetWidth; // restarts the nudge animation
      same.classList.add('again');
      return;
    }
    while (box.children.length >= 3) box.firstElementChild.remove(); // three at most: a stack mustn't cover the page
    const el = document.createElement('div');
    el.className = `toast ${kind}`;
    el.dataset.text = text;
    el.setAttribute('role', kind === 'error' ? 'alert' : 'status');
    el.innerHTML = `${icon(kind === 'ok' ? 'check' : kind === 'info' ? 'info' : 'alert')}<span>${esc(text)}</span>${action ? `<button class="toast-act">${esc(action.label)}</button>` : ''}`;
    const dismiss = () => {
      el.classList.add('out');
      setTimeout(() => el.remove(), 220);
    };
    el._dismiss = dismiss;
    if (action) el.querySelector('.toast-act').addEventListener('click', () => { action.run(); dismiss(); });
    // A click anywhere else on it closes it (like a Windows notification); pointing at it only holds it.
    el.addEventListener('click', e => { if (!e.target.closest('.toast-act')) { clearTimeout(el._timer); dismiss(); } });
    // Kept while the pointer or the focus is on it (being read, or its button about to be pressed).
    const hold = () => clearTimeout(el._timer);
    const resume = () => { clearTimeout(el._timer); el._timer = setTimeout(dismiss, 2500); };
    el.addEventListener('mouseenter', hold);
    el.addEventListener('focusin', hold);
    el.addEventListener('mouseleave', () => { if (!el.contains(document.activeElement)) resume(); });
    el.addEventListener('focusout', () => { if (!el.matches(':hover')) resume(); });
    box.append(el);
    el._timer = setTimeout(dismiss, life);
  }
  /** While a dialog or menu is up, toasts sit behind it (still readable through the backdrop) rather than over its edge. */
  function syncToastLayer() {
    $('#toasts')?.classList.toggle('behind', openModals > 0 || !!menuEl);
    syncMonitor(); // the monitor tab's pictures would cover the dialog or menu
  }
  /** When a dialog or menu last closed: a double click that closed one mustn't also act on the card under it. */
  let lastOverlayClose = 0;

  /* ───────────────────────── modal & menu ───────────────────────── */
  let openModals = 0, busyModals = 0, pageBusy = false;
  /**
   * Tells the app when the page is in the middle of something it would lose with the browser (a form being typed
   * in, a sign-in under way) so it isn't closed in the background. A list or a notice open is no reason to keep it.
   */
  function syncBusy() {
    const busy = busyModals > 0;
    if (busy === pageBusy) return;
    pageBusy = busy;
    call('pageBusy', { busy }, { silent: true });
  }
  /**
   * @param dismissable false keeps forms open on a stray backdrop click (Esc and the X still close).
   * @param busy true for a form with typed input or a sign-in: the app keeps the browser alive in the background meanwhile.
   * @param enterSubmits true for a form: Enter in a field presses the primary button (not in a textarea or a box with its list open).
   * @param askClose returns true when Esc or the X would throw away something typed: the user is asked first.
   */
  function modal({ title, body, actions = [], onMount, onClose, dismissable = true, busy: keepsPage = false, enterSubmits = false, askClose = null }) {
    closeMenu();
    // Gets the focus back on close. Opened from a busy button (What's new, Run's "Open anyway?"), which lost the focus
    // when it was turned off: that button.
    const opener = document.activeElement && document.activeElement !== document.body ? document.activeElement
      : busyFocused?.isConnected ? busyFocused : null;
    // A card's button that opened it may be rebuilt meanwhile (Edit → save): the same button on the new card then.
    const openerCard = opener?.closest('.card')?.dataset.id, openerAct = opener?.dataset.act;
    const openerHost = opener?.parentElement?.closest('[id]:not(#app, main, .view, #layer)'), openerCls = opener?.classList[0];
    const el = document.createElement('div');
    el.className = 'overlay';
    const titleId = `mdl-${++modalSeq}`;
    el.innerHTML = `<div class="modal" role="dialog" aria-modal="true" ${title ? `aria-labelledby="${titleId}"` : ''}>
      <div class="modal-head"><h3 id="${titleId}">${title}</h3><button class="icon-btn" data-x title="${t('common.close')}">${icon('x')}</button></div>
      <div class="modal-body">${body}</div>
      ${actions.length ? `<div class="modal-foot">${actions.map((a, i) => `<button class="btn ${a.kind || ''}" data-i="${i}">${a.text}</button>`).join('')}</div>` : ''}
    </div>`;
    let closed = false;
    const focusables = () => [...el.querySelectorAll('button, input, select, textarea, a[href], [tabindex]:not([tabindex="-1"])')]
      .filter(x => !x.disabled && !x.hidden && x.offsetParent !== null);
    const onKey = e => {
      // Each open modal listens; only the one on top reacts.
      if ([...document.querySelectorAll('#layer .overlay:not(.out)')].pop() !== el) return;
      if (e.key === 'Escape') {
        // A box with its list open (the preset form's game box) closes the list first, not the whole form.
        if (document.activeElement?.getAttribute('aria-expanded') === 'true') return;
        e.stopImmediatePropagation();
        tryClose();
      } else if (e.key === 'Enter' && enterSubmits && !e.repeat && e.target.matches('input, select') && e.target.getAttribute('aria-expanded') !== 'true') {
        const primary = el.querySelector('.modal-foot .btn.primary');
        if (!primary || primary.disabled) return;
        e.preventDefault();
        e.stopImmediatePropagation();
        primary.click();
      } else if (e.key === 'Tab') {
        // Tab goes round inside the modal instead of leaving for the page behind it.
        const f = focusables();
        if (!f.length) return;
        const inside = el.contains(document.activeElement);
        if (e.shiftKey && (!inside || document.activeElement === f[0])) { e.preventDefault(); f[f.length - 1].focus(); }
        else if (!e.shiftKey && (!inside || document.activeElement === f[f.length - 1])) { e.preventDefault(); f[0].focus(); }
      }
    };
    const close = () => {
      if (closed) return;
      closed = true;
      openModals--;
      if (keepsPage) busyModals--;
      document.removeEventListener('keydown', onKey, true);
      el.classList.add('out');
      setTimeout(() => el.remove(), 150);
      lastOverlayClose = performance.now();
      onClose?.();
      syncBusy();
      syncToastLayer();
      $('#app').inert = openModals > 0;
      let back = opener?.isConnected ? opener : null;
      if (!back && openerCard) {
        const card = $(`.card[data-id="${openerCard}"]`);
        back = card && (openerAct && card.querySelector(`[data-act="${openerAct}"]`) || card);
      }
      // Another part drawn again meanwhile (the sidebar's Multi-Roblox card): the same kind of button in it.
      if (!back && openerHost?.isConnected && !el.contains(openerHost)) {
        back = (openerAct && openerHost.querySelector(`[data-act="${CSS.escape(openerAct)}"]`)) || (openerCls && openerHost.querySelector(`.${CSS.escape(openerCls)}`));
      }
      if (back && !el.contains(back)) back.focus();
    };
    let asking = false;
    // A save under way (see hold): the dialog stays until it is through, so Cancel can't look like it undid it.
    let held = 0;
    const tryClose = async () => {
      if (asking || closed || held) return;
      if (askClose?.()) {
        asking = true;
        const leave = await confirmBox(t('common.discardQ'), { title: t('common.discardTitle'), ok: t('common.discard'), danger: true });
        asking = false;
        if (!leave) return;
      }
      close();
    };
    el._close = close; // closeAllModals (lock, sign-out) closes without asking
    openModals++;
    if (keepsPage) busyModals++;
    syncBusy();
    syncToastLayer();
    $('#app').inert = true; // the page behind can't be reached (or tabbed into) while the modal is up
    document.addEventListener('keydown', onKey, true);
    // Not the second press of the double click that opened it (the dialog appears under the pointer).
    const openedAt = performance.now();
    if (dismissable) el.addEventListener('mousedown', e => { if (e.target === el && e.detail < 2 && performance.now() - openedAt > 450) tryClose(); });
    el.querySelector('[data-x]').onclick = tryClose;
    const api = {
      el,
      close: () => { if (!held) close(); },
      /** Runs a save with the dialog kept open meanwhile (Cancel, the X and Esc wait for it). */
      hold: async job => { held++; try { return await job(); } finally { held--; } },
    };
    el.querySelectorAll('.modal-foot .btn').forEach(btn => {
      btn.onclick = () => actions[+btn.dataset.i].onClick?.({ ...api, btn });
    });
    $('#layer').append(el);
    onMount?.(api);
    // Most modals focus one of their fields themselves; the rest start at their first control.
    setTimeout(() => {
      if (closed || el.contains(document.activeElement)) return;
      (el.querySelector('.modal-body').querySelector('button:not(:disabled), input, select, textarea, a[href]') || el.querySelector('.modal-foot .btn.primary') || el.querySelector('[data-x]')).focus();
    }, 60);
    return api;
  }
  /** Closes every open modal, e.g. when the vault locks or the language changes. */
  function closeAllModals() {
    document.querySelectorAll('#layer .overlay:not(.out)').forEach(x => x._close?.());
  }
  let modalSeq = 0;

  /** @param danger a destructive answer: the focus starts on Cancel, so a stray Enter doesn't delete or restore. */
  function confirmBox(message, { title = t('common.confirm'), ok = 'OK', danger = false } = {}) {
    return new Promise(resolve => {
      let answer = false;
      const m = modal({
        title,
        body: `<p class="pre modal-text">${esc(message)}</p>`,
        actions: [
          { text: t('common.cancel'), onClick: ({ close }) => close() },
          { text: ok, kind: danger ? 'danger' : 'primary', onClick: ({ close }) => { answer = true; close(); } },
        ],
        onClose: () => resolve(answer),
      });
      setTimeout(() => m.el.querySelector(`.modal-foot .btn:${danger ? 'first' : 'last'}-child`)?.focus(), 30);
    });
  }

  let menuEl = null, menuOpener = null;
  /** @param refocus true (from the keyboard) puts the focus back on the button that opened the menu. */
  function closeMenu(refocus = false) {
    if (!menuEl) return;
    menuEl.remove();
    menuEl = null;
    lastOverlayClose = performance.now();
    syncToastLayer();
    if (menuOpener?.hasAttribute('aria-haspopup')) menuOpener.setAttribute('aria-expanded', 'false');
    if (refocus === true && menuOpener?.isConnected) menuOpener.focus();
    menuOpener = null;
  }
  function openMenu(items, x, y) {
    closeMenu();
    menuOpener = document.activeElement !== document.body ? document.activeElement : null;
    const byKeyboard = !!menuOpener?.matches(':focus-visible');
    if (menuOpener?.hasAttribute('aria-haspopup')) menuOpener.setAttribute('aria-expanded', 'true');
    const el = document.createElement('div');
    el.className = 'menu';
    el.setAttribute('role', 'menu');
    el.innerHTML = items.map((it, i) => it === '-' ? '<hr>'
      : `<button role="menuitem" data-mi="${i}" class="${it.danger ? 'danger' : ''} ${it.checked ? 'checked' : ''}" ${it.disabled ? 'disabled' : ''}>${icon(it.checked ? 'check' : it.icon)}<span${it.title ? ` title="${it.title}"` : ''}>${it.text}</span>${it.hint ? `<kbd>${it.hint}</kbd>` : ''}</button>`).join('');
    el.addEventListener('click', e => {
      const b = e.target.closest('[data-mi]');
      if (!b) return;
      const item = items[+b.dataset.mi];
      closeMenu(e.detail === 0); // picked with the keyboard: the focus goes back to the button first
      item.run();
    });
    $('#layer').append(el);
    menuEl = el;
    syncToastLayer();
    const r = el.getBoundingClientRect();
    el.style.left = `${Math.max(8, Math.min(x, innerWidth - r.width - 8))}px`;
    el.style.top = `${Math.max(8, Math.min(y, innerHeight - r.height - 8))}px`;
    if (byKeyboard) el.querySelector('button:not(:disabled)')?.focus();
  }
  const menuAt = (btn, items, width) => {
    const r = btn.getBoundingClientRect();
    openMenu(items, width ? r.right - width : r.left, r.bottom + 6);
  };
  document.addEventListener('mousedown', e => { if (menuEl && !menuEl.contains(e.target)) closeMenu(); }, true);
  window.addEventListener('blur', closeMenu);
  window.addEventListener('resize', closeMenu);

  /** Disables a button and shows a spinner while the action runs. */
  let busyFocused = null;
  async function busy(btn, fn) {
    if (btn?.disabled) return undefined;
    // A disabled button loses the focus; it gets it back afterwards unless the focus went somewhere else meanwhile.
    const focused = btn && document.activeElement === btn;
    // The answer's state push may draw the button anew (Back up now, Send test, a preset's Run): the new one is found
    // by what it does.
    const key = focused ? focusKey(btn, document.body).sel : '';
    btn?.classList.add('busy');
    if (btn) btn.disabled = true;
    if (focused) busyFocused = btn; // a dialog it opens gives the focus back to it (see modal)
    try {
      return await fn();
    } finally {
      if (busyFocused === btn) busyFocused = null;
      // A dialog this opened and that just closed (still fading out) holds the focus only for a moment: that is lost too.
      const lost = document.activeElement === document.body || !document.activeElement || !!document.activeElement.closest?.('.overlay.out');
      if (btn?.isConnected) {
        btn.classList.remove('busy');
        btn.disabled = false;
        if (focused && lost) btn.focus();
      } else if (key && lost) {
        const again = document.querySelector(key);
        // Without scrolling: the redrawn one may sit lower (Show more's, under the rows it added) — the view stays put.
        if (again && !again.disabled && !again.hidden) again.focus({ preventScroll: true });
      }
    }
  }

  /** The eye next to a password box: shows or hides it, and says which (tooltip, pressed state). */
  function eyeToggle(btn, inputs) {
    if (!btn) return;
    btn.onclick = () => {
      const show = inputs[0].type === 'password';
      inputs.forEach(i => i && (i.type = show ? 'text' : 'password'));
      btn.innerHTML = icon(show ? 'eyeOff' : 'eye');
      btn.title = t(show ? 'lock.hidePw' : 'lock.showPw');
      btn.setAttribute('aria-label', btn.title);
      btn.setAttribute('aria-pressed', String(show));
    };
  }

  /** A master password's strength as a bar and a word (weak / OK / strong); only a guide, any 6+ characters are accepted. */
  const strengthScore = v => [v.length >= 8, v.length >= 12, /[a-z]/.test(v) && /[A-Z]/.test(v), /\d/.test(v), /[^A-Za-z0-9]/.test(v)].filter(Boolean).length;
  const strengthLevel = v => { const s = strengthScore(v); return s <= 1 ? 'weak' : s <= 3 ? 'ok' : 'strong'; };
  function showStrength(v, meter, label) {
    const score = strengthScore(v), level = strengthLevel(v);
    if (meter) {
      meter.style.width = `${v ? Math.max(12, score * 20) : 0}%`;
      meter.style.background = `var(--${level === 'weak' ? 'bad' : level === 'ok' ? 'warn' : 'ok'})`;
    }
    if (label) {
      label.textContent = v ? t(`lock.strength.${level}`) : '';
      label.className = `pw-strength ${v ? level : ''}`;
    }
  }

  /* ───────────────────────── lock screen ───────────────────────── */
  function renderLock() {
    const mode = S.app.vaultExists ? 'unlock' : 'create';
    if (S.lockMode === mode) return; // keep what the user is typing
    // Rebuilt in another language (same screen): what was typed stays.
    const form0 = $('#lock-form');
    const kept = form0?.dataset.mode === mode ? { pw1: $('#pw1')?.value || '', pw2: $('#pw2')?.value || '' } : null;
    S.lockMode = mode;
    S.shell = false;
    closeMenu();
    closeAllModals();
    syncMonitor();
    const create = mode === 'create';
    $('#app').innerHTML = `<div class="lock">
      <div class="dots"></div>
      <div class="lock-lang">${langSwitch()}</div>
      <form class="lock-card" id="lock-form" autocomplete="off" data-mode="${mode}">
        <img class="logo-img" src="${LOGO}" alt="">
        <h2>${t(create ? 'lock.createTitle' : 'lock.unlockTitle')}</h2>
        <p>${t(create ? 'lock.createText' : 'lock.unlockText')}</p>
        <div class="stack">
          <div class="pw"><input class="field" type="password" id="pw1" placeholder="${t('lock.pw')}" maxlength="256" aria-label="${t('lock.pw')}">
            <button type="button" class="icon-btn" id="pw-eye" title="${t('lock.showPw')}" aria-label="${t('lock.showPw')}" aria-pressed="false">${icon('eye')}</button></div>
          ${create ? `<input class="field" type="password" id="pw2" placeholder="${t('lock.pw2')}" maxlength="256" aria-label="${t('lock.pw2')}">
            <div class="strength-row"><div class="strength"><i id="pw-meter"></i></div><span id="pw-strength" aria-live="polite"></span></div>` : ''}
        </div>
        <div class="lock-caps" id="lock-caps" hidden>${icon('alert')}${t('lock.caps')}</div>
        <div class="lock-error" id="lock-error" role="alert"></div>
        <button class="btn primary big w" type="submit" id="lock-btn">${t(create ? 'lock.create' : 'lock.unlock')}</button>
        ${create ? `<div class="lock-alt"><button type="button" class="link" data-act="restore-backup">${t('lock.restore')}</button></div>` : ''}
      </form>
    </div>`;
    const pw1 = $('#pw1'), pw2 = $('#pw2'), form = $('#lock-form');
    setTimeout(() => pw1.focus(), 50);
    eyeToggle($('#pw-eye'), [pw1, pw2]);
    // An old error goes once the password is being typed again; Caps Lock is pointed out while it is on.
    for (const input of [pw1, pw2]) {
      input?.addEventListener('input', () => setText($('#lock-error'), ''));
      input?.addEventListener('keydown', e => { $('#lock-caps').hidden = !e.getModifierState?.('CapsLock'); });
      input?.addEventListener('keyup', e => { $('#lock-caps').hidden = !e.getModifierState?.('CapsLock'); });
    }
    if (create) {
      pw1.addEventListener('input', () => showStrength(pw1.value, $('#pw-meter'), $('#pw-strength')));
    }
    if (kept) {
      pw1.value = kept.pw1;
      if (pw2) pw2.value = kept.pw2;
      if (create) showStrength(pw1.value, $('#pw-meter'), $('#pw-strength'));
    }
    form.addEventListener('submit', async e => {
      e.preventDefault();
      const err = $('#lock-error');
      const fail = msg => {
        err.textContent = msg;
        form.classList.remove('shake');
        void form.offsetWidth;
        form.classList.add('shake');
      };
      if (create) {
        if (pw1.value.length < 6) return fail(t('lock.short'));
        if (!pw1.value.trim()) return fail(t('lock.blank'));
        if (pw1.value !== pw2.value) return fail(t('lock.mismatch'));
      } else if (!pw1.value) {
        return fail(t('lock.empty'));
      }
      err.textContent = '';
      let state;
      await busy($('#lock-btn'), async () => {
        try {
          state = await bridge.call(create ? 'createVault' : 'unlock', { password: pw1.value });
        } catch (ex) {
          fail(ex.message);
          pw1.select();
        }
      });
      if (state) {
        pw1.value = '';
        if (pw2) pw2.value = '';
        applyState(state);
      }
    });
  }

  /* ───────────────────────── shell ───────────────────────── */
  function buildShell() {
    S.shell = true;
    S.lockMode = null;
    // Dialogs stay open (the lock screen already closed them): a language change from elsewhere mustn't throw away a
    // form being typed in. They keep their old language until closed.
    $('#app').innerHTML = `<div class="shell">
      <aside class="side">
        <div class="brand"><img src="${LOGO}" alt=""><div><div class="brand-name">Bloxkeep</div><div class="brand-sub">${t('brand.sub')}</div></div></div>
        <div class="side-nav">
        <button class="nav" data-act="view" data-view="accounts" title="${t('nav.accounts')}">${icon('users')}<span>${t('nav.accounts')}</span><em id="nav-count">0</em></button>
        <button class="nav" data-act="view" data-view="presets" title="${t('nav.presets')}">${icon('layers')}<span>${t('nav.presets')}</span></button>
        <button class="nav" data-act="view" data-view="monitor" title="${t('nav.monitor')}">${icon('screens')}<span>${t('nav.monitor')}</span></button>
        <button class="nav" data-act="view" data-view="stats" title="${t('nav.stats')}">${icon('clock')}<span>${t('nav.stats')}</span></button>
        <button class="nav" data-act="view" data-view="settings" title="${t('nav.settings')}">${icon('sliders')}<span>${t('nav.settings')}</span></button>
        <button class="nav nav-mini" data-act="mini" title="${t('sys.mini')}" aria-pressed="false">${icon('monitor')}<span>${t('sys.mini')}</span></button>
        </div>
        <div class="side-bottom">
        <div id="side-warn"></div>
        <div class="side-card" id="sys-card">
          <h4>${t('sys.title')}</h4>
          <div class="sys-row"><span><b id="sys-count">0</b> <span id="sys-count-label">${t('sys.windows')}</span></span><span id="sys-ram">0 MB</span></div>
          <div class="meter" id="sys-meter"><i class="roblox"></i><i class="other"></i></div>
          <div class="sys-foot" id="sys-foot"></div>
          <div class="sys-ver" id="sys-ver"></div>
          <button class="btn sm w sys-mini" id="sys-mini" data-act="mini" aria-pressed="false">${icon('monitor')}<span>${t('sys.mini')}</span></button>
          <button class="btn sm w sys-close" id="sys-close" data-act="close-all" hidden>${icon('power')}<span>${t('sys.closeAll')}</span></button>
        </div>
        <div id="multi-card"></div>
        <div class="side-foot">
          <div id="update-slot"></div>
          <button class="btn ghost w" data-act="lock" title="${t('side.lockTip')}">${icon('lock')}<span>${t('side.lock')}</span></button>
        </div>
        </div>
      </aside>
      <main class="main">
        <section class="view" id="view-accounts">
          <header class="top">
            <div class="top-title"><h1>${t('acc.title')}</h1><p id="acc-sub"></p></div>
            <div class="top-tools">
              <label class="search">${icon('search')}<input id="q" placeholder="${t('acc.search')}" maxlength="100" aria-label="${t('acc.search')}"></label>
              <button class="icon-btn" data-act="refresh-presence" title="${t('acc.refreshTip')}">${icon('refresh')}</button>
              <button class="icon-btn" data-act="tools-menu" title="${t('acc.moreTip')}" aria-haspopup="menu">${icon('more')}</button>
              <div class="split">
                <button class="btn primary" data-act="add-choose">${icon('userPlus')}<span>${t('acc.add')}</span></button>
                <button class="btn primary" data-act="add-menu" title="${t('acc.addMoreTip')}" aria-haspopup="menu">${icon('chevronDown')}</button>
              </div>
            </div>
          </header>
          <div class="bar"><div class="chips" id="chips"></div><div class="seg" id="layout-seg"></div><button class="sort-btn" data-act="group-menu" id="group-btn" aria-haspopup="menu" hidden></button><button class="sort-btn" data-act="sort-menu" id="sort-btn" aria-haspopup="menu"></button></div>
          <div class="scroll" id="scroll"><div id="grid"></div></div>
          <div class="dock">
            <button class="game-thumb" id="game-thumb" data-act="browse-games" title="${t('dock.browseTip')}">${icon('gamepad')}</button>
            <div class="picker" id="picker" hidden></div>
            <div class="dock-mid">
              <div class="dock-inputs">
                <label class="inwrap grow" title="${esc(t('dock.placeTip'))}">${icon('search')}<input id="place" autocomplete="off" placeholder="${t('dock.place')}" maxlength="500" spellcheck="false" aria-label="${t('dock.place')}" role="combobox" aria-haspopup="listbox" aria-autocomplete="list" aria-controls="picker-list" aria-expanded="false"></label>
                <label class="inwrap job" title="${esc(t('dock.jobTip'))}">${icon('server')}<input id="job" placeholder="${t('dock.job')}" maxlength="60" spellcheck="false" aria-label="${t('dock.job')}"></label>
                <button class="dock-join" data-act="join-player" title="${esc(t('jp.buttonTip'))}" aria-label="${esc(t('jp.button'))}">${icon('userGo')}</button>
              </div>
              <div class="dock-info" id="dock-info" aria-live="polite"></div>
            </div>
            <div class="dock-actions" id="dock-actions"></div>
          </div>
        </section>
        <section class="view" id="view-presets" hidden>${presetsViewHtml()}</section>
        <section class="view" id="view-stats" hidden>${statsHtml()}</section>
        <section class="view" id="view-monitor" hidden>${monitorHtml()}</section>
        <section class="view" id="view-settings" hidden>${settingsHtml()}</section>
      </main>
    </div>`;

    const st = S.app.settings;
    $('#place').value = st.lastPlaceInput || '';
    $('#job').value = st.lastJobId || '';
    $('#q').value = S.query;
    // Filter once typing pauses, not on every key.
    let searchTimer = 0;
    $('#q').addEventListener('input', e => {
      S.query = e.target.value;
      clearTimeout(searchTimer);
      searchTimer = setTimeout(renderGrid, 120);
    });
    $('#place').addEventListener('input', onPlaceInput);
    $('#place').addEventListener('focus', () => { if (!$('#place').value.trim()) openPicker('browse'); });
    $('#place').addEventListener('keydown', onPlaceKey);
    // Tab on to the Job ID box (or anywhere else on the page) closes the list; clicks are handled on mousedown below.
    $('#place').addEventListener('focusout', e => {
      if (e.relatedTarget && !e.relatedTarget.closest('#picker, #game-thumb')) closePicker();
    });
    // Choosing in the list keeps the focus in the game box (the rows and tabs would otherwise take it).
    $('#picker').addEventListener('mousedown', e => { if (e.target.closest('#picker')) e.preventDefault(); });
    // A card, or a button on it, that gets the focus (a click, Shift+Tab) becomes the list's one Tab stop.
    $('#grid').addEventListener('focusin', e => {
      const card = e.target.closest('.card');
      if (card && card.tabIndex !== 0 && card.parentElement) setTabStop(card.parentElement, card);
    });
    // One row of filter chips: the wheel scrolls it sideways, and the faded edge says there is more.
    const chips = $('#chips');
    chips.addEventListener('wheel', e => {
      if (chips.scrollWidth <= chips.clientWidth || Math.abs(e.deltaX) > Math.abs(e.deltaY)) return;
      e.preventDefault();
      chips.scrollLeft += e.deltaY;
    }, { passive: false });
    chips.addEventListener('scroll', syncChipFade, { passive: true });
    chipObserver.observe(chips);
    dockObserver.observe($('#dock-info'));
    // The list opened from the thumbnail: arrow keys hand over to the game box, which drives it.
    $('#game-thumb').addEventListener('keydown', e => {
      // Esc closes the list first (the page's own Esc would clear the selection and leave the list open).
      if (P.open && e.key === 'Escape') { e.stopPropagation(); return closePicker(); }
      if (!P.open || (e.key !== 'ArrowDown' && e.key !== 'ArrowUp')) return;
      $('#place').focus();
      onPlaceKey(e);
    });
    $('#job').addEventListener('input', scheduleSaveInputs);
    $('#job').addEventListener('keydown', e => { if (e.key === 'Enter') launch([...S.selected], false); });
    wireSettings();
    wirePresetsView();
    setView(S.view);
    resolveGame();
  }

  function update() {
    renderSide();
    renderHeader();
    renderChips();
    renderGrid();
    renderDock();
    renderPresetsView();
    syncSettings();
    syncMultiHelp();
    if (S.view === 'stats') renderStats(); // names shown or hidden follow at once (unchanged figures cost nothing)
  }

  /** Typed passwords don't stay in the page once Settings is left. */
  function clearPwForm() {
    for (const id of ['#pw-cur', '#pw-new', '#pw-new2']) { const el = $(id); if (el) el.value = ''; }
    const msg = $('#pw-msg');
    if (msg) { msg.textContent = ''; msg.className = 'pw-msg'; }
  }

  /** A theme picked with the arrow keys shows at once; the app saves it once the keys rest. */
  let themeTimer = 0;
  function previewTheme(id) {
    clearTimeout(themeTimer);
    S.themePending = id;
    if (id !== 'system') document.documentElement.dataset.theme = id;
    $('#s-themes').querySelectorAll('.swatch').forEach(s => {
      const on = s.dataset.id === id;
      s.classList.toggle('on', on);
      s.setAttribute('aria-checked', String(on));
      s.tabIndex = on ? 0 : -1;
    });
    setText($('#s-theme-name'), t(`theme.${id}`));
    themeTimer = setTimeout(async () => {
      if (id !== S.app.theme) await call('setTheme', { theme: id });
      if (S.themePending !== id) return;
      S.themePending = null;
      // The state that came with the answer was held back while this pick waited ("Sync with Windows" has no
      // preview of its own): the theme the app applied shows now.
      document.documentElement.dataset.theme = S.app.themeApplied || S.app.theme;
    }, 400);
  }

  function setView(view) {
    if (S.view === 'settings' && view !== 'settings') {
      clearPwForm();
      hideDiscordLink();
      // Coming back shows the chosen page (highlighted in the list), not the results of an old search.
      if ($('#set-q')?.value) {
        $('#set-q').value = '';
        searchSettings('');
      }
    }
    S.view = view;
    document.querySelectorAll('.nav').forEach(n => n.classList.toggle('on', n.dataset.view === view));
    $('#view-accounts').hidden = view !== 'accounts';
    $('#view-presets').hidden = view !== 'presets';
    if (view === 'presets') renderPresetsView(true);
    $('#view-settings').hidden = view !== 'settings';
    $('#view-stats').hidden = view !== 'stats';
    $('#view-monitor').hidden = view !== 'monitor';
    watchStats(view === 'stats');
    watchMonitor();
    // Synced folders to offer under Backup are looked for when Settings opens, not with every state.
    if (view === 'settings' && !S.app.locked) call('backupSuggestions', {}, { silent: true });
  }

  function renderSide() {
    const m = S.app.multi;
    // Roblox closed windows when another one opened (Settings › Roblox updates): the card says so and leads there.
    const closed = m.enabled && !!S.app.robloxUpdates?.closed;
    const cls = !m.enabled ? 'off' : m.armed && !closed ? 'ok' : 'warn';
    const text = t(!m.enabled ? 'multi.off' : closed ? 'multi.closed' : m.armed ? 'multi.ready' : 'multi.notReady');
    // The light and text open the details; the switch turns it on or off right here.
    setHtml($('#multi-card'), `<div class="multi ${cls}">
        <button class="multi-open" data-act="${closed ? 'ru-open' : 'multi-help'}" title="${t('multi.title')}: ${esc(closed ? t('multi.closedTip') : text)}"><span class="led"></span><div><b>${t('multi.title')}</b><small>${text}</small></div></button>
        <label class="switch" title="${t(m.enabled ? 'multi.turnOff' : 'multi.turnOn')}"><input type="checkbox" id="side-multi" ${m.enabled ? 'checked' : ''} aria-label="${t('multi.title')}"><span></span></label>
      </div>`);
    setHtml($('#side-warn'), S.app.robloxInstalled ? '' : `<div class="warn-card" title="${esc(t('side.noRoblox'))}">${icon('alert')}<span>${t('side.noRoblox')}</span></div>`);
    const u = S.app.update;
    setHtml($('#update-slot'), u?.status === 'ready'
      ? `<button class="btn primary w update-btn" data-act="apply-update" title="${esc(t(u.auto ? 'upd.restartTipAuto' : 'upd.restartTip', { v: u.version }))}">${icon('refresh')}<span><b>${t('upd.restart')}</b><small>${t('upd.versionN', { v: esc(u.version) })}</small></span></button>` : '');
    setText($('#nav-count'), String(accounts().length));
    // Which window is whose, by process id (other tools pick a client by it).
    const pids = accounts().filter(a => isLocal(a) && a.session?.pid > 0).map(a => `${a.label} · PID ${a.session.pid}`);
    $('#sys-count').parentElement.title = pids.join('\n');
    // The mini window's buttons (the card's, and the icon the narrow sidebar keeps) read as pressed while it is open
    // (the tray menu and Esc can change that too).
    for (const mini of document.querySelectorAll('.side [data-act="mini"]')) {
      mini.setAttribute('aria-pressed', String(!!S.app.miniOpen));
      setText(mini.querySelector('span'), t(S.app.miniOpen ? 'sys.miniHide' : 'sys.mini'));
      if (mini.classList.contains('nav-mini')) mini.title = t(S.app.miniOpen ? 'sys.miniHide' : 'sys.mini');
    }
    const rv = S.app.robloxVersion || {};
    const short = v => (v || '').split('.').slice(0, 2).join('.');
    setHtml($('#sys-ver'), rv.installed
      ? `<span title="${esc(`Roblox ${rv.installed}`)}"><i class="dot ${rv.upToDate === false ? 'warn' : rv.upToDate ? 'ok' : ''}"></i>Roblox ${esc(short(rv.installed))}${rv.upToDate === false
        ? ` · ${short(rv.latest) === short(rv.installed) ? t('rbx.newBuild') : t('rbx.available', { v: esc(short(rv.latest)) })}`
        : rv.upToDate ? ` · ${t('rbx.latestShort')}` : ''}</span>` : '');
    renderMetrics();
  }

  function renderHeader() {
    const list = accounts();
    const n = S.selected.size;
    let html = tn('acc.summary', list.length, { p: list.filter(inGame).length, o: list.filter(isOnline).length });
    // Selected accounts the search or filter hides still count (Play, Delete act on them), so say so.
    const shown = n ? new Set(visible().map(a => a.id)) : null;
    const hidden = n ? [...S.selected].filter(id => !shown.has(id)).length : 0;
    if (n) html += `<span class="sep">·</span>${t(hidden ? 'acc.selectedHidden' : 'acc.selected', { n, h: hidden })}`;
    else if (list.length) html += `<span class="sep">·</span>${t('acc.selectAll')}`;
    setHtml($('#acc-sub'), html);
  }

  function renderChips() {
    const list = accounts();
    // Groups have their own menu next to Sort (they used to be chips here and fell off the end of the row); the
    // status chips count within the chosen group.
    const groups = accountGroups();
    if (S.group && !groups.includes(S.group)) S.group = '';
    const inGroup = S.group ? list.filter(a => a.group === S.group) : list;
    const chips = [
      { f: 'all', text: t('chip.all'), n: inGroup.length },
      { f: 'ingame', text: t('chip.ingame'), n: inGroup.filter(inGame).length, dot: 'var(--ok)' },
      { f: 'online', text: t('chip.online'), n: inGroup.filter(isOnline).length, dot: 'var(--online)' },
      { f: 'offline', text: t('chip.offline'), n: inGroup.filter(a => !isOnline(a)).length, dot: 'var(--dim)' },
      { f: 'local', text: t('chip.local'), n: inGroup.filter(isLocal).length, hide: !list.some(isLocal) },
      { f: 'invalid', text: t('chip.invalid'), n: inGroup.filter(a => a.cookieValid === false).length, dot: 'var(--bad)', hide: !list.some(a => a.cookieValid === false) },
      { f: 'banned', text: t('chip.banned'), n: inGroup.filter(a => a.banned).length, dot: 'var(--bad)', hide: !list.some(a => a.banned) },
    ].filter(c => !c.hide);
    const groupBtn = $('#group-btn');
    groupBtn.hidden = !groups.length;
    groupBtn.classList.toggle('on', !!S.group);
    setHtml(groupBtn, `${icon('tag')}<span>${t('grp.label')}: ${S.group ? esc(S.group) : t('chip.all')}</span>`);
    groupBtn.title = S.group ? `${t('grp.label')}: ${S.group}` : '';
    if (!chips.some(c => c.f === S.filter)) S.filter = 'all';
    setHtml($('#chips'), list.length ? chips.map(c => `<button class="chip ${S.filter === c.f ? 'on' : ''}" data-act="filter" data-f="${esc(c.f)}" aria-pressed="${S.filter === c.f}">${c.dot ? `<i class="dot" style="background:${c.dot}"></i>` : ''}${c.text}<em>${c.n}</em></button>`).join('') : '');
    setHtml($('#layout-seg'), list.length ? ['cards', 'list'].map(l =>
      `<button class="${S.layout === l ? 'on' : ''}" data-act="layout" data-l="${l}" title="${t(`layout.${l}`)}" aria-label="${t(`layout.${l}`)}" aria-pressed="${S.layout === l}">${icon(l === 'cards' ? 'grid' : 'rows')}</button>`).join('') : '');
    const sortBtn = $('#sort-btn');
    sortBtn.hidden = list.length < 2;
    setHtml(sortBtn, `${icon('sort')}<span>${t('sort.label')}: ${t(`sort.${S.sort}`)}</span>`);
    // A newly chosen chip half under the faded edge (or off the row) slides fully into view.
    const row = $('#chips'), on = row?.querySelector('.chip.on');
    if (on && S.filter !== chipShown) {
      chipShown = S.filter;
      const r = on.getBoundingClientRect(), box = row.getBoundingClientRect(), edge = 40;
      if (r.left < box.left + edge) row.scrollLeft -= box.left + edge - r.left;
      else if (r.right > box.right - edge) row.scrollLeft += r.right - (box.right - edge);
    }
    syncChipFade();
  }
  let chipShown = 'all';

  /** The chip row scrolls sideways when it holds more than fits: its edges fade on the side(s) with more chips. */
  function syncChipFade() {
    const el = $('#chips');
    if (!el) return;
    const max = el.scrollWidth - el.clientWidth;
    el.classList.toggle('fade-l', max > 1 && el.scrollLeft > 1);
    el.classList.toggle('fade-r', max > 1 && el.scrollLeft < max - 1);
  }
  const chipObserver = new ResizeObserver(() => syncChipFade());
  /** The account groups in use, sorted by name. */
  const accountGroups = () => [...new Set(accounts().map(a => a.group).filter(Boolean))].sort(compareText);

  const statusRank = a => isLocal(a) ? 0 : a.state === 'launching' ? 1 : a.presence?.type === 'ingame' ? 2
    : ['online', 'studio'].includes(a.presence?.type) ? 3 : a.cookieValid === false || a.banned ? 5 : 4;

  /** One collator per language: localeCompare with a locale builds a new one on every call, slow for big sorts. */
  let collator = null;
  function compareText(x, y) {
    // numeric: "Farm 2" before "Farm 10".
    if (collator?.lang !== lang) collator = { lang, c: new Intl.Collator(locale(), { numeric: true }) };
    return collator.c.compare(x, y);
  }

  /** Lower-case without accents, so "hoang" finds "Hoàng" and "Đ" matches "d". */
  const fold = s => String(s || '').normalize('NFD').replace(/\p{M}/gu, '').replace(/đ/g, 'd').replace(/Đ/g, 'D').toLowerCase();
  function visible() {
    // "@name" finds the username too.
    const q = fold(S.query.trim().replace(/^@/, ''));
    const f = S.filter;
    // Name means the name: groups have their own menu to narrow the list. The real names, also while they are hidden,
    // so hiding them doesn't reshuffle the list.
    const real = a => a.real || a;
    const byName = (x, y) => compareText(real(x).label, real(y).label) || compareText(real(x).username || '', real(y).username || '');
    // A user ID is matched whole (or by its start from 6 digits on): "1" mustn't find nearly every account.
    const idMatch = a => /^\d+$/.test(q) && (String(a.userId) === q || (q.length >= 6 && String(a.userId).startsWith(q)));
    const rank = S.sort === 'order' ? new Map(launchOrder().map((id, i) => [id, i])) : null;
    const compare = S.sort === 'status' ? (x, y) => statusRank(x) - statusRank(y) || byName(x, y)
      : S.sort === 'recent' ? (x, y) => (y.lastUsed || '').localeCompare(x.lastUsed || '') || byName(x, y)
      : S.sort === 'order' ? (x, y) => (rank.get(x.id) ?? 1e9) - (rank.get(y.id) ?? 1e9) || byName(x, y)
      : byName;
    return accounts().filter(a => {
      if (f === 'ingame' && !inGame(a)) return false;
      if (f === 'online' && !isOnline(a)) return false;
      if (f === 'offline' && isOnline(a)) return false;
      if (f === 'local' && !isLocal(a)) return false;
      if (f === 'invalid' && a.cookieValid !== false) return false;
      if (f === 'banned' && !a.banned) return false;
      if (f.startsWith('g:') && a.group !== f.slice(2)) return false;
      if (S.group && a.group !== S.group) return false;
      // Real names match while they are hidden too, and so does the "Account 3" shown instead.
      const r = real(a);
      return !q || idMatch(a) || [r.alias, r.username, r.displayName, a.label, a.group, a.note, a.presence?.game, a.session?.game?.name]
        .some(v => fold(v).includes(q));
    }).sort(compare);
  }

  /** Roblox's disconnect codes, as the client logs them (278 seen in practice; the others are Roblox's documented ones). */
  const KICK = { 278: 'kick.idle', 267: 'kick.game', 273: 'kick.elsewhere', 277: 'kick.network', 279: 'kick.connect' };

  function activityHtml(a) {
    const p = a.presence;
    const gIcon = url => url ? `<img class="gicon" src="${esc(url)}" alt="" loading="lazy" decoding="async">` : `<span class="gicon">${icon('gamepad')}</span>`;
    const box = (cls, iconHtml, k, v, raw = false) => `<div class="activity ${cls}">${iconHtml}<div class="txt"><div class="k" title="${esc(k)}">${k}</div><div class="v"${raw ? '' : ` title="${esc(v)}"`}>${raw ? v : esc(v)}</div></div></div>`;
    if (a.state === 'launching') return box('launching', '<span class="gicon"><span class="spin"></span></span>', t('act.opening'), a.session?.game?.name || t('act.waitWindow'));
    // Auto-rejoin waiting its turn (longer after several drops in a row), or a preset's restart about to open it again.
    if (a.rejoin && !isLocal(a)) return box('launching', `<span class="gicon">${icon(a.rejoin.restart ? 'refresh' : 'repeat')}</span>`, t(a.rejoin.restart ? 'rj.kRestart' : 'rj.k'),
      `<span data-until="${esc(a.rejoin.at)}" data-tpl="rj.inMin" data-soon="rj.soon"></span>${a.rejoin.game?.name ? ` · ${esc(a.rejoin.game.name)}` : ''}`, true);
    if (isLocal(a)) {
      const game = p?.type === 'ingame' && p.game ? { name: p.game, iconUrl: p.iconUrl } : a.session?.game;
      const c = a.session?.connection;
      if (c === 'disconnected') {
        const why = a.session.reason ? (KICK[a.session.reason] ? t(KICK[a.session.reason]) : t('kick.code', { code: a.session.reason })) : '';
        return box('kicked', `<span class="gicon">${icon('alert')}</span>`, t('act.kicked'),
          [why, game?.name].filter(Boolean).join(' · ') || t('act.stillOpen'));
      }
      if (c === 'menu') return box('online', `<span class="gicon">${icon('monitor')}</span>`, t('act.menuHere'), t('act.menuWhere'));
      if (c === 'signedout') return box('signedout', `<span class="gicon">${icon('logIn')}</span>`, t('act.signedOutHere'), t('act.signedOutWhere'));
      return box('ingame', gIcon(game?.iconUrl), t(c === 'ingame' ? 'act.playingHere' : 'act.joiningHere'), game?.name || t('act.gameLoading'));
    }
    switch (p?.type) {
      case 'ingame': return box('ingame', gIcon(p.iconUrl), t('act.playingElse'), p.game || t('act.hidden'));
      case 'online': return box('online', `<span class="gicon">${icon('globe')}</span>`, t('act.online'), t('act.onlineWhere'));
      case 'studio': return box('studio', `<span class="gicon">${icon('code')}</span>`, t('act.studio'), p.game || t('act.studioWhat'));
      case 'offline':
      case 'invisible':
        // Signed out: that is what to act on, not "Offline" under a "Needs sign-in" badge.
        if (a.cookieValid === false) return box('signedout', `<span class="gicon">${icon('logIn')}</span>`, t('act.signedOut'), t('act.signInAgain'));
        return p.lastOnline
          ? box('offline', `<span class="gicon">${icon('moon')}</span>`, t('act.offline'), `<span data-ago="${esc(p.lastOnline)}" data-tpl="act.lastOnline"></span>`, true)
          : box('offline', `<span class="gicon">${icon('moon')}</span>`, t('act.offline'), t('act.inactive'));
      default:
        return a.cookieValid === false
          ? box('signedout', `<span class="gicon">${icon('logIn')}</span>`, t('act.signedOut'), t('act.signInAgain'))
          : box('unknown', `<span class="gicon">${icon('info')}</span>`, t('act.unknown'), t('act.updating'));
    }
  }

  /** The colour of the avatar ring and dot: a client here in the Roblox menu is online, not in a game. */
  const presenceType = a => !isLocal(a) ? a.presence?.type || ''
    : a.session?.connection === 'disconnected' ? 'kicked' : a.session?.connection === 'menu' ? 'online'
    : a.session?.connection === 'signedout' ? 'offline' : 'ingame';

  function cardHtml(a) {
    const pType = presenceType(a);
    const ring = a.cookieValid === false || a.banned ? 'invalid' : ['ingame', 'online', 'kicked'].includes(pType) ? pType : '';
    // A ban says more than the sign-in state, so it takes the badge's place.
    const badge = isLocal(a) ? `<span class="badge run" title="${a.session?.pid > 0 ? esc(t('card.runningPid', { pid: a.session.pid })) : ''}"><i class="pulse"></i>${t('card.local')}</span>`
      : a.state === 'launching' ? `<span class="badge wait"><span class="spin"></span>${t('card.launching')}</span>`
      : a.banned ? `<span class="badge bad" title="${esc(t('card.bannedTip'))}">${t('card.banned')}</span>`
      : a.cookieValid === false ? `<span class="badge bad">${t('card.expired')}</span>` : '';
    const sub = [a.username ? `@${a.username}` : '', a.displayName && a.displayName !== a.username && a.displayName !== a.label ? a.displayName : ''].filter(Boolean).join(' · ');
    // "Opened 8 h ago" only while the account isn't running here (a running card shows its play time instead).
    const opened = isLocal(a) ? '' : `<span class="ago" title="${t('card.lastTip')}">${icon('history')}${a.lastUsed ? `<span data-ago="${esc(a.lastUsed)}" data-tpl="card.opened"></span>` : t('card.never')}</span>`;
    return `<article class="card ${isLocal(a) ? 'local' : ''}" data-id="${a.id}" data-act="card" role="option" tabindex="-1" aria-selected="false" aria-label="${esc(a.label)}">
      <div class="card-top"><span class="check">${icon('check')}</span>${badge}</div>
      <div class="avatar ${ring}">${avatarInner(a)}<i class="pdot ${pType}"></i></div>
      <div class="name" title="${esc(a.label)}">${esc(a.label)}</div>
      <div class="sub" title="${esc(sub)}">${esc(sub)}</div>
      ${activityHtml(a)}
      ${isLocal(a) ? `<div class="live" data-live="${a.id}"><span title="${t('card.ramTip')}">${icon('chip')}<b class="ram">…</b></span><span title="${t('card.upTip')}">${icon('clock')}<b class="up"></b></span>${pidHtml(a)}</div>` : ''}
      <div class="meta">${vngTag(a)}${a.group ? `<span class="tag" title="${esc(a.group)}">${esc(a.group)}</span>` : ''}${rejoinOn(a) ? `<span class="rejoin" title="${t('card.rejoinTip')}">${icon('repeat')}</span>` : ''}${opened}</div>
      ${a.note ? `<div class="note" title="${esc(a.note)}">${esc(a.note)}</div>` : ''}
      <div class="card-actions">
        ${isLocal(a) || a.state === 'launching'
          ? `<button class="pill-btn stop" data-act="close-one">${icon('power')}${t('card.close')}</button>`
          : a.rejoin
            ? `<button class="pill-btn stop" data-act="cancel-rejoin" title="${t('rj.cancelTip')}">${icon('x')}${t('rj.cancel')}</button>`
          : a.cookieValid === false
            ? `<button class="pill-btn relogin" data-act="relogin" title="${t('card.reloginTip')}">${icon('logIn')}${t('card.relogin')}</button>`
            // A Roblox VNG account can't be opened yet (the app skips it): its Play is off and says why.
            : `<button class="pill-btn play" data-act="play-one" ${a.realm === 'vng' ? `title="${t('card.vngNoPlay')}"` : ''} ${a.state === 'launching' || a.realm === 'vng' ? 'disabled' : ''}>${icon('play')}${t('card.play')}</button>`}
        <button class="icon-btn" data-act="edit" title="${t('card.editTip')}">${icon('edit')}</button>
        <button class="icon-btn" data-act="card-menu" title="${t('card.moreTip')}" aria-haspopup="menu">${icon('more')}</button>
      </div>
    </article>`;
  }

  /** One line per account for the compact list; the same actions as a card (select, right-click menu, play). */
  const vngTag = a => a.realm === 'vng' ? `<span class="tag vng" title="${t('card.vngTip')}">VNG</span>` : '';
  /** The client's process id, for tools that pick a Roblox window by it; a click copies it. Nothing until the window is there. */
  const pidHtml = a => a.session?.pid > 0 ? `<button class="pid" data-act="copy-pid" title="${t('card.pidTip')}">PID ${a.session.pid}</button>` : '';

  function rowHtml(a) {
    const pType = presenceType(a);
    const ring = a.cookieValid === false || a.banned ? 'invalid' : ['ingame', 'online', 'kicked'].includes(pType) ? pType : '';
    const st = statusLine(a);
    const action = isLocal(a) || a.state === 'launching' ? `<button class="icon-btn stop" data-act="close-one" title="${t('card.close')}" aria-label="${t('card.close')}">${icon('power')}</button>`
      : a.rejoin ? `<button class="icon-btn stop" data-act="cancel-rejoin" title="${t('rj.cancelTip')}" aria-label="${t('rj.cancelTip')}">${icon('x')}</button>`
      : a.cookieValid === false ? `<button class="icon-btn relogin" data-act="relogin" title="${t('card.relogin')}" aria-label="${t('card.relogin')}">${icon('logIn')}</button>`
      : `<button class="icon-btn play" data-act="play-one" title="${a.realm === 'vng' ? t('card.vngNoPlay') : t('card.play')}" aria-label="${t('card.play')}" ${a.state === 'launching' || a.realm === 'vng' ? 'disabled' : ''}>${icon('play')}</button>`;
    return `<article class="card row ${isLocal(a) ? 'local' : ''}" data-id="${a.id}" data-act="card" role="option" tabindex="-1" aria-selected="false" aria-label="${esc(a.label)}">
      <span class="check">${icon('check')}</span>
      <div class="avatar sm ${ring}">${avatarInner(a)}<i class="pdot ${pType}"></i></div>
      <div class="row-name"><b title="${esc(a.label)}">${esc(a.label)}</b>${a.username ? `<small title="${atName(a)}">${atName(a)}</small>` : ''}</div>
      <div class="row-status ${st.cls}" title="${esc(st.text)}">${rowBadge(a)}${st.until ? `<span data-until="${esc(st.until)}" data-tpl="rj.rowMin" data-soon="rj.rowSoon"></span>` : esc(st.text)}</div>
      <div class="row-meta">${vngTag(a)}${a.group ? `<span class="tag" title="${esc(a.group)}">${esc(a.group)}</span>` : ''}${rejoinOn(a) ? `<span class="rejoin" title="${t('card.rejoinTip')}">${icon('repeat')}</span>` : ''}</div>
      <div class="row-live">${isLocal(a) ? `${pidHtml(a)}<span data-live="${a.id}" title="${t('card.ramTip')}"><b class="ram">…</b><b class="up" hidden></b></span>` : ''}</div>
      <div class="row-actions">${action}<button class="icon-btn" data-act="card-menu" title="${t('card.moreTip')}" aria-label="${t('card.moreTip')}" aria-haspopup="menu">${icon('more')}</button></div>
    </article>`;
  }

  /** The card's red pill in a list row too (banned, or signed out), in front of the status words. */
  const rowBadge = a => isLocal(a) || a.state === 'launching' ? ''
    : a.banned ? `<span class="badge bad" title="${esc(t('card.bannedTip'))}">${t('card.banned')}</span>`
    : a.cookieValid === false ? `<span class="badge bad">${t('card.expired')}</span>` : '';

  /** The account's status in a few words, for the compact list. */
  function statusLine(a) {
    const p = a.presence;
    if (a.state === 'launching') return { cls: 'launching', text: t('act.opening') };
    // The countdown is filled in place (patchTimes); the title keeps the plain words.
    if (a.rejoin && !isLocal(a)) return { cls: 'launching', text: t(a.rejoin.restart ? 'rj.rowRestart' : 'rj.k'), until: a.rejoin.restart ? null : a.rejoin.at };
    if (isLocal(a)) {
      const c = a.session?.connection;
      const game = (p?.type === 'ingame' && p.game) || a.session?.game?.name || '';
      const withGame = text => game ? `${text} · ${game}` : text;
      if (c === 'disconnected') {
        const why = a.session.reason ? (KICK[a.session.reason] ? t(KICK[a.session.reason]) : t('kick.code', { code: a.session.reason })) : t('row.kicked');
        return { cls: 'kicked', text: withGame(why) };
      }
      if (c === 'menu') return { cls: 'online', text: t('act.menuHere') };
      if (c === 'signedout') return { cls: 'signedout', text: t('act.signedOutHere') };
      return { cls: 'ingame', text: withGame(t(c === 'ingame' ? 'row.playing' : 'row.joining')) };
    }
    // A ban or a lost sign-in is the red pill in front (rowBadge); a lost sign-in needs no words after it ("Offline"
    // next to "Needs sign-in" read as two different states).
    switch (p?.type) {
      case 'ingame': return { cls: 'ingame', text: `${t('row.elsewhere')} · ${p.game || t('act.hidden')}` };
      case 'online': return { cls: 'online', text: t('act.online') };
      case 'studio': return { cls: 'studio', text: t('act.studio') };
      case 'offline':
      case 'invisible': return a.cookieValid === false && !a.banned ? { cls: 'signedout', text: '' } : { cls: 'offline', text: t('act.offline') };
      default: return a.cookieValid === false && !a.banned ? { cls: 'signedout', text: '' } : { cls: 'unknown', text: t(a.banned ? 'act.offline' : 'act.unknown') };
    }
  }

  /**
   * One element per account, rebuilt only when that account's own markup changed: a state push, a search or a click
   * then touches a card or two instead of re-parsing hundreds. Selection is applied as a class (see syncSelection).
   */
  function renderGrid() {
    const host = $('#grid');
    if (!host) return;
    const all = accounts();
    const shown = all.length ? visible() : [];
    if (!shown.length) {
      host._cards = null;
      setHtml(host, all.length
        ? `<div class="empty"><h2>${t('nores.title')}</h2><p>${t('nores.text')}</p>
            <div class="row"><button class="btn" data-act="clear-search">${t(S.query.trim() ? 'nores.clear' : 'nores.all')}</button></div></div>`
        : `<div class="empty">
        <img class="art" src="${LOGO}" alt="">
        <h2>${t('empty.title')}</h2>
        <p>${t('empty.text')}</p>
        ${addOptions()}
      </div>`);
      if (all.length) S.introShown = true;
      renderHeader(); // "Selected N (N hidden by the search)" follows a search that hides everything too
      return;
    }
    const asList = S.layout === 'list';
    let box = host._cards ? host.firstElementChild : null;
    if (!box || host._list !== asList) {
      // Cards fade in only on the first render, not on every live update.
      host.innerHTML = `<div class="grid ${asList ? 'list' : ''} ${S.introShown ? '' : 'intro'}" role="listbox" aria-multiselectable="true" aria-label="${t('acc.title')}"></div>`;
      host._html = null;
      host._cards = new Map();
      host._list = asList;
      box = host.firstElementChild;
      if (!S.introShown) setTimeout(() => box.classList.remove('intro'), 800);
    }
    const cards = host._cards, keep = new Set();
    let changed = false, prev = null;
    for (const a of shown) {
      const html = (asList ? rowHtml : cardHtml)(a);
      let entry = cards.get(a.id);
      // The focus — on the card, or on one of its buttons (Play → launching) — survives the card being rebuilt or
      // moved (a sort by status): the same button on the new card, else the card.
      const active = document.activeElement;
      const focused = entry?.el.contains(active) ? (active === entry.el ? 'card' : active.closest('[data-act]')?.dataset.act || 'card') : null;
      if (!entry || entry.html !== html) {
        const tpl = document.createElement('template');
        tpl.innerHTML = html.trim();
        const el = tpl.content.firstElementChild;
        if (entry) entry.el.replaceWith(el);
        cards.set(a.id, entry = { html, el });
        changed = true;
      }
      setSelected(entry.el, S.selected.has(a.id));
      keep.add(a.id);
      const at = prev ? prev.nextElementSibling : box.firstElementChild;
      if (at !== entry.el) box.insertBefore(entry.el, at);
      if (focused && !entry.el.contains(document.activeElement)) (focused !== 'card' && entry.el.querySelector(`[data-act="${focused}"]`) || entry.el).focus();
      prev = entry.el;
    }
    for (const [id, entry] of cards) {
      if (keep.has(id)) continue;
      entry.el.remove();
      cards.delete(id);
    }
    setTabStop(box);
    if (changed) {
      patchTimes();
      renderMetrics();
    }
    S.introShown = true;
    renderHeader(); // the "N hidden" note follows the search and filter
  }

  function setSelected(el, sel) {
    if (el.classList.contains('sel') === sel) return;
    el.classList.toggle('sel', sel);
    el.setAttribute('aria-selected', sel);
  }

  /**
   * One Tab stop for the whole list (roving tabindex, like the theme swatches), so the dock is a Tab away with hundreds
   * of accounts; the arrow keys walk the cards (moveCardFocus). The stop is the card with the focus, else the first
   * selected one, else the first. Only that card's own buttons are Tab stops too: 200 accounts mustn't be 600 presses.
   */
  function setTabStop(box, stop = null) {
    const items = [...box.children];
    const active = document.activeElement?.closest?.('.card');
    stop ||= (active && box.contains(active) ? active : null) || items.find(c => c.classList.contains('sel')) || items[0];
    for (const c of items) {
      const want = c === stop ? 0 : -1;
      if (c.tabIndex !== want) c.tabIndex = want;
      if (c._inner !== want) {
        c._inner = want;
        for (const b of c.querySelectorAll('button')) b.tabIndex = want;
      }
    }
  }

  /** Left/Right go card by card, Up/Down a row (a line in the list), Home/End to the ends; the focus moves, the selection doesn't. */
  function moveCardFocus(from, key) {
    const items = [...from.parentElement.children];
    const i = items.indexOf(from);
    let j;
    if (key === 'Home') j = 0;
    else if (key === 'End') j = items.length - 1;
    else if (key === 'ArrowRight' || key === 'ArrowLeft') j = i + (key === 'ArrowRight' ? 1 : -1);
    else {
      // Cards per row, from the layout itself: how many share the first card's top edge.
      const top = items[0].offsetTop;
      let perRow = items.findIndex(c => c.offsetTop > top);
      if (perRow < 0) perRow = items.length;
      j = i + (key === 'ArrowDown' ? perRow : -perRow);
    }
    const to = items[Math.max(0, Math.min(items.length - 1, j))];
    if (!to || to === from) return;
    setTabStop(from.parentElement, to);
    to.focus();
  }

  /** Selection only flips a class on the cards already on screen. */
  function syncSelection() {
    const cards = $('#grid')?._cards;
    if (cards) for (const [id, { el }] of cards) setSelected(el, S.selected.has(id));
  }

  /** Replaces markup only when it changed, so frequent state pushes do not rebuild the DOM. Returns true if replaced. */
  function setHtml(el, html) {
    if (!el || el._html === html) return false;
    // The focus inside goes to the same control in the new markup (Play ↔ Stop, a filter chip, the layout switch),
    // or the nearest one still usable, instead of dropping to the page.
    const active = document.activeElement;
    const had = active && active !== el && el.contains(active) ? focusKey(active, el) : null;
    el._html = html;
    el.innerHTML = html;
    if (had) refocus(el, had);
    return true;
  }
  const FOCUS_ATTRS = ['act', 'pv', 'move', 'f', 'l', 'place', 'days', 'tab', 'lang', 'id', 'dc'];
  function focusKey(node, root) {
    let sel = FOCUS_ATTRS.filter(k => node.dataset[k] !== undefined).map(k => `[data-${k}="${CSS.escape(node.dataset[k])}"]`).join('');
    // A control in a row (a preset's Edit, its schedule switch): the same control in the same row.
    const row = !node.dataset.id && node.parentElement?.closest('[data-id]');
    if (sel && row && root.contains(row)) sel = `[data-id="${CSS.escape(row.dataset.id)}"] ${sel}`;
    const all = [...root.querySelectorAll('button, input, select, a[href], [tabindex]')];
    return { sel, cls: node.classList[0] || '', index: all.indexOf(node) };
  }
  function refocus(root, { sel, cls, index }) {
    const usable = x => x && !x.disabled && !x.hidden;
    const all = [...root.querySelectorAll('button, input, select, a[href], [tabindex]')];
    const same = sel && root.querySelector(sel);
    // The same control, else one in its place (Play → Stop has another action but the same look), else the nearest.
    const target = [same, !same && cls && root.querySelector(`.${CSS.escape(cls)}`), all[index]].find(usable)
      || all.slice(index).find(usable) || all.slice(0, index).reverse().find(usable);
    // Without scrolling to it: a part drawn again (Show more adding rows above the button) keeps the place being read.
    target?.focus({ preventScroll: true });
  }

  function renderDock() {
    const actions = $('#dock-actions');
    if (!actions) return;
    const n = S.selected.size;
    const sel = [...S.selected].map(byId).filter(Boolean);
    // With no game in the bar, the main button opens Roblox on its home screen (one window per account).
    const hasGame = !!$('#place')?.value.trim();
    setHtml(actions, S.app.launching
      ? `<button class="play-btn stop" data-act="cancel-launch">${icon('stop')}<span>${t('dock.stop')}</span></button>`
      : `<button class="btn ghost" data-act="arrange" title="${t('dock.arrangeTip')}" aria-label="${t('dock.arrangeTip')}">${icon('screens')}</button>
         <button class="btn ghost" data-act="open-app" ${n ? '' : 'disabled'} title="${t('dock.openAppTip')}" aria-label="${t('dock.openAppTip')}">${icon('monitor')}</button>
         <button class="btn ghost" data-act="close-sel" ${sel.some(isLocal) ? '' : 'disabled'} title="${t('dock.closeTip')}" aria-label="${t('dock.closeTip')}">${icon('power')}</button>
         <button class="btn ghost" data-act="preset-menu" title="${t('ps.tip')}" aria-label="${t('ps.tip')}" aria-haspopup="menu">${icon('layers')}</button>
         <button class="play-btn" data-act="launch" ${n ? '' : 'disabled'}>${icon(hasGame ? 'play' : 'monitor')}<span>${t(!n ? 'dock.pick' : hasGame ? 'dock.play' : 'dock.openHome')}</span>${n ? `<em>${n}</em>` : ''}</button>`);
    // Join a player needs selected accounts, like Open Roblox and Close next to it: dimmed until there are some.
    const join = $('#view-accounts .dock-join');
    if (join) join.disabled = !n;

    const g = S.game;
    setHtml($('#game-thumb'), g?.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : icon('gamepad'));
    let info;
    if (S.progress) {
      info = `<span class="spin"></span><span class="progress" title="${esc(S.progress)}">${esc(S.progress)}</span>`;
    } else if (!g) {
      info = `<span class="hint">${t('dock.hint')}</span>`;
    } else if (g.loading) {
      info = `<span class="spin"></span><span>${t('dock.finding')}</span>`;
    } else if (!g.ok) {
      info = `<span class="err" title="${esc(g.error)}">${esc(g.error)}</span>`;
    } else if (g.kind === 'share') {
      info = `<span class="kind">${t('dock.share')}</span><span>${t('dock.shareNote')}</span>`;
    } else {
      const kind = g.kind === 'private' ? `<span class="kind">${t('dock.private')}</span>` : g.kind === 'server' ? `<span class="kind">${t('dock.server')}</span>` : '';
      info = `<span class="gname" title="${esc(g.name)}">${esc(g.name || `Place ${g.placeId}`)}</span>${kind}${g.creator ? `<span class="creator">${t('dock.by', { c: esc(g.creator) })}</span>` : ''}${g.playing ? `<span class="players">${t('dock.playing', { n: fmtNum(g.playing) })}</span>` : ''}`
        + (g.placeId && g.kind !== 'private' ? `<button class="srv-open" data-act="servers">${icon('server')}<span>${t('srv.button')}</span></button>` : '');
    }
    // The game already in the box isn't offered again as a shortcut.
    const recents = (S.app.settings?.recentGames || []).filter(r => r.placeId !== g?.placeId).slice(0, 3);
    if (recents.length && !S.progress) {
      info += `<div class="recents">${recents.map(r => `<div class="recent" title="${esc(r.name)}"><button class="recent-pick" data-act="recent" data-place="${r.placeId}">${r.iconUrl ? `<img src="${esc(r.iconUrl)}" alt="">` : '<i class="ph"></i>'}<span>${esc(r.name)}</span></button><button class="rm" data-act="recent-remove" data-place="${r.placeId}" title="${t('dock.recentRemove')}" aria-label="${t('dock.recentRemove')}">${icon('x')}</button></div>`).join('')}</div>`;
    }
    if (setHtml($('#dock-info'), info)) fitRecents();
  }

  /**
   * The recent-game chips are a shortcut (the picker's Recent tab has them too), the game's name is not: chips are
   * dropped from the end, then the whole row, until the name has room to be read.
   */
  function fitRecents() {
    const box = $('#dock-info'), row = box?.querySelector('.recents');
    if (!row) return;
    const chips = [...row.children];
    chips.forEach(c => { c.hidden = false; });
    row.hidden = false;
    const name = box.querySelector('.gname'), err = box.querySelector('.err');
    const cramped = () => box.scrollWidth > box.clientWidth + 1
      || (name && name.scrollWidth > name.clientWidth && name.clientWidth < 240) // 1 px over already ends it in "…"
      || (err && err.scrollHeight > err.clientHeight + 1); // a reason cut short: the chips make room for it first
    for (let i = chips.length - 1; i >= 0 && cramped(); i--) chips[i].hidden = true;
    if (chips.every(c => c.hidden)) row.hidden = true;
  }
  const dockObserver = new ResizeObserver(() => fitRecents());

  /** Fills relative times ("5 min ago") in place, so they never force the cards to be rebuilt. */
  function patchTimes() {
    document.querySelectorAll('[data-ago]').forEach(el => {
      const text = el.dataset.tpl ? t(el.dataset.tpl, { t: ago(el.dataset.ago) }) : ago(el.dataset.ago);
      if (el.textContent === text) return;
      el.textContent = text;
      if (el.parentElement?.classList.contains('v')) el.parentElement.title = text; // the card's status line, cut short when narrow
    });
    // Countdowns (a rejoin waiting its turn): whole minutes, rounded up, then "in a moment" for the last one.
    document.querySelectorAll('[data-until]').forEach(el => {
      const s = (new Date(el.dataset.until).getTime() - Date.now()) / 1000;
      const text = s < 60 ? t(el.dataset.soon) : t(el.dataset.tpl, { n: Math.ceil(s / 60) });
      if (el.textContent !== text) el.textContent = text;
    });
  }

  /** Patches live RAM and uptime numbers in place; called on every metrics event. */
  function renderMetrics() {
    if (document.hidden) return;
    const m = S.metrics;
    const ram = new Map(m.accounts.map(x => [x.id, x.ramMb]));
    document.querySelectorAll('[data-live]').forEach(el => {
      const a = byId(el.dataset.live);
      const mb = ram.get(el.dataset.live);
      const ramEl = el.querySelector('.ram'), upEl = el.querySelector('.up');
      const ramText = mb ? fmtMb(mb) : '…';
      if (ramEl.textContent !== ramText) ramEl.textContent = ramText;
      if (a?.session) {
        const up = duration(a.session.since);
        if (upEl.textContent !== up) upEl.textContent = up;
      }
    });
    const count = $('#sys-count');
    if (!count) return;
    setText(count, String(m.robloxCount));
    $('#sys-close').hidden = !m.robloxCount;
    setText($('#sys-count-label'), t(m.robloxCount === 1 ? 'sys.window1' : 'sys.windows'));
    // The monitor counts its pictures (windows it can show), not every Roblox process.
    const tiles = m.monitorCount ?? m.robloxCount;
    setText($('#mon-count'), tiles === 1 ? t('mon.count1') : t('mon.countN', { n: fmtNum(tiles) }));
    const hint = $('#mon-hint');
    if (hint) hint.hidden = !tiles;
    setText($('#sys-ram'), fmtMb(m.totalMb));
    // The bar is the whole PC's RAM: Roblox's share first, then everything else in use.
    const robloxPct = m.systemMb ? Math.min(100, (m.totalMb / m.systemMb) * 100) : 0;
    const usedPct = Math.max(robloxPct, m.usedPct || 0);
    // Transforms only: moving the bars never makes the page re-run layout.
    const meter = $('#sys-meter');
    const robloxBar = `scaleX(${(robloxPct / 100).toFixed(3)})`, otherBar = `translateX(${robloxPct.toFixed(1)}%) scaleX(${((usedPct - robloxPct) / 100).toFixed(3)})`;
    if (meter.children[0].style.transform !== robloxBar) meter.children[0].style.transform = robloxBar;
    if (meter.children[1].style.transform !== otherBar) meter.children[1].style.transform = otherBar;
    $('#sys-card').classList.toggle('hot', usedPct >= 90);
    setText($('#sys-foot'), m.systemMb ? t('sys.used', { pct: Math.round(usedPct), free: fmtMb(m.availableMb || 0) }) : '');
  }

  /** Writing identical text still forces a layout pass, so skip no-op updates. */
  function setText(el, text) {
    if (el && el.textContent !== text) el.textContent = text;
  }

  /* ───────────────────────── settings ───────────────────────── */
  /** The kinds of Windows notification that can be turned off one by one (the app maps each event to one of them). */
  const NOTIFY_KINDS = ['drop', 'schedule', 'signin', 'ram', 'launch'];

  /**
   * Settings is a list of pages on the left (with a search box over all of them) and the chosen page on the right:
   * its title, then groups — a small heading over a card of rows — so a page and a group inside it never look alike.
   * The page shown last is remembered in this browser.
   */
  const SET_PAGE_KEY = 'rm.settingsPage';
  /** [id, icon, title key]; Discord is a name, not translated. */
  const SET_PAGES = [
    ['launch', 'play', 'set.launch'], ['roblox', 'gamepad', 'set.p.roblox'], ['perf', 'gauge', 'set.perf'],
    ['notify', 'bell', 'set.notifications'], ['discord', 'message', ''], ['appearance', 'palette', 'set.appearance'],
    ['security', 'key', 'set.p.security'], ['backup', 'cloud', 'set.p.backup'], ['updates', 'download', 'set.p.updates'],
  ];
  const setPageTitle = id => { const key = SET_PAGES.find(p => p[0] === id)?.[2]; return key ? t(key) : 'Discord'; };
  let setPage = (() => { const id = store.get(SET_PAGE_KEY, 'launch'); return SET_PAGES.some(p => p[0] === id) ? id : 'launch'; })();
  /** A group on a settings page: a small heading (none for a page's first, untitled group) over a card of rows. */
  const setGroup = (id, title, body) =>
    `<div class="set-grp" data-grp="${id}" id="sec-${id}">${title ? `<h3 class="set-grp-title">${title}</h3>` : ''}<div class="set-card">${body}</div></div>`;

  function settingsHtml() {
    const row = (title, desc, ctl, extra = '') => `<div class="set-row"><div><div class="t">${title}${extra}</div>${desc ? `<div class="d">${desc}</div>` : ''}</div><div class="ctl">${ctl}</div></div>`;
    const sw = (id, label = '') => `<label class="switch"><input type="checkbox" id="${id}" ${label ? `aria-label="${esc(label)}"` : ''}><span></span></label>`;
    const page = (id, body) => `<section class="set-page" data-page="${id}" id="set-page-${id}" aria-labelledby="set-page-${id}-h" ${id === setPage ? '' : 'hidden'}>
      <h2 class="set-page-title" id="set-page-${id}-h">${setPageTitle(id)}</h2>${body}</section>`;
    return `<header class="top"><div class="top-title"><h1>${t('set.title')}</h1></div></header>
      <div class="set-layout">
        <nav class="set-nav" aria-label="${esc(t('set.title'))}">
          <label class="search set-search">${icon('search')}<input id="set-q" type="search" placeholder="${esc(t('set.search'))}" maxlength="60" aria-label="${esc(t('set.search'))}" autocomplete="off" spellcheck="false"></label>
          ${SET_PAGES.map(([id, ic]) => `<button type="button" class="set-nav-item" data-set-page="${id}" ${id === setPage ? 'aria-current="page"' : ''}>${icon(ic)}<span>${setPageTitle(id)}</span><i class="set-nav-dot" title="${esc(t('set.needsLook'))}" hidden></i></button>`).join('')}
        </nav>
        <div class="scroll set-scroll"><div class="settings">
        ${page('launch', `
          ${setGroup('launch', t('set.g.windows'), `
            ${row(t('set.multi'), `${t('set.multiDesc')} <button type="button" class="link" data-act="multi-help">${t('set.details')}</button>`, sw('s-multi'), ' <span class="state-pill" id="s-multi-pill"></span>')}
            ${row(t('set.rename'), t('set.renameDesc'), sw('s-rename'))}
            ${row(t('set.openAs'), t('set.openAsDesc'), `<select class="select" id="s-openas" aria-label="${esc(t('set.openAs'))}">${['roblox', 'maximize', 'tile'].map(k => `<option value="${k}">${t(`set.openAs.${k}`)}</option>`).join('')}</select>`)}
            ${row(t('set.winLayout'), t('set.winLayoutDesc'), `<select class="select" id="s-winlayout" aria-label="${esc(t('set.winLayout'))}">${['fill', 'small', 'medium', 'large'].map(k => `<option value="${k}">${t(`set.winLayout.${k}`)}</option>`).join('')}</select>`)}`)}
          ${setGroup('client', t('set.g.client'), `
            ${row(t('set.client'), t('set.clientDesc'), `<select class="select" id="s-launch" aria-label="Roblox client"></select><button class="btn sm" data-act="pick-launcher" title="${t('launch.pick')}">${t('launch.pickBtn')}</button>`)}
            <div id="s-store"></div>
            ${row(t('set.delay'), t('set.delayDesc'), '<input type="range" id="s-delay" min="0" max="60" step="1"><b id="s-delay-v" class="range-v"></b>')}`)}`)}
        ${page('roblox', `
          ${setGroup('rbxupd', t('ru.title'), `
            <div id="s-rbxclosed"></div>
            <div id="s-rbxver"></div>
            <div id="s-rbxupd"></div>`)}
          ${setGroup('rc', t('rc.title'), `
            ${row(t('rc.mode'), '<span id="s-rc-hint"></span>', `<select class="select" id="s-rc-mode">${['off', 'all', 'marked'].map(m => `<option value="${m}">${t(`rc.mode.${m}`)}</option>`).join('')}</select>`)}
            ${row(t('rc.graphics'), '', `<select class="select" id="s-rc-graphics"><option value="0">${t('rc.keep')}</option>${[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(n => `<option value="${n}">${n}</option>`).join('')}</select>`)}
            ${row(t('rc.fps'), '', `<select class="select" id="s-rc-fps"><option value="0">${t('rc.keep')}</option>${[30, 60, 144, 240].map(n => `<option value="${n}">${n} FPS</option>`).join('')}</select>`)}
            ${row(t('rc.volume'), '', `<select class="select" id="s-rc-volume"><option value="-1">${t('rc.keep')}</option><option value="0">${t('rc.mute')}</option>${[25, 50, 75, 100].map(n => `<option value="${n}">${n}%</option>`).join('')}</select>`)}`)}`)}
        ${page('perf', `
          ${setGroup('perf', t('set.g.keep'), `
            ${row(t('set.rejoin'), t('set.rejoinDesc'), `<select class="select" id="s-rejoin" aria-label="${esc(t('set.rejoin'))}"></select>`)}
            ${row(t('set.antiAfk'), t('set.antiAfkDesc'), sw('s-antiafk'))}
            ${row(t('set.daily'), `${t('set.dailyDesc')} <span id="s-daily-when"></span>`, sw('s-daily'))}`)}
          ${setGroup('memory', t('set.g.memory'), `
            ${row(t('set.memWarn'), '', sw('s-memwarn'))}
            ${row(t('set.trim'), t('set.trimDesc'), sw('s-trim'))}`)}
          ${setGroup('app', t('set.g.app'), `
            ${row(t('set.tray'), t('set.trayDesc'), sw('s-tray'))}
            ${row(t('set.gpu'), t('set.gpuDesc'), sw('s-gpu'))}`)}`)}
        ${page('notify', `
          ${setGroup('notify', '', row(t('set.notify'), t('set.notifyDesc'), sw('s-notify')))}
          ${setGroup('notify-kinds', t('set.notifyKinds'), `<div class="set-sub" id="s-notify-kinds">${NOTIFY_KINDS.map(k => row(t(`set.notify.${k}`), '', sw(`s-notify-${k}`, t(`set.notify.${k}`)))).join('')}</div>`)}`)}
        ${page('discord', discordHtml(row, sw))}
        ${page('appearance', setGroup('appearance', '', `
          ${row(t('set.theme'), '<span id="s-theme-name"></span>', '')}
          <div class="themes" id="s-themes" role="radiogroup" aria-label="${t('set.theme')}"></div>
          ${row(t('set.language'), '', langSwitch())}
          ${row(t('set.hideNames'), t('set.hideNamesDesc'), sw('s-hidenames', t('set.hideNames')))}`))}
        ${page('security', setGroup('master', t('set.master'), `
          <form class="pw-form" id="pw-form" autocomplete="off">
            <input class="field" type="password" id="pw-cur" placeholder="${t('set.pwCur')}" aria-label="${t('set.pwCur')}" maxlength="256" autocomplete="current-password">
            <input class="field" type="password" id="pw-new" placeholder="${t('set.pwNew')}" aria-label="${t('set.pwNew')}" maxlength="256" autocomplete="new-password">
            <input class="field" type="password" id="pw-new2" placeholder="${t('set.pwNew2')}" aria-label="${t('set.pwNew2')}" maxlength="256" autocomplete="new-password">
            <button class="btn primary" type="submit" id="pw-btn">${t('set.pwBtn')}</button>
            <p class="pw-msg" id="pw-msg" role="alert"></p>
          </form>`))}
        ${page('backup', `
          ${setGroup('backup', t('bk.title'), `
            ${row(t('bk.on'), t('bk.onDesc'), sw('s-bk-on'))}
            ${row(t('bk.folder'), '<span class="path" id="s-bk-path"></span><div class="bk-suggest" id="s-bk-suggest"></div>', `<button class="btn sm" data-act="bk-pick">${t('bk.choose')}</button><button class="btn sm" data-act="bk-open" id="s-bk-open">${icon('folder')}${t('set.openFolder')}</button>`)}
            <div id="s-bk-note"></div>
            <div id="s-bk-status"></div>`)}
          ${setGroup('restore', t('set.g.restore'), `
            ${row(t('bk.restore'), t('bk.restoreDesc'), `<button class="btn sm" data-act="bk-restore">${t('bk.restoreBtn')}</button>`)}
            ${row(t('iv.setRow'), t('iv.setRowDesc'), `<button class="btn sm" data-act="bk-import">${t('iv.setBtn')}</button>`)}`)}
          ${setGroup('data', t('set.data'), `
            ${row(t('set.dataPath'), '<span class="path" id="s-path"></span>', `<button class="btn sm" data-act="open-data">${icon('folder')}${t('set.openFolder')}</button>`)}`)}`)}
        ${page('updates', setGroup('updates', '', `
          <div id="s-update"></div>
          ${row(t('upd.auto'), '', sw('s-autoupd'))}`))}
        <p class="set-empty" id="set-empty" hidden>${t('set.noMatch')}</p>
      </div></div>
      </div>`;
  }

  /* Settings › Discord. The link itself never comes to the page unless the person asks to see it (Show). */
  const DC_PARTS = ['total', 'accounts', 'games', 'sessions', 'kicks'];
  const DC_PING_KINDS = ['summary', ...NOTIFY_KINDS];
  /** A ping choice: a list (nobody, @here, @everyone, a user, a role) and, for a user or role, the ID box next to it. */
  // "alerts" is every alert kind at once; when they differ it reads "Varies by alert" (the kinds' own lists say how).
  const pingCtl = kind => `<div class="dc-ping"><select class="select sm" data-ping="${kind}" aria-label="${esc(t('dc.ping'))}">${['none', 'here', 'everyone', 'user', 'role'].map(v =>
    `<option value="${v}">${t(`dc.ping.${v}`)}</option>`).join('')}${kind === 'alerts' ? `<option value="mixed" disabled hidden>${t('dc.pingMixed')}</option>` : ''}</select><input class="field sm dc-id" data-ping-id="${kind}" inputmode="numeric" maxlength="20" spellcheck="false" autocomplete="off" placeholder="${esc(t('dc.idPh'))}" aria-label="ID" hidden></div>`;

  function discordHtml(row, sw) {
    const time = (id, label) => `<input class="field sm time" type="time" id="${id}" aria-label="${esc(label)}">`;
    const weekdays = [1, 2, 3, 4, 5, 6, 0].map(d => `<option value="${d}">${new Date(2026, 9, 11 + d).toLocaleDateString(locale(), { weekday: 'long' })}</option>`).join('');
    return `
      ${setGroup('discord', '', `
        ${row(t('dc.on'), t('dc.onDesc'), sw('s-dc-on', t('dc.on')))}
        <div id="s-dc-note"></div>
        ${row(t('dc.webhook'), `<span class="path" id="s-dc-hook"></span><div>${t('dc.webhookHelp')}</div>`,
          `<button class="btn sm" data-dc="paste" id="s-dc-paste"></button><button class="btn sm ghost" data-dc="reveal" id="s-dc-reveal">${icon('eye')}${t('dc.show')}</button><button class="btn sm ghost" data-dc="remove" id="s-dc-remove">${t('dc.remove')}</button>`)}
        <div id="s-dc-status"></div>`)}
      ${setGroup('dc-summary', t('dc.summary'), `
        ${row(t('dc.sendAuto'), '', `<select class="select sm" id="s-dc-when" aria-label="${esc(t('dc.sendAuto'))}">${[['none', 'dc.send.none'], ['daily', 'dc.daily'], ['weekly', 'dc.weekly'], ['both', 'dc.send.both']].map(([v, k]) =>
          `<option value="${v}">${t(k)}</option>`).join('')}</select>`)}
        <div class="dc-when" id="s-dc-daily-row">${row(t('dc.dailyAt'), '', time('s-dc-daily-time', t('dc.dailyAt')))}</div>
        <div class="dc-when" id="s-dc-weekly-row">${row(t('dc.weeklyAt'), '', `<select class="select sm" id="s-dc-weekday" aria-label="${esc(t('dc.weekday'))}">${weekdays}</select>${time('s-dc-weekly-time', t('dc.weeklyAt'))}`)}</div>
        ${row(t('dc.after'), t('dc.afterDesc'), sw('s-dc-after', t('dc.after')))}
        ${row(t('dc.period'), t('dc.periodDesc'), `<select class="select sm" id="s-dc-days" aria-label="${esc(t('dc.period'))}">${[1, 7, 30].map(d => `<option value="${d}">${t(`dc.days.${d}`)}</option>`).join('')}</select>`)}
        ${row(t('dc.include'), '', `<button class="btn sm" data-dc="parts" id="s-dc-parts" aria-haspopup="dialog"></button>`)}
        ${row(t('dc.accounts'), '<span id="s-dc-acc"></span>', `<button class="btn sm" data-dc="accounts">${t('dc.choose')}</button>`)}
        ${row(t('dc.ping'), '', pingCtl('summary'))}
        ${row(t('dc.sendNow'), t('dc.sendNowDesc'), `<button class="btn sm" data-dc="summary-now" id="s-dc-now">${t('dc.sendNowBtn')}</button>`)}`)}
      ${setGroup('dc-events', t('dc.events'), `
        ${row(t('dc.alertsOn'), t('dc.eventsHint'), sw('s-dc-alerts', t('dc.alertsOn')))}
        ${row(t('dc.ping'), '', pingCtl('alerts'))}
        ${setMore('s-dc-kinds', t('dc.kinds'), `<div class="set-sub">${NOTIFY_KINDS.map(k => row(t(`set.notify.${k}`), '', `${pingCtl(k)}${sw(`s-dc-ev-${k}`, t(`set.notify.${k}`))}`)).join('')}</div>`)}
        <p class="set-hint">${t('dc.idHelp')}</p>`)}
      ${setGroup('dc-board', '', `
        ${row(t('dc.boardOn'), t('dc.boardDesc'), `<select class="select sm" id="s-dc-minutes" aria-label="${esc(t('dc.every'))}">${[2, 5, 10, 15, 30, 60].map(m => `<option value="${m}">${t('dc.everyN', { n: m })}</option>`).join('')}</select>${sw('s-dc-board', t('dc.boardOn'))}`)}
        ${row(t('dc.quietOn'), t('dc.quietDesc'), `${time('s-dc-quiet-from', t('dc.from'))}<span class="dc-dash">–</span>${time('s-dc-quiet-to', t('dc.to'))}${sw('s-dc-quiet', t('dc.quietOn'))}`)}`)}
      ${setGroup('dc-look', '', setMore('s-dc-look', t('dc.look'), `
        ${row(t('dc.name'), t('dc.nameDesc'), `<input class="field sm" id="s-dc-name" maxlength="80" spellcheck="false" autocomplete="off" aria-label="${esc(t('dc.name'))}">`)}
        ${row(t('dc.avatar'), t('dc.avatarDesc'), `<input class="field sm" id="s-dc-avatar" maxlength="2048" spellcheck="false" autocomplete="off" placeholder="https://…" aria-label="${esc(t('dc.avatar'))}">`)}
        ${row(t('dc.lang'), '', `<select class="select sm" id="s-dc-lang" aria-label="${esc(t('dc.lang'))}"><option value="app">${t('dc.lang.app')}</option><option value="en">English</option><option value="vi">Tiếng Việt</option></select>`)}
        ${row(t('dc.style'), t('dc.styleDesc'), `<select class="select sm" id="s-dc-style" aria-label="${esc(t('dc.style'))}"><option value="embed">${t('dc.style.embed')}</option><option value="plain">${t('dc.style.plain')}</option></select>`)}
        ${row(t('dc.hideNames'), t('dc.hideNamesDesc'), sw('s-dc-hidenames', t('dc.hideNames')))}`))}`;
  }
  /** A part of a settings card that stays closed until wanted: its line reads like a row title, with a chevron. */
  const setMore = (id, title, body) => `<details class="set-more" id="${id}"><summary>${title}${icon('chevronDown')}</summary>${body}</details>`;

  function wireSettings() {
    // A switch, slider or list with no name of its own is named by its row's title: a screen reader said only "checkbox".
    for (const r of document.querySelectorAll('#view-settings .set-row')) {
      const title = r.querySelector('.t')?.cloneNode(true);
      if (!title) continue;
      title.querySelectorAll('.state-pill, button').forEach(x => x.remove());
      const name = title.textContent.trim();
      if (!name) continue;
      for (const c of r.querySelectorAll('.ctl input:not([type="hidden"]), .ctl select')) {
        if (!c.hasAttribute('aria-label') && !c.hasAttribute('aria-labelledby')) c.setAttribute('aria-label', name);
      }
    }
    const toggle = (id, key) => $(id).addEventListener('change', e => call('setSettings', { [key]: e.target.checked }));
    $('#s-multi').addEventListener('change', e => setMulti(e.target));
    toggle('#s-rename', 'renameWindows');
    toggle('#s-memwarn', 'memoryWarnings');
    toggle('#s-trim', 'trimBackground');
    toggle('#s-antiafk', 'antiAfk');
    for (const key of ['mode', 'graphics', 'fps', 'volume']) {
      $(`#s-rc-${key}`).addEventListener('change', e => {
        const value = key === 'mode' ? e.target.value : +e.target.value;
        call('setSettings', { robloxConfig: { [key]: value } });
      });
    }
    $('#s-autoupd').addEventListener('change', e => call('setAutoUpdate', { enabled: e.target.checked }));
    $('#s-tray').addEventListener('change', e => call('setCloseToTray', { enabled: e.target.checked }));
    $('#s-hidenames').addEventListener('change', e => call('setHideNames', { enabled: e.target.checked }));
    $('#s-notify').addEventListener('change', e => call('setNotifications', { enabled: e.target.checked }));
    for (const kind of NOTIFY_KINDS) $(`#s-notify-${kind}`).addEventListener('change', e => call('setNotifications', { kind, enabled: e.target.checked }));
    $('#s-daily').addEventListener('change', e => call('setDailySignInCheck', { enabled: e.target.checked }));
    $('#s-rejoin').addEventListener('change', e => setRejoinAll(e.target));
    $('#s-bk-on').addEventListener('change', e => call('setBackup', { enabled: e.target.checked }));
    // Switching reloads the interface, so the reply may never come back; the reloaded page shows the new state.
    $('#s-gpu').addEventListener('change', e => {
      saveView(true);
      bridge.call('setRendering', { gpu: e.target.checked }).catch(() => {});
    });
    $('#s-launch').addEventListener('change', e => call('setSettings', { launchVia: e.target.value }));
    $('#s-winlayout').addEventListener('change', e => call('setSettings', { windowLayout: e.target.value }));
    // One of three: as Roblox sizes it, maximised, or tiled (the app keeps the other one off).
    $('#s-openas').addEventListener('change', e => call('setSettings', e.target.value === 'tile' ? { multiTile: true }
      : e.target.value === 'maximize' ? { maximizeOnOpen: true } : { multiTile: false, maximizeOnOpen: false }));
    // Theme swatches: one Tab stop; the arrow keys (Home/End to the ends) move between them and pick as they go, like
    // radio buttons. The pick shows at once; it is saved once the keys rest, not once per press.
    $('#s-themes').addEventListener('keydown', e => {
      const step = { ArrowRight: 1, ArrowDown: 1, ArrowLeft: -1, ArrowUp: -1 }[e.key];
      const from = e.target.closest('.swatch');
      if (!from || (!step && e.key !== 'Home' && e.key !== 'End')) return;
      e.preventDefault();
      const all = [...$('#s-themes').querySelectorAll('.swatch')];
      const next = e.key === 'Home' ? all[0] : e.key === 'End' ? all.at(-1) : all[(all.indexOf(from) + step + all.length) % all.length];
      previewTheme(next.dataset.id);
      next.focus();
    });
    const delay = $('#s-delay');
    delay.addEventListener('input', () => { $('#s-delay-v').textContent = t('set.seconds', { n: delay.value }); });
    delay.addEventListener('change', () => call('setSettings', { launchDelaySeconds: +delay.value }));
    // Checked in the order the boxes are filled; problems show under the form, next to the boxes they are about.
    const pwMsg = (text, kind = 'err') => {
      const el = $('#pw-msg');
      el.textContent = text || '';
      el.className = `pw-msg ${text ? kind : ''}`;
    };
    $('#pw-new').addEventListener('input', e => {
      pwMsg(e.target.value ? t('set.pwStrength', { s: t(`lock.strength.${strengthLevel(e.target.value)}`) }) : '', 'hint');
    });
    for (const id of ['#pw-cur', '#pw-new2']) $(id).addEventListener('input', () => { if ($('#pw-msg').classList.contains('err')) pwMsg(''); });
    $('#pw-form').addEventListener('submit', async e => {
      e.preventDefault();
      const cur = $('#pw-cur'), next = $('#pw-new'), next2 = $('#pw-new2');
      const fail = (key, box) => { pwMsg(t(key)); box.focus(); };
      if (!cur.value) return fail('set.pwNeedCur', cur);
      if (next.value.length < 6) return fail('set.pwShort', next);
      if (!next.value.trim()) return fail('lock.blank', next);
      if (next.value === cur.value) return fail('set.pwSame', next);
      if (next.value !== next2.value) return fail('set.pwMismatch', next2);
      pwMsg('');
      let ok = false;
      await busy($('#pw-btn'), async () => {
        try {
          await bridge.call('changePassword', { current: cur.value, next: next.value });
          ok = true;
        } catch (ex) {
          pwMsg(ex.message);
          cur.select();
        }
      });
      if (ok) {
        clearPwForm();
        toast(t('set.pwDone'), 'ok');
      }
    });
    // The list of pages: a click shows that page; ↑/↓ move along the list, as in a menu.
    const nav = $('#view-settings .set-nav');
    nav.addEventListener('click', e => {
      const item = e.target.closest('[data-set-page]');
      if (item) openSettings(item.dataset.setPage);
    });
    nav.addEventListener('keydown', e => {
      // In a narrow window the list is a row of tabs: ←/→ move along it too.
      const row = getComputedStyle(nav).flexDirection === 'row';
      const keys = row ? ['ArrowDown', 'ArrowUp', 'ArrowRight', 'ArrowLeft', 'Home', 'End'] : ['ArrowDown', 'ArrowUp', 'Home', 'End'];
      if (!e.target.matches('[data-set-page]') || !keys.includes(e.key)) return;
      const items = [...nav.querySelectorAll('[data-set-page]')];
      const at = items.indexOf(e.target);
      const step = e.key === 'ArrowDown' || e.key === 'ArrowRight' ? 1 : -1;
      const next = e.key === 'Home' ? 0 : e.key === 'End' ? items.length - 1 : Math.max(0, Math.min(items.length - 1, at + step));
      e.preventDefault();
      items[next].focus();
    });
    const q = $('#set-q');
    q.addEventListener('input', () => searchSettings(q.value));
    // Esc empties the box first (and goes back to the page), before anything else hears it.
    q.addEventListener('keydown', e => {
      if (e.key === 'Escape' && q.value) {
        e.stopPropagation();
        q.value = '';
        searchSettings('');
      }
    });
    wireDiscord();
  }

  /** The same check the app makes (it has the last word): a discord.com webhook link, nothing else. */
  const DC_LINK = /^https:\/\/(?:(?:canary|ptb)\.)?discord(?:app)?\.com\/api(?:\/v\d{1,2})?\/webhooks\/\d{17,20}\/[\w-]{50,100}\/?$/;
  const DC_ID = /^\d{17,20}$/;

  function wireDiscord() {
    const root = $('#view-settings');
    // Refused by the app (the reason is in a toast): every control shows the saved value again, so nothing looks
    // saved that wasn't.
    // Accepted, the box shows what was kept too (an empty name becomes "Bloxkeep"), also while it still has the focus.
    const set = async args => {
      const r = await call('setDiscord', args);
      syncDiscord(false, true);
      return r;
    };
    const on = (id, fn) => $(id).addEventListener('change', fn);
    // A box that is checked here first (the app's own rules) is marked and not sent.
    const check = (el, ok, key) => {
      el.classList.toggle('bad', !ok);
      if (!ok) toast(t(key), 'warn');
      return ok;
    };
    on('#s-dc-on', e => {
      // No link yet: the switch asks for one, and turns on once it is saved.
      if (e.target.checked && !S.app.discord?.hasWebhook) {
        e.target.checked = false;
        return discordLinkModal(true);
      }
      set({ enabled: e.target.checked });
    });
    // Text boxes are saved when left or on Enter ("change"), not on every key.
    on('#s-dc-name', e => {
      const v = e.target.value.trim();
      if (check(e.target, v.length <= 80 && !/discord|clyde/i.test(v), 'dc.badName')) set({ username: v });
    });
    on('#s-dc-avatar', e => {
      const v = e.target.value.trim();
      if (check(e.target, !v || (/^https:\/\/\S+$/i.test(v) && v.length <= 2048), 'dc.badAvatar')) set({ avatarUrl: v });
    });
    on('#s-dc-lang', e => set({ language: e.target.value }));
    on('#s-dc-style', e => set({ style: e.target.value }));
    on('#s-dc-hidenames', e => set({ hideNames: e.target.checked }));
    // One list for the automatic summaries (never, daily, weekly, both); the times show for the ones picked.
    on('#s-dc-when', e => {
      const v = e.target.value;
      syncDiscordWhen(v);
      set({ summaryDaily: v === 'daily' || v === 'both', summaryWeekly: v === 'weekly' || v === 'both' });
    });
    // A time box emptied isn't a time: the saved one comes back.
    const time = (el, key) => (el.value ? set({ [key]: el.value }) : syncDiscord(false, true));
    on('#s-dc-daily-time', e => time(e.target, 'summaryDailyTime'));
    on('#s-dc-weekday', e => set({ summaryWeekday: +e.target.value }));
    on('#s-dc-weekly-time', e => time(e.target, 'summaryWeeklyTime'));
    on('#s-dc-after', e => set({ summaryAfterSchedule: e.target.checked }));
    on('#s-dc-days', e => set({ summaryDays: +e.target.value }));
    on('#s-dc-board', e => set({ board: e.target.checked }));
    on('#s-dc-minutes', e => set({ boardMinutes: +e.target.value }));
    on('#s-dc-quiet', e => set({ quiet: e.target.checked }));
    // From and To the same never applies: said so when it happens.
    const quietSame = () => { if ($('#s-dc-quiet-from').value === $('#s-dc-quiet-to').value) toast(t('dc.quietSame'), 'warn'); };
    on('#s-dc-quiet-from', e => { time(e.target, 'quietFrom'); quietSame(); });
    on('#s-dc-quiet-to', e => { time(e.target, 'quietTo'); quietSame(); });
    for (const kind of NOTIFY_KINDS) on(`#s-dc-ev-${kind}`, e => set({ event: { kind, enabled: e.target.checked } }));
    // Alerts on or off as a whole: every kind follows (the app takes one kind at a time). Picking kinds one by one is
    // in the closed part under it.
    on('#s-dc-alerts', async e => {
      const enabled = e.target.checked, events = S.app.discord?.events || [];
      e.target.disabled = true;
      for (const kind of NOTIFY_KINDS) {
        if (events.includes(kind) !== enabled && await call('setDiscord', { event: { kind, enabled } }) === undefined) break;
      }
      e.target.disabled = false;
      syncDiscord(false, true);
    });
    // A ping for "alerts" is the same ping for every kind of alert.
    const setPing = async (kind, value) => {
      if (kind !== 'alerts') return set({ ping: { kind, value } });
      for (const k of NOTIFY_KINDS) if (await call('setDiscord', { ping: { kind: k, value } }) === undefined) break;
      syncDiscord(false, true);
    };
    // Pings: nobody, @here and @everyone are saved at once; a user or a role waits for its ID.
    root.querySelectorAll('[data-ping]').forEach(sel => sel.addEventListener('change', () => {
      const kind = sel.dataset.ping, box = root.querySelector(`[data-ping-id="${kind}"]`);
      const needsId = sel.value === 'user' || sel.value === 'role';
      box.hidden = !needsId;
      if (!needsId) {
        delete sel.dataset.pending;
        return setPing(kind, sel.value);
      }
      // The ID box starts empty: a person's ID kept as a role's (or the other way round) would ping the wrong one.
      box.value = '';
      box.classList.remove('bad');
      sel.dataset.pending = '1'; // until an ID is typed, the next state push mustn't put the old choice back
      box.focus();
    }));
    root.querySelectorAll('[data-ping-id]').forEach(box => box.addEventListener('change', () => {
      const kind = box.dataset.pingId, sel = root.querySelector(`[data-ping="${kind}"]`), id = box.value.trim();
      if (!id) return;
      const ok = DC_ID.test(id);
      box.classList.toggle('bad', !ok);
      if (!ok) return toast(t(sel.value === 'role' ? 'dc.badRole' : 'dc.badUser'), 'warn');
      delete sel.dataset.pending;
      setPing(kind, `${sel.value}:${id}`);
    }));
    // Left without an ID: the choice isn't waiting any more, and the saved one comes back (unless the person went
    // back to the ping list itself).
    root.querySelectorAll('[data-ping-id]').forEach(box => box.addEventListener('blur', () => setTimeout(() => {
      const sel = root.querySelector(`[data-ping="${box.dataset.pingId}"]`);
      if (!sel.dataset.pending || box.value.trim() || document.activeElement === sel || document.activeElement === box) return;
      delete sel.dataset.pending;
      syncDiscord(false, true);
    })));
    root.addEventListener('click', e => {
      const btn = e.target.closest('[data-dc]');
      if (!btn) return;
      switch (btn.dataset.dc) {
        case 'paste': return discordLinkModal();
        case 'reveal': return revealDiscordLink(btn);
        case 'remove': return removeDiscordLink();
        case 'accounts': return pickDiscordAccounts();
        case 'parts': return pickDiscordParts();
        case 'test': return busy(btn, async () => { if (await call('discordTest') !== undefined) toast(t('dc.testOk'), 'ok'); });
        case 'summary-now': return busy(btn, async () => { if (await call('discordSummaryNow') !== undefined) toast(t('dc.sentOk'), 'ok'); });
      }
    });
  }

  /** @param thenTurnOn opened from the "Send to Discord" switch: sending goes on once the link is saved. */
  function discordLinkModal(thenTurnOn = false) {
    modal({
      title: t('dc.pasteTitle'),
      body: `<p class="modal-text">${t('dc.pasteText')}</p>
        <input class="field" id="dc-link" type="text" maxlength="300" spellcheck="false" autocomplete="off" placeholder="https://discord.com/api/webhooks/…" aria-label="${esc(t('dc.webhook'))}">
        <p class="dc-err" id="dc-link-err" role="alert"></p>`,
      // The "not a webhook link" note goes as soon as the box changes.
      onMount: ({ el }) => el.querySelector('#dc-link').addEventListener('input', () => setText(el.querySelector('#dc-link-err'), '')),
      busy: true,
      enterSubmits: true,
      dismissable: false,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('dc.save'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            const url = el.querySelector('#dc-link').value.trim();
            if (!DC_LINK.test(url)) return setText(el.querySelector('#dc-link-err'), t('dc.badLink'));
            if (await busy(btn, () => call('setDiscordWebhook', { url })) === undefined) return;
            close();
            if (thenTurnOn) await call('setDiscord', { enabled: true });
            toast(t(thenTurnOn ? 'dc.savedOn' : 'dc.saved'), 'ok');
          },
        },
      ],
    });
  }

  /** Show: the whole link in place of the masked one, until it is hidden again (or the page re-renders it). */
  async function revealDiscordLink(btn) {
    const hook = $('#s-dc-hook');
    if (hook.dataset.shown) return syncDiscord(true);
    const url = await busy(btn, () => call('revealDiscordWebhook'));
    if (!url) return;
    hook.dataset.shown = S.app.discord.webhookHint;
    hook.textContent = url;
    setHtml(btn, `${icon('eyeOff')}${t('dc.hide')}`); // through setHtml, or hiding would think "Show" is still there
    btn.setAttribute('aria-pressed', 'true');
    // Not left on screen: hidden again after half a minute, when Settings is left or the window loses focus.
    clearTimeout(dcHideTimer);
    dcHideTimer = setTimeout(hideDiscordLink, 30_000);
  }

  let dcHideTimer = 0;
  function hideDiscordLink() {
    clearTimeout(dcHideTimer);
    if ($('#s-dc-hook')?.dataset.shown) syncDiscord(true);
  }
  window.addEventListener('blur', () => hideDiscordLink());

  async function removeDiscordLink() {
    if (await confirmBox(t('dc.removeConfirm'), { title: t('dc.removeTitle'), ok: t('dc.remove'), danger: true })) call('setDiscordWebhook', { url: '' });
  }

  /** Settings › Discord › Includes: what the summary lists (at least one), as a short list like the accounts picker. */
  function pickDiscordParts() {
    const parts = new Set(S.app.discord?.summary?.parts || []);
    modal({
      title: t('dc.partsTitle'),
      body: `<p class="modal-text">${t('dc.partsText')}</p><div class="rc-list">${DC_PARTS.map(p => `<label class="rc-item">
          <input type="checkbox" value="${p}" ${parts.has(p) ? 'checked' : ''}><span><b>${t(`dc.part.${p}`)}</b></span></label>`).join('')}</div>`,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            const list = [...el.querySelectorAll('.rc-item input')].filter(b => b.checked).map(b => b.value);
            if (!list.length) return toast(t('dc.partsText'), 'warn');
            if (await busy(btn, () => call('setDiscord', { summaryParts: list })) !== undefined) close();
          },
        },
      ],
    });
  }

  function pickDiscordAccounts() {
    const list = accounts().slice().sort((x, y) => compareText(x.label, y.label));
    if (!list.length) return toast(t('rc.noAccounts'), 'info');
    const chosen = new Set(S.app.discord?.summary?.accounts || []);
    modal({
      title: t('dc.accTitle'),
      body: `<p class="modal-text">${t('dc.accText')}</p><div class="rc-list">${list.map(a => `<label class="rc-item">
          <input type="checkbox" value="${a.id}" ${chosen.has(a.id) ? 'checked' : ''}>
          <div class="avatar sm">${avatarInner(a)}</div><span><b title="${esc(a.label)}">${esc(a.label)}</b><small>${atName(a)}</small></span></label>`).join('')}</div>`,
      actions: [
        { text: t('dc.allAccounts'), onClick: async ({ close }) => { if (await call('setDiscord', { summaryAccounts: [] }) !== undefined) close(); } },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            const ids = [...el.querySelectorAll('.rc-item input')].filter(b => b.checked).map(b => b.value);
            if (await busy(btn, () => call('setDiscord', { summaryAccounts: ids })) !== undefined) close();
          },
        },
      ],
    });
  }

  /** Release notes, newest first: shown once after an update, and any time from Settings › Updates. */
  function showWhatsNew(releases, onClose) {
    if (!releases?.length) return;
    const day = d => {
      const date = new Date(`${d}T00:00:00`);
      return isNaN(date) ? '' : date.toLocaleDateString(locale(), { day: 'numeric', month: 'long', year: 'numeric' });
    };
    modal({
      title: t('wn.title', { v: esc(releases[0].version) }),
      body: releases.map(r => `<section class="wn"><div class="wn-head"><b>${esc(r.version)}</b><span>${day(r.date)}</span></div>
        <ul>${((lang === 'vi' && r.vi?.length ? r.vi : r.en) || []).map(n => `<li>${esc(n)}</li>`).join('')}</ul></section>`).join(''),
      actions: [{ text: t('wn.ok'), kind: 'primary', onClick: ({ close }) => close() }],
      onClose,
    });
  }

  function syncUpdate() {
    const u = S.app.update || {};
    // "Up to date" as of the last check, which can be up to an hour old: it says when.
    const at = u.checkedAt ? new Date(u.checkedAt) : null;
    const hm = { hour: '2-digit', minute: '2-digit', ...(lang === 'vi' ? { hourCycle: 'h23' } : {}) };
    const when = at && !isNaN(at) ? (at.toDateString() === new Date().toDateString()
      ? at.toLocaleTimeString(locale(), hm) : at.toLocaleString(locale(), { day: 'numeric', month: 'short', ...hm })) : '';
    const status = !u.supported ? t('upd.unsupported')
      : u.status === 'checking' ? t('upd.checking')
      : u.status === 'downloading' ? t('upd.downloading', { v: u.version, p: u.progress })
      : u.status === 'ready' ? t(u.auto ? 'upd.readyAuto' : 'upd.ready', { v: u.version })
      : u.status === 'upToDate' ? t('upd.latest') + (when ? ' · ' + t('upd.checkedAt', { t: when }) : '')
      : u.status === 'error' ? (u.error ? `${t('upd.error')}: ${u.error}` : t('upd.error')) // the reason, when the app gives one
      : u.auto ? t('upd.idle') : t('upd.off');
    const notes = `<button class="btn sm ghost" data-act="whats-new">${t('wn.link')}</button>`;
    const action = !u.supported ? ''
      : u.status === 'ready' ? `<button class="btn sm primary" data-act="apply-update">${icon('refresh')}${t('upd.restart')}</button>`
      : `<button class="btn sm" data-act="check-updates" ${u.status === 'checking' || u.status === 'downloading' ? 'disabled' : ''}>${icon('refresh')}${t('upd.check')}</button>`;
    setHtml($('#s-update'), `<div class="set-row"><div><div class="t">Bloxkeep ${esc(u.current || '')}</div><div class="d">${esc(status)}</div></div><div class="ctl">${notes}${action}</div></div>`);
    $('#s-autoupd').checked = !!u.auto;
    $('#s-autoupd').disabled = !u.supported;
  }

  const THEMES = {
    basic: ['system', 'light', 'paper', 'dim', 'dark', 'black', 'contrast'],
    scenery: ['aurora', 'sunset', 'city', 'space', 'mist', 'snow', 'desert', 'meadow'],
    color: ['frost', 'ocean', 'forest', 'mocha', 'grape', 'lava', 'neon', 'candy', 'matcha', 'sakura'],
    style: ['retro', 'terminal', 'gameboy', 'blueprint', 'newspaper'],
    season: ['halloween', 'lunar', 'spring'],
  };
  /** One small swatch per theme, one line per group. Each swatch carries data-theme, so it shows that theme's own colours. */
  function themesHtml() {
    const current = S.themePending || S.app.theme || 'dark';
    const button = id => {
      const on = id === current, name = t(`theme.${id}`);
      const inner = id === 'system' ? '<span data-theme="light"></span><span data-theme="dark"></span>' : `<span data-theme="${id}"><em></em></span>`;
      return `<button class="swatch ${on ? 'on' : ''}" data-act="theme" data-id="${id}" role="radio" aria-checked="${on}" tabindex="${on ? 0 : -1}" aria-label="${name}" data-name="${name}">${inner}</button>`; // data-name: the name shown on hover and focus
    };
    return Object.entries(THEMES).map(([group, ids]) =>
      `<div class="tg"><span class="tg-label">${t(`theme.group.${group}`)}</span><div class="tg-list">${ids.map(button).join('')}</div></div>`).join('');
  }

  function syncSettings() {
    const st = S.app.settings, m = S.app.multi;
    if (!st || !$('#s-multi')) return;
    $('#s-multi').checked = m.enabled;
    $('#s-rename').checked = st.renameWindows;
    $('#s-winlayout').value = st.windowLayout || 'fill';
    if (document.activeElement !== $('#s-openas')) $('#s-openas').value = S.app.multi?.tile ? 'tile' : st.maximizeOnOpen ? 'maximize' : 'roblox';
    $('#s-memwarn').checked = st.memoryWarnings;
    $('#s-trim').checked = st.trimBackground;
    $('#s-antiafk').checked = !!st.antiAfk;
    const rc = st.robloxConfig || { mode: 'off', graphics: 0, fps: 0, volume: -1 };
    for (const [key, value] of [['mode', rc.mode], ['graphics', rc.graphics], ['fps', rc.fps], ['volume', rc.volume]]) {
      const el = $(`#s-rc-${key}`);
      if (document.activeElement !== el) {
        el.value = String(value);
        // A value the list doesn't offer would leave the box blank: show "leave as is" (or "nobody") instead.
        if (el.selectedIndex < 0) el.selectedIndex = 0;
      }
      if (key !== 'mode') el.disabled = rc.mode === 'off';
    }
    const chosen = accounts().filter(a => a.useRobloxConfig).length;
    setHtml($('#s-rc-hint'), rc.mode === 'off' ? '' : rc.mode === 'all' ? esc(t('rc.hint.all'))
      : `${esc(tn('rc.hint.chosen', chosen))} <button type="button" class="link" data-act="rc-pick">${t('rc.pick')}</button>`);
    $('#s-gpu').checked = !S.app.safeRendering;
    $('#s-tray').checked = S.app.closeToTray !== false;
    $('#s-hidenames').checked = hidingNames();
    const notify = S.app.notifications !== false, off = S.app.notifyOff || [];
    $('#s-notify').checked = notify;
    // The per-kind switches only mean something while the main one is on.
    $('#s-notify-kinds').classList.toggle('off', !notify);
    for (const kind of NOTIFY_KINDS) {
      const sw = $(`#s-notify-${kind}`);
      sw.checked = !off.includes(kind);
      sw.disabled = !notify;
    }
    $('#s-daily').checked = S.app.dailySignInCheck !== false;
    const checkedAt = S.app.lastSignInCheck ? new Date(S.app.lastSignInCheck) : null;
    setText($('#s-daily-when'), checkedAt && !isNaN(checkedAt) ? t('set.dailyLast', { t: checkedAt.toDateString() === new Date().toDateString()
      ? checkedAt.toLocaleTimeString(locale(), { hour: 'numeric', minute: '2-digit' })
      : checkedAt.toLocaleString(locale(), { day: 'numeric', month: 'short', hour: 'numeric', minute: '2-digit' }) }) : '');
    syncBackup();
    // The swatches are rebuilt on a theme change; the one being used from the keyboard keeps the focus.
    const themesFocused = $('#s-themes').contains(document.activeElement);
    if (setHtml($('#s-themes'), themesHtml()) && themesFocused) $('#s-themes .swatch.on')?.focus();
    setText($('#s-theme-name'), t(`theme.${S.themePending || S.app.theme || 'dark'}`));
    syncUpdate();
    const ins = S.app.installs || {};
    // [value, label, usable, label when not usable]
    const others = ins.others || [];
    const savedOther = (st.launchVia || '').startsWith('other:') ? st.launchVia.slice(6) : null;
    const options = [
      ['auto', ins.handler ? t('launch.auto', { name: esc(ins.handler) }) : t('launch.autoNone'), true],
      ['bloxstrap', 'Bloxstrap', ins.bloxstrap],
      ['fishstrap', 'Fishstrap', ins.fishstrap],
      // Other Bloxstrap-style launchers only appear when found on this PC (or still chosen after being removed).
      ...[...new Set([...others, ...(savedOther ? [savedOther] : [])])].map(n => [`other:${esc(n)}`, esc(n), others.includes(n)]),
      ['roblox', t('launch.roblox'), ins.roblox],
      ['direct', t('launch.direct'), ins.direct],
      ...(ins.store ? [['store', t('launch.storeName'), false, t('launch.storeOpt')]] : []),
      ...(st.customLauncher ? [['custom', t('launch.custom', { name: esc(st.customLauncher) }), true]] : []),
    ];
    const select = $('#s-launch');
    const rebuilt = setHtml(select, options.map(([value, label, ok, off]) =>
      `<option value="${value}" ${ok ? '' : 'disabled'}>${ok ? label : off || t('launch.missing', { name: label })}</option>`).join(''));
    // New options reset the box to the first one, so put the saved choice back even while it has focus.
    if (rebuilt || document.activeElement !== select) {
      select.value = st.launchVia || 'auto';
      if (select.selectedIndex < 0) select.selectedIndex = 0;
    }
    const rv = S.app.robloxVersion || {};
    const rvText = !rv.installed ? t('rbx.notInstalled')
      : rv.upToDate === false ? t(rv.direct ? 'rbx.outdatedDirect' : 'rbx.outdated', { v: esc(rv.latest) })
      : rv.upToDate ? t('rbx.latest') : t('rbx.unknown');
    setHtml($('#s-rbxver'), `<div class="set-row"><div><div class="t">${t('rbx.title')}</div><div class="d">${rvText}</div></div><div class="ctl"><span class="state-pill ${rv.upToDate === false ? 'warn' : rv.upToDate ? 'ok' : 'off'}">${esc(rv.installed || '—')}</span></div></div>`);
    syncRobloxUpdates();
    setHtml($('#s-store'), ins.store ? `<div class="set-note">${icon('alert')}<span>${t('launch.store')}</span></div>` : '');
    syncRejoinAll();
    const delay = $('#s-delay');
    if (document.activeElement !== delay) delay.value = st.launchDelaySeconds;
    // Written only when different: every write, even of the same text, made the page re-style itself.
    setText($('#s-delay-v'), t('set.seconds', { n: delay.value }));
    const pill = $('#s-multi-pill'), pillCls = `state-pill ${!m.enabled ? 'off' : m.armed ? 'ok' : 'warn'}`;
    if (pill.className !== pillCls) pill.className = pillCls;
    setText(pill, t(!m.enabled ? 'pill.off' : m.armed ? 'pill.ready' : 'pill.notReady'));
    setText($('#s-path'), S.app.dataPath || '');
    syncDiscord();
    syncSettingsNav();
    // Parts drawn again by this sync (Roblox updates, Discord status) are searched again while the box has text.
    if ($('#set-q').value) searchSettings($('#set-q').value);
  }

  /** Shows a settings page — or the page a group is on, scrolled to that group (links from elsewhere in the app). */
  function openSettings(target) {
    const group = SET_PAGES.some(p => p[0] === target) ? null : $(`#sec-${target}`);
    const id = group?.closest('.set-page')?.dataset.page || (group ? null : target);
    if (!SET_PAGES.some(p => p[0] === id)) return;
    setView('settings');
    const q = $('#set-q');
    if (q.value) {
      q.value = '';
      searchSettings('');
    }
    setPage = id;
    store.set(SET_PAGE_KEY, id);
    if (id !== 'discord') hideDiscordLink(); // a shown webhook link goes when its page is left
    for (const el of document.querySelectorAll('#view-settings .set-page')) el.hidden = el.dataset.page !== id;
    for (const b of document.querySelectorAll('#view-settings [data-set-page]')) {
      if (b.dataset.setPage !== id) {
        b.removeAttribute('aria-current');
        continue;
      }
      b.setAttribute('aria-current', 'page');
      b.scrollIntoView({ block: 'nearest', inline: 'nearest' }); // the row of tabs in a narrow window
    }
    if (group) group.scrollIntoView({ block: 'start' });
    else $('#view-settings .set-scroll').scrollTop = 0;
  }

  /**
   * The search box: the rows of every page whose title or description matches (all of a group or page whose heading
   * matches), under their page's title; empty, it shows the chosen page again. A part that isn't a row (the theme
   * picker, a note) goes with the row before it.
   */
  function searchSettings(text) {
    const q = fold(text.trim());
    const root = $('#view-settings');
    root.querySelector('.set-layout').classList.toggle('searching', !!q);
    for (const el of root.querySelectorAll('.set-miss')) el.classList.remove('set-miss');
    let found = false;
    for (const pageEl of root.querySelectorAll('.set-page')) {
      if (!q) {
        pageEl.hidden = pageEl.dataset.page !== setPage;
        continue;
      }
      const pageHit = fold(pageEl.querySelector('.set-page-title').textContent).includes(q);
      let pageShown = false;
      for (const grp of pageEl.querySelectorAll('.set-grp')) {
        const grpHit = pageHit || fold(grp.querySelector('.set-grp-title')?.textContent).includes(q);
        // A row in a closed part (Discord's message look, the alert kinds) is found by that part's name too.
        const rowHit = r => grpHit || fold(`${r.querySelector('.t')?.textContent} ${r.querySelector('.d')?.textContent} ${r.closest('.set-more')?.querySelector('summary')?.textContent || ''}`).includes(q);
        let grpShown = false, lastShown = false;
        for (const child of grp.querySelector('.set-card').children) {
          const rows = child.matches('.set-row') ? [child] : [...child.querySelectorAll('.set-row')];
          let shown;
          if (rows.length) {
            shown = false;
            for (const r of rows) {
              const hit = rowHit(r);
              r.classList.toggle('set-miss', !hit);
              shown ||= hit;
            }
            lastShown = shown;
          } else {
            shown = grpHit || lastShown;
          }
          child.classList.toggle('set-miss', !shown);
          grpShown ||= shown && !!child.textContent.trim();
        }
        grp.classList.toggle('set-miss', !grpShown);
        pageShown ||= grpShown;
      }
      pageEl.hidden = !pageShown;
      found ||= pageShown;
    }
    $('#set-empty').hidden = !q || found;
    // Closed parts open while the search shows rows in them, and go back as they were once the box is emptied.
    for (const more of root.querySelectorAll('.set-more')) {
      if (q) {
        if (!('was' in more.dataset)) more.dataset.was = more.open ? '1' : '';
        more.open = !!more.querySelector('.set-row:not(.set-miss)');
      } else if ('was' in more.dataset) {
        more.open = more.dataset.was === '1';
        delete more.dataset.was;
      }
    }
    // New words start at the top of the results; the same words searched again (a state push, a switch flipped in the
    // results) keep the place being read.
    if (q && q !== lastSetQuery) root.querySelector('.set-scroll').scrollTop = 0;
    lastSetQuery = q;
    // A shown Discord link doesn't stay behind on a page the search hides.
    if (root.querySelector('.set-page[data-page="discord"]')?.hidden) hideDiscordLink();
  }
  let lastSetQuery = '';

  /** A date and time in the app's language ("Oct 13, 2026, 8:25 PM"; 24-hour in Vietnamese). */
  const fmtWhen = iso => new Date(iso).toLocaleString(locale(), lang === 'vi'
    ? { dateStyle: 'medium', timeStyle: 'short', hourCycle: 'h23' } : { dateStyle: 'medium', timeStyle: 'short' });
  /** Roblox's id for a build ("version-cec3ad5889b447cf"), shortened to its first 8 characters. */
  const shortBuild = b => String(b || '').replace(/^version-/i, '').slice(0, 8);

  /**
   * Settings › Roblox updates: Roblox closing windows (the warning), the installed build and the newest Roblox serves,
   * and the builds seen on this PC, newest first, each with a link to Roblox's release notes for its version.
   */
  function syncRobloxUpdates() {
    const ru = S.app.robloxUpdates || {}, rv = S.app.robloxVersion || {};
    const row = (title, desc, ctl) => `<div class="set-row"><div><div class="t">${title}</div>${desc ? `<div class="d">${desc}</div>` : ''}</div><div class="ctl">${ctl}</div></div>`;
    const c = ru.closed;
    const closedBox = $('#s-rbxclosed'), closedFocused = closedBox.contains(document.activeElement);
    setHtml(closedBox, c ? `<div class="set-note">${icon('alert')}<span>${tn('ru.closed', c.count, { t: esc(fmtWhen(c.at)) })}${c.updatedAt
      ? ` ${t('ru.closedUpdate', { t: esc(fmtWhen(c.updatedAt)) })}` : ''} <button type="button" class="link" data-act="ru-hide">${t('ru.hide')}</button></span></div>` : '');
    const inst = ru.installed, hist = ru.history || [];
    const items = hist.map((h, i) => {
      const older = hist[i + 1];
      const note = h.first ? t('ru.first') : older && older.version === h.version ? t('ru.sameVersion') : '';
      return `<li class="ru-item"><div class="ru-text"><div class="ru-main"><b>${esc(h.version)}</b>${note ? `<span class="ru-note">${note}</span>` : ''}</div>
        <div class="ru-sub">${esc(fmtWhen(h.at))} · <span title="${esc(h.build)}">${t('ru.build', { b: esc(shortBuild(h.build)) })}</span>${h.source ? ` · ${esc(h.source)}` : ''}</div></div>
        <button type="button" class="link ru-link" data-act="ru-notes" data-v="${esc(h.version)}" title="${esc(`${t('ru.notes')} — ${t('ru.notesHint')}`)}" aria-label="${esc(`${t('ru.notes')} ${h.version}`)}">${t('ru.notesShort')}</button></li>`;
    }).join('');
    // The newest version only when it isn't the one installed (the row above already shows that number).
    setHtml($('#s-rbxupd'), `
      ${row(t('ru.installed'), inst?.source ? esc(t('ru.via', { s: inst.source })) : '', inst ? `<span class="ru-build" title="${esc(inst.build)}">${t('ru.build', { b: esc(shortBuild(inst.build)) })}</span>` : '—')}
      ${rv.latest && rv.latest === rv.installed ? '' : row(t('ru.newest'), '', `<span class="state-pill ${rv.latest ? 'off' : 'warn'}">${esc(rv.latest || t('rbx.unknown'))}</span>`)}
      <div class="ru-head">${t('ru.history')}</div>
      ${items ? `<ul class="ru-list">${items}</ul>` : `<p class="set-hint">${t('ru.none')}</p>`}`);
    // The note went (Hide) with the focus in it: on to the page's next control, not back to the top of the window.
    if (closedFocused && closedBox.closest('.set-page') && !closedBox.contains(document.activeElement) && (document.activeElement === document.body || !document.activeElement)) {
      const page = closedBox.closest('.set-page');
      const next = [...page.querySelectorAll('button, input, select, a[href]')]
        .find(x => closedBox.compareDocumentPosition(x) & Node.DOCUMENT_POSITION_FOLLOWING && !x.disabled && x.offsetParent);
      (next || $(`#view-settings [data-set-page="${page.dataset.page}"]`))?.focus();
    }
  }

  /** A dot on a page in the list when something there needs a look: Roblox closed windows, Discord stopped sending. */
  function syncSettingsNav() {
    const need = { roblox: !!S.app.robloxUpdates?.closed, discord: !!S.app.discord?.stopped };
    for (const dot of document.querySelectorAll('#view-settings .set-nav-dot')) dot.hidden = !need[dot.closest('[data-set-page]').dataset.setPage];
  }

  /** Settings › Discord from the state; a box being typed in keeps what is typed. */
  /** @param force also the box being typed in (a value the app refused, a time box emptied). */
  function syncDiscord(hideLink = false, force = false) {
    const d = S.app.discord, root = $('#view-settings');
    if (!d || !$('#s-dc-on')) return;
    const typing = el => !force && document.activeElement === el;
    // The saved value put back in a box drops the red mark a refused value left there.
    const put = (id, value) => { const el = $(id); if (!typing(el)) { el.value = String(value ?? ''); el.classList.remove('bad'); } };
    $('#s-dc-on').checked = d.enabled;
    $('#s-dc-on').closest('.switch').title = d.hasWebhook ? '' : t('dc.needLink');
    const hook = $('#s-dc-hook'), reveal = $('#s-dc-reveal');
    // A shown link goes back to its masked form when hidden, or when another link replaced it.
    if (hideLink || hook.dataset.shown !== d.webhookHint || !d.hasWebhook) {
      delete hook.dataset.shown;
      setText(hook, d.hasWebhook ? d.webhookHint : t('dc.noLink'));
      setHtml(reveal, `${icon('eye')}${t('dc.show')}`);
      reveal.setAttribute('aria-pressed', 'false');
    }
    setText($('#s-dc-paste'), t(d.hasWebhook ? 'dc.replace' : 'dc.paste'));
    reveal.hidden = $('#s-dc-remove').hidden = !d.hasWebhook;
    setHtml($('#s-dc-note'), d.stopped ? `<div class="set-note">${icon('alert')}<span>${t('dc.gone')}</span></div>` : '');
    const last = d.lastSent ? `<span data-ago="${esc(d.lastSent)}" data-tpl="dc.last">${t('dc.last', { t: ago(d.lastSent) })}</span>` : t('dc.never');
    setHtml($('#s-dc-status'), !d.hasWebhook ? '' : `<div class="set-row"><div><div class="t">${last}</div>${d.error ? `<div class="d err">${esc(d.error)}</div>` : ''}${d.queued ? `<div class="d">${t('dc.queued', { n: d.queued })}</div>` : ''}</div>
      <div class="ctl"><button class="btn sm" data-dc="test">${t('dc.test')}</button></div></div>`);
    put('#s-dc-name', d.username);
    put('#s-dc-avatar', d.avatarUrl);
    put('#s-dc-lang', d.language);
    put('#s-dc-style', d.style);
    $('#s-dc-hidenames').checked = !!d.hideNames;
    const s = d.summary || {};
    const when = s.daily && s.weekly ? 'both' : s.daily ? 'daily' : s.weekly ? 'weekly' : 'none';
    put('#s-dc-when', when);
    if (!typing($('#s-dc-when'))) syncDiscordWhen(when);
    put('#s-dc-daily-time', s.dailyTime);
    put('#s-dc-weekday', s.weekday);
    put('#s-dc-weekly-time', s.weeklyTime);
    $('#s-dc-after').checked = !!s.afterSchedule;
    put('#s-dc-days', s.days);
    setText($('#s-dc-parts'), tn('dc.partsN', (s.parts || []).length));
    const picked = (s.accounts || []).length;
    setText($('#s-dc-acc'), picked ? t('dc.accN', { n: picked }) : t('dc.accAll'));
    $('#s-dc-board').checked = !!d.board?.enabled;
    put('#s-dc-minutes', d.board?.minutes);
    for (const kind of NOTIFY_KINDS) $(`#s-dc-ev-${kind}`).checked = (d.events || []).includes(kind);
    if (!$('#s-dc-alerts').disabled) $('#s-dc-alerts').checked = NOTIFY_KINDS.some(k => (d.events || []).includes(k));
    // "alerts" shows the ping every kind shares, or "Varies by alert".
    const alertPings = NOTIFY_KINDS.map(k => d.pings?.[k] || 'none');
    for (const kind of [...DC_PING_KINDS, 'alerts']) {
      const sel = root.querySelector(`[data-ping="${kind}"]`), box = root.querySelector(`[data-ping-id="${kind}"]`);
      if (sel.dataset.pending || typing(sel) || typing(box)) continue;
      const value = kind !== 'alerts' ? d.pings?.[kind] || 'none' : alertPings.every(p => p === alertPings[0]) ? alertPings[0] : 'mixed';
      const [mode, id = ''] = value.split(':');
      sel.value = mode;
      box.hidden = !id;
      box.value = id;
      box.classList.remove('bad');
    }
    $('#s-dc-quiet').checked = !!d.quiet?.enabled;
    put('#s-dc-quiet-from', d.quiet?.from);
    put('#s-dc-quiet-to', d.quiet?.to);
  }
  /** The time boxes of the automatic summaries picked (daily, weekly, both); none for "Never". */
  function syncDiscordWhen(when) {
    $('#s-dc-daily-row').hidden = when !== 'daily' && when !== 'both';
    $('#s-dc-weekly-row').hidden = when !== 'weekly' && when !== 'both';
  }

  /** Settings › Backup: folder, suggested synced folders, last backup (kept fresh by patchTimes) and any error. */
  function syncBackup() {
    const b = S.app.backup || {};
    $('#s-bk-on').checked = b.enabled !== false;
    setText($('#s-bk-path'), b.folder || t('bk.noFolder'));
    $('#s-bk-open').hidden = !b.folder;
    setHtml($('#s-bk-suggest'), (b.suggested || []).map(s =>
      `<button class="btn sm ghost" data-act="bk-use" data-path="${esc(s.path)}" title="${esc(s.path)}">${t('bk.use', { name: esc(s.name) })}</button>`).join(''));
    setHtml($('#s-bk-note'), b.cloud && b.weakPassword ? `<div class="set-note">${icon('alert')}<span>${t('bk.weak')}</span></div>` : '');
    const last = b.lastBackup ? `<span data-ago="${esc(b.lastBackup)}" data-tpl="bk.last">${t('bk.last', { t: ago(b.lastBackup) })}</span>` : t('bk.never');
    setHtml($('#s-bk-status'), `<div class="set-row"><div><div class="t">${last}</div>${b.error ? `<div class="d err">${esc(b.error)}</div>` : ''}</div>
      <div class="ctl"><button class="btn sm" data-act="bk-now" ${b.folder ? '' : 'disabled'}>${t('bk.now')}</button></div></div>`);
  }

  /** Settings › Backup › Restore…: confirm, then the app asks for the file, keeps a copy of the current data and locks. */
  async function restoreBackup() {
    if (await confirmBox(t('bk.restoreConfirm'), { title: t('bk.restoreConfirmTitle'), ok: t('bk.restoreBtn'), danger: true })) await call('restoreBackup');
  }

  /* ───────────────────────── game picker ─────────────────────────
     Typing a name searches Roblox; an empty box offers recent, pinned, favourite and popular games. */
  const P = { open: false, mode: 'browse', tab: 'recent', lists: null, results: [], query: '', index: -1, seq: 0, loading: false };
  const TABS = ['recent', 'pinned', 'favorites', 'popular'];
  const looksLikeTarget = v => /^\d+$/.test(v) || /roblox\.com|roblox:\/\//i.test(v);
  const pickerRows = () => (P.mode === 'search' ? P.results : P.lists?.[P.tab]) || [];

  function openPicker(mode) {
    P.open = true;
    P.mode = mode;
    P.index = -1;
    renderPicker();
    if (mode === 'browse') loadGameLists();
  }
  function closePicker() {
    if (!P.open) return;
    P.open = false;
    renderPicker();
  }
  async function loadGameLists() {
    P.loading = !P.lists;
    renderPicker();
    const r = await call('gameLists', {}, { silent: true });
    P.loading = false;
    if (r) {
      P.lists = r;
      if (!r[P.tab]?.length) P.tab = TABS.find(k => r[k]?.length) || 'recent';
    }
    renderPicker();
  }
  function renderPicker() {
    const el = $('#picker');
    if (!el) return;
    el.hidden = !P.open;
    $('#place').setAttribute('aria-expanded', String(P.open));
    if (P.open && P.index >= 0) $('#place').setAttribute('aria-activedescendant', `pk-${P.index}`);
    else $('#place').removeAttribute('aria-activedescendant');
    if (!P.open) return;
    const rows = pickerRows();
    const head = P.mode === 'browse'
      ? `<div class="picker-tabs" role="tablist">${TABS.filter(k => k !== 'popular' || P.lists?.popular?.length).map(k =>
          `<button class="ptab ${P.tab === k ? 'on' : ''}" role="tab" data-act="picker-tab" data-tab="${k}">${t(`picker.${k}`)}</button>`).join('')}</div>`
      : `<div class="picker-head">${t('picker.results', { q: esc(P.query) })}</div>`;
    const body = P.loading ? '<div class="game-row skel"><span class="gph"></span><div class="gtxt"><b></b><small></small></div></div>'.repeat(5)
      : !rows.length ? `<div class="picker-empty">${t(P.mode === 'search' ? 'picker.noResults' : `picker.empty.${P.tab}`)}</div>`
      : rows.map((g, i) => `<div class="game-row ${i === P.index ? 'on' : ''}" role="option" id="pk-${i}" aria-selected="${i === P.index}" data-act="pick-game" data-i="${i}">
          ${g.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : `<span class="gph">${icon('gamepad')}</span>`}
          <div class="gtxt"><b title="${esc(g.name)}">${esc(g.name)}</b><small>${g.playing ? t('dock.playing', { n: fmtNum(g.playing) }) : `Place ${g.placeId}`}</small></div>
          <button class="icon-btn pin ${g.pinned ? 'on' : ''}" data-act="pin-game" data-i="${i}" title="${t(g.pinned ? 'picker.unpin' : 'picker.pin')}">${icon(g.pinned ? 'starFill' : 'star')}</button>
        </div>`).join('');
    setHtml(el, `${head}<div class="picker-list ${P.refreshing ? 'refreshing' : ''}" role="listbox" id="picker-list">${body}</div>`);
    el.querySelector('.game-row.on')?.scrollIntoView({ block: 'nearest' });
  }
  let searchTimer = 0;
  // Recent searches, so backspacing or typing a name again shows its results at once; the oldest make room.
  const searchCache = new Map();
  const SEARCH_TTL = 5 * 60 * 1000, SEARCH_KEEP = 50;
  function rememberSearch(query, rows) {
    searchCache.delete(query);
    searchCache.set(query, { at: Date.now(), rows });
    while (searchCache.size > SEARCH_KEEP) searchCache.delete(searchCache.keys().next().value);
  }

  /** Search results show as soon as they arrive; their icons are fetched afterwards and filled in. */
  async function fillIcons(seq) {
    const ids = [...new Set(P.results.filter(g => !g.iconUrl && g.universeId).map(g => g.universeId))];
    if (!ids.length) return;
    const icons = await call('gameIcons', { universeIds: ids }, { silent: true });
    if (!icons) return;
    for (const g of P.results) if (!g.iconUrl && icons[g.universeId]) g.iconUrl = icons[g.universeId];
    if (seq === P.seq && P.open && P.mode === 'search') renderPicker();
  }

  function onPlaceInput() {
    const value = $('#place').value.trim();
    scheduleSaveInputs();
    clearTimeout(searchTimer);
    ++P.seq; // whatever search is still on its way answers an older box
    renderDock();
    if (!value) {
      S.game = null;
      renderDock();
      openPicker('browse');
      return;
    }
    if (looksLikeTarget(value)) {
      closePicker();
      scheduleResolve();
      return;
    }
    S.game = null;
    renderDock();
    const cached = searchCache.get(value.toLowerCase());
    if (cached && Date.now() - cached.at < SEARCH_TTL) {
      Object.assign(P, { open: true, mode: 'search', query: value, results: cached.rows, loading: false, refreshing: false, index: -1 });
      renderPicker();
      return;
    }
    // Keep the last results on screen, dimmed, while the new ones load, instead of flashing placeholders.
    const keep = P.mode === 'search' && P.results?.length > 0;
    Object.assign(P, { open: true, mode: 'search', query: value, loading: !keep, refreshing: keep, index: -1 });
    renderPicker();
    searchTimer = setTimeout(async () => {
      const seq = P.seq;
      const r = await call('searchGames', { query: value }, { silent: true });
      if (seq !== P.seq || !P.open) return;
      P.results = r || [];
      P.loading = P.refreshing = false;
      if (r) rememberSearch(value.toLowerCase(), P.results);
      renderPicker();
      fillIcons(seq);
    }, 200);
  }
  function onPlaceKey(e) {
    const rows = pickerRows();
    if (P.open && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && rows.length) {
      e.preventDefault();
      P.index = (P.index + (e.key === 'ArrowDown' ? 1 : rows.length - 1) + (P.index < 0 && e.key === 'ArrowUp' ? 1 : 0)) % rows.length;
      renderPicker();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (P.loading || P.refreshing) return; // results for what was just typed aren't in yet
      if (P.open && rows.length && (P.index >= 0 || P.mode === 'search')) pickGame(rows[Math.max(0, P.index)]);
      // A search that found nothing isn't a game to open (nor one to remember as the last input).
      else if (!looksLikeTarget($('#place').value.trim()) && $('#place').value.trim()) return;
      else launch([...S.selected], false);
    } else if (e.key === 'Escape' && P.open) {
      e.stopPropagation();
      closePicker();
    }
  }
  function pickGame(g) {
    if (!g) return;
    $('#place').value = `https://www.roblox.com/games/${g.placeId}`;
    $('#job').value = '';
    closePicker();
    $('#place').blur();
    resolveGame();
    scheduleSaveInputs();
  }
  /** Puts a game and one of its servers in the dock; Play then sends the selected accounts there. */
  function useServer(placeId, jobId) {
    S.jobPlace = placeId;
    if ($('#place').value.trim() !== `https://www.roblox.com/games/${placeId}`) {
      $('#place').value = `https://www.roblox.com/games/${placeId}`;
      resolveGame();
    }
    $('#job').value = jobId;
    scheduleSaveInputs();
  }

  /** The game's public servers (like the Servers tab on its Roblox page): pick one to join it. */
  /**
   * The game's public servers (like the Servers tab on its Roblox page): pick one to join it. Roblox sorts by player
   * count itself; ping and FPS are sorted here over the servers loaded so far. "ping" is the average ping of the
   * players already in that server (what Roblox reports), not the ping from this PC.
   */
  function openServers() {
    const g = S.game;
    if (!g?.placeId) return;
    const party = S.selected.size;
    const st = { sort: 'most', room: party > 1, items: [], view: [], next: null, loading: false, busy: false, seq: 0, friends: null };
    const SORTS = ['most', 'fewest', 'ping', 'fps'];
    const local = () => st.sort === 'ping' || st.sort === 'fps';
    let api;
    const visibleServers = () => {
      const list = st.room ? st.items.filter(s => s.maxPlayers - s.playing >= party) : st.items.slice();
      if (st.sort === 'ping') list.sort((a, b) => (a.ping || 1e9) - (b.ping || 1e9));
      if (st.sort === 'fps') list.sort((a, b) => (b.fps || 0) - (a.fps || 0));
      // Full servers can't be joined: they go last (the sort keeps the rest in order).
      return list.sort((a, b) => isFull(a) - isFull(b));
    };
    const isFull = s => s.maxPlayers > 0 && s.playing >= s.maxPlayers;
    const rows = () => st.view.map((s, i) => `<div class="srv-row ${isFull(s) ? 'full' : ''}">
        <div class="srv-fill" title="${t('srv.players', { n: s.playing, max: s.maxPlayers })}"><i style="transform:scaleX(${s.maxPlayers ? (s.playing / s.maxPlayers).toFixed(3) : 0})"></i></div>
        <b>${t('srv.players', { n: s.playing, max: s.maxPlayers })}${friendsIn(s.jobId) ? ` <span class="srv-friend-tag" title="${t('srv.friendsHere')}">${icon('users')}${friendsIn(s.jobId)}</span>` : ''}</b>
        <span class="srv-ping" title="${t('srv.pingTip')}">${s.ping ? t('srv.ping', { ms: s.ping }) : ''}</span>
        <span class="srv-ping">${s.fps ? t('srv.fpsVal', { fps: Math.round(s.fps) }) : ''}</span>
        <button class="btn sm" data-srv="${i}" ${isFull(s) ? `disabled title="${t('srv.full')}"` : `title="${t('srv.pickTip')}"`}>${t('srv.pick')}</button></div>`).join('');
    const friendsIn = jobId => st.friends ? st.friends.filter(f => f.jobId === jobId).length : 0;
    const renderFriends = () => {
      const box = api.el.querySelector('#srv-friends');
      if (!box) return;
      const list = st.friends;
      setHtml(box, list === null ? `<div class="srv-friends-note"><span class="spin"></span>${t('srv.friendsLooking')}</div>`
        : !list.length ? `<div class="srv-friends-note">${t(st.friendsFailed ? 'srv.friendsError' : st.friendsAsked ? 'srv.friendsNone' : 'srv.friendsNoAccount')}</div>`
        : list.map((f, i) => `<div class="srv-friend">
            ${f.avatarUrl ? `<img src="${esc(f.avatarUrl)}" alt="">` : '<span class="ph"></span>'}
            <div class="gtxt"><b title="${esc(f.displayName || f.name || `#${f.userId}`)}">${esc(f.displayName || f.name || `#${f.userId}`)}</b><small>${f.name ? `@${esc(f.name)} · ` : ''}${t('srv.friendOf', { acc: esc(f.via) })}</small></div>
            <button class="btn sm" data-friend="${i}" title="${t('srv.pickTip')}">${t('srv.joinFriend')}</button></div>`).join(''));
    };
    const loadFriends = async () => {
      const r = await call('friendServers', { placeId: g.placeId, ids: [...S.selected] }, { silent: true });
      if (!api.el.isConnected) return;
      st.friends = r?.friends || [];
      st.friendsAsked = !!r?.asked;
      st.friendsFailed = !r;
      renderFriends();
      render();
    };
    const render = () => {
      st.view = visibleServers();
      const list = api.el.querySelector('#srv-list');
      const more = st.next ? `<button class="btn ghost w" data-more ${st.loading ? 'disabled' : ''}>${t('srv.more')}</button>` : '';
      list.innerHTML = st.view.length ? rows() + more
        : st.loading ? '<div class="picker-empty"><span class="spin"></span></div>'
        : st.busy ? `<div class="picker-empty">${t(st.retried ? 'srv.busyStop' : 'srv.busy')} <button class="btn sm" data-retry>${t('srv.retry')}</button></div>`
        : st.items.length ? `<div class="picker-empty">${t('srv.noneRoom', { n: party })}</div>${more}`
        : `<div class="picker-empty">${t('srv.none')}</div>`;
      api.el.querySelectorAll('[data-sort]').forEach(b => {
        b.classList.toggle('on', b.dataset.sort === st.sort);
        b.setAttribute('aria-pressed', String(b.dataset.sort === st.sort));
      });
      const room = api.el.querySelector('[data-room]');
      if (room) {
        room.classList.toggle('on', st.room);
        room.setAttribute('aria-pressed', String(!!st.room));
      }
      api.el.querySelector('[data-random]').disabled = !st.view.some(s => !isFull(s));
    };
    const load = async more => {
      const seq = ++st.seq;
      st.loading = true;
      if (!more) { st.items = []; st.next = null; }
      render();
      // Ping and FPS are sorted here, so start from the fullest servers (the ones people actually play on).
      const r = await call('servers', { placeId: g.placeId, fewest: st.sort === 'fewest', cursor: more ? st.next : null });
      if (seq !== st.seq || !api.el.isConnected) return;
      st.loading = false;
      st.busy = !!r?.busy && !st.items.length;
      if (r && !r.busy) {
        // A server can move between pages while players come and go; keep one row per server.
        const seen = new Set(more ? st.items.map(s => s.jobId) : []);
        st.items = (more ? st.items : []).concat(r.servers.filter(s => !seen.has(s.jobId) && seen.add(s.jobId)));
        st.next = r.next;
      }
      render();
      // A second page gives sorting by ping or FPS more to choose from.
      if (!more && local() && r?.next && !r.busy) load(true);
      // Roblox asked to slow down: try once more by itself a few seconds later.
      if (r?.busy && !st.retried) { st.retried = true; setTimeout(() => api.el.isConnected && seq === st.seq && load(more), 5000); }
    };
    const choose = s => {
      useServer(g.placeId, s.jobId);
      api.close();
      toast(t('srv.chosen'), 'ok');
    };
    api = modal({
      title: t('srv.title', { game: esc(g.name || `Place ${g.placeId}`) }),
      body: `<div class="srv-head">${t('srv.friendsTitle')}</div><div class="srv-friends" id="srv-friends"></div>
        <div class="srv-head">${t('srv.publicTitle')}</div>
        <div class="srv-sort">${SORTS.map(k => `<button class="chip ${k === st.sort ? 'on' : ''}" data-sort="${k}" aria-pressed="${k === st.sort}">${t(`srv.sort.${k}`)}</button>`).join('')}</div>
        <div class="srv-tools">${party > 1 ? `<button class="chip ${st.room ? 'on' : ''}" data-room aria-pressed="${!!st.room}">${icon('users')}${t('srv.room', { n: party })}</button>` : ''}
          <button class="btn sm" data-random disabled>${icon('repeat')}${t('srv.random')}</button></div>
        <div class="srv-list" id="srv-list"></div>`,
      actions: [{ text: t('common.close'), onClick: ({ close }) => close() }],
      onMount: ({ el }) => el.addEventListener('click', e => {
        const sort = e.target.closest('[data-sort]');
        if (sort) {
          const was = st.sort;
          st.sort = sort.dataset.sort;
          // Most/fewest come from Roblox in that order; ping/FPS just re-sort what is loaded (plus a page if needed).
          if (st.sort === 'fewest' || was === 'fewest' || !st.items.length) load(false);
          else if (local() && st.items.length < 150 && st.next) { render(); load(true); }
          else render();
          return;
        }
        if (e.target.closest('[data-room]')) { st.room = !st.room; render(); return; }
        if (e.target.closest('[data-random]')) {
          const open = st.view.filter(s => !isFull(s));
          if (open.length) choose(open[Math.floor(Math.random() * open.length)]);
          return;
        }
        if (e.target.closest('[data-more]')) { load(true); return; }
        if (e.target.closest('[data-retry]')) { st.retried = false; load(false); return; }
        const friend = e.target.closest('[data-friend]');
        if (friend) {
          const f = st.friends[+friend.dataset.friend];
          useServer(f.placeId || g.placeId, f.jobId);
          api.close();
          toast(t('srv.chosen'), 'ok');
          return;
        }
        const pick = e.target.closest('[data-srv]');
        if (pick) choose(st.view[+pick.dataset.srv]);
      }),
    });
    // Its layout (only the list scrolls) hangs on this class: a :has() rule did it, which re-styled the whole page on
    // every change anywhere in it.
    api.el.querySelector('.modal-body').classList.add('srv-body');
    renderFriends();
    loadFriends();
    load(false);
  }

  async function pinGame(g) {
    const pinned = await call('pinGame', { placeId: g.placeId, universeId: g.universeId || 0, name: g.name });
    if (pinned === undefined) return;
    const sync = list => list?.forEach(x => { if (x.placeId === g.placeId) x.pinned = pinned; });
    TABS.forEach(k => sync(P.lists?.[k]));
    sync(P.results);
    if (P.lists) {
      P.lists.pinned = (P.lists.pinned || []).filter(x => x.placeId !== g.placeId);
      if (pinned) P.lists.pinned.unshift({ ...g, pinned: true });
    }
    renderPicker();
  }
  document.addEventListener('mousedown', e => {
    if (P.open && !e.target.closest('#picker, #place, #game-thumb')) closePicker();
  }, true);

  /* ───────────────────────── game preview ───────────────────────── */
  let resolveTimer = 0, saveTimer = 0;
  function scheduleResolve() {
    clearTimeout(resolveTimer);
    resolveTimer = setTimeout(() => resolveGame(true), 450);
  }
  function scheduleSaveInputs() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      if (S.shell) call('setSettings', { lastPlaceInput: $('#place')?.value ?? '', lastJobId: $('#job')?.value ?? '' }, { silent: true });
    }, 900);
  }
  /** @param typed the box was typed in: a Job ID left from another game no longer goes with it and is cleared. */
  async function resolveGame(typed = false) {
    const input = $('#place')?.value.trim() ?? '';
    const seq = ++S.gameSeq;
    if (!input) {
      S.game = null;
      renderDock();
      return;
    }
    S.game = { loading: true };
    renderDock();
    const r = await call('resolveGame', { input }, { silent: true });
    if (seq !== S.gameSeq || !S.shell) return; // a newer input superseded this one
    S.game = r || null;
    const job = $('#job');
    if (typed && r?.placeId && S.jobPlace && r.placeId !== S.jobPlace && job.value.trim()) {
      job.value = '';
      scheduleSaveInputs();
    }
    if (r?.placeId) S.jobPlace = r.placeId;
    renderDock();
  }

  /* ───────────────────────── actions ───────────────────────── */
  function selectOnly(id) {
    S.selected = new Set([id]);
    S.anchor = id;
    renderSelection();
  }
  function renderSelection() {
    syncSelection();
    renderHeader();
    renderDock();
  }
  /** "Select all" means what is on screen: it replaces a selection that included accounts now hidden. */
  function selectAllVisible() {
    S.selected = new Set(visible().map(a => a.id));
    renderSelection();
  }
  function onCardClick(id, e) {
    const ids = e.shiftKey && S.anchor ? visible().map(a => a.id) : [];
    if (e.shiftKey && S.anchor && ids.includes(S.anchor)) {
      const [i, j] = [ids.indexOf(S.anchor), ids.indexOf(id)].sort((a, b) => a - b);
      ids.slice(i, j + 1).forEach(x => S.selected.add(x));
    } else {
      if (S.selected.has(id)) S.selected.delete(id); else S.selected.add(id);
      S.anchor = id;
    }
    renderSelection();
  }

  /** A launch or close already sent and not yet answered: a double click (or Enter twice) sends no second one. */
  const inFlight = new Set();
  async function launch(ids, appOnly, force = false) {
    if (!ids.length) return toast(t('t.selectFirst'), 'warn');
    if (inFlight.has('launch')) return;
    const placeInput = $('#place')?.value.trim() || '';
    // No game picked: open Roblox on its home screen, one window per account.
    if (!placeInput) appOnly = true;
    inFlight.add('launch');
    let r;
    try {
      r = await call('launch', { ids, appOnly, force, placeInput, jobId: $('#job')?.value.trim() || '' });
    } finally {
      inFlight.delete('launch');
    }
    return launchDone(r, () => launch(ids, appOnly, true));
  }

  /** What a launch (Play or a preset) did; a warning that needs an answer runs it again with retry(). */
  async function launchDone(r, retry) {
    if (!r || S.app.locked) return; // locked meanwhile: nothing to ask or report
    if (r.needsConfirm) {
      if (await confirmBox(r.message, { title: t('t.confirmTitle'), ok: t('t.openAnyway') })) return retry();
      return;
    }
    if (r.errors?.length && !S.app.locked) {
      modal({
        title: t(r.launched ? 't.errsSome' : 't.errsAll'),
        body: `<ul class="errs">${r.errors.map(x => `<li>${esc(x)}</li>`).join('')}</ul>`,
        actions: [{ text: t('common.close'), kind: 'primary', onClick: ({ close }) => close() }],
      });
    }
    if (r.launched) toast(tn(r.cancelled ? 't.launchedStopped' : 't.launched', r.launched), 'ok');
    else if (r.cancelled) toast(t('t.stopped'), 'info');
    // Accounts already open where asked were left alone: worth a note, not an error.
    if (r.skipped?.length && !r.cancelled) toast(tn('t.skipped', r.skipped.length), 'info');
  }

  /* ───────────────────────── launch presets ─────────────────────────
     Saved accounts + game + kind of server. The app picks the server when a preset runs (a schedule can run them too). */
  const PRESET_SERVERS = ['any', 'most', 'fewest', 'ping', 'random', 'job'];
  const presets = () => S.app.settings?.presets || [];
  /** The preset's accounts that still exist (deleted ones are skipped when it runs). */
  const presetAccounts = p => p.accountIds.map(byId).filter(Boolean);
  const accCount = n => t(n === 1 ? 'ps.acc1' : 'ps.accN', { n });
  const presetSub = p => [p.gameName || p.placeInput || t('ps.noGame'), accCount(presetAccounts(p).length),
    p.server !== 'any' && p.placeInput ? t(`ps.srv.${p.server}`) : '', nextRunText(p)].filter(Boolean).join(' · ');

  /* Options for the windows a preset opens, as four small dropdowns; null / '' / 0 follow the usual settings. */
  const RESTART_HOURS = [0, 2, 4, 6, 12]; // the app's own choices (RestartChoices)
  const triValue = v => v === true ? 'on' : v === false ? 'off' : '';
  const triOptions = (v, follow) => [['', follow], ['on', t('ps.opt.on')], ['off', t('ps.opt.off')]]
    .map(([k, text]) => `<option value="${k}" ${k === triValue(v) ? 'selected' : ''}>${esc(text)}</option>`).join('');
  function presetOptionsHtml(p) {
    const afkNow = t(S.app.settings?.antiAfk ? 'ps.opt.stOn' : 'ps.opt.stOff');
    return `<div class="ps-opts" role="group" aria-labelledby="ps-opts-title">
        <div class="ps-opts-head"><b id="ps-opts-title">${t('ps.opts')}</b></div>
        <div class="ps-opts-grid">
          <label>${t('ps.opt.afk')}<select class="select" id="ps-afk">${triOptions(p.antiAfk, t('ps.opt.followSet', { state: afkNow }))}</select></label>
          <label>${t('ps.opt.rejoin')}<select class="select" id="ps-rejoin">${triOptions(p.autoRejoin, t('ps.opt.followAcc'))}</select></label>
          <label>${t('ps.opt.profile')}<select class="select" id="ps-profile">${[['', 'ps.opt.normal'], ['afk', 'ps.opt.light']].map(([k, key]) =>
            `<option value="${k}" ${k === (p.profile || '') ? 'selected' : ''}>${t(key)}</option>`).join('')}</select></label>
          <label>${t('ps.opt.restart')}<select class="select" id="ps-restart">${RESTART_HOURS.map(h =>
            `<option value="${h}" ${h === (p.restartHours || 0) ? 'selected' : ''}>${h ? t('ps.opt.every', { n: h }) : t('ps.opt.never')}</option>`).join('')}</select></label>
          <label>${t('ps.opt.arrange')}<select class="select" id="ps-arrange">${triOptions(p.arrange, t('ps.opt.followSet', { state: t(S.app.multi?.enabled && S.app.multi?.tile ? 'ps.opt.stOn' : 'ps.opt.stOff') }))}</select></label>
        </div>
        <p class="ps-opts-note" id="ps-opts-note" hidden></p>
      </div>`;
  }
  const presetOptionsValue = el => {
    const tri = v => v === 'on' ? true : v === 'off' ? false : null;
    return {
      antiAfk: tri(el.querySelector('#ps-afk').value),
      autoRejoin: tri(el.querySelector('#ps-rejoin').value),
      profile: el.querySelector('#ps-profile').value,
      restartHours: +el.querySelector('#ps-restart').value || 0,
      arrange: tri(el.querySelector('#ps-arrange').value),
    };
  };

  /* A preset's own Discord channel: its schedule messages, its summary after a scheduled stop and the alerts of the
     windows it opened go there. The page never holds a saved link, only its masked form. */
  function presetDiscordHtml(p) {
    const on = !!S.app.discord?.enabled;
    return `<div class="ps-opts" role="group" aria-labelledby="ps-dc-title">
        <div class="ps-opts-head"><b id="ps-dc-title">Discord</b></div>
        <label class="ps-dc-mode">${t('ps.dc.where')}<select class="select" id="ps-dc-mode">
          <option value="" ${p.discord ? '' : 'selected'}>${t('ps.dc.main')}</option>
          <option value="own" ${p.discord ? 'selected' : ''}>${t('ps.dc.own')}</option></select></label>
        <div class="ps-dc-own" id="ps-dc-own" ${p.discord ? '' : 'hidden'}>
          <input class="field" id="ps-dc-link" type="text" maxlength="300" spellcheck="false" autocomplete="off"
            placeholder="${esc(p.discord || 'https://discord.com/api/webhooks/…')}" aria-label="${esc(t('ps.dc.link'))}">
          <button type="button" class="btn" id="ps-dc-test">${t('dc.test')}</button>
        </div>
        <p class="ps-opts-note">${p.discord ? `${t('ps.dc.saved')}\n` : ''}${t('ps.dc.note')}${on ? '' : `\n${t('ps.dc.off')}`}</p>
      </div>`;
  }
  /** What the form sends: undefined keeps the saved link, '' goes back to Settings' channel, a link replaces it. */
  function presetDiscordValue(el, p) {
    const own = el.querySelector('#ps-dc-mode').value === 'own', typed = el.querySelector('#ps-dc-link').value.trim();
    if (!own) return p?.discord ? '' : undefined;
    if (typed) return typed;
    if (p?.discord) return undefined;
    throw new Error(t('ps.dc.needLink'));
  }
  /** One short note under the options, only for a choice that needs explaining. */
  function syncPresetOptions(el) {
    const note = el.querySelector('#ps-opts-note');
    if (!note) return;
    const text = [el.querySelector('#ps-profile').value === 'afk' ? t('ps.opt.lightNote') : '',
      +el.querySelector('#ps-restart').value ? t(el.querySelector('#ps-place').value.trim() ? 'ps.opt.restartNote' : 'ps.opt.restartNoGame') : '']
      .filter(Boolean).join('\n'); // one line per option picked
    note.hidden = !text;
    if (note.textContent !== text) note.textContent = text;
  }
  /** The options a preset sets differently from the usual settings, for its row in the Presets tab. */
  const presetOptionsText = p => [
    p.antiAfk === true ? t('pv.optAfkOn') : p.antiAfk === false ? t('pv.optAfkOff') : '',
    p.autoRejoin === true ? t('pv.optRejoinOn') : p.autoRejoin === false ? t('pv.optRejoinOff') : '',
    p.profile === 'afk' ? t('pv.optLight') : '',
    p.restartHours > 0 && p.placeInput ? t('pv.optRestart', { n: p.restartHours }) : '',
    p.arrange === true ? t('pv.optArrangeOn') : p.arrange === false ? t('pv.optArrangeOff') : '',
    p.discord ? t('pv.optDiscord') : '',
  ].filter(Boolean).join(' · ');

  /** "Next: Today 20:00" for a scheduled preset (the app works out the time); empty otherwise. */
  function nextRunText(p) {
    if (!p.nextRun) return '';
    const d = new Date(p.nextRun), now = new Date();
    const time = fmtClock(`${d.getHours()}:${d.getMinutes()}`);
    const day = d.toDateString() === now.toDateString() ? t('ps.today')
      : d.toDateString() === new Date(now.getTime() + 864e5).toDateString() ? t('ps.tomorrow')
      : intl('d', { weekday: 'short', day: 'numeric', month: 'numeric' }).format(d);
    return t('ps.next', { when: `${day} ${time}` });
  }

  /** Accounts in the order Play opens them (from the app; new accounts last). */
  const launchOrder = () => S.app.settings?.launchOrder || [];
  const orderRank = () => new Map(launchOrder().map((id, i) => [id, i]));

  /** Opens above the dock button: the presets to run, then save / edit. */
  function presetMenu(btn) {
    const list = presets();
    menuAt(btn, [
      ...(list.length ? list.map(p => ({
        icon: p.nextRun ? 'clock' : 'play', text: esc(p.name), title: esc(presetSub(p)), hint: esc(nextRunText(p) || accCount(presetAccounts(p).length)),
        disabled: !presetAccounts(p).length, run: () => runPreset(p.id),
      })) : [{ icon: 'layers', text: t('ps.none'), disabled: true }]),
      '-',
      { icon: 'plus', text: t('ps.save'), run: newPreset },
      { icon: 'edit', text: t('ps.manage'), run: () => setView('presets') },
    ], 300);
    // The dock is at the bottom: open above the button, right edges lined up. A long list scrolls inside the menu;
    // Save and Edit stay at its foot.
    menuEl.classList.add('preset-menu');
    const foot = [...menuEl.children].findIndex(x => x.tagName === 'HR');
    const scroller = document.createElement('div');
    scroller.className = 'pm-list';
    // Moving the items drops the focus openMenu gave the first one (opened from the keyboard): it is given back.
    const focused = menuEl.contains(document.activeElement) ? document.activeElement : null;
    scroller.append(...[...menuEl.children].slice(0, foot));
    menuEl.prepend(scroller);
    focused?.focus();
    const r = btn.getBoundingClientRect();
    menuEl.style.maxHeight = `${Math.max(180, r.top - 14)}px`;
    const m = menuEl.getBoundingClientRect();
    menuEl.style.left = `${Math.max(8, r.right - m.width)}px`;
    menuEl.style.top = `${Math.max(8, r.top - m.height - 6)}px`;
  }

  async function runPreset(id, force = false) {
    // One run at a time from the page, as Play has: a double click would ask "Open anyway?" twice and run it twice.
    if (inFlight.has('preset')) return;
    inFlight.add('preset');
    let r;
    try {
      r = await call('runPreset', { id, force });
    } finally {
      inFlight.delete('preset');
    }
    return launchDone(r, () => runPreset(id, true));
  }

  async function deletePreset(p) {
    if (!await confirmBox(t('ps.deleteQ', { name: p.name }), { title: t('ps.deleteTitle'), ok: t('ps.delete'), danger: true })) return false;
    if (await call('deletePreset', { id: p.id }) === undefined) return false;
    toast(t('ps.deleted'), 'ok');
    return true;
  }

  /** New preset (prefilled from the dock: selected accounts, game box, Job ID box) or edit one. */
  function presetModal(p) {
    const isNew = !p;
    const dockJob = $('#job')?.value.trim() || '';
    const rank = orderRank();
    const base = p || {
      name: S.game?.ok && S.game.name ? S.game.name : '',
      // A new preset starts with the global launch order; the saved one keeps its own.
      accountIds: [...S.selected].sort((x, y) => (rank.get(x) ?? 1e9) - (rank.get(y) ?? 1e9)),
      placeInput: $('#place')?.value.trim() || '',
      server: dockJob ? 'job' : 'any',
      jobId: dockJob,
      delaySeconds: null,
      schedule: null,
      // Options for the windows it opens: null follows the usual settings.
      antiAfk: null, autoRejoin: null, profile: '', restartHours: 0, arrange: null, discord: null,
    };
    // The preset's accounts first, in its launch order; the rest in the global order.
    const chosen = base.accountIds.map(byId).filter(Boolean);
    // Accounts deleted since the preset was saved aren't in the list: said so, rather than dropped without a word.
    const gone = base.accountIds.length - chosen.length;
    const rest = accounts().filter(a => !chosen.includes(a)).sort((x, y) => (rank.get(x.id) ?? 1e9) - (rank.get(y.id) ?? 1e9));
    if (!chosen.length && !rest.length) return toast(t('rc.noAccounts'), 'info');
    const globalDelay = S.app.settings?.launchDelaySeconds ?? 8;
    const sc = base.schedule || {};
    const stopHours = sc.stopHours > 0 ? sc.stopHours : 2;
    // What the form holds (boxes, ticks, the accounts' order, the days): Esc or the X asks before throwing a change away.
    let formEl = null, initial = '';
    const snapshot = () => !formEl ? '' : JSON.stringify([
      ...[...formEl.querySelectorAll('.modal-body input, .modal-body select')].map(x => (x.type === 'checkbox' ? x.checked : x.value)),
      ...[...formEl.querySelectorAll('.ord-item')].map(x => x.dataset.id),
      ...[...formEl.querySelectorAll('#ps-sch-days .chip.on')].map(x => x.dataset.day),
    ]);
    modal({
      title: t(isNew ? 'ps.newTitle' : 'ps.editTitle'),
      dismissable: false,
      busy: true,
      enterSubmits: true,
      askClose: () => snapshot() !== initial,
      body: `<div class="form preset-form">
          <label>${t('ps.name')}<input class="field" id="ps-name" maxlength="60" placeholder="${esc(t('ps.namePh'))}" value="${esc(base.name)}" data-auto="${isNew ? esc(base.name) : ''}"></label>
          <label>${t('ps.game')}<input class="field" id="ps-place" maxlength="500" spellcheck="false" autocomplete="off" placeholder="${esc(t('dock.place'))}" value="${esc(base.placeInput)}"></label>
          <div class="ps-pick" id="ps-pick" hidden></div>
          <div class="ps-game" id="ps-game"></div>
          <div class="preset-row">
            <label>${t('ps.server')}<select class="select" id="ps-server">${PRESET_SERVERS.map(k =>
              `<option value="${k}" ${k === base.server ? 'selected' : ''}>${t(`ps.srv.${k}`)}</option>`).join('')}</select></label>
            <label>${t('ps.delay')}<input class="field" id="ps-delay" type="number" min="0" max="120" placeholder="${esc(t('ps.delayPh', { n: globalDelay }))}" value="${base.delaySeconds ?? ''}"></label>
          </div>
          <label id="ps-job-row">${t('ps.job')}<input class="field" id="ps-job" maxlength="60" spellcheck="false" placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx" value="${esc(base.jobId)}"></label>
          <div class="preset-accs">${t('ps.accounts')} <span class="muted" id="ps-count"></span><small>${t('ps.order')}</small></div>
          ${gone ? `<p class="ps-gone">${icon('info')}<span>${tn('ps.gone', gone)}</span></p>` : ''}
        </div>
        ${orderListHtml([...chosen, ...rest], { check: a => chosen.includes(a) })}
        <div class="form preset-form">
          ${presetOptionsHtml(base)}
          ${presetDiscordHtml(base)}
          <div class="ps-sched">
            <div class="ps-sched-head">
              <label class="switch"><input type="checkbox" id="ps-sch-on" ${sc.enabled ? 'checked' : ''} aria-label="${t('ps.sch')}"><span></span></label>
              <div><b>${t('ps.sch')}</b><small>${t('ps.schDesc')}</small></div>
            </div>
            <div class="ps-sched-body" id="ps-sch-body" hidden>
              <div class="preset-row">
                <label>${t('ps.schTime')}<input class="field" id="ps-sch-time" type="time" value="${esc(sc.time || '')}"></label>
                <label>${t('ps.schRepeat')}<select class="select" id="ps-sch-mode">
                  <option value="days" ${sc.date ? '' : 'selected'}>${t('ps.schDays')}</option>
                  <option value="once" ${sc.date ? 'selected' : ''}>${t('ps.schOnce')}</option></select></label>
              </div>
              <div class="day-chips" id="ps-sch-days" role="group" aria-label="${t('ps.schDays')}">${[1, 2, 3, 4, 5, 6, 0].map(d =>
                `<button type="button" class="chip ${(sc.days || []).includes(d) ? 'on' : ''}" data-day="${d}" aria-pressed="${(sc.days || []).includes(d)}">${esc(weekdayName(d))}</button>`).join('')}</div>
              <label id="ps-sch-date-row" hidden>${t('ps.schDate')}<input class="field" id="ps-sch-date" type="date" value="${esc(sc.date || '')}"></label>
              <div class="preset-row">
                <label>${t('ps.schStop')}<select class="select" id="ps-sch-stop">${['never', 'after', 'at'].map(k =>
                  `<option value="${k}" ${(sc.stop || 'never') === k ? 'selected' : ''}>${t(`ps.schStop.${k}`)}</option>`).join('')}</select></label>
                <label id="ps-sch-hours-row">${t('ps.schHours')}<input class="field" id="ps-sch-hours" type="number" min="0.5" max="168" step="0.5" value="${stopHours}"></label>
                <label id="ps-sch-stoptime-row" hidden>${t('ps.schStopTime')}<input class="field" id="ps-sch-stoptime" type="time" value="${esc(sc.stopTime || '')}"></label>
              </div>
              <details class="ps-note"><summary>${icon('info')}${t('ps.schNoteTitle')}</summary><p class="ps-sched-note">${t('ps.schNote')}</p></details>
            </div>
          </div>
        </div>`,
      actions: [
        ...(isNew ? [] : [{ text: t('ps.delete'), kind: 'danger push', onClick: async ({ close }) => { if (await deletePreset(p)) close(); } }]),
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('ps.saveBtn'), kind: 'primary', onClick: async ({ el, close, btn, hold }) => {
            if (btn.disabled) return;
            const val = sel => el.querySelector(sel).value.trim();
            const delay = val('#ps-delay');
            // A refused save is said in the form, above its buttons (a toast sits behind the dialog), and the box to fix
            // is brought into view and focused.
            const errBox = el.querySelector('.ps-err');
            errBox.hidden = true;
            const fail = (msg, box) => {
              errBox.textContent = msg;
              errBox.hidden = false;
              box?.scrollIntoView({ block: 'nearest' });
              box?.focus();
            };
            // Checked here rather than clamped quietly by the app: 1000 hours or a quarter hour would save as something else.
            const hours = +val('#ps-sch-hours');
            if (el.querySelector('#ps-sch-on').checked && val('#ps-sch-stop') === 'after' && !(hours >= 0.5 && hours <= 168 && Number.isInteger(hours * 2))) {
              return fail(t('ps.schHoursBad'), el.querySelector('#ps-sch-hours'));
            }
            // Its own Discord channel: chosen but no link pasted is said here, next to the box.
            let discordLink;
            try { discordLink = presetDiscordValue(el, p); } catch (e) { return fail(e.message, el.querySelector('#ps-dc-link')); }
            let refused = null;
            await busy(btn, () => hold(() => bridge.call('savePreset', {
              id: p?.id || '',
              name: val('#ps-name'),
              placeInput: val('#ps-place'),
              server: val('#ps-server'),
              jobId: val('#ps-job'),
              // Ticked order = launch order.
              ids: [...el.querySelectorAll('.ord-item input:checked')].map(b => b.value),
              delaySeconds: delay === '' ? null : Math.min(120, Math.max(0, Math.round(+delay) || 0)),
              ...presetOptionsValue(el),
              discordWebhook: discordLink,
              // A schedule never set up (off, no start time) is saved as none: no "Schedule off" row, no switch that
              // couldn't be turned on.
              schedule: !el.querySelector('#ps-sch-on').checked && !val('#ps-sch-time') ? null : {
                enabled: el.querySelector('#ps-sch-on').checked,
                time: val('#ps-sch-time'),
                days: val('#ps-sch-mode') === 'once' ? [] : [...el.querySelectorAll('#ps-sch-days .chip.on')].map(b => +b.dataset.day),
                date: val('#ps-sch-mode') === 'once' ? val('#ps-sch-date') : '',
                stop: val('#ps-sch-stop'),
                stopHours: +val('#ps-sch-hours') || 0,
                stopTime: val('#ps-sch-stoptime'),
              },
            })).catch(e => { refused = e; }));
            if (refused) {
              // The box the app's reason is most likely about, from what the form holds.
              const on = el.querySelector('#ps-sch-on').checked, sel = s => el.querySelector(s);
              const box = !val('#ps-name') ? sel('#ps-name')
                : !el.querySelector('.ord-item input:checked') ? sel('.ord-item input')
                : val('#ps-server') === 'job' && !val('#ps-job') ? sel('#ps-job')
                : on && !val('#ps-sch-time') ? sel('#ps-sch-time')
                : on && val('#ps-sch-mode') !== 'once' && !el.querySelector('#ps-sch-days .chip.on') ? sel('#ps-sch-days .chip')
                : on && val('#ps-sch-mode') === 'once' && !val('#ps-sch-date') ? sel('#ps-sch-date') : null;
              return fail(refused.message || t('common.unknownError'), box);
            }
            close();
            toast(t('ps.saved'), 'ok');
          },
        },
      ],
      onMount: ({ el }) => {
        const sync = () => {
          el.querySelector('#ps-job-row').hidden = el.querySelector('#ps-server').value !== 'job';
          el.querySelector('#ps-count').textContent = `(${el.querySelectorAll('.ord-item input:checked').length})`;
          el.querySelector('#ps-sch-body').hidden = !el.querySelector('#ps-sch-on').checked;
          const once = el.querySelector('#ps-sch-mode').value === 'once', stop = el.querySelector('#ps-sch-stop').value;
          el.querySelector('#ps-sch-days').hidden = once;
          el.querySelector('#ps-sch-date-row').hidden = !once;
          el.querySelector('#ps-sch-hours-row').hidden = stop !== 'after';
          el.querySelector('#ps-sch-stoptime-row').hidden = stop !== 'at';
          syncPresetOptions(el);
        };
        el.addEventListener('change', sync);
        el.querySelector('#ps-dc-mode').addEventListener('change', e => {
          el.querySelector('#ps-dc-own').hidden = e.target.value !== 'own';
          if (e.target.value === 'own' && !p?.discord) el.querySelector('#ps-dc-link').focus();
        });
        el.querySelector('#ps-dc-test').addEventListener('click', e => busy(e.currentTarget, async () => {
          if (await call('presetDiscordTest', { id: p?.id || '', url: el.querySelector('#ps-dc-link').value.trim() }) !== undefined) toast(t('dc.testOk'), 'ok');
        }));
        // The options' note depends on the game box too, which the game list also fills (no change event then).
        el.querySelector('#ps-place').addEventListener('input', () => syncPresetOptions(el));
        new MutationObserver(() => syncPresetOptions(el)).observe(el.querySelector('#ps-game'), { childList: true });
        el.querySelector('#ps-sch-days').addEventListener('click', e => {
          const chip = e.target.closest('[data-day]');
          if (!chip) return;
          chip.classList.toggle('on');
          chip.setAttribute('aria-pressed', chip.classList.contains('on'));
        });
        // A date picked for a one-off run: today by default, so the picker opens on the right month.
        if (!el.querySelector('#ps-sch-date').value) el.querySelector('#ps-sch-date').value = new Date(Date.now() - new Date().getTimezoneOffset() * 60000).toISOString().slice(0, 10);
        sync();
        wireOrderList(el.querySelector('.ord-list'));
        presetGameChooser(el);
        // Where a refused save says why (see Save).
        el.querySelector('.modal-foot').insertAdjacentHTML('beforebegin', '<div class="form-error ps-err" role="alert" hidden></div>');
        formEl = el;
        initial = snapshot();
        setTimeout(() => el.querySelector('#ps-name')?.focus(), 30);
      },
    });
  }

  /**
   * The preset form's game box works like the dock's: focusing it offers recent, pinned, favourite and popular games,
   * typing a name searches Roblox, and a link or Place ID can still be pasted. The chosen game shows below the box.
   */
  function presetGameChooser(form) {
    const place = form.querySelector('#ps-place'), pick = form.querySelector('#ps-pick'), card = form.querySelector('#ps-game');
    const G = { mode: 'browse', tab: 'recent', results: [], query: '', seq: 0, loading: false, index: -1 };
    let timer = 0;
    const rows = () => (G.mode === 'search' ? G.results : P.lists?.[G.tab]) || [];
    const showCard = g => setHtml(card, g?.name ? `${g.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : `<span class="gph">${icon('gamepad')}</span>`}<b>${esc(g.name)}</b>` : '');
    // The same keys as the dock's box: arrows walk the list, Enter picks, Esc closes the list (and only the list —
    // the form's own Esc stands back while aria-expanded says a list is open).
    place.setAttribute('role', 'combobox');
    place.setAttribute('aria-haspopup', 'listbox');
    place.setAttribute('aria-autocomplete', 'list');
    place.setAttribute('aria-expanded', 'false');
    const closePick = () => {
      pick.hidden = true;
      G.index = -1;
      place.setAttribute('aria-expanded', 'false');
      place.removeAttribute('aria-activedescendant');
    };
    const paint = () => {
      if (pick.hidden) return;
      const list = rows();
      place.setAttribute('aria-expanded', 'true');
      if (G.index >= 0) place.setAttribute('aria-activedescendant', `psg-${G.index}`);
      else place.removeAttribute('aria-activedescendant');
      const head = G.mode === 'browse'
        ? `<div class="picker-tabs">${TABS.filter(k => k !== 'popular' || P.lists?.popular?.length).map(k =>
            // Out of the Tab order: Tab leaves the box for the next field (the list closes when the box loses the focus).
            `<button type="button" class="ptab ${G.tab === k ? 'on' : ''}" data-gtab="${k}" tabindex="-1">${t(`picker.${k}`)}</button>`).join('')}</div>`
        : '';
      const body = G.loading ? '<div class="game-row skel"><span class="gph"></span><div class="gtxt"><b></b><small></small></div></div>'.repeat(3)
        : !list.length ? `<div class="picker-empty">${t(G.mode === 'search' ? 'picker.noResults' : `picker.empty.${G.tab}`)}</div>`
        : list.map((g, i) => `<div class="game-row ${i === G.index ? 'on' : ''}" role="option" id="psg-${i}" aria-selected="${i === G.index}" data-g="${i}">
            ${g.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : `<span class="gph">${icon('gamepad')}</span>`}
            <div class="gtxt"><b title="${esc(g.name)}">${esc(g.name)}</b><small>${g.playing ? t('dock.playing', { n: fmtNum(g.playing) }) : `Place ${g.placeId}`}</small></div></div>`).join('');
      setHtml(pick, `${head}<div class="ps-pick-list" role="listbox">${body}</div>`);
      pick.querySelector('.game-row.on')?.scrollIntoView({ block: 'nearest' });
    };
    const open = async mode => {
      G.mode = mode;
      G.index = -1;
      pick.hidden = false;
      paint();
      if (mode === 'browse' && !P.lists) {
        G.loading = true;
        paint();
        const r = await call('gameLists', {}, { silent: true });
        G.loading = false;
        if (r) {
          P.lists = r; // shared with the dock's picker
          if (!r[G.tab]?.length) G.tab = TABS.find(k => r[k]?.length) || 'recent';
        }
        paint();
      }
    };
    const choose = g => {
      place.value = `https://www.roblox.com/games/${g.placeId}`;
      closePick();
      showCard(g);
      // A name that is empty or still the one filled in automatically follows the game; a typed one stays.
      const name = form.querySelector('#ps-name');
      if (!name.value.trim() || name.value === name.dataset.auto) {
        name.value = g.name;
        name.dataset.auto = g.name;
      }
    };
    // A link or Place ID typed or pasted: show which game it is.
    const resolve = async () => {
      const value = place.value.trim();
      if (!value || !looksLikeTarget(value)) return showCard(null);
      const r = await call('resolveGame', { input: value }, { silent: true });
      if (place.value.trim() === value) showCard(r?.ok ? r : null);
    };
    place.addEventListener('focus', () => { if (!place.value.trim()) open('browse'); });
    place.addEventListener('input', () => {
      const value = place.value.trim();
      clearTimeout(timer);
      ++G.seq; // a search still on its way answers an older box
      // A search that was still pending is over; its placeholders must not stay on.
      G.loading = false;
      if (!value) {
        showCard(null);
        return open('browse');
      }
      if (looksLikeTarget(value)) {
        closePick();
        timer = setTimeout(resolve, 400);
        return;
      }
      showCard(null);
      const cached = searchCache.get(value.toLowerCase());
      if (cached && Date.now() - cached.at < SEARCH_TTL) {
        Object.assign(G, { results: cached.rows, loading: false, query: value });
        return open('search');
      }
      Object.assign(G, { loading: true, query: value });
      open('search');
      timer = setTimeout(async () => {
        const seq = G.seq;
        const r = await call('searchGames', { query: value }, { silent: true });
        if (seq !== G.seq || !form.isConnected) return;
        G.results = r || [];
        G.loading = false;
        if (r) rememberSearch(value.toLowerCase(), G.results);
        paint();
      }, 200);
    });
    pick.addEventListener('mousedown', e => e.preventDefault()); // keep the box focused while choosing
    pick.addEventListener('click', e => {
      const tab = e.target.closest('[data-gtab]');
      if (tab) {
        G.tab = tab.dataset.gtab;
        G.index = -1;
        return paint();
      }
      const row = e.target.closest('[data-g]');
      if (row) choose(rows()[+row.dataset.g]);
    });
    place.addEventListener('keydown', e => {
      const list = rows();
      if (pick.hidden) return;
      if ((e.key === 'ArrowDown' || e.key === 'ArrowUp') && list.length) {
        e.preventDefault();
        G.index = (G.index + (e.key === 'ArrowDown' ? 1 : list.length - 1) + (G.index < 0 && e.key === 'ArrowUp' ? 1 : 0)) % list.length;
        paint();
      } else if (e.key === 'Enter') {
        e.preventDefault();
        if (G.loading) return; // results for what was just typed aren't in yet
        if (list.length && (G.index >= 0 || G.mode === 'search')) choose(list[Math.max(0, G.index)]);
      } else if (e.key === 'Escape') {
        closePick();
      }
    });
    place.addEventListener('blur', () => setTimeout(() => { if (!pick.hidden) closePick(); }, 120));
    resolve();
  }

  /** Short weekday names from the browser's own calendar (0 = Sunday, like the app). 2024-01-07 was a Sunday. */
  const weekdayName = d => intl('d', { weekday: 'short' }).format(new Date(2024, 0, 7 + d));

  /* ───────────────────────── orderable account list ─────────────────────────
     Rows are dragged by the handle into another order, or moved with the arrow buttons (keyboard); the launch-order
     dialog numbers them, the preset form ticks them instead. */
  function orderListHtml(list, { check = null } = {}) {
    return `<div class="ord-list" role="list">${list.map((a, i) => `<div class="ord-item" role="listitem" draggable="true" data-id="${a.id}">
        <span class="ord-grip" title="${t('lo.drag')}">${icon('grip')}</span>
        ${check ? `<input type="checkbox" value="${a.id}" ${check(a) ? 'checked' : ''} aria-label="${esc(a.label)}">` : `<span class="ord-n">${i + 1}</span>`}
        <div class="avatar sm">${avatarInner(a)}</div>
        <span class="ord-txt"><b title="${esc(a.label)}">${esc(a.label)}</b><small>${atName(a)}</small></span>
        <span class="ord-btns"><button type="button" class="icon-btn" data-move="-1" title="${t('lo.up')}" aria-label="${t('lo.up')}">${icon('chevronUp')}</button><button type="button" class="icon-btn" data-move="1" title="${t('lo.down')}" aria-label="${t('lo.down')}">${icon('chevronDown')}</button></span>
      </div>`).join('')}</div>`;
  }

  function wireOrderList(list) {
    if (!list) return;
    const renumber = () => list.querySelectorAll('.ord-n').forEach((n, i) => { n.textContent = i + 1; });
    let dragging = null;
    list.addEventListener('dragstart', e => {
      dragging = e.target.closest('.ord-item');
      if (!dragging) return;
      dragging.classList.add('dragging');
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', dragging.dataset.id); // Chromium needs some data for the drag to start
    });
    list.addEventListener('dragover', e => {
      if (!dragging) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      const over = e.target.closest('.ord-item');
      if (!over || over === dragging) return;
      const r = over.getBoundingClientRect();
      over.parentNode.insertBefore(dragging, e.clientY < r.top + r.height / 2 ? over : over.nextSibling);
    });
    list.addEventListener('drop', e => e.preventDefault());
    list.addEventListener('dragend', () => {
      dragging?.classList.remove('dragging');
      dragging = null;
      renumber();
    });
    list.addEventListener('click', e => {
      const btn = e.target.closest('[data-move]');
      const row = e.target.closest('.ord-item');
      if (!row) return;
      if (btn) {
        const to = +btn.dataset.move < 0 ? row.previousElementSibling : row.nextElementSibling?.nextElementSibling;
        if (+btn.dataset.move < 0 ? !row.previousElementSibling : !row.nextElementSibling) return;
        list.insertBefore(row, to);
        renumber();
        btn.focus();
        row.scrollIntoView({ block: 'nearest' });
        return;
      }
      // The row itself ticks its box (the box, the handle and the buttons do their own thing).
      const box = row.querySelector('input[type=checkbox]');
      if (box && !e.target.closest('input, .ord-grip, button')) {
        box.checked = !box.checked;
        box.dispatchEvent(new Event('change', { bubbles: true }));
      }
    });
  }

  /** The ⋯ menu's "Launch order…": every account, dragged or moved into the order Play opens them in. */
  function launchOrderModal() {
    const rank = orderRank();
    const list = accounts().slice().sort((x, y) => (rank.get(x.id) ?? 1e9) - (rank.get(y.id) ?? 1e9));
    if (!list.length) return toast(t('rc.noAccounts'), 'info');
    modal({
      title: t('lo.title'),
      dismissable: false,
      enterSubmits: true,
      body: `<p class="modal-text">${t('lo.text')}</p>${orderListHtml(list)}`,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            if (btn.disabled) return;
            const ids = [...el.querySelectorAll('.ord-item')].map(r => r.dataset.id);
            if (await busy(btn, () => call('setLaunchOrder', { ids })) === undefined) return;
            close();
            toast(t('lo.saved'), 'ok');
          },
        },
      ],
      onMount: ({ el }) => wireOrderList(el.querySelector('.ord-list')),
    });
  }

  /* ───────────────────────── presets tab ─────────────────────────
     Every preset as a row: run, edit, duplicate, delete, its schedule on or off, and its place in the list (the dock's
     Presets menu lists them in the same order). A row moves by dragging its handle or with its arrows (keyboard);
     while a search narrows the list there is no whole order to change, so both are hidden. */
  const PV = { query: '', iconsAt: 0, dragging: false };
  const MAX_PRESETS = 50; // the app's own limit (MaxPresets); checked here so a full list doesn't open an empty form
  const AVATARS_SHOWN = 4;
  function presetsViewHtml() {
    return `<header class="top"><div class="top-title"><h1>${t('ps.listTitle')}</h1><p id="pv-sub"></p></div>
      <div class="top-tools" id="pv-tools">
        <label class="search">${icon('search')}<input id="pv-q" placeholder="${t('pv.search')}" maxlength="100" aria-label="${t('pv.search')}"></label>
        <button class="btn primary" data-act="pv-new">${icon('plus')}<span>${t('ps.new')}</span></button>
      </div></header>
      <div class="scroll"><div class="pv-list" id="pv-list" role="list"></div></div>`;
  }

  /** "Mon, Wed, Fri 20:00", "Every day 20:00", or the date of a one-off. */
  function scheduleText(sc) {
    const time = fmtClock(sc.time);
    if (sc.date) {
      const [y, m, d] = sc.date.split('-').map(Number);
      return `${intl('d', { weekday: 'short', day: 'numeric', month: 'numeric' }).format(new Date(y, m - 1, d))} ${time}`.trim();
    }
    // Monday first, as the form shows them; three or more days in a row read as a range ("Mon–Fri").
    const order = [1, 2, 3, 4, 5, 6, 0], days = order.filter(d => (sc.days || []).includes(d));
    const inRow = days.length >= 3 && order.indexOf(days.at(-1)) - order.indexOf(days[0]) === days.length - 1;
    const text = days.length === 7 ? t('pv.everyDay') : inRow ? `${weekdayName(days[0])}–${weekdayName(days.at(-1))}` : days.map(weekdayName).join(', ');
    return `${text} ${time}`.trim();
  }

  function presetRowHtml(p, i, count, ordered) {
    const accs = presetAccounts(p);
    const game = p.placeInput ? p.gameName || p.placeInput : t('ps.noGame');
    const sc = p.schedule;
    const delay = p.delaySeconds ?? null;
    // What it does, in one plain line: server · wait · schedule · options set differently from Settings.
    const summary = [
      p.placeInput ? t(`ps.srv.${p.server}`) : '',
      delay === null ? t('pv.delayDefault', { n: S.app.settings?.launchDelaySeconds ?? 8 }) : delay === 0 ? t('pv.delay0') : t('pv.delayN', { n: delay }),
      !sc?.time ? t('pv.noSchedule') : !sc.enabled ? `${t('pv.schOff')} · ${scheduleText(sc)}` : scheduleText(sc),
      presetOptionsText(p),
    ].filter(Boolean).join(' · ');
    // Then, quieter: when it runs next (a one-off's date is already above) and when it ran last (kept current by patchTimes).
    const next = [
      p.running ? `<span class="pv-live">${esc(tn('pv.running', p.running))}</span>` : '',
      sc?.enabled && p.nextRun && !sc.date ? esc(nextRunText(p)) : '',
      p.lastRun ? `<span data-ago="${esc(p.lastRun)}" data-tpl="pv.lastRun">${esc(t('pv.lastRun', { t: ago(p.lastRun) }))}</span>` : '',
    ].filter(Boolean).join(' · ');
    const accTitle = [accCount(accs.length), ...accs.map(a => a.label)].join('\n');
    const avatars = accs.length
      ? `<div class="pv-accs" title="${esc(accTitle)}" aria-label="${esc(accCount(accs.length))}">${accs.slice(0, AVATARS_SHOWN).map(a => `<span class="avatar sm">${avatarInner(a)}</span>`).join('')}${accs.length > AVATARS_SHOWN ? `<em>+${accs.length - AVATARS_SHOWN}</em>` : ''}</div>`
      : `<div class="pv-accs none" title="${esc(t('pv.noAccountsTip'))}">${icon('alert')}<small>${t('pv.noAccounts')}</small></div>`;
    const name = esc(p.name);
    return `<div class="pv-item" role="listitem" data-id="${esc(p.id)}">
        ${ordered ? `<span class="ord-grip pv-grip" title="${t('lo.drag')}">${icon('grip')}</span>` : ''}
        ${p.iconUrl ? `<img class="pv-icon" src="${esc(p.iconUrl)}" alt="" draggable="false" loading="lazy">` : `<span class="pv-icon">${icon(p.placeInput ? 'gamepad' : 'monitor')}</span>`}
        <div class="pv-main">
          <b title="${name}">${name}</b>
          <small title="${esc(game)}">${esc(game)}</small>
          <span class="pv-sum" title="${esc(summary)}">${esc(summary)}</span>
          <span class="pv-next">${next}</span>
        </div>
        ${avatars}
        <div class="pv-sw">${sc?.time ? `<small aria-hidden="true">${t('ps.sch')}</small><label class="switch" title="${t(sc.enabled ? 'pv.schTurnOff' : 'pv.schTurnOn')}"><input type="checkbox" data-pv="sched" ${sc.enabled ? 'checked' : ''} aria-label="${t('ps.sch')}: ${name}"><span></span></label>` : ''}</div>
        ${p.running ? `<button class="btn sm pv-run pv-close" data-pv="close" title="${esc(tn('pv.runningTip', p.running))}" aria-label="${t('pv.close')}: ${name}">${icon('power')}<span>${t('pv.close')}</span></button>`
          : `<button class="btn sm pv-run" data-pv="run" aria-label="${t('ps.run')}: ${name}" ${accs.length && !S.app.launching ? '' : 'disabled'}>${icon('play')}<span>${t('ps.run')}</span></button>`}
        <span class="pv-acts">
          <button class="icon-btn" data-pv="edit" title="${t('ps.edit')}" aria-label="${t('ps.edit')}: ${name}">${icon('edit')}</button>
          <button class="icon-btn" data-pv="dup" title="${t('pv.duplicate')}" aria-label="${t('pv.duplicate')}: ${name}">${icon('copy')}</button>
          <button class="icon-btn" data-pv="del" title="${t('ps.delete')}" aria-label="${t('ps.delete')}: ${name}">${icon('trash')}</button>
        </span>
        ${ordered ? `<span class="ord-btns"><button class="icon-btn" data-move="-1" title="${t('lo.up')}" aria-label="${t('lo.up')}: ${name}" ${i > 0 ? '' : 'disabled'}>${icon('chevronUp')}</button><button class="icon-btn" data-move="1" title="${t('lo.down')}" aria-label="${t('lo.down')}: ${name}" ${i < count - 1 ? '' : 'disabled'}>${icon('chevronDown')}</button></span>` : ''}
      </div>`;
  }

  /** @param opened the tab was just switched to: icons not known yet are asked for (at most every few minutes). */
  function renderPresetsView(opened = false) {
    const host = $('#pv-list');
    if (!host || !S.app?.settings || PV.dragging) return;
    const all = presets();
    // Accents folded on both sides, as the accounts search does ("man hinh" finds "Màn hình").
    const q = fold(PV.query.trim());
    const shown = q ? all.filter(p => fold(`${p.name}\n${p.placeInput ? p.gameName || p.placeInput : t('ps.noGame')}`).includes(q)) : all;
    const scheduled = all.filter(p => p.schedule?.enabled).length;
    setHtml($('#pv-sub'), all.length ? [t(all.length === 1 ? 'pv.count1' : 'pv.countN', { n: all.length }),
      scheduled ? t('pv.scheduled', { n: scheduled }) : '', all.length >= MAX_PRESETS ? t('pv.atMax') : '']
      .filter(Boolean).join('<span class="sep">·</span>') : ''); // counts only, like the other tabs; the empty state says what a preset is
    $('#pv-tools').hidden = !all.length; // the empty state has its own New button
    setHtml(host, !all.length
      ? `<div class="empty"><span class="pv-empty-icon">${icon('layers')}</span><h2>${t('pv.emptyTitle')}</h2><p>${t('pv.emptyText')}</p>
          <div class="row"><button class="btn primary" data-act="pv-new">${icon('plus')}<span>${t('ps.new')}</span></button></div></div>`
      : !shown.length ? `<div class="empty"><p>${t('pv.noMatch', { q: esc(PV.query.trim()) })}</p></div>`
      : shown.map((p, i) => presetRowHtml(p, i, shown.length, !q && all.length > 1)).join(''));
    if (opened && S.view === 'presets' && all.some(p => p.placeInput && !p.iconUrl) && Date.now() - PV.iconsAt > 5 * 60_000) {
      PV.iconsAt = Date.now();
      call('presetIcons', {}, { silent: true });
    }
  }

  function newPreset() {
    if (presets().length >= MAX_PRESETS) return toast(t('pv.full', { n: MAX_PRESETS }), 'warn');
    presetModal(null);
  }

  // A double click makes one copy: the row is drawn again before the second click lands, on an enabled button.
  const dupLast = new Map();
  async function duplicatePreset(p, btn) {
    if (Date.now() - (dupLast.get(p.id) || 0) < 600) return;
    dupLast.set(p.id, Date.now());
    const id = await busy(btn, () => call('duplicatePreset', { id: p.id }));
    dupLast.set(p.id, Date.now());
    if (id === undefined) return;
    toast(t(p.schedule?.enabled ? 'pv.duplicatedOff' : 'pv.duplicated'), 'ok');
    // The copy is drawn right under the original (or its other copies); the focus stays on the original's Duplicate.
    presetRow(id)?.scrollIntoView({ block: 'nearest' });
    if (!document.activeElement || document.activeElement === document.body) presetRow(p.id)?.querySelector('[data-pv="dup"]')?.focus();
  }

  const presetRow = id => [...document.querySelectorAll('#pv-list .pv-item')].find(r => r.dataset.id === id);

  async function movePreset(id, dir) {
    const ids = presets().map(p => p.id);
    const i = ids.indexOf(id), j = i + dir;
    if (i < 0 || j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    if (await call('setPresetOrder', { ids }) === undefined) return;
    // The list was drawn again: the focus stays on the moved row's arrow; once the row reaches an end (that arrow is
    // off) on the row itself, not on the other arrow, where the next Enter would move it straight back.
    const row = [...document.querySelectorAll('#pv-list .pv-item')].find(r => r.dataset.id === id);
    const same = row?.querySelector(`[data-move="${dir}"]:not(:disabled)`);
    if (same) same.focus();
    else if (row) { row.tabIndex = -1; row.focus(); }
    row?.scrollIntoView({ block: 'nearest' });
  }

  function wirePresetsView() {
    const host = $('#pv-list'), q = $('#pv-q');
    q.value = PV.query;
    let timer = 0;
    q.addEventListener('input', () => {
      PV.query = q.value;
      clearTimeout(timer);
      timer = setTimeout(() => renderPresetsView(), 120);
    });
    const presetOf = el => presets().find(p => p.id === el.closest('.pv-item')?.dataset.id);
    host.addEventListener('click', e => {
      const b = e.target.closest('[data-pv], [data-move]');
      const p = b && !b.disabled && b.tagName === 'BUTTON' ? presetOf(b) : null;
      if (!p) return;
      if (b.dataset.move) return movePreset(p.id, +b.dataset.move);
      switch (b.dataset.pv) {
        case 'run': return busy(b, () => runPreset(p.id));
        case 'close': return busy(b, async () => {
          const n = await call('closePreset', { id: p.id });
          if (n !== undefined) toast(n ? tn('t.closing', n) : t('t.noneRunning'), n ? 'ok' : 'info');
        });
        case 'edit': return presetModal(p);
        case 'dup': return duplicatePreset(p, b);
        case 'del': return deletePreset(p);
      }
    });
    host.addEventListener('change', async e => {
      const box = e.target.closest('[data-pv="sched"]');
      const p = box && presetOf(box);
      if (!p) return;
      const on = box.checked;
      box.disabled = true;
      const r = await call('setPresetSchedule', { id: p.id, enabled: on });
      box.disabled = false;
      // A disabled box lost the focus, and the row may have been drawn again meanwhile: the focus goes back to it.
      if (document.activeElement === document.body || !document.activeElement) presetRow(p.id)?.querySelector('[data-pv="sched"]')?.focus();
      // Refused (a one-off whose date has passed): the switch goes back; the reason is in the toast.
      if (r === undefined) box.checked = !on;
      else toast(t(on ? 't.schOn' : 't.schOff', { name: p.name }), 'ok');
    });
    // Only the handle starts a drag, so the row's buttons and switch work as usual.
    let dragging = null;
    host.addEventListener('pointerdown', e => {
      const row = e.target.closest('.pv-grip') && e.target.closest('.pv-item');
      if (row) row.draggable = true;
    });
    host.addEventListener('pointerup', () => host.querySelectorAll('.pv-item[draggable="true"]').forEach(r => { r.draggable = false; }));
    host.addEventListener('dragstart', e => {
      dragging = e.target.closest?.('.pv-item[draggable="true"]') || null;
      if (!dragging) return;
      PV.dragging = true; // a state push meanwhile must not redraw the rows under the pointer
      dragging.classList.add('dragging');
      e.dataTransfer.effectAllowed = 'move';
      e.dataTransfer.setData('text/plain', dragging.dataset.id); // Chromium needs some data for the drag to start
    });
    host.addEventListener('dragover', e => {
      if (!dragging) return;
      e.preventDefault();
      e.dataTransfer.dropEffect = 'move';
      const over = e.target.closest('.pv-item');
      if (!over || over === dragging) return;
      const r = over.getBoundingClientRect();
      host.insertBefore(dragging, e.clientY < r.top + r.height / 2 ? over : over.nextSibling);
    });
    host.addEventListener('drop', e => e.preventDefault());
    host.addEventListener('dragend', async e => {
      if (!dragging) return;
      // Esc (or a drop outside the list) cancels the drag: the rows go back, nothing is saved.
      const cancelled = e.dataTransfer?.dropEffect === 'none';
      dragging.classList.remove('dragging');
      dragging.draggable = false;
      dragging = null;
      PV.dragging = false;
      const ids = [...host.querySelectorAll('.pv-item')].map(r => r.dataset.id);
      host._html = null; // the rows were moved by hand: drawn again from the app's order (the old one if it refused)
      if (!cancelled && ids.join() !== presets().map(p => p.id).join()) await call('setPresetOrder', { ids });
      renderPresetsView();
    });
  }

  /** The sign-in window can add several accounts in a row; select whatever was added. */
  async function addBrowser(btn) {
    const r = await busy(btn, () => call('addViaBrowser'));
    const added = r?.added || [];
    if (!added.length) return;
    toast(added.length === 1 ? t(added[0].merged ? 't.merged' : 't.added', { name: added[0].label }) : t('t.addedMany', { n: added.length }), 'ok');
    S.selected = new Set(added.map(a => a.id));
    S.anchor = added[added.length - 1].id;
    renderSelection();
  }

  /* ───────────────────────── adding accounts ───────────────────────── */
  /**
   * The ways to add an account, each with one line on what it means. The phone comes first: Roblox asks it for no
   * password and no puzzle, while a password sign-in may meet checks only Roblox's own site can show.
   */
  const phoneOption = (act = 'data-act="add-phone"') => `<button type="button" class="start-opt rec" ${act}>${icon('qr')}<span><b>${t('start.phone')}<em class="rec-tag">${t('start.rec')}</em></b><small>${t('start.phoneSub')}</small></span>${icon('chevronRight')}</button>`;
  const addOptions = () => `<div class="start-opts">
      ${phoneOption()}
      <button class="start-opt" data-act="add-password">${icon('key')}<span><b>${t('start.pw')}</b><small>${t('start.pwSub')}</small></span>${icon('chevronRight')}</button>
      <button class="start-opt" data-act="add-browser">${icon('globe')}<span><b>${t('start.browser')}</b><small>${t('start.browserSub')}</small></span>${icon('chevronRight')}</button>
    </div>
    <div class="start-more">
      <div class="ways"><span>${t('start.more')}</span><button class="btn sm ghost" data-act="add-cookie">${t('start.cookie')}</button></div>
      <div class="ways"><span>${t('start.importFrom')}</span><button class="btn sm ghost" data-act="import-vault" title="${t('start.vaultTip')}">${t('start.vault')}</button></div>
    </div>`;

  /** The sign-in screen lists every way to add an account. */
  const addChooser = () => passwordLogin();
  /** Picking a way closes the chooser it was picked from. */
  const leaveChooser = el => el.closest('.overlay')?.querySelector('[data-x]')?.click();

  /**
   * The sign-in screen: username + password, with the other ways (phone, browser) under it. Works on a PC with no
   * phone and no roblox.com: the values go only to Roblox's/VNG's official auth host, over HTTPS. Handles a 2-step code
   * and a captcha (shown in its own window, for the user to solve).
   */
  const METHOD_LABEL = { authenticator: 'pw.auth', email: 'pw.email', sms: 'pw.sms' };

  function passwordLogin(forRealm) {
    const added = [];
    let realm = forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : '');
    let step = 'login', method = 'authenticator', user = '';
    // The 2-step methods this account has that take a typed code (the app asks Roblox), the preferred one first.
    let methods = ['authenticator', 'email'];
    // Email/SMS ways a code has gone out by (Roblox sends one by the preferred way itself), and per way when "Resend"
    // works again. Both belong to one code screen: a new one (another account) starts empty.
    const sent = new Set();
    const resendAt = {};
    let resendTimer = 0, sending = false;
    const RESEND_WAIT = 30_000;
    // A sign-in request on its way: every button that would send another stays off until the answer, also after a
    // re-render (the realm tabs), so one click never sends the password twice.
    let pending = false;
    // Roblox said "too many tries" to a code: Verify (and Send) wait until then.
    let verifyAt = 0, verifyTimer = 0;
    // The account's authenticator key is saved here (the app said so): its code is filled in for the person, who still
    // presses Verify. Never sent by itself, and never another code after Roblox refused one. A code still untouched in
    // the box follows it when it changes.
    let autofill = false, filled = '', fillTimer = 0, fillSeq = 0;
    async function fillCode() {
      clearTimeout(fillTimer);
      const seq = ++fillSeq;
      const box = () => m.el.querySelector('#pw-otp');
      const untouched = () => box() && (!box().value || box().value === filled);
      if (!autofill || method !== 'authenticator' || !untouched()) return;
      let r;
      try { r = await bridge.call('loginTwoStepCode'); } catch { return; }
      if (seq !== fillSeq || !r?.code || method !== 'authenticator' || !untouched()) return;
      box().value = filled = r.code;
      const note = m.el.querySelector('#pw-filled');
      if (note) note.hidden = false;
      fillTimer = setTimeout(fillCode, Math.max(1, r.seconds) * 1000 + 300);
    }
    const m = modal({
      title: '',
      busy: true,
      dismissable: false, // a stray click beside it mustn't end a sign-in half way
      // Esc or the X asks first when that would throw away what was typed, or a sign-in half way (a code already sent).
      askClose: () => pending || step !== 'login'
        || !!(m.el.querySelector('#pw-user')?.value.trim() || m.el.querySelector('#pw-pass')?.value),
      body: `<div id="pw-body"></div>`,
      onClose: () => {
        clearTimeout(resendTimer);
        clearTimeout(verifyTimer);
        clearTimeout(fillTimer);
        fillSeq++;
        call('loginCancel', {}, { silent: true }); // the app drops the half-done sign-in and its password
        if (!added.length) return;
        S.selected = new Set(added.map(a => a.id));
        S.anchor = added[added.length - 1].id;
        renderSelection();
      },
    });
    m.el.querySelector('.modal').classList.add('auth');
    m.el.querySelector('.modal').setAttribute('aria-label', t('pw.title'));
    const body = () => m.el.querySelector('#pw-body');
    const err = msg => { const e = m.el.querySelector('#pw-err'); if (e) { e.textContent = msg; e.hidden = !msg; } };

    const realmTabs = () => `<div class="seg wide" role="tablist">
        <button type="button" role="tab" data-realm="" class="${realm === '' ? 'on' : ''}" aria-selected="${realm === ''}">Roblox</button>
        <button type="button" role="tab" data-realm="vng" class="${realm === 'vng' ? 'on' : ''}" aria-selected="${realm === 'vng'}">Roblox VNG</button>
      </div>`;
    const back = `<button type="button" class="auth-back" data-back>${t('pw.back')}</button>`;

    function render() {
      if (step === 'captcha') {
        body().innerHTML = `<div class="auth-step">
            <span class="auth-badge">${icon('shield')}</span>
            <h4>${t('pw.captchaTitle')}</h4>
            <p>${t('pw.captcha')}</p>
          </div>
          <div class="auth-acts">
            <div class="form-error" id="pw-err" hidden></div>
            <button type="button" class="btn go w" id="pw-solve">${t('pw.solve')}</button>
            ${back}
          </div>`;
      } else if (step === 'finish') {
        // Roblox handed out the sign-in but the account couldn't be looked up: only that is tried again.
        body().innerHTML = `<div class="auth-step">
            <span class="auth-badge">${icon('shieldCheck')}</span>
            <h4>${t('pw.finishTitle')}</h4>
            <p>${t('pw.finish')}</p>
          </div>
          <div class="auth-acts">
            <div class="form-error" id="pw-err" hidden></div>
            <button type="button" class="btn go w" id="pw-finish">${t('pw.retry')}</button>
            ${back}
          </div>`;
      } else if (step === 'twostep') {
        body().innerHTML = `<div class="auth-step">
            <span class="auth-badge">${icon('shieldCheck')}</span>
            <h4>${t('pw.twoTitle')}</h4>
            <p>${t('pw.twoLead', { name: `<b>${esc(user)}</b>` })}</p>
          </div>
          ${methods.length > 1 ? `<div class="seg wide" role="tablist">
            ${methods.map(x => `<button type="button" role="tab" data-m="${x}" class="${method === x ? 'on' : ''}" aria-selected="${method === x}">${t(METHOD_LABEL[x])}</button>`).join('')}
          </div>` : ''}
          <form id="pw-two" class="auth-form" autocomplete="off">
            <input class="field auth-code" id="pw-otp" inputmode="numeric" maxlength="40" autocomplete="one-time-code" placeholder="000000" aria-label="${t('pw.code')}">
            <p class="auth-hint" id="pw-filled" hidden>${t('pw.filled')}</p>
            <div class="auth-sendrow" id="pw-sendrow" ${method === 'authenticator' ? 'hidden' : ''}>
              <span class="auth-sentnote" id="pw-sentnote"></span>
              <button type="button" class="link auth-send" id="pw-send"></button>
            </div>
            <div class="form-error" id="pw-err" hidden></div>
            <button class="btn go w" type="submit" id="pw-verify">${t('pw.verify')}</button>
            ${back}
          </form>`;
      } else {
        // The phone comes first (no password, no puzzle); the browser sign-in is roblox.com only, so it isn't offered
        // for Roblox VNG.
        body().innerHTML = `<div class="auth-head"><img src="${LOGO}" alt=""><h3>${t('pw.title')}</h3></div>
          ${realmTabs()}
          ${added.length ? `<div class="auth-added">${icon('check')}<span>${t('pw.addedList', { names: esc(added.map(x => x.label).join(', ')) })}</span></div>` : ''}
          <div class="auth-phone">${phoneOption('data-x2="phone"')}</div>
          <div class="auth-or"><span>${t('pw.orPw')}</span></div>
          <form id="pw-login" class="auth-form tight" autocomplete="off">
            <label class="auth-label">${t('pw.user')}
              <span class="lead-ic">${icon('user')}<input class="field" id="pw-user" maxlength="60" autocomplete="off" spellcheck="false" value="${esc(user)}"></span></label>
            <label class="auth-label">${t('pw.pass')}
              <span class="pw lead-ic">${icon('lock')}<input class="field" type="password" id="pw-pass" maxlength="200" autocomplete="off">
              <button type="button" class="icon-btn" id="pw-eye" title="${t('lock.showPw')}" aria-label="${t('lock.showPw')}" aria-pressed="false">${icon('eye')}</button></span></label>
            <div class="form-error" id="pw-err" hidden></div>
            <button class="btn w" type="submit" id="pw-go">${t('pw.go')}</button>
            <p class="auth-hint">${t('pw.extra')}</p>
          </form>
          <div class="auth-more">
            <div class="ways"><span>${t('start.more')}</span>${realm === 'vng' ? '' : `<button type="button" class="btn sm ghost" data-x2="browser">${t('pw.withBrowser')}</button>`}<button type="button" class="btn sm ghost" data-x2="cookie">${t('pw.cookie')}</button></div>
            <div class="ways"><span>${t('start.importFrom')}</span><button type="button" class="btn sm ghost" data-x2="vault" title="${t('start.vaultTip')}">${t('pw.vault')}</button></div>
          </div>
          <p class="auth-note">${icon('lock')}<span>${t('pw.note')}</span></p>`;
      }
      wire();
    }

    function wire() {
      m.el.querySelectorAll('[data-realm]').forEach(b => b.onclick = () => {
        if (pending || b.dataset.realm === realm) return; // the answer on its way is for the realm it was sent to
        realm = b.dataset.realm;
        store.set('rm.qlRealm', realm);
        // Switching between Roblox and Roblox VNG keeps what was typed.
        user = m.el.querySelector('#pw-user')?.value ?? user;
        const pass = m.el.querySelector('#pw-pass')?.value || '';
        render();
        const box = m.el.querySelector('#pw-pass');
        if (box) box.value = pass;
      });
      // Switching where the code comes from keeps what was typed. Email and SMS codes go out on their own, as on
      // roblox.com: Roblox already sent one by the preferred way; another way gets one sent when it's picked.
      m.el.querySelectorAll('[data-m]').forEach(b => b.onclick = () => {
        method = b.dataset.m;
        m.el.querySelectorAll('[data-m]').forEach(x => { x.classList.toggle('on', x === b); x.setAttribute('aria-selected', String(x === b)); });
        syncSend();
        if (method !== 'authenticator' && !sent.has(method)) sendCode();
        // A filled-in authenticator code isn't an email or SMS code: it goes with another way, and comes back with its own.
        const otpBox = m.el.querySelector('#pw-otp');
        if (method !== 'authenticator' && filled && otpBox?.value === filled) {
          otpBox.value = '';
          m.el.querySelector('#pw-filled').hidden = true;
        }
        fillCode();
        otpBox?.focus();
      });
      const send = m.el.querySelector('#pw-send');
      if (send) send.onclick = () => sendCode();
      syncSend();
      syncButtons();
      // Back ends this attempt in the app too: the password goes, and a reply still on its way is dropped.
      m.el.querySelectorAll('[data-back]').forEach(b => b.onclick = () => {
        call('loginCancel', {}, { silent: true });
        resetTwoStep();
        step = 'login';
        render();
      });
      m.el.querySelectorAll('[data-x2]').forEach(b => b.onclick = () => {
        m.close();
        const way = b.dataset.x2;
        if (way === 'browser') addBrowser(null);
        else if (way === 'phone') quickLogin(realm);
        else if (way === 'cookie') accountModal(null);
        else if (way === 'vault') importVault();
      });
      const solve = m.el.querySelector('#pw-solve');
      if (solve) solve.onclick = async () => {
        err('');
        let r;
        try { r = await request(solve, 'loginCaptcha', {}); }
        catch (e) { return err(e.message); }
        // The puzzle window was closed: "Solve" asks for a fresh one. Any other answer moves on (or is a new puzzle).
        if (r?.status === 'captcha' && r.closed) return;
        handle(r);
      };
      const finish = m.el.querySelector('#pw-finish');
      if (finish) finish.onclick = async () => {
        err('');
        let r;
        try { r = await request(finish, 'loginFinish', {}); }
        catch (e) { return err(e.message); }
        handle(r);
      };
      eyeToggle(m.el.querySelector('#pw-eye'), [m.el.querySelector('#pw-pass')]);
      // A pasted "123 456" or "123-456" keeps its six digits; letters can't be typed.
      const otp = m.el.querySelector('#pw-otp');
      if (otp) otp.addEventListener('input', () => {
        const digits = otp.value.replace(/\D/g, '').slice(0, 6);
        if (otp.value !== digits) otp.value = digits;
        // Typed over: the person's own code now, left alone.
        if (otp.value !== filled) m.el.querySelector('#pw-filled').hidden = true;
      });
      const loginForm = m.el.querySelector('#pw-login');
      if (loginForm) loginForm.onsubmit = e => { e.preventDefault(); submitLogin(); };
      const twoForm = m.el.querySelector('#pw-two');
      if (twoForm) twoForm.onsubmit = e => { e.preventDefault(); submitTwoStep(); };
      // The puzzle and "Try again" screens have no box: their button takes the focus.
      setTimeout(() => m.el.querySelector(step === 'login' && user ? '#pw-pass' : '#pw-user, #pw-otp, #pw-solve, #pw-finish')?.focus(), 30);
    }

    async function submitLogin() {
      user = m.el.querySelector('#pw-user').value.trim();
      const pass = m.el.querySelector('#pw-pass').value;
      if (!user || !pass) return err(t('pw.need'));
      err('');
      let r;
      try { r = await request(m.el.querySelector('#pw-go'), 'loginPassword', { username: user, password: pass, realm }); }
      catch (e) {
        err(e.message);
        // Typing again replaces the refused password instead of adding to it.
        const box = m.el.querySelector('#pw-pass');
        box?.focus();
        box?.select();
        return;
      }
      handle(r);
    }

    /** One sign-in request at a time; see `pending`. */
    async function request(btn, cmd, args) {
      if (pending) return undefined;
      try {
        return await busy(btn, () => { pending = true; syncButtons(); return bridge.call(cmd, args); });
      } finally {
        pending = false;
        syncButtons();
      }
    }

    /** The buttons that send a sign-in request: off while one is on its way, Verify also while Roblox wants a pause. */
    function syncButtons() {
      clearTimeout(verifyTimer);
      m.el.querySelectorAll('#pw-go, #pw-solve, #pw-finish, [data-realm]').forEach(b => { b.disabled = pending; });
      const verify = m.el.querySelector('#pw-verify');
      if (!verify) return;
      const wait = Math.ceil((verifyAt - Date.now()) / 1000);
      verify.disabled = pending || wait > 0;
      verify.textContent = wait > 0 ? t('pw.verifyIn', { s: wait }) : t('pw.verify');
      if (wait > 0) verifyTimer = setTimeout(syncButtons, 1000);
    }

    /** Leaving a code screen: what was sent, and the waits, belong to that sign-in, not to the next one. */
    function resetTwoStep() {
      sent.clear();
      for (const k of Object.keys(resendAt)) delete resendAt[k];
      verifyAt = 0;
      clearTimeout(resendTimer);
      clearTimeout(verifyTimer);
      autofill = false;
      filled = '';
      clearTimeout(fillTimer);
      fillSeq++;
    }

    async function submitTwoStep() {
      const code = m.el.querySelector('#pw-otp').value.trim();
      if (!code) return err(t('pw.needCode'));
      err('');
      let r;
      try { r = await request(m.el.querySelector('#pw-verify'), 'loginTwoStep', { method, code }); }
      catch (e) { return err(e.message); }
      handle(r);
    }

    /** "Code sent to your email · Resend (25 s)": shown for email and SMS codes, the resend held back a little. */
    function syncSend() {
      clearTimeout(resendTimer);
      const row = m.el.querySelector('#pw-sendrow'), btn = m.el.querySelector('#pw-send'), note = m.el.querySelector('#pw-sentnote');
      if (!row || !btn) return;
      row.hidden = method === 'authenticator';
      if (row.hidden) return;
      const wasSent = sent.has(method);
      note.textContent = wasSent ? t(method === 'sms' ? 'pw.sentSms' : 'pw.sentEmail') : '';
      const wait = Math.ceil(((resendAt[method] || 0) - Date.now()) / 1000);
      const held = verifyAt > Date.now();
      // Off while a send is on its way too, so flipping tabs fast can't send a second one.
      btn.disabled = sending || held || (wasSent && wait > 0);
      btn.textContent = !wasSent ? t('pw.send') : wait > 0 ? t('pw.resendIn', { s: wait }) : t('pw.resend');
      if (held || (wasSent && wait > 0)) resendTimer = setTimeout(syncSend, 1000);
    }

    async function sendCode() {
      const btn = m.el.querySelector('#pw-send');
      if (!btn || btn.disabled || sending) return;
      err('');
      const way = method;
      let r;
      sending = true;
      try { r = await busy(btn, () => bridge.call('loginTwoStepSend', { method: way })); }
      catch (e) { return err(e.message); }
      finally { sending = false; syncSend(); }
      if (r?.status !== 'sent') return handle(r);
      sent.add(way);
      resendAt[way] = Date.now() + RESEND_WAIT;
      syncSend();
    }

    function handle(r) {
      if (!r) return;
      // The attempt is over (wrong password, account locked, too many tries), or what it asked for was answered and
      // then failed: back to the first step with the reason, the username kept, instead of offering the puzzle or the
      // code again.
      if (r.status === 'login') {
        resetTwoStep();
        step = 'login';
        render();
        err(r.error || '');
        return;
      }
      // Signed in, but the account couldn't be looked up: "Try again" only does that.
      if (r.status === 'finish') {
        resetTwoStep();
        step = 'finish';
        render();
        err(r.error || '');
        return;
      }
      // Too many codes tried: the code screen stays, Verify waits a minute.
      if (r.status === 'wait') {
        verifyAt = Date.now() + (r.seconds || 60) * 1000;
        const hadFocus = m.el.querySelector('#pw-verify') === document.activeElement;
        syncButtons();
        syncSend();
        err(r.error || '');
        // Verify turns itself off while it has the focus: the code box takes it, ready for the next code.
        if (hadFocus || !m.el.contains(document.activeElement)) m.el.querySelector('#pw-otp')?.focus();
        return;
      }
      if (r.status === 'captcha') {
        // Solved, and Roblox wants another one (it can chain them): say so, or it looks like nothing happened.
        const again = step === 'captcha';
        step = 'captcha';
        render();
        if (again) err(t('pw.captchaAgain'));
        return;
      }
      if (r.status === 'twostep') {
        resetTwoStep();
        step = 'twostep';
        const known = (r.methods || []).filter(x => METHOD_LABEL[x]);
        methods = known.length ? known : ['authenticator', 'email'];
        // Roblox may already have sent the code a given way (e.g. by email): start on that tab.
        method = methods.includes(r.media) ? r.media : methods[0];
        // Roblox sends the code by the preferred way (or the one it named) as soon as it asks for it.
        if (method === 'email' || method === 'sms') { sent.add(method); resendAt[method] = Date.now() + RESEND_WAIT; }
        autofill = !!r.autofill;
        render();
        fillCode(); // on the authenticator tab (now or once picked), with the key saved here
        return;
      }
      if (r.status === 'added') {
        const a = r.added;
        added.push(a);
        toast(t(a.merged ? 't.merged' : 't.added', { name: a.label }), 'ok');
        resetTwoStep();
        step = 'login';
        user = '';
        render();
      }
    }

    render();
  }

  /**
   * Quick Login: Roblox shows a code that the user confirms on any device already signed in (phone, or the Roblox app).
   * The code is renewed after each account, so several can be added in a row, like the browser sign-in.
   */
  /** @param forName the account being signed in again (Sign in on a signed-out card), named at the top. */
  function quickLogin(forRealm, forName = '') {
    const added = [];
    let gen = 0, timer = 0, ticker = 0, seconds = 0, phase = '', expiredInRow = 0, closed = false;
    // Roblox VNG (Vietnam) accounts confirm the code in the Roblox VNG app and sign in on VNG's servers.
    let realm = forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : '');
    const m = modal({
      title: t('ql.title'),
      busy: true,
      body: `${forName ? `<p class="ql-for">${t('ql.for', { name: `<b>${esc(forName)}</b>` })}</p>` : ''}<div class="ql-realm" role="tablist">
          <button role="tab" data-realm="">Roblox</button><button role="tab" data-realm="vng">Roblox VNG</button>
        </div>
        <div class="ql">
          <div class="ql-qr" id="ql-qr"></div>
          <div class="ql-side">
            <div class="ql-code" id="ql-code"></div>
            <ol class="ql-steps"><li id="ql-step1"></li><li>${t('ql.step2')}</li><li>${t('ql.step3')}</li></ol>
          </div>
        </div>
        <div class="ql-status" id="ql-status"></div>
        <div class="ql-added" id="ql-added"></div>`,
      actions: [
        { text: t('ql.browser'), onClick: ({ close }) => { close(); addBrowser(null); } },
        { text: t('common.done'), kind: 'primary', onClick: ({ close }) => close() },
      ],
      onClose: () => {
        closed = true;
        clearTimeout(timer);
        clearInterval(ticker);
        call('quickLoginCancel', {}, { silent: true });
        if (!added.length) return;
        S.selected = new Set(added.map(a => a.id));
        S.anchor = added[added.length - 1].id;
        renderSelection();
      },
    });
    const $q = sel => m.el.querySelector(sel);
    const mmss = n => `${Math.floor(n / 60)}:${String(n % 60).padStart(2, '0')}`;
    const show = (next, html, kind = '') => {
      phase = next;
      const box = $q('#ql-status');
      box.className = `ql-status ${kind}`;
      box.innerHTML = html;
    };
    const newCodeBtn = `<button class="btn sm" data-new>${t('ql.newCode')}</button>`;
    // A declined or expired code can't be used any more: it goes, so nobody types it in.
    const clearCode = () => { $q('#ql-code').textContent = '······'; $q('#ql-qr').innerHTML = ''; };
    const showWaiting = () => show('waiting', `<span class="spin"></span><span>${t('ql.waiting', { time: mmss(seconds) })}</span>`);
    const showRealm = () => {
      m.el.querySelectorAll('[data-realm]').forEach(b => {
        b.classList.toggle('on', b.dataset.realm === realm);
        b.setAttribute('aria-selected', String(b.dataset.realm === realm));
      });
      $q('#ql-step1').textContent = t(realm === 'vng' ? 'ql.step1Vng' : 'ql.step1');
      // The browser sign-in is roblox.com only.
      $q('.modal-foot .btn[data-i="0"]').hidden = realm === 'vng';
    };
    m.el.addEventListener('click', e => {
      const pick = e.target.closest('[data-realm]');
      if (pick) {
        if (pick.dataset.realm === realm) return;
        realm = pick.dataset.realm;
        store.set('rm.qlRealm', realm);
        showRealm();
      } else if (!e.target.closest('[data-new]')) {
        return;
      }
      expiredInRow = 0;
      start();
    });
    showRealm();

    async function start() {
      const my = ++gen;
      clearTimeout(timer);
      clearInterval(ticker);
      $q('#ql-code').textContent = '······';
      $q('#ql-qr').innerHTML = '';
      show('getting', `<span class="spin"></span><span>${t('ql.getting')}</span>`);
      let r;
      try {
        r = await bridge.call('quickLoginStart', { realm });
      } catch (e) {
        if (my === gen && !closed) show('error', `<span>${esc(e.message)}</span>${newCodeBtn}`, 'bad');
        return;
      }
      if (my !== gen || closed) return;
      $q('#ql-code').textContent = r.code;
      $q('#ql-qr').innerHTML = r.qr ? `<img src="${r.qr}" alt="QR">` : '';
      seconds = r.seconds;
      showWaiting();
      ticker = setInterval(() => {
        if (seconds > 0) seconds--;
        if (phase === 'waiting') showWaiting();
      }, 1000);
      timer = setTimeout(() => poll(my), 2500);
    }

    async function poll(my) {
      let r;
      try {
        r = await bridge.call('quickLoginPoll', {});
      } catch (e) {
        if (my !== gen || closed) return;
        // That code is spent: it goes with the error, as it does when it is declined or runs out.
        clearInterval(ticker);
        clearCode();
        show('error', `<span>${esc(e.message)}</span>${newCodeBtn}`, 'bad');
        return;
      }
      if (my !== gen || closed) return;
      switch (r.status) {
        case 'added': {
          const a = r.added;
          added.push(a);
          toast(t(a.merged ? 't.merged' : 't.added', { name: a.label }), 'ok');
          $q('#ql-added').textContent = t('ql.addedList', { names: added.map(x => x.label).join(', ') });
          expiredInRow = 0;
          return start();
        }
        case 'linked':
          expiredInRow = 0;
          show('linked', `${r.picture ? `<img class="ql-pic" src="${esc(r.picture)}" alt="">` : '<span class="spin"></span>'}<span>${hidingNames() ? t('ql.linkedHidden') : t('ql.linked', { name: esc(r.name || '') })}</span>`);
          break;
        case 'adding':
          show('adding', `<span class="spin"></span><span>${t('ql.adding')}</span>`);
          break;
        case 'cancelled':
          clearCode();
          return show('cancelled', `<span>${t('ql.cancelled')}</span>${newCodeBtn}`, 'bad');
        case 'expired':
        case 'none':
          // Renew quietly a few times; after that stop asking Roblox until the user comes back.
          if (++expiredInRow <= 3) return start();
          clearCode();
          return show('expired', `<span>${t('ql.expired')}</span>${newCodeBtn}`);
        default:
          if (typeof r.seconds === 'number') seconds = r.seconds;
          if (phase !== 'waiting') showWaiting();
      }
      timer = setTimeout(() => poll(my), 2500);
    }

    start();
  }

  async function checkAccounts(ids) {
    const r = await call('checkAccounts', { ids });
    if (!r) return;
    // Signed-out accounts are gathered under the "Needs sign-in" filter; offer to jump there.
    const show = r.bad && accounts().some(a => a.cookieValid === false)
      ? { label: t('t.showSignedOut'), run: () => { setView('accounts'); S.filter = 'invalid'; S.group = ''; update(); $('#scroll').scrollTop = 0; } } : null;
    // "1 needs", not "1 need": the singular goes by the signed-out count (tn picks it by n).
    toast(tn(r.failed ? 't.checkedFail' : 't.checked', r.bad, { ok: r.ok, bad: r.bad, f: r.failed }), r.bad ? 'warn' : 'ok', show);
  }

  /** force: end the clients at once (frozen ones) instead of closing their windows first. */
  async function closeIds(ids, force = false) {
    if (inFlight.has('close')) return;
    inFlight.add('close');
    let n;
    try {
      n = await call('closeAccounts', { ids, force });
    } finally {
      inFlight.delete('close');
    }
    if (n !== undefined) toast(n ? tn(force ? 't.killing' : 't.closing', n) : t('t.noneRunning'), n ? 'ok' : 'info');
  }

  async function closeAll(force = false) {
    if (!await confirmBox(t(force ? 't.killAllQ' : 't.closeAllQ'), { ok: t(force ? 't.killAllOk' : 't.closeAllOk'), danger: true })) return;
    const n = await call('closeAll', { force });
    if (n !== undefined) toast(n ? tn(force ? 't.killing' : 't.closingAll', n) : t('t.noRoblox'), 'ok');
  }

  async function removeIds(ids) {
    const names = ids.map(byId).filter(Boolean).map(a => a.label);
    const list = names.slice(0, 5).join(', ') + (names.length > 5 ? ', …' : '');
    if (!await confirmBox(tn('t.deleteQ', ids.length, { list }), { title: tn('t.deleteTitle', ids.length), ok: t('t.deleteOk'), danger: true })) return;
    const n = await call('deleteAccounts', { ids });
    if (n !== undefined) {
      ids.forEach(id => S.selected.delete(id));
      renderSelection();
      toast(tn('t.deleted', n), 'ok');
    }
  }

  async function copy(id, field) {
    const r = await call('copy', { id, field });
    // Says what was copied; a secret also says it leaves the clipboard again, and a 2FA code how long it still works.
    if (r && field === 'totp') toast(t('t.copiedCode', { s: r.seconds }), 'ok');
    else if (r) toast(t(r.secret ? 't.copiedSecret' : { username: 't.copiedUser', pid: 't.copiedPid' }[field] || 't.copied'), 'ok');
  }

  const confirmMulti = () => confirmBox(t('set.multiConfirm'), { title: t('set.multiConfirmTitle'), ok: t('set.multiOk') });
  /** Turns Multi-Roblox on (after the risk warning) or off; a switch that was flipped goes back if the warning is declined. */
  async function setMulti(on) {
    const box = typeof on === 'object' ? on : null;
    const enable = box ? box.checked : on;
    if (enable && !await confirmMulti()) {
      if (box) box.checked = false;
      return;
    }
    call('setSettings', { multiInstance: enable });
  }

  /**
   * Accounts from another Bloxkeep vault file (another PC's, or a backup): pick the file, type that file's own master
   * password, tick what to bring over. The app keeps the opened file; this page only ever sees names and states,
   * never cookies or passwords. Closing the dialog (or locking) makes the app forget it.
   */
  function importVault() {
    let step = 'pick', file = null, preview = null, done = false, waitTimer = 0;
    const m = modal({
      title: t('iv.title'),
      dismissable: false,
      busy: true,
      enterSubmits: true,
      body: '<div id="iv-body"></div>',
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        { text: t('iv.choose'), kind: 'primary', onClick: ({ btn }) => (step === 'pick' ? pick(btn) : step === 'password' ? open(btn) : apply(btn)) },
      ],
      onClose: () => {
        clearInterval(waitTimer);
        if (!done) call('vaultImportCancel', {}, { silent: true });
      },
    });
    const q = sel => m.el.querySelector(sel);
    const primary = () => q('.modal-foot .btn.primary');
    const err = msg => { const e = q('#iv-err'); if (e) { e.textContent = msg || ''; e.hidden = !msg; } };
    const fileLine = () => `<div class="iv-file">${icon('folder')}<b title="${esc(file.name)}">${esc(file.name)}</b><button type="button" class="link" id="iv-other">${t('iv.otherFile')}</button></div>`;
    const signIn = a => a.cookieValid === true ? `<span class="iv-ck ok">${t('iv.ckOk')}</span>`
      : a.cookieValid === false ? `<span class="iv-ck bad">${t('iv.ckBad')}</span>` : `<span class="iv-ck">${t('iv.ckUnknown')}</span>`;
    // Names hidden: the file's accounts too, numbered by their place in the file.
    const row = x => {
      const a = hidingNames() ? { ...x, label: t('hide.account', { n: x.key + 1 }), username: '' } : x;
      return `<label class="rc-item iv-item">
        <input type="checkbox" value="${a.key}" ${a.exists ? '' : 'checked'}>
        <div class="avatar sm">${avatarInner(a)}</div>
        <span class="iv-name"><b title="${esc(a.label)}">${esc(a.label)}</b>
          <small>${[atName(a), a.group ? esc(a.group) : '', signIn(a), a.hasPassword ? t('iv.pwSaved') : ''].filter(Boolean).join(' · ')}</small></span>
        ${vngTag(a)}<span class="state-pill ${a.exists ? 'off' : 'ok'}">${t(a.exists ? 'iv.exists' : 'iv.new')}</span></label>`;
    };

    function render() {
      clearInterval(waitTimer);
      const body = q('#iv-body');
      // The list step greys the button with nothing ticked; another file starts over with it usable.
      if (step !== 'preview') primary().disabled = false;
      if (step === 'pick') {
        body.innerHTML = `<p class="modal-text">${t('iv.intro')}</p>
          <p class="fine">${icon('shield')}<span>${t('iv.safe')}</span></p>
          <div class="form-error" id="iv-err" role="alert" hidden></div>`;
        primary().textContent = t('iv.choose');
      } else if (step === 'password') {
        body.innerHTML = `${fileLine()}
          <div class="form"><label>${t('iv.pw')}
            <span class="pw"><input class="field" type="password" id="iv-pw" maxlength="256" autocomplete="off" spellcheck="false">
            <button type="button" class="icon-btn" id="iv-eye" title="${t('lock.showPw')}" aria-label="${t('lock.showPw')}" aria-pressed="false">${icon('eye')}</button></span></label></div>
          <p class="fine">${icon('shield')}<span>${t('iv.safe')}</span></p>
          <div class="form-error" id="iv-err" role="alert" hidden></div>`;
        primary().textContent = t('iv.open');
        eyeToggle(q('#iv-eye'), [q('#iv-pw')]);
        setTimeout(() => q('#iv-pw')?.focus(), 30);
      } else {
        const list = preview.accounts;
        body.innerHTML = `${fileLine()}
          ${list.length ? `<div class="iv-bar"><span id="iv-count"></span><button type="button" class="link" id="iv-all">${t('iv.all')}</button><button type="button" class="link" id="iv-none">${t('iv.none')}</button></div>
          <div class="rc-list iv-list">${list.map(row).join('')}</div>` : `<p class="modal-text">${t('iv.empty')}</p>`}
          ${preview.presets ? `<label class="iv-presets"><input type="checkbox" id="iv-presets"><span><b>${t('iv.presets', { n: preview.presets })}</b><small>${t('iv.presetsSub')}</small></span></label>` : ''}
          ${list.some(a => a.exists) ? `<p class="fine">${icon('info')}<span>${t('iv.existsNote')}</span></p>` : ''}
          <div class="form-error" id="iv-err" role="alert" hidden></div>`;
        primary().textContent = t('iv.importBtn');
        const boxes = () => [...m.el.querySelectorAll('.iv-item input')];
        const sync = () => {
          const n = boxes().filter(b => b.checked).length;
          const count = q('#iv-count');
          if (count) count.textContent = t('iv.count', { n, total: list.length });
          primary().disabled = !n && !q('#iv-presets')?.checked;
        };
        m.el.querySelectorAll('.iv-item input, #iv-presets').forEach(b => b.addEventListener('change', sync));
        const all = on => () => { boxes().forEach(b => { b.checked = on; }); sync(); };
        q('#iv-all')?.addEventListener('click', all(true));
        q('#iv-none')?.addEventListener('click', all(false));
        sync();
        // The password box that had the focus is gone: Import takes it (the first account when nothing is ticked).
        if (!m.el.contains(document.activeElement)) (primary().disabled ? q('.iv-item input, #iv-presets') : primary())?.focus();
      }
      q('#iv-other')?.addEventListener('click', e => pick(e.currentTarget));
    }

    /** After three wrong passwords the app makes each try wait: Open counts down meanwhile. */
    function countdown(seconds) {
      clearInterval(waitTimer);
      const until = Date.now() + seconds * 1000;
      const tick = () => {
        const btn = primary(), left = Math.ceil((until - Date.now()) / 1000);
        if (!btn || step !== 'password') return clearInterval(waitTimer);
        btn.disabled = left > 0;
        btn.textContent = left > 0 ? t('iv.wait', { n: left }) : t('iv.open');
        if (left <= 0) clearInterval(waitTimer);
      };
      tick();
      waitTimer = setInterval(tick, 500);
    }

    async function pick(btn) {
      err('');
      let r;
      try { r = await busy(btn, () => bridge.call('vaultImportPick')); }
      catch (e) { return err(e.message); }
      if (!r) return; // the file dialog was cancelled: things stay as they were
      file = r;
      preview = null;
      step = 'password';
      render();
      if (r.wait) countdown(r.wait);
    }

    async function open(btn) {
      const pw = q('#iv-pw').value;
      if (!pw) return err(t('iv.needPw'));
      err('');
      let r;
      try { r = await busy(btn, () => bridge.call('vaultImportOpen', { password: pw })); }
      catch (e) { return err(e.message); }
      if (!r) return;
      if (r.status !== 'ok') {
        err(r.error);
        const box = q('#iv-pw');
        box.select();
        box.focus();
        if (r.wait) countdown(r.wait);
        return;
      }
      preview = r;
      step = 'preview';
      render();
    }

    async function apply(btn) {
      err('');
      const keys = [...m.el.querySelectorAll('.iv-item input')].filter(b => b.checked).map(b => +b.value);
      let r;
      try { r = await busy(btn, () => bridge.call('vaultImportApply', { token: preview.token, keys, presets: !!q('#iv-presets')?.checked })); }
      catch (e) { return err(e.message); }
      done = true;
      m.close();
      const parts = r.added && r.updated ? t('iv.doneBoth', { n: r.added, u: r.updated })
        : r.added ? tn('iv.done', r.added) : r.updated ? tn('iv.doneUpd', r.updated) : '';
      // Presets past the limit are left out: said, so the count above doesn't read as all of them.
      const presets = [r.presets ? tn('iv.donePresets', r.presets) : '', r.presetsSkipped ? tn('iv.presetsSkipped', r.presetsSkipped) : ''].filter(Boolean).join(' ');
      toast(parts || presets ? `${parts}${parts && presets ? ' ' : ''}${presets}` : t('iv.doneNone'), r.presetsSkipped ? 'warn' : parts || presets ? 'ok' : 'info');
      if (r.ids?.length) {
        S.selected = new Set(r.ids);
        S.anchor = r.ids[r.ids.length - 1];
        renderSelection();
      }
    }

    render();
  }

  /** The server an account is in: from its window's log here, or from Roblox's status when visible. */
  function serverOf(a) {
    if (!a) return null;
    // The server id goes with the place it belongs to (a game can move players to another of its places).
    if (isLocal(a) && a.session?.jobId && a.session.connection === 'ingame' && (a.session.jobPlaceId || a.session.game?.placeId))
      return { placeId: a.session.jobPlaceId || a.session.game.placeId, jobId: a.session.jobId };
    if (a.presence?.type === 'ingame' && a.presence.jobId && (a.presence.joinPlaceId || a.presence.placeId))
      return { placeId: a.presence.joinPlaceId || a.presence.placeId, jobId: a.presence.jobId };
    return null;
  }

  /** "Chosen accounts": tick them in one list instead of going through each account's ⋯ menu. */
  function pickRobloxConfigAccounts() {
    const list = accounts().slice().sort((x, y) => compareText(x.label, y.label));
    if (!list.length) return toast(t('rc.noAccounts'), 'info');
    modal({
      title: t('rc.pickTitle'),
      body: `<p class="modal-text">${t('rc.pickText')}</p><div class="rc-list">${list.map(a => `<label class="rc-item">
          <input type="checkbox" value="${a.id}" ${a.useRobloxConfig ? 'checked' : ''}>
          <div class="avatar sm">${avatarInner(a)}</div><span><b title="${esc(a.label)}">${esc(a.label)}</b><small>${atName(a)}</small></span></label>`).join('')}</div>`,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            if (btn.disabled) return;
            const boxes = [...el.querySelectorAll('.rc-item input')];
            const on = boxes.filter(b => b.checked).map(b => b.value), off = boxes.filter(b => !b.checked).map(b => b.value);
            let ok = true;
            await busy(btn, async () => {
              if (on.length) ok = await call('setUseRobloxConfig', { ids: on, enabled: true }) !== undefined;
              if (ok && off.length) ok = await call('setUseRobloxConfig', { ids: off, enabled: false }) !== undefined;
            });
            if (!ok) return; // the error is shown; keep the list open to try again
            close();
            toast(tn('rc.picked', on.length), 'ok');
          },
        },
      ],
    });
  }

  async function toggleRobloxConfig(ids) {
    const enable = !ids.every(i => byId(i)?.useRobloxConfig);
    const n = await call('setUseRobloxConfig', { ids, enabled: enable });
    if (n !== undefined) toast(tn(enable ? 't.rcOn' : 't.rcOff', n), 'ok');
  }

  /** Settings › Auto-rejoin: on for every account, off for every one, or (shown only) some of them. */
  function syncRejoinAll() {
    const sel = $('#s-rejoin');
    if (!sel || document.activeElement === sel) return;
    const all = accounts(), on = all.filter(a => a.autoRejoin).length;
    const state = !all.length ? 'off' : on === all.length ? 'on' : on === 0 ? 'off' : 'some';
    setHtml(sel, [['on', t('set.rejoinAll')], ['off', t('set.rejoinNone')], ['some', t('set.rejoinSome', { n: on, total: all.length })]]
      .map(([v, text]) => `<option value="${v}" ${v === 'some' ? 'disabled hidden' : ''} ${v === state ? 'selected' : ''}>${esc(text)}</option>`).join(''));
    sel.value = state;
    sel.disabled = !all.length;
  }
  async function setRejoinAll(sel) {
    const enable = sel.value === 'on';
    const n = await call('setAutoRejoin', { ids: accounts().map(a => a.id), enabled: enable });
    if (n !== undefined) toast(tn(enable ? 't.rejoinOn' : 't.rejoinOff', n), 'ok');
    syncRejoinAll();
  }

  async function toggleRejoin(ids) {
    const enable = !ids.every(i => byId(i) && rejoinOn(byId(i))); // the opposite of the tick the menu showed
    const n = await call('setAutoRejoin', { ids, enabled: enable });
    if (n !== undefined) toast(tn(enable ? 't.rejoinOn' : 't.rejoinOff', n), 'ok');
  }

  /** Tiles all Roblox windows; with several monitors the user picks one first. */
  async function arrange(anchor) {
    const monitors = S.app.monitors || [];
    const run = async monitor => {
      const n = await call('arrangeWindows', { monitor });
      if (n !== undefined) toast(n ? tn('t.arranged', n) : t('t.noWindows'), n ? 'ok' : 'info');
    };
    if (monitors.length <= 1 || !anchor) return run(monitors.find(m => m.primary)?.index ?? 0);
    menuAt(anchor, monitors.map(m => ({
      icon: 'monitor',
      text: t('m.monitor', { n: m.index + 1, w: m.width, h: m.height }) + (m.primary ? ` · ${t('m.primary')}` : ''),
      run: () => run(m.index),
    })), 260);
  }

  function cardMenu(id, x, y) {
    const a = byId(id);
    if (!a) return;
    if (!S.selected.has(id)) selectOnly(id);
    const ids = [...S.selected];
    const many = ids.length > 1;
    openMenu([
      { icon: 'play', text: many ? t('m.playN', { n: ids.length }) : t('m.play'), hint: 'Enter', run: () => launch(ids, false) },
      { icon: 'monitor', text: t('m.openApp'), run: () => launch(ids, true) },
      { icon: 'power', text: t('m.close'), disabled: !ids.some(i => isLocal(byId(i) || {})), run: () => closeIds(ids) },
      { icon: 'x', text: t('m.kill'), danger: true, disabled: !ids.some(i => isLocal(byId(i) || {})), run: () => closeIds(ids, true) },
      '-',
      { icon: 'edit', text: t('m.edit'), hint: 'F2', disabled: many, run: () => editAccount(id) },
      { icon: 'refresh', text: t('m.check'), run: () => checkAccounts(ids) },
      { icon: 'server', text: t('m.follow'), disabled: many || !serverOf(a), run: () => { const s = serverOf(a); useServer(s.placeId, s.jobId); toast(t('srv.followed', { name: a.label }), 'ok'); } },
      // Ticked as the card shows it (a running window's preset may decide), and says so when it does.
      { icon: 'repeat', text: t('m.autoRejoin'), checked: ids.every(i => byId(i) && rejoinOn(byId(i))), run: () => toggleRejoin(ids),
        title: ids.some(i => { const x = byId(i); return x?.session && x.session.autoRejoin !== undefined && x.session.autoRejoin !== x.autoRejoin; }) ? esc(t('m.rejoinPreset')) : '' },
      ...(S.app.settings?.robloxConfig?.mode === 'marked'
        ? [{ icon: 'sliders', text: t('m.robloxConfig'), checked: ids.every(i => byId(i)?.useRobloxConfig), run: () => toggleRobloxConfig(ids) }] : []),
      { icon: 'external', text: t('m.profile'), disabled: many || !a.userId, run: () => call('openProfile', { userId: a.userId }) },
      '-',
      { icon: 'copy', text: t('m.copyUser'), disabled: many, run: () => copy(id, 'username') },
      { icon: 'key', text: t('m.copyPw'), disabled: many || !a.hasPassword, run: () => copy(id, 'password') },
      { icon: 'shieldCheck', text: t('m.copyCode'), disabled: many || !a.hasAuthenticator, run: () => copy(id, 'totp') },
      '-',
      { icon: 'trash', text: many ? t('m.deleteN', { n: ids.length }) : t('m.delete'), hint: 'Del', danger: true, run: () => removeIds(ids) },
    ], x, y);
  }

  /** The Multi-Roblox dialog while it is open: it follows the state (Roblox closed meanwhile, turned on or off from the tray). */
  let multiHelpOpen = null;
  const multiHelpKey = m => `${m.enabled}|${m.armed}|${!!m.trayAutostart}`;
  function syncMultiHelp() {
    const o = multiHelpOpen, m = S.app.multi;
    if (!o || !m || multiHelpKey(m) === o.key) return;
    if (m.enabled !== o.enabled) {
      o.close(); // its buttons change too: opened again as it now is
      multiHelp();
      return;
    }
    o.key = multiHelpKey(m);
    const box = o.el.querySelector('.modal-body'), had = box.contains(document.activeElement);
    box.innerHTML = multiHelpBody(m);
    if (had) o.el.querySelector('.modal-foot .btn.primary')?.focus();
  }
  function multiHelp() {
    const m = S.app.multi;
    modal({
      title: t('multi.title'),
      body: multiHelpBody(m),
      actions: [
        ...(m.enabled ? [{ text: t('mh.disable'), kind: 'danger push', onClick: ({ close }) => { close(); setMulti(false); } }] : []),
        { text: t('common.done'), kind: 'primary', onClick: ({ close }) => close() },
      ],
      onMount: ({ el, close }) => {
        multiHelpOpen = { el, close, key: multiHelpKey(m), enabled: m.enabled };
        // On the dialog, not the switches: the body is drawn again when the state changes.
        el.addEventListener('change', ev => {
          if (ev.target.id === 'mh-windowed') call('setSettings', { multiWindowed: ev.target.checked });
          else if (ev.target.id === 'mh-tile') call('setSettings', { multiTile: ev.target.checked });
        });
        el.addEventListener('click', async e => {
          const b = e.target.closest('[data-local]');
          if (!b) return;
          if (b.dataset.local === 'enable') {
            close();
            setMulti(true);
          } else if (b.dataset.local === 'close-all') {
            const n = await busy(b, () => call('closeAll'));
            if (n !== undefined) toast(n ? tn('t.closingAll', n) : t('t.noRoblox'), 'ok');
          } else if (b.dataset.local === 'tray') {
            if ((await busy(b, () => call('disableTrayAutostart'))) !== undefined && b.isConnected) {
              b.disabled = true;
              b.textContent = t('mh.s2done');
            }
          }
        });
      },
      onClose: () => { multiHelpOpen = null; },
    });
  }
  function multiHelpBody(m) {
    let body;
    if (!m.enabled) {
      body = `<p class="modal-text">${t('mh.off')}</p><button class="btn primary" data-local="enable">${icon('layers')}${t('mh.enable')}</button>`;
    } else if (m.armed) {
      body = `<div class="hero-ok">${icon('check')}</div><p class="modal-text center">${t('mh.ok1')}</p><p class="muted center">${t('mh.ok2')}</p>`;
    } else {
      body = `<p class="modal-text">${t('mh.bad')}</p>
        <ol class="steps">
          <li><div>${t('mh.s1')}<br><button class="btn sm" data-local="close-all">${icon('power')}${t('mh.s1btn')}</button></div></li>
          ${m.trayAutostart ? `<li><div>${t('mh.s2')}<br><button class="btn sm" data-local="tray">${t('mh.s2btn')}</button></div></li>` : ''}
          <li><div>${t('mh.s3')}</div></li>
        </ol>`;
    }
    if (m.enabled) {
      // How the windows open.
      const opt = (id, key, on) => `<label class="mh-flag"><span><b>${t(key)}</b><small>${t(key + 'Desc')}</small></span>
        <span class="switch"><input type="checkbox" id="${id}" ${on ? 'checked' : ''}><span></span></span></label>`;
      body += opt('mh-windowed', 'mh.windowed', m.windowed) + opt('mh-tile', 'mh.tile', m.tile);
    }
    return body;
  }

  function accountModal(a) {
    const isNew = !a;
    const groups = [...new Set(accounts().map(x => x.group).filter(Boolean))];
    const dirty = new Set();
    const head = isNew
      ? `<div class="callout">${icon('info')}<div>${t('am.callout')}</div></div>`
      // Names hidden: no @username, and no user ID either (anyone can look an account up by it).
      : `<div class="acc-head"><div class="avatar">${avatarInner(a)}</div><div><b>${esc(a.label)}</b><div class="muted">${[atName(a), hidingNames() ? '' : `ID ${a.userId}`].filter(Boolean).join(' · ')}</div></div></div>`;
    const reveal = name => `<button type="button" class="icon-btn" data-reveal="${name}" title="${t('common.show')}">${icon('eye')}</button>`;
    // While names are hidden the alias can't be shown, so it can't be edited either: it isn't sent, and stays as it is.
    const aliasHidden = !isNew && hidingNames();
    // Authenticator key (an account already here): the saved key's code of the moment with Copy and Remove, or a box for
    // a key. The key itself never comes to the page; one typed here is checked by the app as it is typed, which also
    // shows the code it makes, to compare with the authenticator app before saving.
    const hasKey = !isNew && !!a.hasAuthenticator;
    let removeKey = false;
    const totpHtml = isNew ? '' : `<div class="totp">
        <span class="totp-label">${t('am.totp')} <span class="muted">${t('am.totpHint')}</span></span>
        <div class="totp-live" id="am-totp-live" ${hasKey ? '' : 'hidden'}>
          <b class="totp-code" id="am-totp-code">··· ···</b><span class="totp-left" id="am-totp-left"></span>
          <button type="button" class="btn sm" data-totp="copy">${icon('copy')}${t('am.totpCopy')}</button>
          <button type="button" class="btn sm ghost" data-totp="remove">${t('am.totpRemove')}</button>
        </div>
        <div class="pw" id="am-totp-box" ${hasKey ? 'hidden' : ''}><input class="field mono" type="password" name="authenticator" maxlength="512" placeholder="${t('am.totpPh')}" spellcheck="false" autocomplete="off" aria-label="${t('am.totp')}" aria-describedby="am-totp-msg">${reveal('authenticator')}</div>
        <p class="totp-msg" id="am-totp-msg" aria-live="polite"></p>
        <p class="totp-note">${t('am.totpNote')}</p>
      </div>`;
    // What the boxes held when the form opened: Esc or the X asks before throwing away a pasted cookie or edits.
    let initial = null, form = null;
    // A saved password only shown (Show fills it in) isn't an edit: a secret box counts once something was typed in it.
    const values = () => JSON.stringify([...[...form.querySelectorAll('input[name], textarea[name]')]
      .map(x => (x.name === 'password' || x.name === 'cookie') && !dirty.has(x.name) ? '' : x.value), removeKey]);
    // The code on show (the saved key's, or the typed key's preview), counted down and asked for again when it changes.
    let codeSeq = 0, codeTick = 0, typeWait = 0, codeUntil = 0;
    const split = c => c.length > 4 ? `${c.slice(0, Math.ceil(c.length / 2))} ${c.slice(Math.ceil(c.length / 2))}` : c;
    const typedKey = () => form?.querySelector('[name=authenticator]')?.value.trim() || '';
    function totpMsg(html, kind = '') {
      const msg = form?.querySelector('#am-totp-msg');
      if (!msg) return;
      msg.innerHTML = html;
      msg.className = `totp-msg ${kind}`;
    }
    async function showCode() {
      clearTimeout(codeTick);
      const seq = ++codeSeq;
      const typed = typedKey(), saved = hasKey && !removeKey && !typed;
      if (!saved && !typed) {
        totpMsg(removeKey ? `${esc(t('am.totpRemoved'))} <button type="button" class="link" data-totp="undo">${t('am.totpUndo')}</button>` : '');
        return;
      }
      const r = await call(saved ? 'totpCode' : 'totpCheck', saved ? { id: a.id } : { key: typed }, { silent: true });
      if (seq !== codeSeq || !form?.isConnected || !r) return;
      if (!r.ok) return totpMsg(esc(r.error), 'err');
      codeUntil = Date.now() + r.seconds * 1000;
      if (saved) setText(form.querySelector('#am-totp-code'), split(r.code));
      else totpMsg(esc(t('am.totpNow', { code: split(r.code) })), 'ok');
      countDown(seq, saved);
    }
    function countDown(seq, saved) {
      if (seq !== codeSeq || !form?.isConnected) return;
      const left = Math.ceil((codeUntil - Date.now()) / 1000);
      if (left <= 0) return showCode();
      if (saved) setText(form.querySelector('#am-totp-left'), t('am.totpLeft', { s: left }));
      codeTick = setTimeout(() => countDown(seq, saved), 1000);
    }
    modal({
      title: t(isNew ? 'am.addTitle' : 'am.editTitle'),
      dismissable: false,
      busy: true,
      enterSubmits: true,
      askClose: () => !!form && values() !== initial,
      onClose: () => {
        codeSeq++;
        clearTimeout(codeTick);
        clearTimeout(typeWait);
      },
      body: `${head}
        <div class="form">
          <label>${t('am.alias')}<input class="field" name="alias" maxlength="60" ${aliasHidden ? `disabled placeholder="${t('am.aliasHidden')}"` : `placeholder="${t('am.aliasPh')}" value="${esc(a?.alias)}"`}></label>
          <label>${t('am.group')}<input class="field" name="group" maxlength="40" list="dl-groups" placeholder="${t('am.groupPh')}" value="${esc(a?.group)}"></label>
          <datalist id="dl-groups">${groups.map(g => `<option value="${esc(g)}">`).join('')}</datalist>
          <label><span>${t('am.pw')} <span class="muted">${t('am.pwHint')}</span></span>
            <div class="pw"><input class="field" type="password" name="password" maxlength="200" placeholder="${!isNew && a.hasPassword ? t('am.pwSaved') : t('am.pwNone')}">${reveal('password')}</div></label>
          ${isNew ? `<label>${t('am.cookie')}
            <div class="pw"><input class="field mono" type="password" name="cookie" maxlength="4096" placeholder="_|WARNING:-DO-NOT-SHARE-THIS…" spellcheck="false">${reveal('cookie')}</div></label>` : ''}
          ${totpHtml}
          <label>${t('am.note')}<textarea class="field" name="note" maxlength="500" rows="2">${esc(a?.note)}</textarea></label>
        </div>
        <p class="fine">${icon('shield')}<span>${t('am.fine')}</span></p>
        <div class="form-error" role="alert" hidden></div>`,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t(isNew ? 'am.add' : 'am.save'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            const val = name => el.querySelector(`[name=${name}]`).value;
            const err = el.querySelector('.form-error');
            err.hidden = true;
            if (isNew && !val('cookie').trim()) {
              err.textContent = t('am.needCookie');
              err.hidden = false;
              err.scrollIntoView({ block: 'nearest' });
              return;
            }
            const args = { group: val('group'), note: val('note') };
            if (!aliasHidden) args.alias = val('alias');
            if (isNew) Object.assign(args, { cookie: val('cookie'), password: val('password') });
            else {
              args.id = a.id;
              if (dirty.has('password')) args.password = val('password');
              // A key typed replaces the saved one (the app checks it); Remove sends none; otherwise it stays.
              if (typedKey()) args.authenticator = typedKey();
              else if (removeKey) args.authenticator = '';
            }
            if (btn.disabled) return; // a second click while the first is still saving
            try {
              const r = await busy(btn, () => bridge.call(isNew ? 'addViaCookie' : 'updateAccount', args));
              close();
              if (isNew && r) {
                toast(t(r.merged ? 't.merged' : 't.added', { name: r.label }), 'ok');
                selectOnly(r.id);
              } else {
                toast(t('t.saved'), 'ok');
              }
            } catch (ex) {
              err.textContent = ex.message;
              err.hidden = false;
              err.scrollIntoView({ block: 'nearest' });
            }
          },
        },
      ],
      onMount: ({ el, close }) => {
        form = el;
        initial = values();
        el.querySelectorAll('input[name=password], input[name=cookie]').forEach(i => i.addEventListener('input', () => dirty.add(i.name)));
        // A key being typed is checked once the typing rests (the app says what's wrong, or shows its code).
        el.querySelector('[name=authenticator]')?.addEventListener('input', () => {
          clearTimeout(typeWait);
          codeSeq++; // a reply for what was typed before doesn't count any more
          typeWait = setTimeout(showCode, 300);
        });
        if (hasKey) showCode();
        el.addEventListener('click', async e => {
          if (e.target.closest('[data-local=browser]')) {
            close();
            addBrowser(null);
            return;
          }
          const k = e.target.closest('[data-totp]');
          if (k) {
            const live = el.querySelector('#am-totp-live'), box = el.querySelector('#am-totp-box');
            if (k.dataset.totp === 'copy') return copy(a.id, 'totp');
            // Remove (or its Undo) only changes the form: the key goes once the form is saved.
            removeKey = k.dataset.totp === 'remove';
            live.hidden = removeKey;
            box.hidden = !removeKey;
            if (!removeKey) box.querySelector('input').value = '';
            showCode();
            (removeKey ? box.querySelector('input') : live.querySelector('[data-totp=copy]')).focus();
            return;
          }
          const b = e.target.closest('[data-reveal]');
          if (!b) return;
          const input = b.parentElement.querySelector('input');
          if (input.type === 'text') {
            input.type = 'password';
            b.innerHTML = icon('eye');
            return;
          }
          // Only the saved password can be shown from the app: an authenticator key never comes back to the page.
          if (!isNew && input.name === 'password' && !dirty.has(input.name) && !input.value) {
            const v = await call('reveal', { id: a.id, field: input.name });
            if (typeof v === 'string') input.value = v;
          }
          input.type = 'text';
          b.innerHTML = icon('eyeOff');
        });
        setTimeout(() => el.querySelector(isNew ? '[name=cookie]' : aliasHidden ? '[name=group]' : '[name=alias]')?.focus(), 40);
      },
    });
  }
  const editAccount = id => { const a = byId(id); if (a) accountModal(a); };

  /* ───────────────────────── statistics ─────────────────────────
     Play time per account and game. The figures are asked for when the view opens and every 30 s while it is on
     screen; they are not part of the state the app pushes. The chart is plain divs, sized from the busiest bucket. */
  const STATS_PERIODS = [1, 7, 30];
  const ST = { days: [1, 7, 30].includes(+store.get('rm.statsDays')) ? +store.get('rm.statsDays') : 7, data: null, timer: 0, seq: 0,
    tab: store.get('rm.statsTab') === 'activity' ? 'activity' : 'stats' };
  const BAR_HEIGHT = 140;
  /** Two things on one tab, one at a time: Statistics (with its period chips) | Activity (with its own filters). */
  function statsHtml() {
    const onStats = ST.tab === 'stats';
    return `<header class="top"><div class="top-title"><h1>${t('stats.title')}</h1><p id="stats-sub">${t(onStats ? 'stats.note' : 'alog.note')}</p></div>
      <div class="top-tools"><div class="seg" id="stats-tabs" role="tablist" aria-label="${t('stats.title')}">${[['stats', 'stats.title'], ['activity', 'alog.title']].map(([id, key]) =>
        `<button type="button" role="tab" class="${ST.tab === id ? 'on' : ''}" data-act="stats-tab" data-tab="${id}" aria-selected="${ST.tab === id}">${t(key)}</button>`).join('')}</div>
        <div class="chips" id="stats-period" role="group" aria-label="${t('stats.title')}" ${onStats ? '' : 'hidden'}>${STATS_PERIODS.map(d =>
        `<button class="chip ${d === ST.days ? 'on' : ''}" data-act="stats-period" data-days="${d}" aria-pressed="${d === ST.days}">${t(`stats.period.${d}`)}</button>`).join('')}</div></div></header>
      <div class="scroll"><div class="stats"><div class="stats-in" id="stats-body" ${onStats ? '' : 'hidden'}></div>${actLogHtml()}</div></div>`;
  }
  function setStatsTab(tab) {
    ST.tab = tab === 'activity' ? 'activity' : 'stats';
    store.set('rm.statsTab', ST.tab);
    const onStats = ST.tab === 'stats';
    document.querySelectorAll('#stats-tabs [data-tab]').forEach(b => {
      b.classList.toggle('on', b.dataset.tab === ST.tab);
      b.setAttribute('aria-selected', String(b.dataset.tab === ST.tab));
    });
    $('#stats-period').hidden = !onStats;
    $('#stats-body').hidden = !onStats;
    $('#alog-sec').hidden = onStats;
    setText($('#stats-sub'), t(onStats ? 'stats.note' : 'alog.note'));
    $('#view-stats .scroll').scrollTop = 0;
  }
  function setStatsPeriod(days) {
    if (!STATS_PERIODS.includes(days) || days === ST.days) return;
    ST.days = days;
    store.set('rm.statsDays', String(days));
    document.querySelectorAll('#stats-period .chip').forEach(c => {
      c.classList.toggle('on', +c.dataset.days === days);
      c.setAttribute('aria-pressed', +c.dataset.days === days);
    });
    // The old period's figures never show under the new period's chip: loading until the new ones are in.
    ST.data = null;
    ST.failed = false;
    renderStats();
    loadStats();
  }
  /** Asks the app for the chosen period; a reply to an older request (or for another period) is dropped. */
  async function loadStats() {
    if (!S.shell || !S.app || S.app.locked) return;
    const seq = ++ST.seq;
    const data = await call('playStats', { days: ST.days }, { silent: true });
    if (seq !== ST.seq || !$('#stats-body')) return;
    // A failed refresh keeps figures already shown for this period; with none, it says so.
    ST.failed = !data;
    if (data) ST.data = data;
    renderStats();
  }
  /** Refreshes while the view is on screen (a stay still going grows), not while the window is hidden. */
  function watchStats(on) {
    clearInterval(ST.timer);
    ST.timer = on ? setInterval(() => { if (!document.hidden) { loadStats(); loadActivity(); } }, 30_000) : 0;
    if (!on) return;
    renderStats(); // last figures at once (a shell rebuilt in the other language), fresh ones a moment later
    loadStats();
    renderActivity();
    loadActivity();
  }
  // Fresh figures as soon as the window comes back, not up to 30 s later.
  document.addEventListener('visibilitychange', () => { if (!document.hidden && S.shell && S.view === 'stats') { loadStats(); loadActivity(); } });
  /** "2 h", "2 h 5 min", "1,234 h": hours grouped like other numbers, no "0 min". */
  const fmtSecs = s => {
    const m = Math.floor((s || 0) / 60), h = Math.floor(m / 60);
    return m < 60 ? t('dur.min', { m }) : m % 60 ? t('dur.hour', { h: fmtNum(h), m: m % 60 }) : t('dur.hours', { h: fmtNum(h) });
  };
  const sessionsText = n => (n === 1 ? t('stats.session1') : t('stats.sessionsN', { n: fmtNum(n) }));
  function renderStats() {
    const d = ST.data, host = $('#stats-body');
    if (!host) return;
    if (!d) {
      setHtml(host, ST.failed
        ? `<div class="empty"><h2>${t('stats.error')}</h2><div class="row"><button class="btn" data-act="stats-retry">${icon('refresh')}${t('srv.retry')}</button></div></div>`
        : '<div class="empty"><span class="spin"></span></div>');
      return;
    }
    if (!d.sessions) {
      // The same empty state as the other tabs: the tab's icon, a title, one line.
      setHtml(host, `<div class="empty"><span class="pv-empty-icon">${icon('clock')}</span>${d.recorded ? `<h2>${t('stats.emptyPeriod')}</h2>` : `<h2>${t('stats.emptyTitle')}</h2><p>${t('stats.emptyText')}</p>`}</div>`);
      return;
    }
    const top = d.games[0];
    const tiles = [[t('stats.total'), fmtSecs(d.totalSeconds)], [t('stats.sessions'), fmtNum(d.sessions)], [t('stats.kicks'), fmtNum(d.kicks)], [t('stats.topGame'), top ? esc(top.name) : '—']]
      .map(([label, value]) => `<div class="stat"><small>${label}</small><b title="${value}">${value}</b></div>`).join('');
    // Labels: every hour of 6 today, every day of the week, every 5th day of the month counted back from today (so
    // today always has one). The bar holding this moment
    // is marked as now; on Today the hours still to come are drawn fainter.
    const max = Math.max(1, ...d.buckets.map(b => b.seconds));
    const every = d.hourly ? 6 : d.buckets.length > 10 ? 5 : 1;
    const nowMs = Date.now();
    const nowIndex = d.buckets.findLastIndex(b => new Date(b.start).getTime() <= nowMs);
    const bars = d.buckets.map((b, i) => {
      const at = new Date(b.start);
      // Hours as people say them: "6 AM" in English, "06:00" (24-hour) in Vietnamese.
      const label = d.hourly ? at.toLocaleTimeString(locale(), lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric' })
        : every > 1 ? at.toLocaleDateString(locale(), { day: 'numeric', month: 'numeric' }) : at.toLocaleDateString(locale(), { weekday: 'short' });
      const when = d.hourly ? label : at.toLocaleDateString(locale(), { weekday: 'short', day: 'numeric', month: 'short' });
      const h = b.seconds ? Math.max(3, Math.round(b.seconds / max * BAR_HEIGHT)) : 2;
      const state = i === nowIndex ? 'now' : i > nowIndex ? 'future' : '';
      return `<div class="sbar ${state} ${b.seconds ? '' : 'zero'}" title="${esc(`${when} · ${fmtSecs(b.seconds)}`)}"><i style="height:${h}px"></i><span>${(d.hourly ? i : d.buckets.length - 1 - i) % every === 0 ? label : ''}</span></div>`;
    }).join('');
    const rows = d.accounts.map(a => {
      // A deleted account keeps the name it had when it played (older records have none). Names hidden: the list's
      // "Account N", and a deleted one is just that.
      const hide = hidingNames();
      const label = hide ? byId(a.id)?.label || t('stats.deleted') : a.label || t('stats.deleted');
      const gone = a.deleted && a.label && !hide ? ` <small class="muted">· ${t('stats.deletedTag')}</small>` : '';
      return `<tr><td><div class="sacc"><span class="avatar sm">${avatarInner({ avatarUrl: a.avatarUrl, username: hide ? a.id : a.username || a.id, label })}</span><b title="${esc(label)}">${esc(label)}${gone}</b></div></td>
        <td class="n">${fmtSecs(a.seconds)}</td><td class="n">${fmtNum(a.sessions)}</td><td class="n">${fmtNum(a.kicks)}</td></tr>`;
    }).join('');
    const games = d.games.map((g, i) => `<div class="sgame"><em>${i + 1}</em>${g.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="" loading="lazy">` : `<span class="gph">${icon('gamepad')}</span>`}
        <div class="gtxt"><b title="${esc(g.name)}">${esc(g.name)}</b><small>${sessionsText(g.sessions)}</small><div class="sg-meter"><i style="transform:scaleX(${(g.seconds / Math.max(1, top.seconds)).toFixed(3)})"></i></div></div>
        <span class="sg-time">${fmtSecs(g.seconds)}</span></div>`).join('');
    setHtml(host, `<div class="stat-tiles">${tiles}</div>
      <div class="stat-card"><h3>${t(d.hourly ? 'stats.byHour' : 'stats.byDay')}</h3><div class="sbars" style="--bar-h:${BAR_HEIGHT}px">${bars}</div></div>
      <div class="stat-cols">
        <div class="stat-card"><h3>${t('stats.byAccount')}</h3>
          <table class="stat-table"><thead><tr><th>${t('stats.account')}</th><th class="n">${t('stats.total')}</th><th class="n">${t('stats.sessions')}</th><th class="n">${t('stats.kicks')}</th></tr></thead><tbody>${rows}</tbody></table></div>
        <div class="stat-card"><h3>${t('stats.topGames')}</h3><div class="sgames">${games}</div></div>
      </div>`);
  }

  /* ───────────────────────── activity log ─────────────────────────
     Statistics › Activity: what the app saw happen, newest first, a page at a time. The app words each line in the
     page's language (and hides account names when Settings does); the page groups them by day and shows today's
     times as "5 min ago", older ones as clock times. Asked for with the figures above, and with them every 30 s. */
  const AL_GROUPS = ['play', 'drops', 'schedule', 'signin', 'updates'];
  const AL_PAGE = 100;
  const AL = { account: '', group: '', limit: AL_PAGE, data: null, failed: false, seq: 0 };
  function actLogHtml() {
    return `<section class="stat-card alog" id="alog-sec" aria-labelledby="alog-h" ${ST.tab === 'activity' ? '' : 'hidden'}>
        <div class="alog-head"><h3 id="alog-h">${t('alog.title')}</h3>
          <div class="alog-tools">
            <select class="select sm" id="alog-account" aria-label="${t('alog.filterAccount')}"><option value="">${t('alog.allAccounts')}</option></select>
            <select class="select sm" id="alog-group" aria-label="${t('alog.filterKind')}"><option value="">${t('alog.allKinds')}</option>${AL_GROUPS.map(g => `<option value="${g}">${t(`alog.g.${g}`)}</option>`).join('')}</select>
            <button class="btn sm" data-act="alog-export">${icon('download')}<span>${t('alog.export')}</span></button>
          </div>
        </div>
        <div id="alog-body"></div>
      </section>`;
  }
  /** Asks the app for the list as filtered; a reply to an older request is dropped. */
  async function loadActivity() {
    if (!S.shell || !S.app || S.app.locked) return;
    const seq = ++AL.seq;
    const data = await call('activity', { account: AL.account || null, group: AL.group || null, limit: AL.limit }, { silent: true });
    if (seq !== AL.seq || !$('#alog-body')) return;
    AL.failed = !data;
    if (data) AL.data = data;
    renderActivity();
  }
  /** The filters as chosen (they outlive the shell rebuilt in another language), the accounts as the log names them. */
  function syncActFilters() {
    const acc = $('#alog-account'), grp = $('#alog-group');
    if (!acc || !grp) return;
    const options = `<option value="">${t('alog.allAccounts')}</option>${(AL.data?.accounts || []).map(a => `<option value="${esc(a.id)}">${esc(a.label)}</option>`).join('')}`;
    // Not while its list may be open: the options would change under the pointer.
    if (acc._html !== options && document.activeElement !== acc) {
      acc.innerHTML = options;
      acc._html = options;
    }
    if (acc.value !== AL.account) acc.value = AL.account;
    if (grp.value !== AL.group) grp.value = AL.group;
  }
  const clockOf = d => d.toLocaleTimeString(locale(), lang === 'vi' ? { hour: '2-digit', minute: '2-digit', hourCycle: 'h23' } : { hour: 'numeric', minute: '2-digit' });
  function renderActivity() {
    const host = $('#alog-body');
    if (!host) return;
    syncActFilters();
    const d = AL.data;
    const exportBtn = $('#alog-sec [data-act="alog-export"]');
    if (exportBtn && !exportBtn.classList.contains('busy')) exportBtn.disabled = !d?.total;
    if (!d) {
      setHtml(host, AL.failed
        ? `<p class="alog-empty">${t('alog.error')} <button type="button" class="link" data-act="alog-retry">${t('srv.retry')}</button></p>`
        : '<div class="alog-empty"><span class="spin"></span></div>');
      return;
    }
    if (!d.entries.length) {
      setHtml(host, `<p class="alog-empty">${t(AL.account || AL.group ? 'alog.emptyFilter' : 'alog.empty')}</p>`);
      return;
    }
    const now = new Date(), today = now.toDateString();
    const yesterday = new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1).toDateString();
    let day = '', html = '';
    for (const e of d.entries) {
      const at = new Date(e.at), key = at.toDateString();
      if (key !== day) {
        if (day) html += '</ol>';
        day = key;
        const name = key === today ? t('alog.today') : key === yesterday ? t('alog.yesterday')
          : at.toLocaleDateString(locale(), { weekday: 'long', day: 'numeric', month: 'long', ...(at.getFullYear() !== now.getFullYear() ? { year: 'numeric' } : {}) });
        html += `<h4 class="alog-day">${esc(name)}</h4><ol class="alog-list">`;
      }
      const full = esc(at.toLocaleString(locale(), { dateStyle: 'full', timeStyle: 'medium' }));
      // Today's as "5 min ago" (kept current by patchTimes), the exact time on hover; older ones as the clock time.
      const when = key === today ? `<time datetime="${esc(e.at)}" data-ago="${esc(e.at)}" title="${full}">${ago(e.at)}</time>`
        : `<time datetime="${esc(e.at)}" title="${full}">${clockOf(at)}</time>`;
      const a = e.accountId ? byId(e.accountId) : null;
      const who = e.account
        ? `<span class="alog-acc" title="${esc(e.account)}">${a ? `<span class="avatar sm">${avatarInner(a)}</span>` : ''}<b>${esc(e.account)}</b></span>`
        : '<span class="alog-acc none" aria-hidden="true">—</span>';
      html += `<li class="alog-row ${esc(e.tone)}"><i class="dot" aria-hidden="true"></i>${when}${who}<span class="alog-text">${esc(e.text)}</span></li>`;
    }
    html += '</ol>';
    if (d.total > d.entries.length) {
      html += `<div class="alog-more"><button class="btn sm" data-act="alog-more">${t('alog.more')}</button><small>${t('alog.shown', { n: fmtNum(d.entries.length), total: fmtNum(d.total) })}</small></div>`;
    }
    setHtml(host, html);
  }
  document.addEventListener('change', e => {
    if (e.target.id !== 'alog-account' && e.target.id !== 'alog-group') return;
    AL.account = $('#alog-account').value;
    AL.group = $('#alog-group').value;
    AL.limit = AL_PAGE;
    loadActivity();
  });
  /** Saves what the filters show as a CSV file (the app asks where, with Windows' own Save dialog). */
  function exportActivity(btn) {
    return busy(btn, async () => {
      const n = await call('exportActivity', { account: AL.account || null, group: AL.group || null });
      if (n) toast(tn('alog.exported', n, { n: fmtNum(n) }), 'ok');
    });
  }

  /* ───────────────────────── game monitor ─────────────────────────
     A page can't show other programs' windows: the live pictures of the Roblox windows are the app's, laid over this
     tab's empty area. The page tells the app where that area is (CSS pixels and the pixel ratio) whenever it may have
     moved, and { hidden } whenever something of the page must show there: another tab, a dialog or menu, the lock
     screen, the window hidden. Windows draws the pictures above the whole page, so nothing could show through them. */
  const MON = { sent: null, frame: 0, dpr: null };
  function monitorHtml() {
    // In the browser preview there is no app to draw the pictures: say what goes here.
    const preview = !(window.chrome && window.chrome.webview);
    return `<header class="top"><div class="top-title"><h1>${t('mon.title')}</h1><p><b id="mon-count"></b><span id="mon-hint" hidden><span class="sep">·</span>${t('mon.hint')}</span></p></div></header>
      <div class="mon-area ${preview ? 'preview' : ''}" id="mon-area">${preview ? `<p>${t('mon.preview')}</p>` : ''}</div>`;
  }
  function monitorArea() {
    const area = $('#mon-area');
    if (!S.shell || S.view !== 'monitor' || !area || document.hidden || openModals > 0 || menuEl) return { hidden: true };
    const r = area.getBoundingClientRect();
    let w = r.width, h = r.height;
    // Toasts show at the bottom right: while they overlap the area, the pictures make room on whichever side keeps
    // more of them (usually the right, so a toast doesn't halve the pictures' height).
    const toasts = $('#toasts');
    const tr = toasts?.children.length ? toasts.getBoundingClientRect() : null;
    if (tr && tr.width && tr.left < r.right && tr.right > r.left && tr.top < r.bottom && tr.bottom > r.top) {
      const keepW = Math.max(0, tr.left - 10 - r.left), keepH = Math.max(0, tr.top - 10 - r.top);
      if (keepW * r.height >= keepH * r.width) w = keepW; else h = keepH;
    }
    if (w < 1 || h < 1) return { hidden: true };
    return { x: r.left, y: r.top, w, h, scale: devicePixelRatio };
  }
  /** One report per frame at most, and none that repeats the last; a hidden page says so at once (it gets no frames). */
  function syncMonitor() {
    const send = () => {
      MON.frame = 0;
      const area = monitorArea(), json = JSON.stringify(area);
      if (json === MON.sent) return;
      MON.sent = json;
      call('monitorArea', area, { silent: true });
    };
    if (document.hidden) {
      cancelAnimationFrame(MON.frame);
      return send();
    }
    MON.frame ||= requestAnimationFrame(send);
  }
  /** Follows the tab's area (a resize, the header wrapping, toasts coming and going) and the pixel ratio (another screen). */
  // Toasts coming and going one after another settle first, so the pictures aren't laid out again for each.
  let toastSettle = 0;
  const monitorObserver = new ResizeObserver(entries => {
    if (entries.every(e => e.target.id === 'toasts')) {
      clearTimeout(toastSettle);
      toastSettle = setTimeout(syncMonitor, 150);
    } else {
      syncMonitor();
    }
  });
  function watchMonitor() {
    monitorObserver.disconnect();
    if (S.view === 'monitor' && $('#mon-area')) {
      monitorObserver.observe($('#mon-area'));
      if ($('#toasts')) monitorObserver.observe($('#toasts'));
    }
    if (MON.dpr !== devicePixelRatio) {
      MON.dpr = devicePixelRatio;
      matchMedia(`(resolution: ${devicePixelRatio}dppx)`).addEventListener('change', watchMonitor, { once: true });
    }
    syncMonitor();
  }
  window.addEventListener('resize', syncMonitor);
  document.addEventListener('visibilitychange', syncMonitor);

  /* ───────────────────────── quick search (Ctrl+K) ─────────────────────────
     One box for everything on the page: accounts, presets, recent and pinned games, the tabs, Settings sections and
     the app's own actions, ranked by a small fuzzy match. paletteEntries() is the registry, built from the state each
     time the box opens; an entry is { id, text, hint, keywords, icon | img | avatar, run, alts }, so a new action is
     one line in its list. Enter runs the highlighted entry; an account's other actions (Play, Edit) are on
     Ctrl+Enter and Shift+Enter. */
  const PAL = { close: null, el: null, all: [], rows: [], index: 0, mx: -1, my: -1 };
  const PAL_MAX = 60;
  const PAL_ALT_KEYS = ['Ctrl+Enter', 'Shift+Enter'];

  /** Puts a game in the dock's game box (no server), as the recent-game chips do. */
  function putGame(placeId) {
    setView('accounts');
    $('#place').value = `https://www.roblox.com/games/${placeId}`;
    $('#job').value = '';
    resolveGame();
    scheduleSaveInputs();
  }

  /** Selects an account and brings its card into view; a search or filter that hides it is cleared first. */
  function revealAccount(id) {
    setView('accounts');
    if (!visible().some(a => a.id === id)) {
      S.query = '';
      $('#q').value = '';
      S.filter = 'all';
      S.group = '';
      renderChips();
      renderGrid();
    }
    selectOnly(id);
    const card = $('#grid')?._cards?.get(id)?.el;
    if (!card) return;
    setTabStop(card.parentElement, card);
    card.scrollIntoView({ block: 'nearest', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    card.focus({ preventScroll: true });
  }

  function paletteEntries() {
    const st = S.app.settings || {};
    const go = (view, then) => () => { setView(view); then?.(); };
    const openSection = id => () => openSettings(id);
    const toActivity = () => setStatsTab('activity');
    // The app's own actions, as its menus and buttons have them; "off" ones (nothing to act on) aren't offered.
    const actions = [
      { id: 'join', icon: 'userGo', text: t('jp.button'), keywords: ['follow', 'player', 'username', 'server'], run: () => joinPlayerModal() },
      { id: 'add', icon: 'userPlus', text: t('acc.add'), keywords: ['sign in', 'login', 'cookie'], run: () => addChooser() },
      { id: 'check', icon: 'shieldCheck', text: t('m.checkAll'), off: !accounts().length || S.app.checking, run: () => checkAccounts([]) },
      { id: 'refresh', icon: 'refresh', text: t('acc.refreshTip'), keywords: ['presence', 'status'], run: () => $('[data-act="refresh-presence"]')?.click() },
      { id: 'arrange', icon: 'screens', text: t('m.arrange'), keywords: ['tile', 'windows'], run: () => arrange(null) },
      { id: 'closeAll', icon: 'power', text: t('m.closeAll'), keywords: ['quit', 'exit'], off: !accounts().some(isLocal) && !S.metrics.robloxCount, run: () => closeAll() },
      { id: 'mini', icon: 'monitor', text: t(S.app.miniOpen ? 'sys.miniHide' : 'sys.mini'), run: () => call('toggleMini') },
      { id: 'selectAll', icon: 'check', text: t('m.selectAll'), off: !accounts().length, run: () => { setView('accounts'); selectAllVisible(); } },
      { id: 'order', icon: 'sort', text: t('m.launchOrder'), off: accounts().length < 2, run: () => launchOrderModal() },
      { id: 'preset', icon: 'layers', text: t('ps.save'), run: () => newPreset() },
      { id: 'backup', icon: 'cloud', text: t('bk.now'), keywords: ['backup'],
        run: () => S.app.backup?.folder ? busy(null, async () => { if (await call('backupNow') !== undefined) toast(t('bk.done'), 'ok'); }) : openSection('backup')() },
      { id: 'updates', icon: 'download', text: t('pal.checkUpdates'), run: () => { openSection('updates')(); $('#sec-updates [data-act="check-updates"]')?.click(); } },
      { id: 'export', icon: 'history', text: t('pal.exportActivity'), keywords: ['csv', 'log'], run: () => { setView('stats'); toActivity(); exportActivity(null); } }, // the list may not be loaded yet: the app knows
      { id: 'lang', icon: 'languages', text: lang === 'vi' ? 'Switch to English' : 'Chuyển sang tiếng Việt', keywords: ['language', 'ngôn ngữ'], run: () => call('setLanguage', { language: lang === 'vi' ? 'en' : 'vi' }) },
      { id: 'lock', icon: 'lock', text: t('side.lock'), keywords: ['password'], run: () => call('lock') },
    ].filter(x => !x.off).map(x => ({ ...x, kind: 'action' }));
    const tabs = [['accounts', 'users'], ['presets', 'layers'], ['monitor', 'screens'], ['stats', 'clock'], ['settings', 'sliders']]
      .map(([view, ic]) => ({ id: `tab:${view}`, kind: 'tab', icon: ic, text: t(`nav.${view}`), run: go(view) }));
    tabs.push({ id: 'tab:activity', kind: 'tab', icon: 'history', text: t('alog.title'), keywords: ['log', 'history', 'events'], run: go('stats', toActivity) });
    const presetList = presets().filter(p => presetAccounts(p).length).map(p => ({
      id: `ps:${p.id}`, kind: 'preset', icon: 'layers', text: p.name, hint: presetSub(p), keywords: [p.gameName], run: () => runPreset(p.id),
    }));
    const seen = new Set();
    const games = [...(st.pinnedGames || []).map(g => [g, 'pal.pinned']), ...(st.recentGames || []).map(g => [g, 'pal.recent'])]
      .filter(([g]) => !seen.has(g.placeId) && seen.add(g.placeId))
      .map(([g, hint]) => ({ id: `game:${g.placeId}`, kind: 'game', img: g.iconUrl, icon: 'gamepad', text: g.name, hint: t(hint), keywords: [String(g.placeId)], run: () => putGame(g.placeId) }));
    const list = accounts().map(a => ({
      id: `acc:${a.id}`, kind: 'account', avatar: a, text: a.label,
      hint: [a.username && a.username !== a.label ? `@${a.username}` : '', a.group].filter(Boolean).join(' · '),
      // The real names find an account while names are hidden too, as the search box does.
      keywords: [a.real?.label, a.real?.username, a.real?.displayName, a.username, a.displayName, a.group, a.userId ? String(a.userId) : ''],
      run: () => revealAccount(a.id),
      alts: [
        { text: t('m.play'), run: () => { revealAccount(a.id); launch([a.id], false); } },
        { text: t('pal.edit'), run: () => editAccount(a.id) },
      ],
    }));
    // Settings pages as the page has them (a page added later comes along), their groups' and rows' titles to find them by.
    const sections = [...document.querySelectorAll('#view-settings .set-page')].map(pg => ({
      id: `set:${pg.dataset.page}`, kind: 'setting', iconHtml: $(`#view-settings [data-set-page="${pg.dataset.page}"] .i`)?.outerHTML,
      text: pg.querySelector('.set-page-title')?.textContent || pg.dataset.page,
      keywords: [...pg.querySelectorAll('.set-row .t, .set-grp-title')].map(x => x.textContent.trim()),
      run: openSection(pg.dataset.page),
    }));
    return [...actions, ...tabs, ...presetList, ...games, ...list, ...sections];
  }

  /**
   * How well a query matches a text, both folded (fold: no accents, lower case): the start of the text beats the
   * start of a word, which beats the middle of one, then every word somewhere, then the letters in order (fewer gaps
   * first). -1 for no match.
   */
  const PAL_WORD = /[\s@_\-.·(/:[\]]/;
  const wordStart = (s, j) => j === 0 || PAL_WORD.test(s[j - 1]);
  function fuzzyScore(q, s) {
    if (!s) return -1;
    const at = s.indexOf(q);
    if (at === 0) return 1000 - Math.min(s.length, 200);
    if (at > 0) return (wordStart(s, at) ? 800 : 600) - Math.min(at, 100);
    const words = q.split(/\s+/).filter(Boolean);
    if (words.length > 1 && words.every(w => s.includes(w))) return 500;
    // Letters in order, the first one starting a word: "clall" finds "Close all", "anti" doesn't find "sang tieng".
    const letters = q.replace(/\s+/g, '');
    let first = s.indexOf(letters[0]);
    while (first >= 0 && !wordStart(s, first)) first = s.indexOf(letters[0], first + 1);
    if (first < 0) return -1;
    let score = 306, from = first + 1, last = first;
    for (const c of letters.slice(1)) {
      const j = s.indexOf(c, from);
      if (j < 0) return -1;
      if (j !== last + 1) score -= 5 + Math.min(j - from, 15);
      if (wordStart(s, j)) score += 6;
      last = j;
      from = j + 1;
    }
    return Math.max(1, score);
  }
  /** The entries for a query, best first (ties keep the registry's order); the text counts most, keywords only whole. */
  function rankPalette(query) {
    const q = fold(query.trim());
    if (!q) {
      PAL.more = Math.max(0, PAL.all.length - PAL_MAX);
      return PAL.all.slice(0, PAL_MAX);
    }
    const hits = PAL.all.map((e, i) => {
      const byText = fuzzyScore(q, fold(e.text));
      const byOther = Math.max(-1, ...[e.hint, ...(e.keywords || [])].filter(Boolean).map(k => {
        const s = fuzzyScore(q, fold(k));
        return s >= 500 ? s - 300 : -1;
      }));
      return { e, i, score: Math.max(byText, byOther) };
    }).filter(x => x.score > 0).sort((a, b) => b.score - a.score || a.i - b.i);
    // Only the best ones are listed: the rest are counted, so a long list doesn't read as all there is.
    PAL.more = Math.max(0, hits.length - PAL_MAX);
    return hits.slice(0, PAL_MAX).map(x => x.e);
  }
  function paletteLead(e) {
    if (e.avatar) return `<span class="avatar sm">${avatarInner(e.avatar)}</span>`;
    if (e.img) return `<span class="pal-ic"><img src="${esc(e.img)}" alt=""></span>`;
    return `<span class="pal-ic">${e.iconHtml || icon(e.icon || 'search')}</span>`;
  }
  function renderPalette() {
    const list = PAL.el?.querySelector('.pal-list'), input = PAL.el?.querySelector('.pal-input');
    if (!list || !input) return;
    if (!PAL.rows.length) {
      list.innerHTML = `<div class="pal-empty">${t('pal.none', { q: esc(input.value.trim()) })}</div>`;
      input.removeAttribute('aria-activedescendant');
      return;
    }
    // The kind of each row ("Action", "Account"…) only when the list mixes kinds; a list of one kind needs no label.
    const mixed = new Set(PAL.rows.map(e => e.kind)).size > 1;
    list.innerHTML = PAL.rows.map((e, i) => {
      const on = i === PAL.index;
      // The other actions show on the highlighted row only, with their keys.
      const alts = on && e.alts ? e.alts.map((x, j) => `<button type="button" class="pal-alt" data-alt="${j}" tabindex="-1">${esc(x.text)}<kbd>${PAL_ALT_KEYS[j]}</kbd></button>`).join('') : '';
      return `<div class="pal-row ${on ? 'on' : ''}" role="option" id="pal-${i}" aria-selected="${on}" data-pi="${i}">${paletteLead(e)}<span class="pal-txt"><b>${esc(e.text)}</b>${e.hint ? `<small>${esc(e.hint)}</small>` : ''}</span>${alts}${mixed ? `<em class="pal-cat">${t(`pal.cat.${e.kind}`)}</em>` : ''}</div>`;
    }).join('') + (PAL.more ? `<div class="pal-more">${esc(t('pal.more', { n: PAL.more }))}</div>` : '');
    input.setAttribute('aria-activedescendant', `pal-${PAL.index}`);
    list.querySelector('.pal-row.on')?.scrollIntoView({ block: 'nearest' });
  }
  function runPaletteEntry(e, alt = -1) {
    const run = alt >= 0 ? e?.alts?.[alt]?.run : e?.run;
    if (!run) return;
    PAL.close?.(); // first: the focus goes back, then the entry may open a dialog of its own or move it
    run();
  }
  function openPalette() {
    if (PAL.close || !S.shell || !S.app || S.app.locked) return;
    PAL.all = paletteEntries();
    PAL.rows = rankPalette('');
    PAL.index = 0;
    modal({
      title: t('pal.title'),
      body: `<div class="pal">
          <label class="inwrap pal-search">${icon('search')}<input class="pal-input" type="text" autocomplete="off" spellcheck="false" maxlength="100"
            placeholder="${esc(t('pal.placeholder'))}" aria-label="${esc(t('pal.title'))}" role="combobox" aria-expanded="true" aria-autocomplete="list" aria-controls="pal-list"></label>
          <div class="pal-list" id="pal-list" role="listbox" aria-label="${esc(t('pal.title'))}"></div>
          <div class="pal-foot" aria-hidden="true"><span><kbd>↑</kbd><kbd>↓</kbd>${t('pal.move')}</span><span><kbd>Enter</kbd>${t('pal.open')}</span><span><kbd>Esc</kbd>${t('pal.close')}</span></div>
        </div>`,
      onMount: ({ el, close }) => {
        PAL.el = el;
        PAL.close = close;
        el.querySelector('.modal').classList.add('palette');
        const input = el.querySelector('.pal-input'), list = el.querySelector('.pal-list');
        input.addEventListener('input', () => {
          PAL.rows = rankPalette(input.value);
          PAL.index = 0;
          renderPalette();
        });
        input.addEventListener('keydown', e => {
          const n = PAL.rows.length;
          if (['ArrowDown', 'ArrowUp', 'PageDown', 'PageUp'].includes(e.key)) {
            e.preventDefault();
            if (!n) return;
            const step = e.key === 'ArrowDown' ? 1 : e.key === 'ArrowUp' ? -1 : e.key === 'PageDown' ? 8 : -8;
            PAL.index = e.key.startsWith('Page') ? Math.max(0, Math.min(n - 1, PAL.index + step)) : (PAL.index + step + n) % n;
            renderPalette();
          } else if (e.key === 'Enter' && !e.isComposing) {
            e.preventDefault();
            runPaletteEntry(PAL.rows[PAL.index], e.ctrlKey ? 0 : e.shiftKey ? 1 : -1);
          } else if (e.key === 'Tab') {
            e.preventDefault(); // the box keeps the focus: the arrows walk the list
          } else if (e.key === 'Escape') {
            // modal() leaves Esc to a box whose list is open (aria-expanded), as this one's always is: closed here,
            // and not passed on to the page (where Esc would clear the selection).
            e.stopPropagation();
            close();
          }
        });
        // The focus stays in the box whatever is clicked in the list.
        list.addEventListener('mousedown', e => e.preventDefault());
        list.addEventListener('mousemove', e => {
          // Only a real move: rows scrolled under a resting pointer (the arrow keys) don't take the highlight.
          if (e.clientX === PAL.mx && e.clientY === PAL.my) return;
          PAL.mx = e.clientX;
          PAL.my = e.clientY;
          const row = e.target.closest('.pal-row');
          if (!row || +row.dataset.pi === PAL.index) return;
          PAL.index = +row.dataset.pi;
          renderPalette();
        });
        list.addEventListener('click', e => {
          const row = e.target.closest('.pal-row');
          if (row) runPaletteEntry(PAL.rows[+row.dataset.pi], e.target.closest('.pal-alt') ? +e.target.closest('.pal-alt').dataset.alt : -1);
        });
        renderPalette();
        input.focus();
      },
      onClose: () => {
        PAL.close = PAL.el = null;
        PAL.all = PAL.rows = [];
      },
    });
  }

  /* ───────────────────────── join a player ─────────────────────────
     The selected accounts open into the server a Roblox user is in. The app looks the name up and asks Roblox where
     that person is with the selected accounts' own sign-ins; when Roblox won't say (privacy settings, offline, in
     Studio, on the website) the dialog says exactly that and nothing opens. Names looked up are kept in the vault. */
  function joinPlayerModal() {
    const ids = [...S.selected];
    if (!ids.length) return toast(t('t.selectFirst'), 'warn');
    let gone = false;
    const say = (html, tone = 'info') => {
      const box = $('#jp-msg');
      if (!box) return;
      box.hidden = !html;
      box.className = `jp-msg ${tone}`;
      box.innerHTML = html;
    };
    const fillRecent = () => {
      const list = S.app.settings?.recentPlayers || [];
      const host = $('#jp-recent');
      if (!host) return;
      host.hidden = !list.length;
      host.innerHTML = list.length ? `<small>${t('jp.recent')}</small>${list.map(name => `<span class="jp-chip"><button type="button" class="jp-pick" data-jp="${esc(name)}">${esc(name)}</button><button type="button" class="jp-x" data-jp-x="${esc(name)}" title="${esc(t('jp.forget'))}" aria-label="${esc(t('jp.forgetN', { name }))}">${icon('x')}</button></span>`).join('')}` : '';
    };
    const go = async (btn, close) => {
      const box = $('#jp-name'), input = box.value.trim();
      if (!input) {
        say(t('jp.needName'), 'error');
        return box.focus();
      }
      // One launch at a time: while one runs, the join wouldn't start (and the lookup would be for nothing).
      const launchBusy = () => inFlight.has('launch') || !!S.app.launching;
      if (launchBusy()) return say(esc(t('jp.launching')), 'warn');
      say(`<span class="spin"></span>${t('jp.looking', { name: esc(input) })}`);
      let r;
      try {
        r = await busy(btn, () => bridge.call('findPlayer', { input, ids }));
      } catch (e) {
        if (!gone) say(esc(e.message), 'error');
        return box.isConnected && box.focus();
      }
      if (gone || !r || S.app.locked) return;
      const who = r.displayName && r.displayName !== r.name ? `${r.displayName} (@${r.name})` : r.name || input;
      if (r.status !== 'found') {
        say(t(`jp.${r.status}`, { name: `<b>${esc(who)}</b>` }), r.status === 'notFound' ? 'error' : 'warn');
        fillRecent(); // a name Roblox knows is remembered even when it can't be joined now
        box.focus();
        return box.select();
      }
      if (launchBusy()) return say(esc(t('jp.launching')), 'warn'); // one started meanwhile (a scheduled preset)
      close();
      // Toasts escape their text themselves.
      toast(tn('jp.found', ids.length, { name: who, game: r.game || `Place ${r.placeId}` }), 'info');
      joinServer(ids, r.placeId, r.jobId);
    };
    modal({
      title: t('jp.title'),
      busy: true,
      enterSubmits: true,
      body: `<div class="form jp">
          <p class="modal-text">${tn('jp.note', ids.length)}</p>
          <label>${t('jp.label')}<input class="field" id="jp-name" maxlength="100" autocomplete="off" spellcheck="false" placeholder="${esc(t('jp.placeholder'))}"></label>
          <div class="jp-recent" id="jp-recent" role="group" aria-label="${esc(t('jp.recent'))}" hidden></div>
          <div class="jp-msg" id="jp-msg" role="status" aria-live="polite" hidden></div>
        </div>
        <p class="fine">${icon('shield')}<span>${t('jp.privacy')}</span></p>`,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        { text: t('jp.go'), kind: 'primary', onClick: ({ btn, close }) => go(btn, close) },
      ],
      onMount: ({ el }) => {
        const box = el.querySelector('#jp-name');
        el.querySelector('#jp-recent').addEventListener('click', async e => {
          const forget = e.target.closest('[data-jp-x]');
          if (forget) {
            await call('forgetPlayer', { name: forget.dataset.jpX });
            fillRecent();
            return box.focus();
          }
          const pick = e.target.closest('[data-jp]');
          if (!pick) return;
          box.value = pick.dataset.jp;
          el.querySelector('.modal-foot .btn.primary').click(); // a name from the list is looked up at once
        });
        box.addEventListener('input', () => say(''));
        fillRecent();
        setTimeout(() => box.focus(), 30);
      },
      onClose: () => { gone = true; },
    });
  }
  /** Opens the accounts into the server found, through the usual launch: its warnings ask first, as Play's do. */
  async function joinServer(ids, placeId, jobId, force = false) {
    if (inFlight.has('launch')) return;
    inFlight.add('launch');
    let r;
    try {
      r = await call('joinServer', { ids, placeId, jobId, force });
    } finally {
      inFlight.delete('launch');
    }
    return launchDone(r, () => joinServer(ids, placeId, jobId, true));
  }

  /* ───────────────────────── events ───────────────────────── */
  // A game icon that fails to load (gone from Roblox's CDN, offline) shows the same placeholder as a game without one,
  // not the browser's broken-image mark. Inline onerror isn't allowed by the page's CSP, so one listener catches them.
  document.addEventListener('error', e => {
    const img = e.target;
    if (!(img instanceof HTMLImageElement)) return;
    // An account's picture that doesn't load (offline, gone from the CDN): its initials, as without one.
    if (img.parentElement?.classList.contains('avatar') && img.dataset.ini) {
      const ini = document.createElement('span');
      ini.className = 'initials';
      ini.style.cssText = img.style.cssText;
      ini.textContent = img.dataset.ini;
      img.replaceWith(ini);
      return;
    }
    if (!img.closest('.sgame, .game-row, .ps-game, .recent, #game-thumb, .pv-item, .activity, .pal-ic')) return;
    const ph = document.createElement(img.closest('.recent') ? 'i' : 'span');
    ph.className = img.closest('.recent') ? 'ph' : img.matches('.gicon, .pv-icon') ? img.className : 'gph';
    if (!img.closest('.recent')) ph.innerHTML = icon('gamepad');
    img.replaceWith(ph);
  }, true);
  document.addEventListener('change', e => { if (e.target.id === 'side-multi') setMulti(e.target); });
  document.addEventListener('click', e => {
    const el = e.target.closest('[data-act]');
    if (!el || !S.app) return;
    const id = el.closest('.card')?.dataset.id;
    switch (el.dataset.act) {
      case 'card': return onCardClick(id, e);
      case 'play-one': return launch([id], false);
      case 'close-one': return closeIds([id]);
      case 'cancel-rejoin': return call('cancelRejoin', { id }).then(r => r !== undefined && toast(t('rj.cancelled'), 'ok'));
      case 'copy-pid': return copy(id, 'pid');
      case 'edit': return editAccount(id);
      case 'card-menu': {
        const r = el.getBoundingClientRect();
        return cardMenu(id, r.left, r.bottom + 6);
      }
      // The app answers with a state push in the new language.
      case 'theme':
        clearTimeout(themeTimer); // a click wins over a pick still waiting to be saved
        S.themePending = null;
        if (el.dataset.id !== S.app.theme) return call('setTheme', { theme: el.dataset.id });
        document.documentElement.dataset.theme = S.app.themeApplied || S.app.theme; // back from a preview
        return syncSettings();
      case 'lang': return el.dataset.lang !== lang && call('setLanguage', { language: el.dataset.lang });
      case 'view': return setView(el.dataset.view);
      case 'stats-period': return setStatsPeriod(+el.dataset.days);
      case 'stats-tab': return setStatsTab(el.dataset.tab);
      case 'lock': return call('lock');
      case 'add-choose': return addChooser();
      case 'add-password': leaveChooser(el); return passwordLogin();
      case 'add-phone': leaveChooser(el); return quickLogin();
      case 'import-vault': leaveChooser(el); return importVault();
      case 'relogin': { const a = byId(id); return quickLogin(a?.realm, a?.label); }
      case 'add-browser': leaveChooser(el); return addBrowser(el);
      case 'add-cookie': leaveChooser(el); return accountModal(null);
      case 'add-menu':
        return menuAt(el, [
          { icon: 'qr', text: t('m.addCode'), run: () => quickLogin() },
          { icon: 'globe', text: t('m.addBrowser'), run: () => addBrowser(null) },
          { icon: 'cookie', text: t('m.addCookie'), run: () => accountModal(null) },
          '-',
          { icon: 'folder', text: t('m.importVault'), run: importVault },
        ], 320);
      case 'tools-menu':
        return menuAt(el, [
          { icon: 'refresh', text: t('m.checkAll'), disabled: !accounts().length || S.app.checking, run: () => checkAccounts([]) },
          { icon: 'check', text: t('m.selectAll'), hint: 'Ctrl+A', disabled: !accounts().length, run: selectAllVisible },
          { icon: 'screens', text: t('m.arrange'), run: () => arrange($('[data-act=tools-menu]')) },
          { icon: 'sort', text: t('m.launchOrder'), disabled: accounts().length < 2, run: launchOrderModal },
          '-',
          { icon: 'power', text: t('m.closeAll'), danger: true, run: () => closeAll() },
          { icon: 'x', text: t('m.killAll'), danger: true, run: () => closeAll(true) },
        ], 280);
      case 'layout':
        S.layout = el.dataset.l === 'list' ? 'list' : 'cards';
        call('setSettings', { layout: S.layout }, { silent: true });
        renderChips();
        renderGrid();
        return;
      case 'sort-menu':
        return menuAt(el, ['name', 'status', 'recent', 'order'].map(k => ({
          icon: 'sort', text: t(`sort.${k}`), checked: S.sort === k,
          run: () => {
            S.sort = k;
            call('setSettings', { sort: k }, { silent: true });
            renderChips();
            renderGrid();
          },
        })), 200);
      // Account groups: one menu next to Sort ("Group: All"), apart from the status chips.
      case 'group-menu': {
        const pick = g => () => {
          S.group = g;
          renderChips();
          renderGrid();
          $('#scroll').scrollTop = 0;
        };
        return menuAt(el, [
          { icon: 'tag', text: t('chip.all'), checked: !S.group, run: pick('') },
          '-',
          ...accountGroups().map(g => ({ icon: 'tag', text: esc(g), checked: S.group === g, run: pick(g) })),
        ], 240);
      }
      case 'refresh-presence':
        // refreshPresence replies null on success, { skipped } when one was already running; call() yields undefined after showing an error.
        return busy(el, () => call('refreshPresence')).then(r => {
          if (r?.skipped) toast(t('t.refreshBusy'), 'info');
          else if (r !== undefined) toast(t('t.refreshed'), 'ok');
        });
      case 'filter':
        S.filter = el.dataset.f;
        renderChips();
        renderGrid();
        $('#scroll').scrollTop = 0;
        return;
      case 'launch': return launch([...S.selected], false);
      case 'open-app': return launch([...S.selected], true);
      case 'arrange': return arrange(el);
      case 'close-sel': return closeIds([...S.selected]);
      case 'close-all': return closeAll();
      // A double click would open the window and close it again: the second click is dropped.
      case 'mini': return busy(el, () => call('toggleMini'));
      case 'rc-pick': return pickRobloxConfigAccounts();
      case 'pick-launcher': return busy(el, () => call('pickLauncher')); // a double click opens one file window
      case 'cancel-launch': return call('cancelLaunch');
      case 'preset-menu': return presetMenu(el);
      case 'pv-new': return newPreset();
      case 'multi-help': return multiHelp();
      case 'ru-notes': return call('openReleaseNotes', { version: el.dataset.v });
      case 'ru-hide': return call('hideRobloxClosed');
      case 'ru-open': return openSettings('rbxupd');
      case 'select-all': return selectAllVisible();
      case 'clear-search':
        S.query = '';
        $('#q').value = '';
        S.filter = 'all';
        S.group = '';
        renderChips();
        renderGrid();
        return $('#q').focus();
      case 'stats-retry': return busy(el, loadStats);
      case 'alog-retry': return busy(el, loadActivity);
      case 'alog-more':
        AL.limit += AL_PAGE;
        return busy(el, loadActivity);
      case 'alog-export': return exportActivity(el);
      case 'join-player': return joinPlayerModal();
      case 'clear-sel':
        S.selected.clear();
        return renderSelection();
      case 'recent-remove':
        e.stopPropagation();
        return call('removeRecent', { placeId: +el.dataset.place });
      case 'recent':
        $('#place').value = `https://www.roblox.com/games/${el.dataset.place}`;
        $('#job').value = '';
        resolveGame();
        scheduleSaveInputs();
        return;
      case 'open-data': return call('openDataFolder');
      case 'bk-pick': return busy(el, () => call('pickBackupFolder'));
      case 'bk-use': return call('setBackup', { folder: el.dataset.path });
      case 'bk-open': return call('openBackupFolder');
      case 'bk-now': return busy(el, async () => { if (await call('backupNow') !== undefined) toast(t('bk.done'), 'ok'); });
      case 'bk-restore': return restoreBackup();
      case 'bk-import': return importVault();
      // First-run screen: the chosen file becomes the vault; the app then shows the unlock screen.
      case 'restore-backup': return busy(el, () => call('restoreBackup'));
      case 'check-updates': return busy(el, () => call('checkUpdates'));
      case 'whats-new': return busy(el, async () => {
        const list = await call('whatsNew');
        if (list?.length) showWhatsNew(list); else if (list) toast(t('wn.none'), 'info');
      });
      // Bloxkeep closes and reopens as the new version; Roblox windows stay open.
      // busy(): a second click while it closes mustn't start a second update.
      case 'apply-update': return busy(el, () => call('applyUpdate'));
      case 'browse-games': return P.open && P.mode === 'browse' ? closePicker() : openPicker('browse');
      case 'picker-tab':
        P.tab = el.dataset.tab;
        P.index = -1;
        return renderPicker();
      case 'pick-game': return pickGame(pickerRows()[+el.dataset.i]);
      case 'servers': return openServers();
      case 'pin-game':
        e.stopPropagation();
        return pinGame(pickerRows()[+el.dataset.i]);
    }
  });

  // The second click of a double click that closed a dialog or menu lands on what was under it (a card's tick, its
  // Sign in button): swallowed, with its mousedown, rather than acting on something the person never aimed at.
  for (const type of ['mousedown', 'click'])
    document.addEventListener(type, e => {
      if (e.detail > 1 && performance.now() - lastOverlayClose < 500 && !e.target.closest?.('.overlay:not(.out), .menu')) {
        e.preventDefault();
        e.stopImmediatePropagation();
      }
    }, true);
  // Likewise an Enter still held (or pressed twice) after it closed a dialog or the palette: the focus is back on the
  // button that opened it (Edit), or on the page with accounts chosen (Play).
  document.addEventListener('keydown', e => {
    if (e.key === 'Enter' && performance.now() - lastOverlayClose < (e.repeat ? 1000 : 300) && !e.target.closest?.('.overlay:not(.out), .menu')) {
      e.preventDefault();
      e.stopImmediatePropagation();
    }
  }, true);

  document.addEventListener('dblclick', e => {
    // The second click of a double click that closed a dialog or menu lands on the card behind it: not a Play.
    if (performance.now() - lastOverlayClose < 500) return;
    const card = e.target.closest('.card');
    // The tick box, buttons and links on a card do their own thing: a quick double click on them isn't a Play.
    if (card && !e.target.closest('button, .check, a, input')) {
      selectOnly(card.dataset.id);
      launch([card.dataset.id], false);
    }
  });

  document.addEventListener('contextmenu', e => {
    if (e.target.closest('input, textarea')) return;
    e.preventDefault();
    const card = e.target.closest('.card');
    if (card) cardMenu(card.dataset.id, e.clientX, e.clientY);
  });

  document.addEventListener('keydown', e => {
    if (menuEl) {
      if (e.key === 'Escape') return closeMenu(true);
      if (e.key === 'Tab') return closeMenu(true); // then Tab moves on from the button as usual
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const items = [...menuEl.querySelectorAll('button:not(:disabled)')];
        const i = items.indexOf(document.activeElement);
        // Nothing in the menu focused yet: Down starts at the first item, Up at the last.
        items[i < 0 ? (e.key === 'ArrowDown' ? 0 : items.length - 1) : (i + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length]?.focus();
        return;
      }
    }
    // Ctrl+K: quick search, from anywhere on the page (also while typing in a box); again closes it. Not over another
    // dialog, whose form it would hide, and not on the lock screen.
    if (e.ctrlKey && !e.altKey && !e.shiftKey && !e.metaKey && e.key.toLowerCase() === 'k' && S.shell && !S.app?.locked) {
      e.preventDefault();
      if (PAL.close) PAL.close();
      else if (!openModals) openPalette();
      return;
    }
    if (!S.shell || openModals > 0) return;
    const typing = /^(INPUT|TEXTAREA)$/.test(document.activeElement?.tagName);
    const key = e.key.toLowerCase();
    if (e.key === 'Escape') {
      if (typing) return document.activeElement.blur();
      if (S.selected.size) {
        S.selected.clear();
        renderSelection();
      }
      return;
    }
    if (e.ctrlKey && key === 'f') {
      e.preventDefault();
      // The Presets tab has its own search (shown once there is a preset).
      if (S.view === 'presets' && !$('#pv-tools').hidden) return $('#pv-q').focus();
      // So does Settings.
      if (S.view === 'settings') { $('#set-q').focus(); $('#set-q').select(); return; }
      setView('accounts');
      $('#q').focus();
      return;
    }
    // Ctrl+L (like a browser's address bar) or "/" goes to the game box, however far down the card list the focus is.
    if ((e.ctrlKey && key === 'l') || (e.key === '/' && !typing && !e.ctrlKey && !e.altKey)) {
      e.preventDefault();
      if (S.view !== 'accounts') setView('accounts');
      $('#place').focus();
      $('#place').select();
      return;
    }
    if (typing || S.view !== 'accounts') return;
    const card = e.target.closest?.('.card');
    if (e.key === ' ' && card && !e.target.closest('button')) {
      e.preventDefault(); // Space on a focused card selects it, like a click
      return onCardClick(card.dataset.id, e);
    }
    if (card && !e.target.closest('button') && ['ArrowRight', 'ArrowLeft', 'ArrowDown', 'ArrowUp', 'Home', 'End'].includes(e.key)) {
      e.preventDefault();
      return moveCardFocus(card, e.key);
    }
    if (e.ctrlKey && key === 'a') {
      e.preventDefault();
      selectAllVisible();
    } else if (e.key === 'Delete' && S.selected.size) {
      removeIds([...S.selected]);
    } else if (e.key === 'F2' && S.selected.size === 1) {
      editAccount([...S.selected][0]);
    } else if (e.key === 'Enter' && S.selected.size && !e.defaultPrevented && !e.target.closest('button, input, select, textarea, a, .overlay')) {
      // Enter plays from a card or the page itself. Not when a box or a dialog already used it and moved the focus
      // here (a game picked from the list, a quick-search entry run): that Enter wasn't meant as Play.
      launch([...S.selected], false);
    }
  });

  /* ───────────────────────── wiring ───────────────────────── */
  function applyState(st) {
    if (!st) return;
    if (st.language && st.language !== lang && DICT[st.language]) {
      lang = st.language;
      document.documentElement.lang = lang;
      // Rebuild everything in the new language; inputs come back from settings / page state.
      S.shell = false;
      S.lockMode = null;
      // What was being typed in Settings (a search, a new master password) and the sections left open aren't settings:
      // put back into the rebuilt page.
      const typed = ['#set-q', '#pw-cur', '#pw-new', '#pw-new2'].map(sel => [sel, $(sel)?.value]).filter(([, v]) => v);
      const open = [...document.querySelectorAll('details.set-more')].map(d => d.open);
      setTimeout(() => {
        for (const [sel, v] of typed) {
          const f = $(sel);
          if (f && !f.value) { f.value = v; f.dispatchEvent(new Event('input', { bubbles: true })); }
        }
        document.querySelectorAll('details.set-more').forEach((d, i) => { if (open[i]) d.open = true; });
      });
      // Picked with a language button: the focus goes to that button in the rebuilt page, not to the page itself.
      if (document.activeElement?.closest?.('[data-act="lang"]')) {
        setTimeout(() => {
          if (document.activeElement === document.body || !document.activeElement)
            [...document.querySelectorAll(`[data-act="lang"][data-lang="${lang}"]`)].find(b => b.offsetParent)?.focus();
        });
      }
    }
    // The activity list's sentences name accounts as the app wrote them: once names are hidden or shown again, the kept
    // list goes (it isn't drawn again with the old names) and a fresh one is asked for.
    const namesFlipped = !!S.app && !!S.app.hideNames !== !!st.hideNames;
    S.app = st;
    if (namesFlipped) {
      Object.assign(AL, { data: null, failed: false });
      AL.seq++;
      if (S.shell && S.view === 'stats' && !st.locked) { renderActivity(); loadActivity(); }
    }
    maskNames(st); // before anything renders a name
    if (st.themeApplied && !S.themePending) document.documentElement.dataset.theme = st.themeApplied;
    if (st.locked) {
      // Toasts from before the lock may name accounts: they don't stay on the lock screen.
      if (S.shell) $('#toasts')?.replaceChildren();
      S.bootDone = S.bootTimedOut = false;
      S.viewPrefsLoaded = false; // the next vault brings its own layout and sort
      stopBoot();
      S.selected.clear();
      S.metrics = { accounts: [], robloxCount: 0, totalMb: 0, systemMb: 0 };
      // Play figures name accounts: they go with the lock, and a reply still on its way is dropped.
      ST.data = null;
      ST.failed = false;
      ST.seq++;
      // So does the activity list (and its account filter: the next vault has other accounts).
      Object.assign(AL, { data: null, failed: false, account: '', limit: AL_PAGE });
      AL.seq++;
      renderLock();
      return;
    }
    // The list's look is kept with the other settings (the browser's own storage is lost when GPU drawing is switched
    // or the app is reinstalled). Older versions kept it in the browser: carry that over once.
    if (st.settings && !S.viewPrefsLoaded) {
      S.viewPrefsLoaded = true;
      if (st.settings.layout || st.settings.sort) {
        if (st.settings.layout) S.layout = st.settings.layout;
        if (st.settings.sort) S.sort = st.settings.sort;
      } else if (S.layout !== 'cards' || S.sort !== 'name') {
        call('setSettings', { layout: S.layout, sort: S.sort }, { silent: true });
      }
    }
    const ids = new Set(st.accounts.map(a => a.id));
    for (const id of [...S.selected]) if (!ids.has(id)) S.selected.delete(id);
    // Right after unlocking, a loading screen (like other apps have at start) with the app's real steps, so the
    // accounts appear with their status and avatars instead of "unknown". Never longer than BOOT_MAX.
    if (!S.shell && !S.bootDone) {
      if (st.boot && !S.bootTimedOut) {
        showBoot(st.boot);
        return;
      }
      if ($('#boot')) {
        // Let the bar reach the end before the accounts appear.
        if (!S.bootFinishing) {
          S.bootFinishing = true;
          finishBoot(() => {
            S.bootFinishing = false;
            S.bootDone = true;
            applyState(S.app);
          });
        }
        return;
      }
    }
    S.bootDone = true;
    stopBoot();
    if (!S.shell) buildShell();
    update();
    if (st.whatsNew && !S.whatsNewShown) {
      S.whatsNewShown = true;
      showWhatsNew(st.whatsNew.releases, () => call('seenWhatsNew'));
    }
  }

  /* ───────────────────────── loading screen ─────────────────────────
     A progress bar driven by the app's real steps ({ stage, percent }). While a step runs the bar creeps towards the
     next step's mark but never past it, and jumps when the step finishes — the way browsers and Discord do it.
     Reusable for any long start: add a stage here with its label and where it ends. */
  const BOOT_MAX = 8000;
  const BOOT_STEPS = {
    status: { label: 'boot.status', end: 0.8 },
    avatars: { label: 'boot.avatars', end: 1 },
  };
  function showBoot(boot) {
    if (!$('#boot')) {
      const app = $('#app');
      app._html = null;
      app.innerHTML = `<div class="boot" id="boot" role="progressbar" aria-valuemin="0" aria-valuemax="100"><img src="${LOGO}" alt=""><b>Bloxkeep</b>
        <div class="boot-bar"><i id="boot-fill"></i></div><span id="boot-text"></span></div>`;
      S.lockMode = null;
      S.bootValue = 0.05;
    }
    const step = BOOT_STEPS[boot.stage] || { label: 'boot.loading', end: 1 };
    S.bootValue = Math.max(S.bootValue, boot.percent);
    S.bootCeil = Math.max(S.bootValue, step.end - 0.04);
    setText($('#boot-text'), t(step.label));
    paintBoot();
    S.bootTrickle ||= setInterval(() => {
      S.bootValue += (S.bootCeil - S.bootValue) * 0.05;
      paintBoot();
    }, 200);
    // Set once: later pushes while loading must not push the deadline back.
    S.bootTimer ||= setTimeout(() => {
      S.bootTimer = 0;
      S.bootTimedOut = true;
      applyState(S.app);
    }, BOOT_MAX);
  }
  function paintBoot() {
    const fill = $('#boot-fill');
    if (!fill) return;
    fill.style.transform = `scaleX(${Math.min(1, S.bootValue).toFixed(3)})`;
    $('#boot').setAttribute('aria-valuenow', Math.round(S.bootValue * 100));
  }
  function finishBoot(then) {
    stopBoot();
    S.bootValue = 1;
    paintBoot();
    S.bootFinish = setTimeout(() => {
      S.bootFinish = 0;
      then();
    }, 260);
  }
  function stopBoot() {
    clearInterval(S.bootTrickle);
    clearTimeout(S.bootTimer);
    clearTimeout(S.bootFinish); // locking during the last step must not mark the next unlock as loaded
    S.bootTrickle = S.bootTimer = S.bootFinish = 0;
    S.bootFinishing = false;
  }

  bridge.on('state', applyState);
  bridge.on('toast', m => toast(m.text, m.kind));
  bridge.on('progress', p => {
    S.progress = p?.text || null;
    if (S.shell) renderDock();
  });
  bridge.on('metrics', m => {
    S.metrics = m;
    if (S.shell) renderMetrics();
  });
  const refreshTimes = () => {
    if (!S.shell || document.hidden) return;
    patchTimes();
    renderMetrics();
  };
  document.addEventListener('visibilitychange', refreshTimes);
  // Relative times ("5 min ago") drift; refresh them once a minute while the window is visible.
  setInterval(refreshTimes, 60_000);

  /**
   * The browser is closed while the window stays minimised (and recreated when GPU drawing is switched); the app
   * keeps the view in memory meanwhile, so the page comes back exactly as it was. Nothing is written to disk.
   */
  /** @param restart the page itself asks for a browser restart next (the app otherwise only keeps a view sent from the background). */
  function saveView(restart = false) {
    if (!S.shell) return;
    const view = { selected: [...S.selected], filter: S.filter, group: S.group, query: S.query, view: S.view, scroll: $('#scroll')?.scrollTop || 0, presetQuery: PV.query };
    bridge.call('keepView', { view: JSON.stringify(view), restart }).catch(() => {});
  }
  document.addEventListener('visibilitychange', () => { if (document.hidden) saveView(); });
  // Bloxkeep is about to install an update in the background and start again: the new page comes back like this one.
  bridge.on('saveView', () => saveView(true));

  bridge.call('getState').then(first => {
    let saved = null;
    try { saved = first?.view ? JSON.parse(first.view) : null; } catch { saved = null; }
    if (saved && first.state && !first.state.locked) {
      S.selected = new Set(saved.selected || []);
      // An older saved view kept a group as a chip ("g:Farm"): it is the Group menu's choice now.
      S.filter = String(saved.filter || 'all').startsWith('g:') ? 'all' : saved.filter || 'all';
      S.group = typeof saved.group === 'string' ? saved.group : String(saved.filter || '').startsWith('g:') ? saved.filter.slice(2) : '';
      S.query = saved.query || '';
      S.view = ['presets', 'settings', 'stats', 'monitor'].includes(saved.view) ? saved.view : 'accounts';
      PV.query = typeof saved.presetQuery === 'string' ? saved.presetQuery : '';
      S.introShown = true;
    }
    const hadShell = S.shell;
    applyState(first?.state);
    if (saved && hadShell && S.shell) {
      // The shell was already built from an earlier push: apply the kept view to it.
      $('#q').value = S.query;
      $('#pv-q').value = PV.query;
      setView(S.view);
      update();
    }
    if (saved?.scroll && $('#scroll')) $('#scroll').scrollTop = saved.scroll;
    pageShown();
  }, e => toast(e.message, 'error'));

  // A page hidden in the background is drawn again when the window comes back; the app covers that moment too.
  document.addEventListener('visibilitychange', () => { if (!document.hidden) pageShown(); });
  /**
   * The app covers the window (its logo on start, a picture of the old page after the browser was closed in the
   * background) until the page is on screen: tell it once a frame (with the visible avatars, briefly waited for) is drawn.
   */
  function pageShown() {
    const imgs = [...document.querySelectorAll('#scroll img')].slice(0, 40).map(img => img.decode().catch(() => {}));
    Promise.race([Promise.all(imgs), new Promise(r => setTimeout(r, 400))])
      .then(() => requestAnimationFrame(() => requestAnimationFrame(() => bridge.call('pageShown').catch(() => {}))));
  }
})();
