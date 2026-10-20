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
    book: '<path d="M2 4h6a4 4 0 0 1 4 4v13a3 3 0 0 0-3-3H2z"/><path d="M22 4h-6a4 4 0 0 0-4 4v13a3 3 0 0 1 3-3h7z"/>',
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
    image: '<rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="9" cy="9" r="2"/><path d="m21 15-3.1-3.1a2 2 0 0 0-2.8 0L6 21"/>',
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
    // Robux: a hexagon with a square inside, Bloxkeep's own drawing of the currency's shape.
    robux: '<path d="M12 2.6 20.2 7.3v9.4L12 21.4l-8.2-4.7V7.3z"/><rect x="9" y="9" width="6" height="6" rx="1"/>',
    tag: '<path d="M20.6 13.4 13.4 20.6a2 2 0 0 1-2.8 0L3 13V3h10l7.6 7.6a2 2 0 0 1 0 2.8z"/><circle cx="7.5" cy="7.5" r="1.5"/>',
    chevronUp: '<path d="m6 15 6-6 6 6"/>',
    grip: '<circle cx="9" cy="6" r="1.3" fill="currentColor"/><circle cx="15" cy="6" r="1.3" fill="currentColor"/><circle cx="9" cy="12" r="1.3" fill="currentColor"/><circle cx="15" cy="12" r="1.3" fill="currentColor"/><circle cx="9" cy="18" r="1.3" fill="currentColor"/><circle cx="15" cy="18" r="1.3" fill="currentColor"/>',
    calendar: '<rect x="3" y="4" width="18" height="17" rx="2"/><path d="M3 9h18M8 2v4M16 2v4"/>',
    screens: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M12 3v14M2 10h20M8 21h8M12 17v4"/>',
    bell: '<path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9"/><path d="M10.3 21a1.94 1.94 0 0 0 3.4 0"/>',
    message: '<path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z"/>',
    wrench: '<path d="M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z"/>',
  };
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
  /**
   * The accounts as last shown, by id: one whose data didn't change keeps its object, so the React parts (which skip
   * what they drew from the same object) draw only the cards that changed with a state push, not every card.
   */
  let shownAccounts = new Map();
  function maskNames(st) {
    if (!st.accounts) return;
    const next = new Map();
    // Copies: what the bridge handed over stays as it came (the preview's fake bridge hands over its own objects).
    st.accounts = st.accounts.map((a, i) => {
      // What the copy shown depends on: the account's own data and, while names are hidden, its place and the language.
      const key = `${st.hideNames ? `${lang}|${i}` : ''}|${JSON.stringify(a)}`;
      const was = shownAccounts.get(a.id);
      let shown = was?.key === key ? was.account : null;
      if (!shown) {
        const real = a.real || { label: a.label, username: a.username, displayName: a.displayName, alias: a.alias };
        shown = { ...a, real, ...(st.hideNames ? { label: t('hide.account', { n: i + 1 }), username: '', displayName: '', alias: '' } : real) };
      }
      next.set(a.id, { key, account: shown });
      return shown;
    });
    shownAccounts = next;
  }

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

  /* ───────────────────────── toasts ───────────────────────── */
  /**
   * A message in the corner. @param action optional { label, run } shown as a button; the toast then stays a little
   * longer. Drawn by React (ui/src/Overlays.tsx Toasts): how long each stays, three at most, the same one again.
   */
  let toastUi = null; // mounted once the page has started (below, before getState)
  function toast(text, kind = 'info', action = null) {
    // Not over the lock screen that came up by itself: an answer still on its way could name an account.
    if (!text || S.app?.screenLock) return;
    toastUi?.api.show(text, kind, action);
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
  function modal({ title, body, actions = [], onMount, onClose, dismissable = true, busy: keepsPage = false, enterSubmits = false, askClose = null, top = false, onRemoved,
    boxClass, bodyClass, label, footError }) {
    closeMenu();
    // Gets the focus back on close. Opened from a busy button (What's new, Run's "Open anyway?"), which lost the focus
    // when it was turned off: that button.
    const opener = document.activeElement && document.activeElement !== document.body ? document.activeElement
      : busyFocused?.isConnected ? busyFocused : null;
    // A card's button that opened it may be rebuilt meanwhile (Edit → save): the same button on the new card then.
    const openerCard = opener?.closest('.card')?.dataset.id, openerAct = opener?.dataset.act;
    const openerHost = opener?.parentElement?.closest('[id]:not(#app, main, .view, #layer)'), openerCls = opener?.classList[0];
    const el = document.createElement('div');
    // top: a box whose content changes height after it opens (steps, lists that load) sits near the top instead of
    // in the middle, so a change moves only its bottom edge, not what is being read.
    el.className = top ? 'overlay at-top' : 'overlay';
    const titleId = `mdl-${++modalSeq}`;
    // The frame is React's (ui/src/Overlays.tsx ModalFrame); its body too when given as one of its parts (an ask, the
    // errors…), else a React part goes into .modal-body in onMount.
    const unmountFrame = window.BloxkeepUI.modalFrame(el, uiHost(), {
      title, titleId, body: body || null, actions: actions.map(a => ({ text: a.text, kind: a.kind })),
      act: (i, btn) => actions[i].onClick?.({ ...api, btn }), close: () => tryClose(), boxClass, bodyClass, label, footError,
    });
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
      setTimeout(() => { el.remove(); onRemoved?.(); unmountFrame(); }, 150);
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
        const leave = await confirmBox(t('common.discardQ'), { title: t('common.discardTitle'), ok: t('common.discard'), danger: true, remember: 'discard' });
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
    const api = {
      el,
      close: () => { if (!held) close(); },
      /** Runs a save with the dialog kept open meanwhile (Cancel, the X and Esc wait for it). */
      hold: async job => { held++; try { return await job(); } finally { held--; } },
    };
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

  /**
   * @param danger a destructive answer: the focus starts on Cancel, so a stray Enter doesn't delete or restore.
   * @param remember the question offers "Don't ask again" (the app's SkippableConfirms): once ticked with a yes, it
   * isn't asked any more and its yes is assumed.
   */
  function confirmBox(message, { title = t('common.confirm'), ok = 'OK', danger = false, remember = null } = {}) {
    if (remember && S.app?.skipConfirm?.includes(remember)) return Promise.resolve(true);
    return new Promise(resolve => {
      let answer = false;
      const m = modal({
        title,
        body: window.BloxkeepUI.ask(message, !!remember),
        actions: [
          { text: t('common.cancel'), onClick: ({ close }) => close() },
          { text: ok, kind: danger ? 'danger' : 'primary', onClick: ({ el, close }) => {
            answer = true;
            if (remember && el.querySelector('#dont-ask')?.checked) call('skipConfirm', { key: remember });
            close();
          } },
        ],
        onClose: () => resolve(answer),
      });
      setTimeout(() => m.el.querySelector(`.modal-foot .btn:${danger ? 'first' : 'last'}-child`)?.focus(), 30);
    });
  }

  let menuEl = null, menuOpener = null, menuUnmount = null;
  /** @param refocus true (from the keyboard) puts the focus back on the button that opened the menu. */
  function closeMenu(refocus = false) {
    if (!menuEl) return;
    menuEl.remove();
    // After the click that chose an item has been handled (the menu's items are React's).
    const unmount = menuUnmount;
    queueMicrotask(() => unmount?.());
    menuEl = menuUnmount = null;
    lastOverlayClose = performance.now();
    syncToastLayer();
    if (menuOpener?.hasAttribute('aria-haspopup')) menuOpener.setAttribute('aria-expanded', 'false');
    if (refocus === true && menuOpener?.isConnected) menuOpener.focus();
    menuOpener = null;
  }
  /**
   * Items: each an icon, text, title, hint, danger, checked, disabled and run, or '-' for a line; their words as they
   * read (not HTML). split: the items before the first line scroll in a list of their own (the dock's presets).
   */
  function openMenu(items, x, y, { cls = '', split = false } = {}) {
    closeMenu();
    menuOpener = document.activeElement !== document.body ? document.activeElement : null;
    const byKeyboard = !!menuOpener?.matches(':focus-visible');
    if (menuOpener?.hasAttribute('aria-haspopup')) menuOpener.setAttribute('aria-expanded', 'true');
    const el = document.createElement('div');
    el.className = cls ? `menu ${cls}` : 'menu';
    el.setAttribute('role', 'menu');
    $('#layer').append(el);
    // The items are React's (ui/src/Overlays.tsx MenuItems).
    menuUnmount = window.BloxkeepUI.menu(el, uiHost(), items, (i, kb) => {
      closeMenu(kb); // picked with the keyboard: the focus goes back to the button first
      items[i].run();
    }, split);
    menuEl = el;
    syncToastLayer();
    const r = el.getBoundingClientRect();
    el.style.left = `${Math.max(8, Math.min(x, innerWidth - r.width - 8))}px`;
    el.style.top = `${Math.max(8, Math.min(y, innerHeight - r.height - 8))}px`;
    if (byKeyboard) el.querySelector('button:not(:disabled)')?.focus();
  }
  const menuAt = (btn, items, width, opts) => {
    const r = btn.getBoundingClientRect();
    openMenu(items, width ? r.right - width : r.left, r.bottom + 6, opts);
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

  /** A master password's strength (weak / OK / strong); only a guide, any 6+ characters are accepted. */
  const strengthScore = v => [v.length >= 8, v.length >= 12, /[a-z]/.test(v) && /[A-Z]/.test(v), /\d/.test(v), /[^A-Za-z0-9]/.test(v)].filter(Boolean).length;
  const strengthLevel = v => { const s = strengthScore(v); return s <= 1 ? 'weak' : s <= 3 ? 'ok' : 'strong'; };

  /* ───────────────────────── lock screen ───────────────────────── */
  /** "15 minutes", "1 hour", "2 hours": how long the PC went unused before Bloxkeep locked itself. */
  const autoLockText = n => n >= 60 ? (n === 60 ? t('lock.autoHour1') : t('lock.autoHours', { n: n / 60 })) : t('lock.autoMin', { n });
  function renderLock() {
    const mode = S.app.vaultExists ? 'unlock' : 'create';
    if (S.lockMode === mode) return; // keep what the user is typing
    // Rebuilt in another language (same screen): what was typed stays.
    const form0 = $('#lock-form');
    const kept = form0?.dataset.mode === mode ? { pw1: $('#pw1')?.value || '', pw2: $('#pw2')?.value || '' } : null;
    S.lockMode = mode;
    // Locked while in use (not at start-up): the lock screen says what waits for the unlock.
    if (S.shell) S.lockedInUse = true;
    S.shell = false;
    closeMenu();
    closeAllModals();
    syncMonitor();
    takeDownUiParts();
    // The screen is React's (ui/src/Lock.tsx); the vault it opens comes back here. Locked while in use (not at
    // start-up), it says what waits for the unlock.
    const notice = mode === 'create' ? '' : S.app.screenLock ? t('lock.auto', { t: autoLockText(S.app.screenLock.minutes) }) : S.lockedInUse ? t('lock.paused') : '';
    $('#app').innerHTML = '';
    uiParts.push(window.BloxkeepUI.lock($('#app'), uiHost(), { mode, notice, kept }, applyState));
  }

  /* ───────────────────────── shell ───────────────────────── */
  /** The parts of the main page React draws (ui/src): mounted once it is built, taken down before it is replaced. */
  let uiParts = [];
  function takeDownUiParts() {
    uiParts.forEach(unmount => unmount());
    uiParts = [];
  }
  function buildShell() {
    S.shell = true;
    S.lockMode = null;
    S.lockedInUse = false;
    // Dialogs stay open (the lock screen already closed them): a language change from elsewhere mustn't throw away a
    // form being typed in. They keep their old language until closed.
    takeDownUiParts();
    // The page is React's (ui/src/Shell.tsx): the tabs, Settings' pages, Presets and Statistics; the parts it marks
    // data-vanilla are still drawn here (update).
    $('#app').innerHTML = '';
    const shell = window.BloxkeepUI.shell($('#app'), uiHost(), {
      pages: SET_PAGES.map(([id, ic]) => ({ id, icon: ic, title: setPageTitle(id) })), page: setPage, preview: !(window.chrome && window.chrome.webview),
    });
    uiParts.push(shell.unmount);
    statsUi = { api: shell.stats };

    const st = S.app.settings;
    $('#place').value = st.lastPlaceInput || '';
    $('#job').value = st.lastJobId || '';
    $('#q').value = S.query;
    // Filter once typing pauses, not on every key.
    let searchTimer = 0;
    $('#q').addEventListener('input', e => {
      S.query = e.target.value;
      clearTimeout(searchTimer);
      searchTimer = setTimeout(pageChanged, 120);
    });
    $('#place').addEventListener('input', onPlaceInput);
    // Into the box, the list opens whatever is in it, like a browser's address bar: a game or link already there gets
    // the tabs (recent, pinned, saved…) with its text selected, so typing replaces it and searches; words typed earlier
    // get their results again. No more emptying the box to reach the list.
    let selectOnUp = false;
    $('#place').addEventListener('mousedown', () => { selectOnUp = document.activeElement !== $('#place'); });
    // The click that focused the box would put the caret where it landed; a drag that selected part of it stays.
    $('#place').addEventListener('mouseup', e => {
      const box = $('#place');
      if (selectOnUp && box.value && box.selectionStart === box.selectionEnd) { e.preventDefault(); box.select(); }
      selectOnUp = false;
    });
    $('#place').addEventListener('focus', () => {
      const value = $('#place').value.trim();
      if (value && !looksLikeTarget(value)) return onPlaceInput();
      openPicker('browse');
      if (value && !selectOnUp) $('#place').select();
    });
    $('#place').addEventListener('keydown', onPlaceKey);
    // Tab on to the Job ID box (or anywhere else on the page) closes the list; clicks are handled on mousedown below.
    $('#place').addEventListener('focusout', e => {
      if (e.relatedTarget && !e.relatedTarget.closest('#picker, #game-thumb')) closePicker();
    });
    // Choosing in the list keeps the focus in the game box (the rows and tabs would otherwise take it).
    $('#picker').addEventListener('mousedown', e => { if (e.target.closest('#picker')) e.preventDefault(); });
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
    setView(S.view);
    resolveGame();
  }

  function update() {
    pageChanged();
    syncSettings();
    syncMultiHelp();
  }

  /** Typed passwords don't stay in the page once Settings is left (Settings › Security, React, empties its boxes). */
  function clearPwForm() { uiSignal('clearPassword'); }

  /** A theme picked with the arrow keys shows at once; the app saves it once the keys rest. */
  let themeTimer = 0;
  function previewTheme(id) {
    clearTimeout(themeTimer);
    setThemePending(id);
    if (id !== 'system') document.documentElement.dataset.theme = id;
    themeTimer = setTimeout(async () => {
      if (id !== S.app.theme) await call('setTheme', { theme: id });
      if (S.themePending !== id) return;
      setThemePending(null);
      // The state that came with the answer was held back while this pick waited ("Sync with Windows" has no
      // preview of its own): the theme the app applied shows now.
      document.documentElement.dataset.theme = S.app.themeApplied || S.app.theme;
    }, 400);
  }

  /** The theme being previewed, or null: Settings › Appearance (React) marks it as the chosen one. */
  function setThemePending(id) {
    S.themePending = id;
    uiListeners.forEach(f => f());
  }

  /**
   * Settings › Appearance › Custom: app.css's colour tokens worked out from the person's two colours. Dark or light by
   * the background, as Themes.cs decides it (relative luminance under 0.179: white words read better); surfaces a step
   * from it the way Dark and Light have theirs; words in its own hue; buttons and switches in the accent. With a
   * picture the page goes behind glass like the scenery themes, the background colour lying over it as much as asked.
   */
  function customVars(ct) {
    const rgb = h => [1, 3, 5].map(i => parseInt(h.slice(i, i + 2), 16));
    const hex = c => '#' + c.map(v => Math.round(Math.max(0, Math.min(255, v))).toString(16).padStart(2, '0')).join('');
    const rgba = (c, a) => `rgba(${c.map(Math.round).join(', ')}, ${a})`;
    const mix = (a, b, t) => a.map((v, i) => v + (b[i] - v) * t);
    const lum = c => {
      const [r, g, b] = c.map(v => (v /= 255) <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4);
      return .2126 * r + .7152 * g + .0722 * b;
    };
    const W = [255, 255, 255], K = [0, 0, 0];
    const bg = rgb(ct.background), ac = rgb(ct.accent);
    const dark = lum(bg) < .179, words = dark ? W : K;
    const surface = dark ? mix(bg, W, .05) : mix(bg, W, .75);
    const raised = dark ? mix(bg, W, .07) : mix(bg, W, .9);
    const card = dark ? mix(bg, W, .035) : mix(bg, W, .8), cardHover = dark ? mix(bg, W, .06) : mix(bg, W, .92);
    const accentHover = mix(ac, dark ? W : K, .14), accentInk = lum(ac) < .179 ? '#ffffff' : '#0b0d12';
    const vars = {
      '--bg': hex(bg), '--bg-2': hex(dark ? mix(bg, W, .02) : mix(bg, K, .03)),
      '--surface': hex(surface), '--surface-2': hex(dark ? mix(bg, W, .08) : mix(bg, W, .5)),
      '--surface-3': hex(dark ? mix(bg, W, .13) : mix(bg, K, .07)), '--raised': hex(raised),
      '--field': hex(dark ? mix(bg, K, .3) : mix(bg, W, .9)), '--card': hex(card), '--card-hover': hex(cardHover),
      '--text': hex(mix(words, bg, .08)), '--text-2': hex(mix(words, bg, .28)), '--muted': hex(mix(words, bg, .4)), '--dim': hex(mix(words, bg, .55)),
      '--accent': hex(ac), '--accent-hover': hex(accentHover), '--accent-ink': accentInk,
      '--inv': hex(ac), '--inv-hover': hex(accentHover), '--inv-ink': accentInk,
    };
    if (ct.picture) {
      const fade = Math.max(0, Math.min(90, ct.fade ?? 45)) / 100;
      Object.assign(vars, {
        '--backdrop': `url("${ct.picture}") center / cover no-repeat, ${hex(bg)}`, '--blur': 'blur(18px) saturate(140%)',
        '--bg': rgba(bg, fade), '--bg-2': rgba(bg, Math.min(.95, fade + .25)),
        '--surface': rgba(surface, .72), '--surface-2': rgba(words, .08), '--surface-3': rgba(words, .15), '--raised': rgba(raised, .94),
        '--field': dark ? 'rgba(0, 0, 0, .3)' : 'rgba(255, 255, 255, .6)', '--card': rgba(card, .62), '--card-hover': rgba(cardHover, .72),
      });
    }
    return { tone: dark ? 'dark' : 'light', vars };
  }
  const CUSTOM_VARS = Object.keys(customVars({ background: '#000000', accent: '#000000', picture: 'x' }).vars);
  const customNow = () => S.customPreview || S.app?.customTheme
    || (() => { try { return JSON.parse(localStorage.getItem('customTheme')); } catch { return null; } })();
  /** The custom theme on the page while it is the one shown (by data-theme, whoever set it); gone again when it isn't. */
  function applyCustomTheme() {
    const root = document.documentElement, ct = root.dataset.theme === 'custom' ? customNow() : null;
    if (!ct) {
      delete root.dataset.tone;
      CUSTOM_VARS.forEach(k => root.style.removeProperty(k));
      return;
    }
    const { tone, vars } = customVars(ct);
    root.dataset.tone = tone;
    CUSTOM_VARS.forEach(k => k in vars ? root.style.setProperty(k, vars[k]) : root.style.removeProperty(k));
    // So the next start shows it from the first frame, before the app's state comes.
    try { localStorage.setItem('customTheme', JSON.stringify(ct)); } catch { /* the page still shows it now */ }
  }
  new MutationObserver(applyCustomTheme).observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] });
  applyCustomTheme();

  /** A colour or the fade being changed in Settings: shown at once, saved once the changes rest. */
  let customTimer = 0;
  function previewCustom(change) {
    S.customPreview = { ...customNow(), ...change };
    clearTimeout(themeTimer);
    setThemePending(null);
    document.documentElement.dataset.theme = 'custom';
    applyCustomTheme();
    uiListeners.forEach(f => f());
    clearTimeout(customTimer);
    customTimer = setTimeout(async () => {
      const c = S.customPreview;
      await call('setCustomTheme', { background: c.background, accent: c.accent, fade: c.fade });
      if (S.customPreview !== c) return;
      S.customPreview = null;
      applyCustomTheme();
      uiListeners.forEach(f => f());
    }, 300);
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
    pageChanged(); // the sidebar's tabs
    $('#view-accounts').hidden = view !== 'accounts';
    $('#view-presets').hidden = view !== 'presets';
    if (view === 'presets') uiSignal('presetsShown');
    $('#view-settings').hidden = view !== 'settings';
    $('#view-stats').hidden = view !== 'stats';
    $('#view-monitor').hidden = view !== 'monitor';
    $(`#view-${view}`)?.classList.add('seen'); // keeps its layout once it's left (app.css: .view.seen[hidden])
    uiSignal(view === 'stats' ? 'statsShown' : 'statsHidden');
    watchMonitor();
    // Synced folders to offer under Backup are looked for when Settings opens, not with every state.
    if (view === 'settings' && !S.app.locked) call('backupSuggestions', {}, { silent: true });
  }

  /** The account groups in use, sorted by name. */
  const accountGroups = () => [...new Set(accounts().map(a => a.group).filter(Boolean))].sort(compareText);
  /**
   * A group or status chip that no longer has a place (its accounts gone, renamed) goes back to all: the Accounts tab's
   * bar (React) shows the groups in use, and the chips that apply (Needs sign-in, Banned, Open here only when some do).
   */
  function syncFilters() {
    if (S.group && !accountGroups().includes(S.group)) S.group = '';
    const list = accounts();
    const shown = { all: true, ingame: true, online: true, offline: true, invalid: list.some(a => a.cookieValid === false), banned: list.some(a => a.banned), local: list.some(isLocal) };
    if (!shown[S.filter]) S.filter = 'all';
  }

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
      // Every time: React may have drawn a button anew (Play → Close). Set as an attribute even where it is the
      // default (0), as it always was.
      for (const b of c.querySelectorAll('button')) if (b.getAttribute('tabindex') !== String(want)) b.tabIndex = want;
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

  /** Writing identical text still forces a layout pass, so skip no-op updates. */
  function setText(el, text) {
    if (el && el.textContent !== text) el.textContent = text;
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

  /* ───────────────────────── settings ───────────────────────── */
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
  /* Settings › Discord: what a summary can list. */
  const DC_PARTS = ['total', 'accounts', 'games', 'sessions', 'kicks'];

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

  /** Settings › Discord's buttons that open this page's dialogs (the page itself is React's). */
  function wireDiscord() {
    $('#view-settings').addEventListener('click', e => {
      const btn = e.target.closest('[data-dc]');
      if (!btn) return;
      switch (btn.dataset.dc) {
        case 'paste': return discordLinkModal();
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
      body: window.BloxkeepUI.discordLink(),
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

  /**
   * A shown webhook link (Settings › Discord's Show) goes back to its masked form: when Settings or its page is left, or
   * the window loses focus.
   */
  const hideDiscordLink = () => uiSignal('hideDiscordLink');
  window.addEventListener('blur', () => hideDiscordLink());

  async function removeDiscordLink() {
    if (await confirmBox(t('dc.removeConfirm'), { title: t('dc.removeTitle'), ok: t('dc.remove'), danger: true })) call('setDiscordWebhook', { url: '' });
  }

  /** A picker's row for an account: its label, @username, and its picture as the cards draw it (PickList.tsx). */
  const pickAccount = (a, checked) => ({ value: a.id, checked, label: a.label, sub: a.username ? `@${a.username}` : '', avatar: a });

  /** Settings › Discord › Includes: what the summary lists (at least one), as a short list like the accounts picker. */
  function pickDiscordParts() {
    const parts = new Set(S.app.discord?.summary?.parts || []);
    let ui = null;
    modal({
      title: t('dc.partsTitle'),
      body: '',
      onMount: ({ el }) => {
        ui = window.BloxkeepUI.pickList(el.querySelector('.modal-body'), uiHost(), t('dc.partsText'),
          DC_PARTS.map(p => ({ value: p, checked: parts.has(p), label: t(`dc.part.${p}`) })));
      },
      onRemoved: () => ui?.unmount(),
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ close, btn }) => {
            const list = ui.list.values();
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
    let ui = null;
    modal({
      title: t('dc.accTitle'),
      body: '',
      onMount: ({ el }) => { ui = window.BloxkeepUI.pickList(el.querySelector('.modal-body'), uiHost(), t('dc.accText'), list.map(a => pickAccount(a, chosen.has(a.id)))); },
      onRemoved: () => ui?.unmount(),
      actions: [
        { text: t('dc.allAccounts'), onClick: async ({ close }) => { if (await call('setDiscord', { summaryAccounts: [] }) !== undefined) close(); } },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ close, btn }) => {
            const ids = ui.list.values();
            if (await busy(btn, () => call('setDiscord', { summaryAccounts: ids })) !== undefined) close();
          },
        },
      ],
    });
  }

  /**
   * What the React parts (ui/src, built into ui.js) use of this page: its words, icons and commands, and where their
   * buttons lead. Made afresh for each part (a language change closes every dialog first).
   */
  const uiListeners = new Set();
  /**
   * The page's own state the React parts draw (host.page): the tab shown, the live figures. pageChanged() after any of
   * it changes; the snapshot is made again only when asked for after that.
   */
  const pageListeners = new Set(), metricsListeners = new Set();
  let pageSnap = null;
  /** Drawn at once (flushSync): what runs next may look for what was drawn (a card to focus, a scroll to reset). */
  function pageChanged() {
    syncFilters();
    pageSnap = null;
    if (pageListeners.size) window.BloxkeepUI.flush(() => pageListeners.forEach(f => f()));
  }
  let lastSnap = null;
  const sameSet = (a, b) => a.size === b.size && [...a].every(x => b.has(x));
  /** The parts that didn't change keep their object, so a part that shows only those isn't drawn again. */
  const pageState = () => pageSnap ||= lastSnap = {
    view: S.view, selected: lastSnap && sameSet(lastSnap.selected, S.selected) ? lastSnap.selected : new Set(S.selected),
    query: S.query, filter: S.filter, group: S.group, layout: S.layout, sort: S.sort,
    introShown: S.introShown, game: S.game, progress: S.progress, hasGame: !!$('#place')?.value.trim(), picker: { ...P },
  };
  /** What app.js tells the React parts by name (host.on): "hideDiscordLink", "clearPassword". */
  const uiSignals = new Map();
  const uiSignal = name => uiSignals.get(name)?.forEach(f => f());
  const uiHost = () => ({
    t, tn, tIn: (l, key) => DICT[l]?.[key] ?? DICT.en[key] ?? key, lang, locale: locale(), logo: LOGO, iconPaths: name => ICONS[name] || '', call,
    busy, toast, saveView, setMulti, ago,
    themes: THEMES, themePending: () => S.themePending ?? null, previewTheme,
    customTheme: () => S.customPreview || S.app?.customTheme || null, customVars, previewCustom,
    discordLink: thenTurnOn => discordLinkModal(thenTurnOn),
    on(name, f) {
      if (!uiSignals.has(name)) uiSignals.set(name, new Set());
      uiSignals.get(name).add(f);
      return () => uiSignals.get(name).delete(f);
    },
    ask: (cmd, args) => bridge.call(cmd, args), strength: strengthLevel,
    focusKey, refocus,
    presetQuery: () => PV.query, setPresetQuery: q => { PV.query = q; },
    scheduleText, nextRunText, presetOptionsText, runPreset, deletePreset, editPreset: presetModal,
    weekdayName, fmtNum, looksLikeTarget, store, copyField: copy,
    gameListsCached: () => P.lists,
    async loadGameLists() {
      const r = await call('gameLists', {}, { silent: true });
      if (r) P.lists = r; // shared with the dock's picker
      return r;
    },
    searchCached(q) {
      const c = searchCache.get(q);
      return c && Date.now() - c.at < SEARCH_TTL ? c.rows : null;
    },
    rememberSearch,
    settingsDrawn() {
      const q = $('#set-q');
      if (q?.value) searchSettings(q.value);
    },
    state: () => S.app,
    subscribe: f => { uiListeners.add(f); return () => uiListeners.delete(f); },
    page: pageState,
    subscribePage: f => { pageListeners.add(f); return () => pageListeners.delete(f); },
    metrics: () => S.metrics,
    subscribeMetrics: f => { metricsListeners.add(f); return () => metricsListeners.delete(f); },
    fmtMb, duration, compareText, visible, setTabStop,
    introShown() { S.introShown = true; },
    go(target) {
      if (target.startsWith('settings:')) openSettings(target.slice(9));
      else if (target === 'guide') call('openGuide');
      else if (target === 'web') call('openWebsiteBlank');
      else if (target === 'feedback') feedbackModal();
      else if (target === 'github') call('openGitHub');
    },
  });

  /**
   * Release notes: shown once after an update, and any time from Settings › Updates. The dialog is drawn here; what is
   * in it (headline cards, New / Improved / Fixed, older releases folded) by React (ui/src/WhatsNew.tsx).
   */
  function showWhatsNew(releases, onClose) {
    if (!releases?.length) return;
    const [latest] = releases;
    const date = new Date(`${latest.date}T00:00:00`);
    const day = isNaN(date) ? '' : date.toLocaleDateString(locale(), { day: 'numeric', month: 'long', year: 'numeric' });
    let unmount = null;
    modal({
      title: window.BloxkeepUI.whatsNewTitle(`Bloxkeep ${latest.version}${day ? ` · ${day}` : ''}`),
      body: '',
      actions: [{ text: t('wn.ok'), kind: 'primary', onClick: ({ close }) => close() }],
      onMount: ({ el, close }) => { unmount = window.BloxkeepUI.whatsNew(el.querySelector('.modal-body'), uiHost(), releases, close); },
      onClose,
      onRemoved: () => unmount?.(),
    });
  }

  const THEMES = {
    basic: ['system', 'light', 'paper', 'gray', 'dim', 'dark', 'midnight', 'black', 'contrast', 'contrastlight'],
    scenery: ['aurora', 'sunset', 'city', 'space', 'moon', 'mist', 'snow', 'desert', 'meadow', 'halong', 'terraces', 'hoian', 'fuji', 'jungle', 'underwater', 'skyislands'],
    color: ['frost', 'ocean', 'teal', 'forest', 'mocha', 'amber', 'grape', 'indigo', 'crimson', 'lava', 'neon', 'candy', 'matcha', 'mint', 'sky', 'lavender', 'peach', 'sakura'],
    style: ['macos', 'macosdark', 'gnome', 'ubuntu', 'kde', 'retro', 'macclassic', 'terminal', 'gameboy', 'arcade', 'blueprint', 'newspaper', 'notebook', 'chalkboard', 'brutal', 'cyberpunk'],
    season: ['lunar', 'valentine', 'spring', 'flametree', 'summer', 'kites', 'rainy', 'school', 'midautumn', 'autumn', 'skylanterns', 'halloween', 'winter', 'christmas', 'newyear'],
    custom: ['custom'],
  };
  function syncSettings() {
    const st = S.app.settings;
    if (!st || !$('#set-q')) return;
    syncSettingsNav();
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

  /** The watch found the lock changed in the build Multi-Roblox meets next, and it hasn't worked on it here since. */
  function watchWarns() {
    const w = S.app.robloxUpdates?.watch;
    return !!w && ['changed', 'renamed', 'gone'].includes(w.check) && !w.worked;
  }

  /** A dot on a page in the list when something there needs a look: Roblox closed windows, Discord stopped sending. */
  function syncSettingsNav() {
    const need = { roblox: !!S.app.robloxUpdates?.closed || !!S.app.multi?.enabled && watchWarns(), discord: !!S.app.discord?.stopped };
    for (const dot of document.querySelectorAll('#view-settings .set-nav-dot')) dot.hidden = !need[dot.closest('[data-set-page]').dataset.setPage];
  }

  /** Settings › Backup › Restore…: confirm, then the app asks for the file, keeps a copy of the current data and locks. */
  async function restoreBackup() {
    if (await confirmBox(t('bk.restoreConfirm'), { title: t('bk.restoreConfirmTitle'), ok: t('bk.restoreBtn'), danger: true })) await call('restoreBackup');
  }

  /* ───────────────────────── game picker ─────────────────────────
     Typing a name searches Roblox; going into the box (empty, or with a game or link in it) offers recent, pinned,
     saved, favourite and popular games. */
  const P = { open: false, mode: 'browse', tab: 'recent', lists: null, results: [], query: '', index: -1, seq: 0, loading: false };
  const TABS = ['recent', 'pinned', 'saved', 'favorites', 'popular'];
  const looksLikeTarget = v => /^\d+$/.test(v) || /roblox\.com|roblox:\/\//i.test(v);
  // Saved servers come with the app's state (they change as they are saved), the other lists with gameLists.
  const savedServers = () => S.app?.settings?.savedServers || [];
  const tabRows = k => (k === 'saved' ? savedServers() : P.lists?.[k]) || [];
  const pickerRows = () => (P.mode === 'search' ? P.results : tabRows(P.tab)) || [];

  function openPicker(mode) {
    // Already showing the lists (the box emptied right after a click opened them): not asked for again.
    const showing = P.open && P.mode === 'browse';
    P.open = true;
    P.mode = mode;
    P.index = -1;
    pageChanged();
    if (mode === 'browse' && !showing) loadGameLists();
  }
  function closePicker() {
    if (!P.open) return;
    P.open = false;
    pageChanged();
  }
  async function loadGameLists() {
    P.loading = !P.lists;
    pageChanged();
    const r = await call('gameLists', {}, { silent: true });
    P.loading = false;
    if (r) {
      P.lists = r;
      if (!tabRows(P.tab).length) P.tab = TABS.find(k => tabRows(k).length) || 'recent';
    }
    pageChanged();
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
    if (seq === P.seq && P.open && P.mode === 'search') pageChanged();
  }

  function onPlaceInput() {
    const value = $('#place').value.trim();
    scheduleSaveInputs();
    clearTimeout(searchTimer);
    ++P.seq; // whatever search is still on its way answers an older box
    if (!value) {
      S.game = null;
      openPicker('browse');
      return;
    }
    if (looksLikeTarget(value)) {
      pageChanged(); // the dock's Play follows what is in the box
      closePicker();
      scheduleResolve();
      return;
    }
    S.game = null;
    const cached = searchCache.get(value.toLowerCase());
    if (cached && Date.now() - cached.at < SEARCH_TTL) {
      Object.assign(P, { open: true, mode: 'search', query: value, results: cached.rows, loading: false, refreshing: false, index: -1 });
      pageChanged();
      return;
    }
    // Keep the last results on screen, dimmed, while the new ones load, instead of flashing placeholders.
    const keep = P.mode === 'search' && P.results?.length > 0;
    Object.assign(P, { open: true, mode: 'search', query: value, loading: !keep, refreshing: keep, index: -1 });
    pageChanged();
    searchTimer = setTimeout(async () => {
      const seq = P.seq;
      const r = await call('searchGames', { query: value }, { silent: true });
      if (seq !== P.seq || !P.open) return;
      P.results = r || [];
      P.loading = P.refreshing = false;
      if (r) rememberSearch(value.toLowerCase(), P.results);
      pageChanged();
      fillIcons(seq);
    }, 200);
  }
  function onPlaceKey(e) {
    const rows = pickerRows();
    if (P.open && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && rows.length) {
      e.preventDefault();
      P.index = (P.index + (e.key === 'ArrowDown' ? 1 : rows.length - 1) + (P.index < 0 && e.key === 'ArrowUp' ? 1 : 0)) % rows.length;
      pageChanged();
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
    } else if (e.key === 'Tab' && P.open) {
      closePicker(); // leaving the box (to the Job ID box, or back to the thumbnail): the list goes with it
    }
  }
  function pickGame(g) {
    if (!g) return;
    if (P.mode === 'browse' && P.tab === 'saved' && g.id) return pickSaved(g);
    $('#place').value = `https://www.roblox.com/games/${g.placeId}`;
    $('#job').value = '';
    closePicker();
    $('#place').blur();
    resolveGame();
    scheduleSaveInputs();
  }
  /**
   * A saved server into the dock: its private server's link (Play sends the selected accounts there), or its game; a
   * code the game asks for goes on the clipboard, to paste in the game once in (Bloxkeep never types into Roblox).
   */
  async function pickSaved(s) {
    $('#place').value = s.link || `https://www.roblox.com/games/${s.placeId}`;
    $('#job').value = '';
    closePicker();
    $('#place').blur();
    resolveGame();
    scheduleSaveInputs();
    if (s.code && await call('copyServerCode', { id: s.id })) toast(t('sv.codeCopied', { code: s.code }), 'ok');
  }
  /** "Save server": the private server's link in the box (with a code if the game asks for one too), or the game and a code. */
  function saveServerModal() {
    const g = S.game;
    if (!g?.ok) return;
    const place = $('#place').value.trim();
    const hasLink = g.kind === 'private' || g.kind === 'share';
    const game = g.name || (g.placeId ? `Place ${g.placeId}` : '');
    modal({
      title: t('sv.title'),
      body: window.BloxkeepUI.saveServer(game, hasLink),
      onMount: ({ el }) => setTimeout(() => el.querySelector(hasLink ? '#sv-name' : '#sv-code')?.focus(), 30),
      enterSubmits: true,
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('sv.saveBtn'), kind: 'primary', onClick: async ({ el, close, btn }) => {
            const code = el.querySelector('#sv-code').value.trim();
            if (!hasLink && !code) return setText(el.querySelector('#sv-err'), t('sv.needCode'));
            // A refusal (not a private server's link, too many saved) is said in the dialog, not as a toast.
            const r = await busy(btn, async () => {
              try {
                return await bridge.call('saveServer', {
                  name: el.querySelector('#sv-name').value.trim(), code, link: hasLink ? place : '',
                  placeId: g.placeId || 0, universeId: g.universeId || 0, gameName: g.name || '',
                });
              } catch (err) {
                setText(el.querySelector('#sv-err'), err.message);
                return undefined;
              }
            });
            if (!r) return;
            close();
            toast(t('sv.saved', { name: r.name }), 'ok');
            // Where it went, at once: the game box's list on its Saved tab.
            P.tab = 'saved';
            if (document.activeElement === $('#place')) openPicker('browse'); else $('#place').focus();
          },
        },
      ],
    });
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

  /**
   * The game's servers, to pick one to join: the selected accounts' friends in it, then its public servers (like the
   * Servers tab on its Roblox page). Drawn by React (ui/src/Servers.tsx); a pick goes in the dock's Job ID box.
   */
  function openServers() {
    const g = S.game;
    if (!g?.placeId) return;
    let unmount = null;
    const api = modal({
      title: t('srv.title', { game: esc(g.name || `Place ${g.placeId}`) }),
      top: true,
      body: '',
      // Its layout (only the list scrolls) hangs on this class: a :has() rule did it, which re-styled the whole page on
      // every change anywhere in it.
      bodyClass: 'srv-body',
      actions: [{ text: t('common.close'), onClick: ({ close }) => close() }],
      onRemoved: () => unmount?.(),
    });
    const body = api.el.querySelector('.modal-body');
    unmount = window.BloxkeepUI.servers(body, uiHost(), g.placeId, [...S.selected], (placeId, jobId) => {
      useServer(placeId, jobId);
      api.close();
      toast(t('srv.chosen'), 'ok');
    });
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
    pageChanged();
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
      pageChanged();
      return;
    }
    S.game = { loading: true };
    pageChanged();
    const r = await call('resolveGame', { input }, { silent: true });
    if (seq !== S.gameSeq || !S.shell) return; // a newer input superseded this one
    S.game = r || null;
    const job = $('#job');
    if (typed && r?.placeId && S.jobPlace && r.placeId !== S.jobPlace && job.value.trim()) {
      job.value = '';
      scheduleSaveInputs();
    }
    if (r?.placeId) S.jobPlace = r.placeId;
    pageChanged();
  }

  /* ───────────────────────── actions ───────────────────────── */
  function selectOnly(id) {
    S.selected = new Set([id]);
    S.anchor = id;
    renderSelection();
  }
  /** The selection changed: the cards, the heading line and the dock's buttons follow (React). */
  const renderSelection = () => pageChanged();
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
    return launchDone(r, force => launch(ids, appOnly, force));
  }

  /**
   * Locking keeps the Roblox windows open but pauses what Bloxkeep does for them until it is unlocked (AppController.Lock):
   * auto-rejoin, scheduled presets and Discord. When any of that is in use, it says so first.
   */
  async function lockApp() {
    const paused = [];
    const rejoining = accounts().filter(a => (isLocal(a) && rejoinOn(a)) || a.rejoin).length;
    if (rejoining) paused.push(tn('lockq.rejoin', rejoining));
    const scheduled = presets().filter(p => p.schedule?.enabled).length;
    if (scheduled) paused.push(tn('lockq.schedule', scheduled));
    if (S.app.discord?.enabled && S.app.discord.hasWebhook) paused.push(t('lockq.discord'));
    if (paused.length && !await confirmBox(`${t('lockq.text')}\n${paused.map(p => `• ${p}`).join('\n')}`,
      { title: t('lockq.title'), ok: t('side.lock'), remember: 'lock' })) return;
    return call('lock');
  }

  /** The launch warnings with their own "Don't ask again" (AppController.SkippableLaunchWarnings). */
  const SKIPPABLE_ASKS = ['multiOff', 'watch', 'memory'];
  /**
   * The warnings before a launch, each with the way out that goes with it: Multi-Roblox off → turn it on and play; on
   * but not working → its fix. "Don't ask again" only when every warning shown can be skipped, and then for those kinds
   * alone (never for accounts being moved out of their game). Resolves 'open', 'multi', 'fix', or null for Cancel.
   */
  function launchAsk(r) {
    const asks = r.asks || [];
    const canSkip = asks.length > 0 && asks.every(k => SKIPPABLE_ASKS.includes(k));
    const multiOff = asks.includes('multiOff'), notReady = asks.includes('multiNotReady');
    return new Promise(resolve => {
      let answer = null;
      const m = modal({
        title: t('t.confirmTitle'),
        body: window.BloxkeepUI.ask(r.message, canSkip),
        actions: [
          { text: t('common.cancel'), onClick: ({ close }) => close() },
          ...(notReady ? [{ text: t('t.fixMulti'), onClick: ({ close }) => { answer = 'fix'; close(); } }] : []),
          { text: t('t.openAnyway'), kind: multiOff || notReady ? '' : 'primary', onClick: ({ el, close }) => {
            if (canSkip && el.querySelector('#dont-ask')?.checked) asks.forEach(k => call('skipConfirm', { key: `launch.${k}` }));
            answer = 'open';
            close();
          } },
          ...(multiOff ? [{ text: t('t.multiAndPlay'), kind: 'primary', onClick: ({ close }) => { answer = 'multi'; close(); } }] : []),
        ],
        onClose: () => resolve(answer),
      });
      setTimeout(() => (m.el.querySelector('.modal-foot .btn.primary') || m.el.querySelector('.modal-foot .btn'))?.focus(), 30);
    });
  }

  /** What a launch (Play or a preset) did; a warning that needs an answer runs it again with retry(force). */
  async function launchDone(r, retry) {
    if (!r || S.app.locked) return; // locked meanwhile: nothing to ask or report
    if (r.needsConfirm) {
      const answer = await launchAsk(r);
      if (S.app.locked) return;
      if (answer === 'open') return retry(true);
      // Turned on, the launch runs again with its other questions still asked (and Multi-Roblox's own, if Roblox was
      // already open and it can't work yet).
      if (answer === 'multi' && await setMulti(true)) return retry(false);
      if (answer === 'fix') multiHelp();
      return;
    }
    if (r.errors?.length && !S.app.locked) {
      modal({
        title: t(r.launched ? 't.errsSome' : 't.errsAll'),
        body: window.BloxkeepUI.errors(r.errors),
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
  const presets = () => S.app.settings?.presets || [];
  /** The preset's accounts that still exist (deleted ones are skipped when it runs). */
  const presetAccounts = p => p.accountIds.map(byId).filter(Boolean);
  const accCount = n => t(n === 1 ? 'ps.acc1' : 'ps.accN', { n });
  const presetSub = p => [p.gameName || p.placeInput || t('ps.noGame'), accCount(presetAccounts(p).length),
    p.server !== 'any' && p.placeInput ? t(`ps.srv.${p.server}`) : '', nextRunText(p)].filter(Boolean).join(' · ');

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
        icon: p.nextRun ? 'clock' : 'play', text: p.name, title: presetSub(p), hint: nextRunText(p) || accCount(presetAccounts(p).length),
        disabled: !presetAccounts(p).length, run: () => runPreset(p.id),
      })) : [{ icon: 'layers', text: t('ps.none'), disabled: true }]),
      '-',
      { icon: 'plus', text: t('ps.save'), run: newPreset },
      { icon: 'edit', text: t('ps.manage'), run: () => setView('presets') },
    ], 300, { cls: 'preset-menu', split: true });
    // The dock is at the bottom: open above the button, right edges lined up. A long list scrolls inside the menu;
    // Save and Edit stay at its foot.
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
    return launchDone(r, force => runPreset(id, force));
  }

  async function deletePreset(p) {
    if (!await confirmBox(t('ps.deleteQ', { name: p.name }), { title: t('ps.deleteTitle'), ok: t('ps.delete'), danger: true, remember: 'deletePreset' })) return false;
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
    let ui = null;
    modal({
      title: t(isNew ? 'ps.newTitle' : 'ps.editTitle'),
      top: true,
      dismissable: false,
      busy: true,
      enterSubmits: true,
      // Anything changed (boxes, ticks, the accounts' order, the days): Esc or the X asks before throwing it away.
      askClose: () => !!ui?.form.dirty(),
      body: '',
      footError: true,
      actions: [
        ...(isNew ? [] : [{ text: t('ps.delete'), kind: 'danger push', onClick: async ({ close }) => { if (await deletePreset(p)) close(); } }]),
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('ps.saveBtn'), kind: 'primary', onClick: async ({ el, close, btn, hold }) => {
            if (btn.disabled) return;
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
            const r = ui.form.collect();
            if (r.error) return fail(r.error, r.box);
            let refused = null;
            await busy(btn, () => hold(() => bridge.call('savePreset', r.args)).catch(e => { refused = e; }));
            if (refused) return fail(refused.message || t('common.unknownError'), ui.form.refusedBox());
            close();
            toast(t('ps.saved'), 'ok');
          },
        },
      ],
      onMount: ({ el }) => {
        // The form is React's (ui/src/PresetForm.tsx): the preset's accounts first, ticked, then the rest.
        ui = window.BloxkeepUI.presetForm(el.querySelector('.modal-body'), uiHost(),
          { base, isNew, accounts: [...chosen, ...rest], ticked: chosen.map(a => a.id), gone });
        setTimeout(() => el.querySelector('#ps-name')?.focus(), 30);
      },
      onRemoved: () => ui?.unmount(),
    });
  }

  /** Short weekday names from the browser's own calendar (0 = Sunday, like the app). 2024-01-07 was a Sunday. */
  const weekdayName = d => intl('d', { weekday: 'short' }).format(new Date(2024, 0, 7 + d));

  /** The ⋯ menu's "Launch order…": every account, dragged or moved into the order Play opens them in. */
  function launchOrderModal() {
    const rank = orderRank();
    const list = accounts().slice().sort((x, y) => (rank.get(x.id) ?? 1e9) - (rank.get(y.id) ?? 1e9));
    if (!list.length) return toast(t('rc.noAccounts'), 'info');
    let ui = null;
    modal({
      title: t('lo.title'),
      dismissable: false,
      enterSubmits: true,
      body: '',
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ close, btn }) => {
            if (btn.disabled) return;
            const ids = ui.list.ids();
            if (await busy(btn, () => call('setLaunchOrder', { ids })) === undefined) return;
            close();
            toast(t('lo.saved'), 'ok');
          },
        },
      ],
      // Its text and the list are React's (ui/src/OrderList.tsx).
      onMount: ({ el }) => { ui = window.BloxkeepUI.launchOrder(el.querySelector('.modal-body'), uiHost(), list); },
      onRemoved: () => ui?.unmount(),
    });
  }

  /* ───────────────────────── presets tab ─────────────────────────
     Drawn by React (ui/src/PresetsView.tsx): every preset as a row, run, edit, duplicate, delete, its schedule, its
     place in the list. What it uses of this page comes through uiHost(): scheduleText, nextRunText, runPreset… */
  const PV = { query: '' }; // the Presets tab's search (PresetsView.tsx), kept with the view
  const MAX_PRESETS = 50; // the app's own limit (MaxPresets); checked here so a full list doesn't open an empty form
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

  function newPreset() {
    if (presets().length >= MAX_PRESETS) return toast(t('pv.full', { n: MAX_PRESETS }), 'warn');
    presetModal(null);
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
  /** The sign-in screen lists every way to add an account. */
  const addChooser = () => passwordLogin();
  /** Picking a way closes the chooser it was picked from. */
  const leaveChooser = el => el.closest('.overlay')?.querySelector('[data-x]')?.click();

  /**
   * The sign-in screen: username + password, with the other ways (phone, browser) under it. Works on a PC with no
   * phone and no roblox.com: the values go only to Roblox's/VNG's official auth host, over HTTPS. Handles a 2-step code
   * and a captcha (shown in its own window, for the user to solve).
   */
  function passwordLogin(forRealm) {
    let ui = null;
    const m = modal({
      title: '',
      top: true,
      busy: true,
      dismissable: false, // a stray click beside it mustn't end a sign-in half way
      // Esc or the X asks first when that would throw away what was typed, or a sign-in half way (a code already sent).
      askClose: () => !!ui?.api.asksToClose(),
      body: '',
      boxClass: 'auth',
      label: t('pw.title'),
      onClose: () => {
        ui?.api.stop();
        call('loginCancel', {}, { silent: true }); // the app drops the half-done sign-in and its password
        const added = ui?.api.added() || [];
        if (!added.length) return;
        S.selected = new Set(added.map(a => a.id));
        S.anchor = added[added.length - 1].id;
        renderSelection();
      },
      onRemoved: () => ui?.unmount(),
    });
    // The screens are React's (ui/src/PasswordLogin.tsx); a way picked there closes this and opens that way.
    ui = window.BloxkeepUI.passwordLogin(m.el.querySelector('.modal-body'), uiHost(), forRealm ?? null, (way, realm) => {
      m.close();
      if (way === 'browser') addBrowser(null);
      else if (way === 'phone') quickLogin(realm);
      else if (way === 'cookie') accountModal(null);
      else if (way === 'vault') importVault();
    });
  }

  /**
   * Quick Login: Roblox shows a code that the user confirms on any device already signed in (phone, or the Roblox app).
   * The code is renewed after each account, so several can be added in a row, like the browser sign-in.
   */
  /** @param forName the account being signed in again (Sign in on a signed-out card), named at the top. */
  function quickLogin(forRealm, forName = '') {
    let ui = null;
    const m = modal({
      title: t('ql.title'),
      top: true,
      busy: true,
      body: '',
      actions: [
        { text: t('ql.browser'), onClick: ({ close }) => { close(); addBrowser(null); } },
        { text: t('common.done'), kind: 'primary', onClick: ({ close }) => close() },
      ],
      onClose: () => {
        ui?.api.stop();
        call('quickLoginCancel', {}, { silent: true });
        const added = ui?.api.added() || [];
        if (!added.length) return;
        S.selected = new Set(added.map(a => a.id));
        S.anchor = added[added.length - 1].id;
        renderSelection();
      },
      onRemoved: () => ui?.unmount(),
    });
    // The codes, their QR and how it goes are React's (ui/src/QuickLogin.tsx); it hides the browser button for VNG.
    ui = window.BloxkeepUI.quickLogin(m.el.querySelector('.modal-body'), uiHost(), forRealm ?? null, forName,
      () => m.el.querySelector('.modal-foot .btn[data-i="0"]'));
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
    if (!await confirmBox(t(force ? 't.killAllQ' : 't.closeAllQ'), { ok: t(force ? 't.killAllOk' : 't.closeAllOk'), danger: true, remember: force ? 'killAll' : 'closeAll' })) return;
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

  const confirmMulti = () => confirmBox(t('set.multiConfirm'), { title: t('set.multiConfirmTitle'), ok: t('set.multiOk'), remember: 'multi' });
  /** Turns Multi-Roblox on (after the risk warning) or off; a switch that was flipped goes back if the warning is declined. */
  /** @returns whether the change was made. */
  async function setMulti(on) {
    const box = typeof on === 'object' ? on : null;
    const enable = box ? box.checked : on;
    if (enable && !await confirmMulti()) {
      if (box) box.checked = false;
      return false;
    }
    return (await call('setSettings', { multiInstance: enable })) !== undefined;
  }

  /**
   * Accounts from another Bloxkeep vault file (another PC's, or a backup): pick the file, type that file's own master
   * password, tick what to bring over. The app keeps the opened file; this page only ever sees names and states,
   * never cookies or passwords. Closing the dialog (or locking) makes the app forget it.
   */
  function importVault() {
    let done = false, ui = null;
    const m = modal({
      title: t('iv.title'),
      top: true,
      dismissable: false,
      busy: true,
      enterSubmits: true,
      body: '',
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        { text: t('iv.choose'), kind: 'primary', onClick: ({ btn }) => ui?.api.next(btn) },
      ],
      onClose: () => {
        if (!done) call('vaultImportCancel', {}, { silent: true });
      },
      onRemoved: () => ui?.unmount(),
    });
    // The steps are React's (ui/src/ImportVault.tsx); it sets the main button's words and state for each.
    ui = window.BloxkeepUI.importVault(m.el.querySelector('.modal-body'), uiHost(), () => m.el.querySelector('.modal-foot .btn.primary'), r => {
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
    });
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
    let ui = null;
    modal({
      title: t('rc.pickTitle'),
      body: '',
      onMount: ({ el }) => { ui = window.BloxkeepUI.pickList(el.querySelector('.modal-body'), uiHost(), t('rc.pickText'), list.map(a => pickAccount(a, !!a.useRobloxConfig))); },
      onRemoved: () => ui?.unmount(),
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('common.done'), kind: 'primary', onClick: async ({ close, btn }) => {
            if (btn.disabled) return;
            const on = ui.list.values(), off = list.map(a => a.id).filter(id => !on.includes(id));
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
        title: ids.some(i => { const x = byId(i); return x?.session && x.session.autoRejoin !== undefined && x.session.autoRejoin !== x.autoRejoin; }) ? t('m.rejoinPreset') : '' },
      ...(S.app.settings?.robloxConfig?.mode === 'marked'
        ? [{ icon: 'sliders', text: t('m.robloxConfig'), checked: ids.every(i => byId(i)?.useRobloxConfig), run: () => toggleRobloxConfig(ids) }] : []),
      { icon: 'external', text: t('m.profile'), disabled: many || !a.userId, run: () => call('openProfile', { userId: a.userId }) },
      // roblox.com signed in as this account, in a window of Bloxkeep's own (one per account, several side by side).
      { icon: 'globe', text: t('m.website'), disabled: many || a.realm === 'vng', title: a.realm === 'vng' ? t('card.vngNoWebsite') : '',
        run: () => call('openWebsite', { id }) },
      '-',
      { icon: 'copy', text: t('m.copyUser'), disabled: many, run: () => copy(id, 'username') },
      // Nothing saved to copy: picking it says why (a sign-in never keeps the password) instead of a dimmed line.
      { icon: 'key', text: t('m.copyPw'), disabled: many, title: a.hasPassword ? '' : t('m.noPw'),
        run: () => a.hasPassword ? copy(id, 'password') : toast(t('m.noPw'), 'info') },
      { icon: 'shieldCheck', text: t('m.copyCode'), disabled: many, title: a.hasAuthenticator ? '' : t('m.noCode'),
        run: () => a.hasAuthenticator ? copy(id, 'totp') : toast(t('m.noCode'), 'info') },
      '-',
      { icon: 'trash', text: many ? t('m.deleteN', { n: ids.length }) : t('m.delete'), hint: 'Del', danger: true, run: () => removeIds(ids) },
    ], x, y);
  }

  /** The Multi-Roblox dialog while it is open: it follows the state (Roblox closed meanwhile, turned on or off from the tray). */
  let multiHelpOpen = null;
  /** Multi-Roblox turned on or off with its dialog open: opened again, its footer's buttons being different. */
  function syncMultiHelp() {
    const o = multiHelpOpen, m = S.app.multi;
    if (!o || !m || m.enabled === o.enabled) return;
    o.close();
    multiHelp();
  }
  /**
   * Send feedback: an idea or a bug report for Bloxkeep's developer. Preview shows exactly what goes, the error log
   * with account names already taken out by the app.
   */
  function feedbackModal() {
    let ui = null;
    const written = () => { const v = ui?.form.values(); return !!v && (v.text.trim() !== '' || v.contact.trim() !== ''); };
    modal({
      title: t('fb.title'),
      top: true,
      dismissable: false,
      busy: true,
      askClose: written,
      body: '',
      onMount: ({ el }) => { ui = window.BloxkeepUI.feedback(el.querySelector('.modal-body'), uiHost(), !!S.app.locked); },
      onRemoved: () => ui?.unmount(),
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t('fb.send'), kind: 'primary', onClick: async ({ close, btn }) => {
            const v = ui.form.values();
            ui.form.showError(null);
            if (v.text.trim().length < 3) {
              ui.form.showError(t('fb.needText'));
              ui.form.focusText();
              return;
            }
            if (btn.disabled) return;
            try {
              await busy(btn, () => bridge.call('sendFeedback', v));
              close();
              toast(t('fb.sent'), 'ok');
            } catch (ex) {
              ui.form.showError(ex.message);
            }
          },
        },
      ],
    });
  }

  function multiHelp() {
    const m = S.app.multi;
    let unmount = null;
    modal({
      title: t('multi.title'),
      body: '',
      actions: [
        ...(m.enabled ? [{ text: t('mh.disable'), kind: 'danger push', onClick: ({ close }) => { close(); setMulti(false); } }] : []),
        { text: t('common.done'), kind: 'primary', onClick: ({ close }) => close() },
      ],
      onMount: ({ el, close }) => {
        multiHelpOpen = { close, enabled: m.enabled };
        unmount = window.BloxkeepUI.multiHelp(el.querySelector('.modal-body'), uiHost(), () => { close(); setMulti(true); });
      },
      onClose: () => { multiHelpOpen = null; },
      onRemoved: () => unmount?.(),
    });
  }

  /** Adding an account by its cookie (a null), or editing one: the form is React's (ui/src/AccountForm.tsx). */
  function accountModal(a) {
    const isNew = !a;
    let ui = null;
    modal({
      title: t(isNew ? 'am.addTitle' : 'am.editTitle'),
      top: true,
      dismissable: false,
      busy: true,
      enterSubmits: true,
      // What the boxes held when the form opened: Esc or the X asks before throwing away a pasted cookie or edits.
      askClose: () => !!ui?.form.dirty(),
      body: '',
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        {
          text: t(isNew ? 'am.add' : 'am.save'), kind: 'primary', onClick: async ({ close, btn }) => {
            const r0 = ui.form.collect();
            if (r0.error) return;
            if (btn.disabled) return; // a second click while the first is still saving
            try {
              const r = await busy(btn, () => bridge.call(isNew ? 'addViaCookie' : 'updateAccount', r0.args));
              close();
              if (isNew && r) {
                toast(t(r.merged ? 't.merged' : 't.added', { name: r.label }), 'ok');
                selectOnly(r.id);
              } else {
                toast(t('t.saved'), 'ok');
              }
            } catch (ex) {
              ui.form.fail(ex.message);
            }
          },
        },
      ],
      onMount: ({ el, close }) => {
        ui = window.BloxkeepUI.accountForm(el.querySelector('.modal-body'), uiHost(), a, () => { close(); addBrowser(null); });
        const aliasHidden = !isNew && hidingNames();
        setTimeout(() => el.querySelector(isNew ? '[name=cookie]' : aliasHidden ? '[name=group]' : '[name=alias]')?.focus(), 40);
      },
      onRemoved: () => ui?.unmount(),
    });
  }
  const editAccount = id => { const a = byId(id); if (a) accountModal(a); };

  /* ───────────────────────── statistics ─────────────────────────
     Drawn by React (ui/src/StatsView.tsx): play time per account, game and preset, and the activity log. */
  let statsUi = null;

  /* ───────────────────────── game monitor ─────────────────────────
     A page can't show other programs' windows: the live pictures of the Roblox windows are the app's, laid over this
     tab's empty area. The page tells the app where that area is (CSS pixels and the pixel ratio) whenever it may have
     moved, and { hidden } whenever something of the page must show there: another tab, a dialog or menu, the lock
     screen, the window hidden. Windows draws the pictures above the whole page, so nothing could show through them. */
  const MON = { sent: null, frame: 0, dpr: null };
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
     the app's own actions. paletteEntries() is the registry, built from the state each time the box opens; an entry
     is { id, text, hint, keywords, icon | img | avatar, run, alts }, so a new action is one line in its list. The box,
     the list, the ranking and the keys are React's (ui/src/Palette.tsx). */
  const PAL = { close: null };

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
      pageChanged();
    }
    selectOnly(id);
    const card = $(`#grid .card[data-id="${CSS.escape(id)}"]`);
    if (!card) return;
    setTabStop(card.parentElement, card);
    card.scrollIntoView({ block: 'nearest', behavior: matchMedia('(prefers-reduced-motion: reduce)').matches ? 'auto' : 'smooth' });
    card.focus({ preventScroll: true });
  }

  function paletteEntries() {
    const st = S.app.settings || {};
    const go = (view, then) => () => { setView(view); then?.(); };
    const openSection = id => () => openSettings(id);
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
      { id: 'feedback', icon: 'message', text: t('fb.title'), keywords: ['bug', 'report', 'idea', 'feedback', 'góp ý', 'báo lỗi'], run: () => feedbackModal() },
      { id: 'guide', icon: 'book', text: t('help.title'), keywords: ['help', 'guide', 'manual', 'how', 'hướng dẫn', 'trợ giúp'], run: () => call('openGuide') },
      { id: 'star', icon: 'star', text: t('star.title'), keywords: ['github', 'star', 'sao', 'open source', 'mã nguồn'], run: () => call('openGitHub') },
      { id: 'export', icon: 'history', text: t('pal.exportActivity'), keywords: ['csv', 'log'], run: () => { setView('stats'); statsUi?.api.showActivity(); statsUi?.api.exportActivity(); } }, // the list may not be loaded yet: the app knows
      { id: 'lang', icon: 'languages', text: lang === 'vi' ? 'Switch to English' : 'Chuyển sang tiếng Việt', keywords: ['language', 'ngôn ngữ'], run: () => call('setLanguage', { language: lang === 'vi' ? 'en' : 'vi' }) },
      { id: 'lock', icon: 'lock', text: t('side.lock'), keywords: ['password'], run: () => lockApp() },
    ].filter(x => !x.off).map(x => ({ ...x, kind: 'action' }));
    const tabs = [['accounts', 'users'], ['presets', 'layers'], ['monitor', 'screens'], ['stats', 'clock'], ['settings', 'sliders']]
      .map(([view, ic]) => ({ id: `tab:${view}`, kind: 'tab', icon: ic, text: t(`nav.${view}`), run: go(view) }));
    tabs.push({ id: 'tab:activity', kind: 'tab', icon: 'history', text: t('alog.title'), keywords: ['log', 'history', 'events'], run: go('stats', () => statsUi?.api.showActivity()) });
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
      id: `set:${pg.dataset.page}`, kind: 'setting', icon: SET_PAGES.find(p => p[0] === pg.dataset.page)?.[1],
      text: pg.querySelector('.set-page-title')?.textContent || pg.dataset.page,
      keywords: [...pg.querySelectorAll('.set-row .t, .set-grp-title')].map(x => x.textContent.trim()),
      run: openSection(pg.dataset.page),
    }));
    return [...actions, ...tabs, ...presetList, ...games, ...list, ...sections];
  }

  function openPalette() {
    if (PAL.close || !S.shell || !S.app || S.app.locked) return;
    let unmount = null;
    modal({
      title: t('pal.title'),
      body: '',
      boxClass: 'palette',
      onMount: ({ el, close }) => {
        PAL.close = close;
        unmount = window.BloxkeepUI.palette(el.querySelector('.modal-body'), uiHost(), paletteEntries(), close);
        el.querySelector('.pal-input').focus();
      },
      onClose: () => { PAL.close = null; },
      onRemoved: () => unmount?.(),
    });
  }

  /* ───────────────────────── join a player ─────────────────────────
     The selected accounts open into the server a Roblox user is in. The app looks the name up and asks Roblox where
     that person is with the selected accounts' own sign-ins; when Roblox won't say (privacy settings, offline, in
     Studio, on the website) the dialog says exactly that and nothing opens. Names looked up are kept in the vault. */
  function joinPlayerModal() {
    const ids = [...S.selected];
    if (!ids.length) return toast(t('t.selectFirst'), 'warn');
    let gone = false, ui = null;
    const go = async (btn, close) => {
      const input = ui.form.name().trim();
      if (!input) {
        ui.form.say(t('jp.needName'), 'error');
        return ui.form.focusName();
      }
      // One launch at a time: while one runs, the join wouldn't start (and the lookup would be for nothing).
      const launchBusy = () => inFlight.has('launch') || !!S.app.launching;
      if (launchBusy()) return ui.form.say(t('jp.launching'), 'warn');
      ui.form.say(t('jp.looking', { name: input }), 'info', { spin: true });
      let r;
      try {
        r = await busy(btn, () => bridge.call('findPlayer', { input, ids }));
      } catch (e) {
        if (!gone) { ui.form.say(e.message, 'error'); ui.form.focusName(); }
        return;
      }
      if (gone || !r || S.app.locked) return;
      const who = r.displayName && r.displayName !== r.name ? `${r.displayName} (@${r.name})` : r.name || input;
      if (r.status !== 'found') {
        // A name Roblox knows is remembered even when it can't be joined now: the list shows it with the app's next state.
        ui.form.say(t(`jp.${r.status}`, { name: '\u0000' }), r.status === 'notFound' ? 'error' : 'warn', { bold: who });
        return ui.form.focusName(true);
      }
      if (launchBusy()) return ui.form.say(t('jp.launching'), 'warn'); // one started meanwhile (a scheduled preset)
      close();
      // Toasts escape their text themselves.
      toast(tn('jp.found', ids.length, { name: who, game: r.game || `Place ${r.placeId}` }), 'info');
      joinServer(ids, r.placeId, r.jobId);
    };
    modal({
      title: t('jp.title'),
      top: true,
      busy: true,
      enterSubmits: true,
      body: '',
      actions: [
        { text: t('common.cancel'), onClick: ({ close }) => close() },
        { text: t('jp.go'), kind: 'primary', onClick: ({ btn, close }) => go(btn, close) },
      ],
      onMount: ({ el }) => {
        ui = window.BloxkeepUI.joinPlayer(el.querySelector('.modal-body'), uiHost(), ids.length, () => el.querySelector('.modal-foot .btn.primary').click());
      },
      onClose: () => { gone = true; },
      onRemoved: () => ui?.unmount(),
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
    return launchDone(r, force => joinServer(ids, placeId, jobId, force));
  }

  /* ───────────────────────── events ───────────────────────── */
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
      // Settings › Appearance › Custom: a picture behind the page, or none.
      case 'ct-pick': return busy(el, () => call('pickThemePicture'));
      case 'ct-clear': return call('clearThemePicture');
      case 'theme':
        clearTimeout(themeTimer); // a click wins over a pick still waiting to be saved
        setThemePending(null);
        if (el.dataset.id !== S.app.theme) return call('setTheme', { theme: el.dataset.id });
        document.documentElement.dataset.theme = S.app.themeApplied || S.app.theme; // back from a preview
        return;
      case 'lang': return el.dataset.lang !== lang && call('setLanguage', { language: el.dataset.lang });
      case 'view': return setView(el.dataset.view);
      case 'lock': return lockApp();
      case 'reset-asks': return call('resetSkipConfirm').then(n => n !== undefined && toast(t('set.asksDone'), 'ok'));
      case 'add-choose': return addChooser();
      case 'add-password': leaveChooser(el); return passwordLogin();
      case 'add-phone': leaveChooser(el); return quickLogin();
      case 'import-vault': leaveChooser(el); return importVault();
      case 'relogin': { const a = byId(id); return quickLogin(a?.realm, a?.label); }
      case 'add-browser': leaveChooser(el); return addBrowser(el);
      case 'add-cookie': leaveChooser(el); return accountModal(null);
      // roblox.com in a blank window of Bloxkeep's own, to sign in on the site (several side by side).
      case 'open-web': leaveChooser(el); return call('openWebsiteBlank');
      case 'add-menu':
        return menuAt(el, [
          { icon: 'qr', text: t('m.addCode'), run: () => quickLogin() },
          { icon: 'globe', text: t('m.addBrowser'), run: () => addBrowser(null) },
          { icon: 'cookie', text: t('m.addCookie'), run: () => accountModal(null) },
          '-',
          { icon: 'folder', text: t('m.importVault'), run: importVault },
          '-',
          { icon: 'external', text: t('m.openWeb'), title: t('start.webTip'), run: () => call('openWebsiteBlank') },
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
          '-',
          { icon: 'book', text: t('help.title'), run: () => call('openGuide') },
        ], 280);
      case 'layout':
        S.layout = el.dataset.l === 'list' ? 'list' : 'cards';
        call('setSettings', { layout: S.layout }, { silent: true });
        pageChanged();
        return;
      case 'sort-menu':
        return menuAt(el, ['name', 'status', 'recent', 'order'].map(k => ({
          icon: 'sort', text: t(`sort.${k}`), checked: S.sort === k,
          run: () => {
            S.sort = k;
            call('setSettings', { sort: k }, { silent: true });
            pageChanged();
          },
        })), 200);
      // Account groups: one menu next to Sort ("Group: All"), apart from the status chips.
      case 'group-menu': {
        const pick = g => () => {
          S.group = g;
          pageChanged();
          $('#scroll').scrollTop = 0;
        };
        return menuAt(el, [
          { icon: 'tag', text: t('chip.all'), checked: !S.group, run: pick('') },
          '-',
          ...accountGroups().map(g => ({ icon: 'tag', text: g, checked: S.group === g, run: pick(g) })),
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
        pageChanged();
        $('#scroll').scrollTop = 0;
        return;
      case 'launch': return launch([...S.selected], false);
      case 'open-app': return launch([...S.selected], true);
      case 'arrange': return arrange(el);
      case 'close-sel': return closeIds([...S.selected]);
      case 'close-all': return closeAll();
      // A double click would open the window and close it again: the second click is dropped.
      case 'mini': return busy(el, () => call('toggleMini'));
      // The macOS themes' window buttons (shown only there): the same as the title bar's ✕, ─ and ☐.
      case 'win-close': return call('window', { action: 'close' });
      case 'win-min': return call('window', { action: 'minimize' });
      case 'win-zoom': return call('window', { action: 'zoom' });
      case 'rc-pick': return pickRobloxConfigAccounts();
      case 'pick-launcher': return busy(el, () => call('pickLauncher')); // a double click opens one file window
      case 'cancel-launch': return call('cancelLaunch');
      case 'preset-menu': return presetMenu(el);
      case 'pv-new': return newPreset();
      case 'multi-help': return multiHelp();
      case 'ru-notes': return call('openReleaseNotes', { version: el.dataset.v });
      case 'ru-hide': return call('hideRobloxClosed');
      // Roblox's launcher here installs the new Roblox without opening it. With windows open it may close them (when it
      // can't update in the background), so that is asked first. Not busy(): the button stays disabled from the app's
      // state ("Updating…") until the launcher is done, which busy() would undo when the call returns.
      case 'rbx-update': return (async () => {
        const rv = S.app.robloxVersion || {}, n = S.metrics.robloxCount;
        if (rv.updating || (n && !await confirmBox(tn('rbx.updateQ', n, { l: rv.updater }), { title: t('rbx.updateTitle'), ok: t('rbx.updateOk') }))) return;
        if (await call('updateRoblox') !== undefined) toast(t('rbx.updateStarted', { l: rv.updater }), 'info');
      })();
      case 'ru-open': return openSettings('rbxupd');
      case 'rw-how': return call('openRobloxWatch');
      case 'open-guide': return call('openGuide');
      case 'open-github': return call('openGitHub');
      case 'feedback': return feedbackModal();
      case 'select-all': return selectAllVisible();
      case 'clear-search':
        S.query = '';
        $('#q').value = '';
        S.filter = 'all';
        S.group = '';
        pageChanged();
        return $('#q').focus();
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
      // First start without a vault: Multi-Roblox only, for someone who signs in to Roblox in their own browser. It
      // asks the same as the sidebar's switch, then Bloxkeep goes to the tray.
      case 'multi-only': return (async () => {
        if (!await confirmBox(t('set.multiConfirm'), { title: t('set.multiConfirmTitle'), ok: t('set.multiOk') })) return;
        if (await call('multiOnly', { on: true }) === undefined) return;
        toast(t('lock.multiOnlyDone'), 'ok');
      })();
      case 'multi-only-off': return call('multiOnly', { on: false });
      case 'check-updates': return busy(el, () => call('checkUpdates'));
      // From the Multi-Roblox check (Settings › Roblox): the answer comes here, not only on the updates page.
      case 'rw-check': return busy(el, async () => {
        if (await call('checkUpdates') === undefined) return;
        const u = S.app.update || {};
        toast(!u.supported ? t('upd.unsupported') : u.status === 'upToDate' ? t('upd.latest')
          : u.status === 'ready' ? (u.quiet ? t('upd.readyQuiet') : t(u.auto ? 'upd.readyAuto' : 'upd.ready', { v: u.version }))
          : u.status === 'downloading' ? t('upd.downloading', { v: u.version, p: u.progress })
          : u.status === 'error' ? t('upd.error') : t('upd.checking'), u.status === 'error' ? 'warn' : 'info');
      });
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
        return pageChanged();
      case 'pick-game': return pickGame(pickerRows()[+el.dataset.i]);
      case 'servers': return openServers();
      case 'pin-game':
        e.stopPropagation();
        return pinGame(pickerRows()[+el.dataset.i]);
      case 'save-server': return saveServerModal();
      case 'copy-server-code': {
        e.stopPropagation();
        const s = pickerRows()[+el.dataset.i];
        return s && call('copyServerCode', { id: s.id }).then(r => { if (r) toast(t('sv.codeCopiedShort'), 'ok'); });
      }
      case 'delete-server': {
        e.stopPropagation();
        const s = pickerRows()[+el.dataset.i];
        return s && call('deleteServer', { id: s.id }).then(() => { P.index = -1; pageChanged(); });
      }
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
            [...document.querySelectorAll(`[data-act="lang"][data-lang="${lang}"]`)].find(b => b.offsetParent && !b.closest('.view[hidden]'))?.focus();
        });
      }
    }
    S.app = st;
    maskNames(st); // before anything renders a name
    uiListeners.forEach(f => f()); // the React parts (useAppState) draw again with it
    if (st.themeApplied && !S.themePending) document.documentElement.dataset.theme = st.themeApplied;
    applyCustomTheme(); // new colours or picture, with the theme itself unchanged
    if (st.locked) {
      // Toasts from before the lock may name accounts: they don't stay on the lock screen.
      if (S.shell) toastUi?.api.clear();
      S.bootDone = S.bootTimedOut = false;
      S.viewPrefsLoaded = false; // the next vault brings its own layout and sort
      stopBoot();
      S.selected.clear();
      S.metrics = { accounts: [], robloxCount: 0, totalMb: 0, systemMb: 0 };
      metricsListeners.forEach(f => f());
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
  let bootUi = null;
  function showBoot(boot) {
    if (!$('#boot')) {
      takeDownUiParts();
      const app = $('#app');
      app._html = null;
      app.innerHTML = '';
      // The screen is React's (ui/src/Lock.tsx, Boot): its bar and words are painted from here.
      bootUi = window.BloxkeepUI.boot(app, uiHost());
      uiParts.push(bootUi.unmount);
      S.lockMode = null;
      S.bootValue = 0.05;
    }
    const step = BOOT_STEPS[boot.stage] || { label: 'boot.loading', end: 1 };
    S.bootValue = Math.max(S.bootValue, boot.percent);
    S.bootCeil = Math.max(S.bootValue, step.end - 0.04);
    S.bootText = t(step.label);
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
    if ($('#boot')) bootUi?.api.paint(S.bootText || '', S.bootValue);
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
    if (S.shell) pageChanged();
  });
  bridge.on('metrics', m => {
    S.metrics = m;
    if (!document.hidden) metricsListeners.forEach(f => f());
  });
  document.addEventListener('visibilitychange', () => { if (!document.hidden) metricsListeners.forEach(f => f()); });

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

  toastUi = window.BloxkeepUI.toasts($('#toasts'), uiHost());
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
      uiSignal('presetQuery');
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
