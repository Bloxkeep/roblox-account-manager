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
    gamepad: '<path d="M6 12h4M8 10v4M15 13h.01M18 11h.01"/><rect x="2" y="6" width="20" height="12" rx="6"/>',
    power: '<path d="M12 2v10M18.4 6.6a9 9 0 1 1-12.8 0"/>',
    server: '<rect x="2" y="3" width="20" height="7" rx="2"/><rect x="2" y="14" width="20" height="7" rx="2"/><path d="M6 6.5h.01M6 17.5h.01"/>',
    link: '<path d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7"/><path d="M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7"/>',
    userPlus: '<path d="M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2"/><circle cx="9" cy="7" r="4"/><path d="M19 8v6M22 11h-6"/>',
    monitor: '<rect x="2" y="3" width="20" height="14" rx="2"/><path d="M8 21h8M12 17v4"/>',
    globe: '<circle cx="12" cy="12" r="10"/><path d="M2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"/>',
    cookie: '<path d="M12 2a10 10 0 1 0 10 10 4 4 0 0 1-5-5 4 4 0 0 1-5-5"/><path d="M8.5 8.5h.01M16 15.5h.01M12 12h.01M11 17h.01M7 14h.01"/>',
    chip: '<rect x="5" y="5" width="14" height="14" rx="2"/><path d="M9 9h6v6H9zM9 2v3M15 2v3M9 19v3M15 19v3M2 9h3M2 15h3M19 9h3M19 15h3"/>',
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
  const locale = () => (lang === 'vi' ? 'vi-VN' : 'en-US');
  const langSwitch = () => `<div class="lang" role="group" aria-label="Language">${['en', 'vi'].map(l =>
    `<button data-act="lang" data-lang="${l}" class="${l === lang ? 'on' : ''}" title="${esc(DICT[l]['lang.name'])}">${l.toUpperCase()}</button>`).join('')}</div>`;

  /* ───────────────────────── bridge ─────────────────────────
     The page never talks to the network: it sends commands to the app and receives replies/events. */
  const bridge = (() => {
    const wv = window.chrome && window.chrome.webview;
    const mock = window.__mockBridge;
    const waiting = new Map();
    const listeners = {};
    let seq = 0;
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
    layout: store.get('rm.layout', 'cards') === 'list' ? 'list' : 'cards',
    sort: ['name', 'status', 'recent'].includes(store.get('rm.sort')) ? store.get('rm.sort') : 'name',
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
  /** A window open here whose account is no longer in a server (kicked / disconnected), or opened without a game. */
  const isStranded = a => isLocal(a) && ['disconnected', 'menu'].includes(a.session?.connection);
  const inGame = a => (isLocal(a) && !isStranded(a)) || a.presence?.type === 'ingame';
  const isOnline = a => isLocal(a) || ['online', 'ingame', 'studio'].includes(a.presence?.type);

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
  const fmtNum = n => new Intl.NumberFormat(locale()).format(n);
  const fmtMb = mb => mb >= 1024 ? `${(mb / 1024).toLocaleString(locale(), { maximumFractionDigits: 1 })} GB` : `${Math.round(mb)} MB`;
  const hue = s => { let h = 7; for (const c of String(s)) h = (h * 31 + c.charCodeAt(0)) >>> 0; return h % 360; };
  const initials = s => (String(s).match(/[\p{L}\p{N}]/gu) || ['?']).slice(0, 2).join('').toUpperCase();
  const avatarInner = a => a.avatarUrl
    ? `<img src="${esc(a.avatarUrl)}" alt="" draggable="false" loading="lazy" decoding="async">`
    : `<span class="initials" style="--h:${hue(a.username || a.label)}">${esc(initials(a.label || a.username))}</span>`;

  /* ───────────────────────── toasts ───────────────────────── */
  /** @param action optional { label, run } shown as a button; the toast then stays a little longer. */
  function toast(text, kind = 'info', action = null) {
    const box = $('#toasts');
    if (!box || !text) return;
    while (box.children.length >= 4) box.firstElementChild.remove();
    const el = document.createElement('div');
    el.className = `toast ${kind}`;
    el.setAttribute('role', kind === 'error' ? 'alert' : 'status');
    el.innerHTML = `${icon(kind === 'ok' ? 'check' : kind === 'info' ? 'info' : 'alert')}<span>${esc(text)}</span>${action ? `<button class="toast-act">${esc(action.label)}</button>` : ''}`;
    const dismiss = () => {
      el.classList.add('out');
      setTimeout(() => el.remove(), 220);
    };
    if (action) el.querySelector('.toast-act').addEventListener('click', () => { action.run(); dismiss(); });
    box.append(el);
    setTimeout(dismiss, kind === 'error' ? 6500 : action ? 9000 : 3800);
  }

  /* ───────────────────────── modal & menu ───────────────────────── */
  let openModals = 0;
  /** @param dismissable false keeps forms open on a stray backdrop click (Esc and the X still close). */
  function modal({ title, body, actions = [], onMount, onClose, dismissable = true }) {
    closeMenu();
    const el = document.createElement('div');
    el.className = 'overlay';
    el.innerHTML = `<div class="modal" role="dialog" aria-modal="true">
      <div class="modal-head"><h3>${title}</h3><button class="icon-btn" data-x title="${t('common.close')}">${icon('x')}</button></div>
      <div class="modal-body">${body}</div>
      ${actions.length ? `<div class="modal-foot">${actions.map((a, i) => `<button class="btn ${a.kind || ''}" data-i="${i}">${a.text}</button>`).join('')}</div>` : ''}
    </div>`;
    let closed = false;
    const onKey = e => {
      if (e.key === 'Escape') {
        e.stopPropagation();
        close();
      }
    };
    const close = () => {
      if (closed) return;
      closed = true;
      openModals--;
      document.removeEventListener('keydown', onKey, true);
      el.classList.add('out');
      setTimeout(() => el.remove(), 150);
      onClose?.();
    };
    openModals++;
    document.addEventListener('keydown', onKey, true);
    if (dismissable) el.addEventListener('mousedown', e => { if (e.target === el) close(); });
    el.querySelector('[data-x]').onclick = close;
    const api = { el, close };
    el.querySelectorAll('.modal-foot .btn').forEach(btn => {
      btn.onclick = () => actions[+btn.dataset.i].onClick?.({ ...api, btn });
    });
    $('#layer').append(el);
    onMount?.(api);
    return api;
  }
  /** Closes every open modal, e.g. when the vault locks or the language changes. */
  function closeAllModals() {
    document.querySelectorAll('#layer .overlay:not(.out) [data-x]').forEach(x => x.click());
  }

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
      setTimeout(() => m.el.querySelector('.modal-foot .btn:last-child')?.focus(), 30);
    });
  }

  let menuEl = null;
  function closeMenu() {
    menuEl?.remove();
    menuEl = null;
  }
  function openMenu(items, x, y) {
    closeMenu();
    const el = document.createElement('div');
    el.className = 'menu';
    el.setAttribute('role', 'menu');
    el.innerHTML = items.map((it, i) => it === '-' ? '<hr>'
      : `<button role="menuitem" data-mi="${i}" class="${it.danger ? 'danger' : ''} ${it.checked ? 'checked' : ''}" ${it.disabled ? 'disabled' : ''}>${icon(it.checked ? 'check' : it.icon)}<span>${it.text}</span>${it.hint ? `<kbd>${it.hint}</kbd>` : ''}</button>`).join('');
    el.addEventListener('click', e => {
      const b = e.target.closest('[data-mi]');
      if (!b) return;
      const item = items[+b.dataset.mi];
      closeMenu();
      item.run();
    });
    $('#layer').append(el);
    menuEl = el;
    const r = el.getBoundingClientRect();
    el.style.left = `${Math.max(8, Math.min(x, innerWidth - r.width - 8))}px`;
    el.style.top = `${Math.max(8, Math.min(y, innerHeight - r.height - 8))}px`;
  }
  const menuAt = (btn, items, width) => {
    const r = btn.getBoundingClientRect();
    openMenu(items, width ? r.right - width : r.left, r.bottom + 6);
  };
  document.addEventListener('mousedown', e => { if (menuEl && !menuEl.contains(e.target)) closeMenu(); }, true);
  window.addEventListener('blur', closeMenu);
  window.addEventListener('resize', closeMenu);

  /** Disables a button and shows a spinner while the action runs. */
  async function busy(btn, fn) {
    if (btn?.disabled) return undefined;
    btn?.classList.add('busy');
    if (btn) btn.disabled = true;
    try {
      return await fn();
    } finally {
      if (btn?.isConnected) {
        btn.classList.remove('busy');
        btn.disabled = false;
      }
    }
  }

  /* ───────────────────────── lock screen ───────────────────────── */
  function renderLock() {
    const mode = S.app.vaultExists ? 'unlock' : 'create';
    if (S.lockMode === mode) return; // keep what the user is typing
    S.lockMode = mode;
    S.shell = false;
    closeMenu();
    closeAllModals();
    const create = mode === 'create';
    $('#app').innerHTML = `<div class="lock">
      <div class="dots"></div>
      <div class="lock-lang">${langSwitch()}</div>
      <form class="lock-card" id="lock-form" autocomplete="off">
        <img class="logo-img" src="${LOGO}" alt="">
        <h2>${t(create ? 'lock.createTitle' : 'lock.unlockTitle')}</h2>
        <p>${t(create ? 'lock.createText' : 'lock.unlockText')}</p>
        <div class="stack">
          <div class="pw"><input class="field" type="password" id="pw1" placeholder="${t('lock.pw')}" maxlength="256" aria-label="${t('lock.pw')}">
            <button type="button" class="icon-btn" id="pw-eye" title="${t('lock.showPw')}">${icon('eye')}</button></div>
          ${create ? `<input class="field" type="password" id="pw2" placeholder="${t('lock.pw2')}" maxlength="256" aria-label="${t('lock.pw2')}">
            <div class="strength"><i id="pw-meter"></i></div>` : ''}
        </div>
        <div class="lock-error" id="lock-error" role="alert"></div>
        <button class="btn primary big w" type="submit" id="lock-btn">${t(create ? 'lock.create' : 'lock.unlock')}</button>
      </form>
    </div>`;
    const pw1 = $('#pw1'), pw2 = $('#pw2'), form = $('#lock-form');
    setTimeout(() => pw1.focus(), 50);
    $('#pw-eye').onclick = () => {
      const show = pw1.type === 'password';
      [pw1, pw2].forEach(i => i && (i.type = show ? 'text' : 'password'));
      $('#pw-eye').innerHTML = icon(show ? 'eyeOff' : 'eye');
    };
    if (create) {
      pw1.addEventListener('input', () => {
        const v = pw1.value;
        const score = [v.length >= 8, v.length >= 12, /[a-z]/.test(v) && /[A-Z]/.test(v), /\d/.test(v), /[^A-Za-z0-9]/.test(v)].filter(Boolean).length;
        const meter = $('#pw-meter');
        meter.style.width = `${v ? Math.max(12, score * 20) : 0}%`;
        meter.style.background = score <= 1 ? 'var(--bad)' : score <= 3 ? 'var(--warn)' : 'var(--ok)';
      });
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
        if (!create && whoAtUnlock() && accounts().length) openWho();
      }
    });
  }

  /* ───────────────────────── shell ───────────────────────── */
  function buildShell() {
    S.shell = true;
    S.lockMode = null;
    closeAllModals();
    $('#app').innerHTML = `<div class="shell">
      <aside class="side">
        <div class="brand"><img src="${LOGO}" alt=""><div><div class="brand-name">Bloxkeep</div><div class="brand-sub">${t('brand.sub')}</div></div></div>
        <button class="nav" data-act="view" data-view="accounts" title="${t('nav.accounts')}">${icon('users')}<span>${t('nav.accounts')}</span><em id="nav-count">0</em></button>
        <button class="nav" data-act="who" title="${t('who.title')}">${icon('user')}<span>${t('who.nav')}</span></button>
        <button class="nav" data-act="view" data-view="settings" title="${t('nav.settings')}">${icon('sliders')}<span>${t('nav.settings')}</span></button>
        <div class="side-fill"></div>
        <div id="side-warn"></div>
        <div class="side-card" id="sys-card">
          <h4>${t('sys.title')}</h4>
          <div class="sys-row"><span><b id="sys-count">0</b> <span id="sys-count-label">${t('sys.windows')}</span></span><span id="sys-ram">0 MB</span></div>
          <div class="meter" id="sys-meter"><i class="roblox"></i><i class="other"></i></div>
          <div class="sys-foot" id="sys-foot"></div>
          <div class="sys-ver" id="sys-ver"></div>
        </div>
        <div id="multi-card"></div>
        <div class="side-foot">
          <div id="update-slot"></div>
          <button class="btn ghost w" data-act="lock" title="${t('side.lockTip')}">${icon('lock')}<span>${t('side.lock')}</span></button>
        </div>
      </aside>
      <main class="main">
        <section class="view" id="view-accounts">
          <header class="top">
            <div class="top-title"><h1>${t('acc.title')}</h1><p id="acc-sub"></p></div>
            <div class="top-tools">
              <label class="search">${icon('search')}<input id="q" placeholder="${t('acc.search')}" maxlength="100" aria-label="${t('acc.search')}"></label>
              <button class="icon-btn" data-act="refresh-presence" title="${t('acc.refreshTip')}">${icon('refresh')}</button>
              <button class="icon-btn" data-act="tools-menu" title="${t('acc.moreTip')}">${icon('more')}</button>
              <div class="split">
                <button class="btn primary" data-act="add-choose">${icon('userPlus')}<span>${t('acc.add')}</span></button>
                <button class="btn primary" data-act="add-menu" title="${t('acc.addMoreTip')}">${icon('chevronDown')}</button>
              </div>
            </div>
          </header>
          <div class="bar"><div class="chips" id="chips"></div><div class="seg" id="layout-seg"></div><button class="sort-btn" data-act="sort-menu" id="sort-btn"></button></div>
          <div class="scroll" id="scroll"><div id="grid"></div></div>
          <div class="dock">
            <button class="game-thumb" id="game-thumb" data-act="browse-games" title="${t('dock.browseTip')}">${icon('gamepad')}</button>
            <div class="picker" id="picker" hidden></div>
            <div class="dock-mid">
              <div class="dock-inputs">
                <label class="inwrap grow" title="${esc(t('dock.placeTip'))}">${icon('search')}<input id="place" autocomplete="off" placeholder="${t('dock.place')}" maxlength="500" spellcheck="false" aria-label="${t('dock.place')}"></label>
                <label class="inwrap job" title="${esc(t('dock.jobTip'))}">${icon('server')}<input id="job" placeholder="${t('dock.job')}" maxlength="60" spellcheck="false" aria-label="${t('dock.job')}"></label>
              </div>
              <div class="dock-info" id="dock-info" aria-live="polite"></div>
            </div>
            <div class="dock-actions" id="dock-actions"></div>
          </div>
        </section>
        <section class="view" id="view-settings" hidden>${settingsHtml()}</section>
      </main>
    </div>`;

    const st = S.app.settings;
    $('#place').value = st.lastPlaceInput || '';
    $('#job').value = st.lastJobId || '';
    $('#q').value = S.query;
    $('#q').addEventListener('input', e => { S.query = e.target.value; renderGrid(); });
    $('#place').addEventListener('input', onPlaceInput);
    $('#place').addEventListener('focus', () => { if (!$('#place').value.trim()) openPicker('browse'); });
    $('#place').addEventListener('keydown', onPlaceKey);
    $('#job').addEventListener('input', scheduleSaveInputs);
    $('#job').addEventListener('keydown', e => { if (e.key === 'Enter') launch([...S.selected], false); });
    wireSettings();
    setView(S.view);
    resolveGame();
  }

  function update() {
    renderSide();
    renderHeader();
    renderChips();
    renderGrid();
    renderDock();
    syncSettings();
  }

  function setView(view) {
    S.view = view;
    document.querySelectorAll('.nav').forEach(n => n.classList.toggle('on', n.dataset.view === view));
    $('#view-accounts').hidden = view !== 'accounts';
    $('#view-settings').hidden = view !== 'settings';
  }

  function renderSide() {
    const m = S.app.multi;
    const cls = !m.enabled ? 'off' : m.armed ? 'ok' : 'warn';
    const text = t(!m.enabled ? 'multi.off' : m.armed ? 'multi.ready' : 'multi.notReady');
    setHtml($('#multi-card'), `<button class="multi ${cls}" data-act="multi-help" title="${t('multi.title')}: ${esc(text)}"><span class="led"></span><div><b>${t('multi.title')}</b><small>${text}</small></div>${icon('chevronRight')}</button>`);
    setHtml($('#side-warn'), S.app.robloxInstalled ? '' : `<div class="warn-card">${t('side.noRoblox')}</div>`);
    const u = S.app.update;
    setHtml($('#update-slot'), u?.status === 'ready'
      ? `<button class="btn primary w update-btn" data-act="apply-update" title="${esc(t('upd.restartTip', { v: u.version }))}">${icon('refresh')}<span><b>${t('upd.restart')}</b><small>${t('upd.versionN', { v: esc(u.version) })}</small></span></button>` : '');
    $('#nav-count').textContent = accounts().length;
    const rv = S.app.robloxVersion || {};
    const short = v => (v || '').split('.').slice(0, 2).join('.');
    setHtml($('#sys-ver'), rv.installed
      ? `<span title="${esc(`Roblox ${rv.installed}`)}"><i class="dot ${rv.upToDate === false ? 'warn' : rv.upToDate ? 'ok' : ''}"></i>Roblox ${esc(short(rv.installed))}${rv.upToDate === false
        ? ` · ${t('rbx.available', { v: esc(short(rv.latest)) })}` : rv.upToDate ? ` · ${t('rbx.latestShort')}` : ''}</span>` : '');
    renderMetrics();
  }

  function renderHeader() {
    const list = accounts();
    const n = S.selected.size;
    let html = t('acc.summary', { n: list.length, p: list.filter(inGame).length, o: list.filter(isOnline).length });
    if (n) html += `<span class="sep">·</span>${t('acc.selected', { n })}`;
    else if (list.length) html += `<span class="sep">·</span>${t('acc.selectAll')}`;
    setHtml($('#acc-sub'), html);
  }

  function renderChips() {
    const list = accounts();
    const groups = [...new Set(list.map(a => a.group).filter(Boolean))].sort((a, b) => a.localeCompare(b, locale()));
    const chips = [
      { f: 'all', text: t('chip.all'), n: list.length },
      { f: 'ingame', text: t('chip.ingame'), n: list.filter(inGame).length, dot: 'var(--ok)' },
      { f: 'online', text: t('chip.online'), n: list.filter(isOnline).length, dot: 'var(--online)' },
      { f: 'offline', text: t('chip.offline'), n: list.filter(a => !isOnline(a)).length, dot: 'var(--dim)' },
      { f: 'local', text: t('chip.local'), n: list.filter(isLocal).length, hide: !list.some(isLocal) },
      { f: 'invalid', text: t('chip.invalid'), n: list.filter(a => a.cookieValid === false).length, dot: 'var(--bad)', hide: !list.some(a => a.cookieValid === false) },
      ...groups.map(g => ({ f: `g:${g}`, text: esc(g), n: list.filter(a => a.group === g).length })),
    ].filter(c => !c.hide);
    if (!chips.some(c => c.f === S.filter)) S.filter = 'all';
    setHtml($('#chips'), list.length ? chips.map(c => `<button class="chip ${S.filter === c.f ? 'on' : ''}" data-act="filter" data-f="${esc(c.f)}" aria-pressed="${S.filter === c.f}">${c.dot ? `<i class="dot" style="background:${c.dot}"></i>` : ''}${c.text}<em>${c.n}</em></button>`).join('') : '');
    setHtml($('#layout-seg'), list.length ? ['cards', 'list'].map(l =>
      `<button class="${S.layout === l ? 'on' : ''}" data-act="layout" data-l="${l}" title="${t(`layout.${l}`)}" aria-label="${t(`layout.${l}`)}" aria-pressed="${S.layout === l}">${icon(l === 'cards' ? 'grid' : 'rows')}</button>`).join('') : '');
    const sortBtn = $('#sort-btn');
    sortBtn.hidden = list.length < 2;
    setHtml(sortBtn, `${icon('sort')}<span>${t('sort.label')}: ${t(`sort.${S.sort}`)}</span>`);
  }

  const statusRank = a => isLocal(a) ? 0 : a.state === 'launching' ? 1 : a.presence?.type === 'ingame' ? 2
    : ['online', 'studio'].includes(a.presence?.type) ? 3 : a.cookieValid === false ? 5 : 4;

  function visible() {
    const q = S.query.trim().toLowerCase();
    const f = S.filter;
    const byName = (x, y) => (x.group || '￿').localeCompare(y.group || '￿', locale()) || x.label.localeCompare(y.label, locale());
    const compare = S.sort === 'status' ? (x, y) => statusRank(x) - statusRank(y) || byName(x, y)
      : S.sort === 'recent' ? (x, y) => (y.lastUsed || '').localeCompare(x.lastUsed || '') || byName(x, y)
      : byName;
    return accounts().filter(a => {
      if (f === 'ingame' && !inGame(a)) return false;
      if (f === 'online' && !isOnline(a)) return false;
      if (f === 'offline' && isOnline(a)) return false;
      if (f === 'local' && !isLocal(a)) return false;
      if (f === 'invalid' && a.cookieValid !== false) return false;
      if (f.startsWith('g:') && a.group !== f.slice(2)) return false;
      return !q || [a.alias, a.username, a.displayName, a.group, a.note, String(a.userId), a.presence?.game]
        .some(v => (v || '').toLowerCase().includes(q));
    }).sort(compare);
  }

  /** Roblox's disconnect codes, as the client logs them (278 seen in practice; the others are Roblox's documented ones). */
  const KICK = { 278: 'kick.idle', 267: 'kick.game', 273: 'kick.elsewhere', 277: 'kick.network', 279: 'kick.connect' };

  function activityHtml(a) {
    const p = a.presence;
    const gIcon = url => url ? `<img class="gicon" src="${esc(url)}" alt="" loading="lazy" decoding="async">` : `<span class="gicon">${icon('gamepad')}</span>`;
    const box = (cls, iconHtml, k, v, raw = false) => `<div class="activity ${cls}">${iconHtml}<div class="txt"><div class="k">${k}</div><div class="v"${raw ? '' : ` title="${esc(v)}"`}>${raw ? v : esc(v)}</div></div></div>`;
    if (a.state === 'launching') return box('launching', '<span class="gicon"><span class="spin"></span></span>', t('act.opening'), a.session?.game?.name || t('act.waitWindow'));
    if (isLocal(a)) {
      const game = p?.type === 'ingame' && p.game ? { name: p.game, iconUrl: p.iconUrl } : a.session?.game;
      const c = a.session?.connection;
      if (c === 'disconnected') {
        const why = a.session.reason ? (KICK[a.session.reason] ? t(KICK[a.session.reason]) : t('kick.code', { code: a.session.reason })) : '';
        return box('kicked', `<span class="gicon">${icon('alert')}</span>`, t('act.kicked'),
          why ? `${why} · ${game?.name || 'Roblox'}` : t('act.kickedWhere', { game: game?.name || 'Roblox' }));
      }
      if (c === 'menu') return box('online', `<span class="gicon">${icon('monitor')}</span>`, t('act.menuHere'), t('act.menuWhere'));
      return box('ingame', gIcon(game?.iconUrl), t(c === 'ingame' ? 'act.playingHere' : 'act.joiningHere'), game?.name || 'Roblox');
    }
    switch (p?.type) {
      case 'ingame': return box('ingame', gIcon(p.iconUrl), t('act.playingElse'), p.game || t('act.hidden'));
      case 'online': return box('online', `<span class="gicon">${icon('globe')}</span>`, t('act.online'), t('act.onlineWhere'));
      case 'studio': return box('studio', `<span class="gicon">${icon('code')}</span>`, t('act.studio'), p.game || t('act.studioWhat'));
      case 'offline':
      case 'invisible':
        return p.lastOnline
          ? box('offline', `<span class="gicon">${icon('moon')}</span>`, t('act.offline'), `<span data-ago="${esc(p.lastOnline)}" data-tpl="act.lastOnline"></span>`, true)
          : box('offline', `<span class="gicon">${icon('moon')}</span>`, t('act.offline'), t('act.inactive'));
      default:
        return a.cookieValid === false
          ? box('signedout', `<span class="gicon">${icon('logIn')}</span>`, t('act.signedOut'), t('act.signInAgain'))
          : box('unknown', `<span class="gicon">${icon('info')}</span>`, t('act.unknown'), t('act.updating'));
    }
  }

  function cardHtml(a) {
    const sel = S.selected.has(a.id);
    const pType = a.session?.connection === 'disconnected' && isLocal(a) ? 'kicked' : isLocal(a) ? 'ingame' : a.presence?.type || '';
    const ring = a.cookieValid === false ? 'invalid' : ['ingame', 'online', 'kicked'].includes(pType) ? pType : '';
    const badge = isLocal(a) ? `<span class="badge run"><i class="pulse"></i>${t('card.local')}</span>`
      : a.state === 'launching' ? `<span class="badge wait"><span class="spin"></span>${t('card.launching')}</span>`
      : a.cookieValid === false ? `<span class="badge bad">${t('card.expired')}</span>` : '';
    const sub = [`@${a.username}`, a.displayName && a.displayName !== a.username && a.displayName !== a.label ? a.displayName : ''].filter(Boolean).join(' · ');
    return `<article class="card ${sel ? 'sel' : ''} ${isLocal(a) ? 'local' : ''}" data-id="${a.id}" data-act="card" aria-selected="${sel}">
      <div class="card-top"><span class="check">${icon('check')}</span>${badge}</div>
      <div class="avatar ${ring}">${avatarInner(a)}<i class="pdot ${pType}"></i></div>
      <div class="name" title="${esc(a.label)}">${esc(a.label)}</div>
      <div class="sub" title="${esc(sub)}">${esc(sub)}</div>
      ${activityHtml(a)}
      ${isLocal(a) ? `<div class="live" data-live="${a.id}"><span title="${t('card.ramTip')}">${icon('chip')}<b class="ram">…</b></span><span title="${t('card.upTip')}">${icon('clock')}<b class="up"></b></span></div>` : ''}
      <div class="meta">${vngTag(a)}${a.group ? `<span class="tag" title="${esc(a.group)}">${esc(a.group)}</span>` : ''}${a.autoRejoin ? `<span class="rejoin" title="${t('card.rejoinTip')}">${icon('repeat')}</span>` : ''}<span class="ago" title="${t('card.lastTip')}">${icon('history')}${a.lastUsed ? `<span data-ago="${esc(a.lastUsed)}"></span>` : t('card.never')}</span></div>
      ${a.note ? `<div class="note" title="${esc(a.note)}">${esc(a.note)}</div>` : ''}
      <div class="card-actions">
        ${isLocal(a)
          ? `<button class="pill-btn stop" data-act="close-one">${icon('power')}${t('card.close')}</button>`
          : a.cookieValid === false
            ? `<button class="pill-btn relogin" data-act="relogin" title="${t('card.reloginTip')}">${icon('logIn')}${t('card.relogin')}</button>`
            : `<button class="pill-btn play" data-act="play-one" ${a.state === 'launching' ? 'disabled' : ''}>${icon('play')}${t('card.play')}</button>`}
        <button class="icon-btn" data-act="edit" title="${t('card.editTip')}">${icon('edit')}</button>
        <button class="icon-btn" data-act="card-menu" title="${t('card.moreTip')}">${icon('more')}</button>
      </div>
    </article>`;
  }

  /** One line per account for the compact list; the same actions as a card (select, right-click menu, play). */
  const vngTag = a => a.realm === 'vng' ? `<span class="tag vng" title="${t('card.vngTip')}">VNG</span>` : '';

  function rowHtml(a) {
    const sel = S.selected.has(a.id);
    const pType = a.session?.connection === 'disconnected' && isLocal(a) ? 'kicked' : isLocal(a) ? 'ingame' : a.presence?.type || '';
    const ring = a.cookieValid === false ? 'invalid' : ['ingame', 'online', 'kicked'].includes(pType) ? pType : '';
    const st = statusLine(a);
    const action = isLocal(a) ? `<button class="icon-btn stop" data-act="close-one" title="${t('card.close')}" aria-label="${t('card.close')}">${icon('power')}</button>`
      : a.cookieValid === false ? `<button class="icon-btn relogin" data-act="relogin" title="${t('card.relogin')}" aria-label="${t('card.relogin')}">${icon('logIn')}</button>`
      : `<button class="icon-btn play" data-act="play-one" title="${t('card.play')}" aria-label="${t('card.play')}" ${a.state === 'launching' ? 'disabled' : ''}>${icon('play')}</button>`;
    return `<article class="card row ${sel ? 'sel' : ''} ${isLocal(a) ? 'local' : ''}" data-id="${a.id}" data-act="card" aria-selected="${sel}">
      <span class="check">${icon('check')}</span>
      <div class="avatar sm ${ring}">${avatarInner(a)}<i class="pdot ${pType}"></i></div>
      <div class="row-name"><b title="${esc(a.label)}">${esc(a.label)}</b><small>@${esc(a.username)}</small></div>
      <div class="row-status ${st.cls}" title="${esc(st.text)}">${esc(st.text)}</div>
      <div class="row-meta">${vngTag(a)}${a.group ? `<span class="tag" title="${esc(a.group)}">${esc(a.group)}</span>` : ''}${a.autoRejoin ? `<span class="rejoin" title="${t('card.rejoinTip')}">${icon('repeat')}</span>` : ''}</div>
      <div class="row-live">${isLocal(a) ? `<span data-live="${a.id}" title="${t('card.ramTip')}"><b class="ram">…</b><b class="up" hidden></b></span>` : ''}</div>
      <div class="row-actions">${action}<button class="icon-btn" data-act="card-menu" title="${t('card.moreTip')}" aria-label="${t('card.moreTip')}">${icon('more')}</button></div>
    </article>`;
  }

  /** The account's status in a few words, for the compact list. */
  function statusLine(a) {
    const p = a.presence;
    if (a.state === 'launching') return { cls: 'launching', text: t('act.opening') };
    if (isLocal(a)) {
      const c = a.session?.connection;
      const game = (p?.type === 'ingame' && p.game) || a.session?.game?.name || 'Roblox';
      if (c === 'disconnected') {
        const why = a.session.reason ? (KICK[a.session.reason] ? t(KICK[a.session.reason]) : t('kick.code', { code: a.session.reason })) : t('row.kicked');
        return { cls: 'kicked', text: `${why} · ${game}` };
      }
      if (c === 'menu') return { cls: 'online', text: t('act.menuHere') };
      return { cls: 'ingame', text: `${t(c === 'ingame' ? 'row.playing' : 'row.joining')} · ${game}` };
    }
    switch (p?.type) {
      case 'ingame': return { cls: 'ingame', text: `${t('row.elsewhere')} · ${p.game || t('act.hidden')}` };
      case 'online': return { cls: 'online', text: t('act.online') };
      case 'studio': return { cls: 'studio', text: t('act.studio') };
      case 'offline':
      case 'invisible': return { cls: 'offline', text: t('act.offline') };
      default: return a.cookieValid === false ? { cls: 'signedout', text: t('act.signedOut') } : { cls: 'unknown', text: t('act.unknown') };
    }
  }

  function renderGrid() {
    const grid = $('#grid');
    if (!grid) return;
    const list = accounts();
    let html;
    if (!list.length) {
      html = `<div class="empty">
        <img class="art" src="${LOGO}" alt="">
        <h2>${t('empty.title')}</h2>
        <p>${t('empty.text')}</p>
        ${addOptions()}
      </div>`;
    } else {
      const shown = visible();
      // Cards fade in only on the first render, not on every live update.
      const list = S.layout === 'list';
      html = shown.length ? `<div class="grid ${list ? 'list' : ''} ${S.introShown ? '' : 'intro'}">${shown.map(list ? rowHtml : cardHtml).join('')}</div>`
        : `<div class="empty"><h2>${t('nores.title')}</h2><p>${t('nores.text')}</p></div>`;
    }
    if (setHtml(grid, html)) {
      patchTimes();
      renderMetrics();
    }
    if (list.length) S.introShown = true;
  }

  /** Replaces markup only when it changed, so frequent state pushes do not rebuild the DOM. Returns true if replaced. */
  function setHtml(el, html) {
    if (!el || el._html === html) return false;
    el._html = html;
    el.innerHTML = html;
    return true;
  }

  function renderDock() {
    const actions = $('#dock-actions');
    if (!actions) return;
    const n = S.selected.size;
    const sel = [...S.selected].map(byId).filter(Boolean);
    setHtml(actions, S.app.launching
      ? `<button class="play-btn stop" data-act="cancel-launch">${icon('stop')}<span>${t('dock.stop')}</span></button>`
      : `<button class="btn ghost" data-act="arrange" title="${t('dock.arrangeTip')}" aria-label="${t('dock.arrangeTip')}">${icon('grid')}</button>
         <button class="btn ghost" data-act="open-app" ${n ? '' : 'disabled'} title="${t('dock.openAppTip')}" aria-label="${t('dock.openAppTip')}">${icon('monitor')}</button>
         <button class="btn ghost" data-act="close-sel" ${sel.some(isLocal) ? '' : 'disabled'} title="${t('dock.closeTip')}" aria-label="${t('dock.closeTip')}">${icon('power')}</button>
         <button class="play-btn" data-act="launch" ${n ? '' : 'disabled'}>${icon('play')}<span>${t(n ? 'dock.play' : 'dock.pick')}</span>${n ? `<em>${n}</em>` : ''}</button>`);

    const g = S.game;
    setHtml($('#game-thumb'), g?.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : icon('gamepad'));
    let info;
    if (S.progress) {
      info = `<span class="spin"></span><span class="progress">${esc(S.progress)}</span>`;
    } else if (!g) {
      info = `<span class="hint">${t('dock.hint')}</span>`;
    } else if (g.loading) {
      info = `<span class="spin"></span><span>${t('dock.finding')}</span>`;
    } else if (!g.ok) {
      info = `<span class="err">${esc(g.error)}</span>`;
    } else if (g.kind === 'share') {
      info = `<span class="kind">${t('dock.share')}</span><span>${t('dock.shareNote')}</span>`;
    } else {
      const kind = g.kind === 'private' ? `<span class="kind">${t('dock.private')}</span>` : g.kind === 'server' ? `<span class="kind">${t('dock.server')}</span>` : '';
      info = `<span class="gname" title="${esc(g.name)}">${esc(g.name || `Place ${g.placeId}`)}</span>${kind}${g.creator ? `<span class="creator">${t('dock.by', { c: esc(g.creator) })}</span>` : ''}${g.playing ? `<span class="players">${t('dock.playing', { n: fmtNum(g.playing) })}</span>` : ''}`
        + (g.placeId && g.kind !== 'private' ? `<button class="srv-open" data-act="servers">${icon('server')}<span>${t('srv.button')}</span></button>` : '');
    }
    const recents = (S.app.settings?.recentGames || []).slice(0, 3);
    if (recents.length && !S.progress) {
      info += `<div class="recents">${recents.map(r => `<button class="recent" data-act="recent" data-place="${r.placeId}" title="${esc(r.name)}">${r.iconUrl ? `<img src="${esc(r.iconUrl)}" alt="">` : '<i class="ph"></i>'}<span>${esc(r.name)}</span><i class="rm" data-act="recent-remove" data-place="${r.placeId}" title="${t('dock.recentRemove')}">${icon('x')}</i></button>`).join('')}</div>`;
    }
    setHtml($('#dock-info'), info);
  }

  /** Fills relative times ("5 min ago") in place, so they never force the cards to be rebuilt. */
  function patchTimes() {
    document.querySelectorAll('[data-ago]').forEach(el => {
      const text = el.dataset.tpl ? t(el.dataset.tpl, { t: ago(el.dataset.ago) }) : ago(el.dataset.ago);
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
    setText($('#sys-count-label'), t(m.robloxCount === 1 ? 'sys.window1' : 'sys.windows'));
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
  function settingsHtml() {
    const row = (title, desc, ctl, extra = '') => `<div class="set-row"><div><div class="t">${title}${extra}</div>${desc ? `<div class="d">${desc}</div>` : ''}</div><div class="ctl">${ctl}</div></div>`;
    const sw = id => `<label class="switch"><input type="checkbox" id="${id}"><span></span></label>`;
    return `<header class="top"><div class="top-title"><h1>${t('set.title')}</h1></div></header>
      <div class="scroll"><div class="settings">
        <div class="set-card">
          <h3>${icon('layers')}${t('set.launch')}</h3>
          ${row(t('set.multi'), `${t('set.multiDesc')} <a data-act="multi-help">${t('set.details')}</a>`, sw('s-multi'), ' <span class="state-pill" id="s-multi-pill"></span>')}
          ${row(t('set.rename'), t('set.renameDesc'), sw('s-rename'))}
          ${row(t('set.client'), t('set.clientDesc'), '<select class="select" id="s-launch" aria-label="Roblox client"></select>')}
          <div id="s-store"></div>
          <div id="s-rbxver"></div>
          ${row(t('set.delay'), t('set.delayDesc'), '<input type="range" id="s-delay" min="0" max="60" step="1"><b id="s-delay-v" class="range-v"></b>')}
        </div>
        <div class="set-card">
          <h3>${icon('sliders')}${t('rc.title')}</h3>
          ${row(t('rc.mode'), '<span id="s-rc-hint"></span>', `<select class="select" id="s-rc-mode">${['off', 'all', 'marked'].map(m => `<option value="${m}">${t(`rc.mode.${m}`)}</option>`).join('')}</select>`)}
          ${row(t('rc.graphics'), '', `<select class="select" id="s-rc-graphics"><option value="0">${t('rc.keep')}</option>${[1, 2, 3, 4, 5, 6, 7, 8, 9, 10].map(n => `<option value="${n}">${n}</option>`).join('')}</select>`)}
          ${row(t('rc.fps'), '', `<select class="select" id="s-rc-fps"><option value="0">${t('rc.keep')}</option>${[30, 60, 144, 240].map(n => `<option value="${n}">${n} FPS</option>`).join('')}</select>`)}
          ${row(t('rc.volume'), '', `<select class="select" id="s-rc-volume"><option value="-1">${t('rc.keep')}</option><option value="0">${t('rc.mute')}</option>${[25, 50, 75, 100].map(n => `<option value="${n}">${n}%</option>`).join('')}</select>`)}
        </div>
        <div class="set-card">
          <h3>${icon('gauge')}${t('set.perf')}</h3>
          ${row(t('set.rejoin'), t('set.rejoinDesc'), '<span class="state-pill off" id="s-rejoin-count"></span>')}
          ${row(t('set.memWarn'), '', sw('s-memwarn'))}
          ${row(t('set.antiAfk'), t('set.antiAfkDesc'), sw('s-antiafk'))}
          ${row(t('set.trim'), t('set.trimDesc'), sw('s-trim'))}
          ${row(t('set.gpu'), t('set.gpuDesc'), sw('s-gpu'))}
        </div>
        <div class="set-card">
          <h3>${icon('languages')}${t('set.language')}</h3>
          ${row(t('set.language'), '', langSwitch())}
        </div>
        <div class="set-card">
          <h3>${icon('key')}${t('set.master')}</h3>
          <form class="pw-form" id="pw-form" autocomplete="off">
            <input class="field" type="password" id="pw-cur" placeholder="${t('set.pwCur')}" aria-label="${t('set.pwCur')}" maxlength="256">
            <input class="field" type="password" id="pw-new" placeholder="${t('set.pwNew')}" aria-label="${t('set.pwNew')}" maxlength="256">
            <input class="field" type="password" id="pw-new2" placeholder="${t('set.pwNew2')}" aria-label="${t('set.pwNew2')}" maxlength="256">
            <button class="btn primary" type="submit" id="pw-btn">${t('set.pwBtn')}</button>
          </form>
        </div>
        <div class="set-card">
          <h3>${icon('download')}${t('upd.title')}</h3>
          <div id="s-update"></div>
          ${row(t('upd.auto'), '', sw('s-autoupd'))}
        </div>
        <div class="set-card">
          <h3>${icon('folder')}${t('set.data')}</h3>
          ${row(t('set.dataPath'), '<span class="path" id="s-path"></span>', `<button class="btn sm" data-act="open-data">${icon('folder')}${t('set.openFolder')}</button>`)}
        </div>
      </div></div>`;
  }

  function wireSettings() {
    const toggle = (id, key) => $(id).addEventListener('change', e => call('setSettings', { [key]: e.target.checked }));
    $('#s-multi').addEventListener('change', async e => {
      if (e.target.checked && !await confirmMulti()) {
        e.target.checked = false;
        return;
      }
      call('setSettings', { multiInstance: e.target.checked });
    });
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
    // Switching reloads the interface, so the reply may never come back; the reloaded page shows the new state.
    $('#s-gpu').addEventListener('change', e => {
      saveView();
      bridge.call('setRendering', { gpu: e.target.checked }).catch(() => {});
    });
    $('#s-launch').addEventListener('change', e => call('setSettings', { launchVia: e.target.value }));
    const delay = $('#s-delay');
    delay.addEventListener('input', () => { $('#s-delay-v').textContent = t('set.seconds', { n: delay.value }); });
    delay.addEventListener('change', () => call('setSettings', { launchDelaySeconds: +delay.value }));
    $('#pw-form').addEventListener('submit', async e => {
      e.preventDefault();
      const cur = $('#pw-cur'), next = $('#pw-new'), next2 = $('#pw-new2');
      if (next.value.length < 6) return toast(t('set.pwShort'), 'warn');
      if (next.value !== next2.value) return toast(t('set.pwMismatch'), 'warn');
      const ok = await busy($('#pw-btn'), () => call('changePassword', { current: cur.value, next: next.value }));
      if (ok) {
        cur.value = next.value = next2.value = '';
        toast(t('set.pwDone'), 'ok');
      }
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
    const status = !u.supported ? t('upd.unsupported')
      : u.status === 'checking' ? t('upd.checking')
      : u.status === 'downloading' ? t('upd.downloading', { v: u.version, p: u.progress })
      : u.status === 'ready' ? t('upd.ready', { v: u.version })
      : u.status === 'upToDate' ? t('upd.latest')
      : u.status === 'error' ? t('upd.error')
      : t('upd.idle');
    const notes = `<button class="btn sm ghost" data-act="whats-new">${t('wn.link')}</button>`;
    const action = !u.supported ? ''
      : u.status === 'ready' ? `<button class="btn sm primary" data-act="apply-update">${icon('refresh')}${t('upd.restart')}</button>`
      : `<button class="btn sm" data-act="check-updates" ${u.status === 'checking' || u.status === 'downloading' ? 'disabled' : ''}>${icon('refresh')}${t('upd.check')}</button>`;
    setHtml($('#s-update'), `<div class="set-row"><div><div class="t">Bloxkeep ${esc(u.current || '')}</div><div class="d">${esc(status)}</div></div><div class="ctl">${notes}${action}</div></div>`);
    $('#s-autoupd').checked = !!u.auto;
    $('#s-autoupd').disabled = !u.supported;
  }

  function syncSettings() {
    const st = S.app.settings, m = S.app.multi;
    if (!st || !$('#s-multi')) return;
    $('#s-multi').checked = m.enabled;
    $('#s-rename').checked = st.renameWindows;
    $('#s-memwarn').checked = st.memoryWarnings;
    $('#s-trim').checked = st.trimBackground;
    $('#s-antiafk').checked = !!st.antiAfk;
    const rc = st.robloxConfig || { mode: 'off', graphics: 0, fps: 0, volume: -1 };
    for (const [key, value] of [['mode', rc.mode], ['graphics', rc.graphics], ['fps', rc.fps], ['volume', rc.volume]]) {
      const el = $(`#s-rc-${key}`);
      if (document.activeElement !== el) el.value = String(value);
      if (key !== 'mode') el.disabled = rc.mode === 'off';
    }
    setText($('#s-rc-hint'), rc.mode === 'off' ? '' : t(rc.mode === 'all' ? 'rc.hint.all' : 'rc.hint.marked', { n: accounts().filter(a => a.useRobloxConfig).length }));
    $('#s-gpu').checked = !S.app.safeRendering;
    syncUpdate();
    const ins = S.app.installs || {};
    const options = [
      ['auto', ins.handler ? t('launch.auto', { name: esc(ins.handler) }) : t('launch.autoNone'), true],
      ['bloxstrap', 'Bloxstrap', ins.bloxstrap],
      ['fishstrap', 'Fishstrap', ins.fishstrap],
      ['roblox', t('launch.roblox'), ins.roblox],
      ['direct', t('launch.direct'), ins.direct],
    ];
    const select = $('#s-launch');
    setHtml(select, options.map(([value, label, ok]) =>
      `<option value="${value}" ${ok ? '' : 'disabled'}>${ok ? label : t('launch.missing', { name: label })}</option>`).join(''));
    if (document.activeElement !== select) select.value = st.launchVia || 'auto';
    const rv = S.app.robloxVersion || {};
    const rvText = !rv.installed ? t('rbx.notInstalled')
      : rv.upToDate === false ? t(rv.direct ? 'rbx.outdatedDirect' : 'rbx.outdated', { v: esc(rv.latest) })
      : rv.upToDate ? t('rbx.latest') : t('rbx.unknown');
    setHtml($('#s-rbxver'), `<div class="set-row"><div><div class="t">${t('rbx.title')}</div><div class="d">${rvText}</div></div><div class="ctl"><span class="state-pill ${rv.upToDate === false ? 'warn' : rv.upToDate ? 'ok' : 'off'}">${esc(rv.installed || '—')}</span></div></div>`);
    setHtml($('#s-store'), ins.store ? `<div class="set-note">${icon('alert')}<span>${t('launch.store')}</span></div>` : '');
    $('#s-rejoin-count').textContent = t('set.rejoinCount', { n: accounts().filter(a => a.autoRejoin).length });
    const delay = $('#s-delay');
    if (document.activeElement !== delay) delay.value = st.launchDelaySeconds;
    $('#s-delay-v').textContent = t('set.seconds', { n: delay.value });
    const pill = $('#s-multi-pill');
    pill.className = `state-pill ${!m.enabled ? 'off' : m.armed ? 'ok' : 'warn'}`;
    pill.textContent = t(!m.enabled ? 'pill.off' : m.armed ? 'pill.ready' : 'pill.notReady');
    $('#s-path').textContent = S.app.dataPath;
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
    if (!P.open) return;
    const rows = pickerRows();
    const head = P.mode === 'browse'
      ? `<div class="picker-tabs" role="tablist">${TABS.filter(k => k !== 'popular' || P.lists?.popular?.length).map(k =>
          `<button class="ptab ${P.tab === k ? 'on' : ''}" role="tab" data-act="picker-tab" data-tab="${k}">${t(`picker.${k}`)}</button>`).join('')}</div>`
      : `<div class="picker-head">${t('picker.results', { q: esc(P.query) })}</div>`;
    const body = P.loading ? '<div class="game-row skel"><span class="gph"></span><div class="gtxt"><b></b><small></small></div></div>'.repeat(5)
      : !rows.length ? `<div class="picker-empty">${t(P.mode === 'search' ? 'picker.noResults' : `picker.empty.${P.tab}`)}</div>`
      : rows.map((g, i) => `<div class="game-row ${i === P.index ? 'on' : ''}" role="option" data-act="pick-game" data-i="${i}">
          ${g.iconUrl ? `<img src="${esc(g.iconUrl)}" alt="">` : `<span class="gph">${icon('gamepad')}</span>`}
          <div class="gtxt"><b title="${esc(g.name)}">${esc(g.name)}</b><small>${g.playing ? t('dock.playing', { n: fmtNum(g.playing) }) : `Place ${g.placeId}`}</small></div>
          <button class="icon-btn pin ${g.pinned ? 'on' : ''}" data-act="pin-game" data-i="${i}" title="${t(g.pinned ? 'picker.unpin' : 'picker.pin')}">${icon(g.pinned ? 'starFill' : 'star')}</button>
        </div>`).join('');
    setHtml(el, `${head}<div class="picker-list" role="listbox">${body}</div>`);
    el.querySelector('.game-row.on')?.scrollIntoView({ block: 'nearest' });
  }
  let searchTimer = 0;
  function onPlaceInput() {
    const value = $('#place').value.trim();
    scheduleSaveInputs();
    clearTimeout(searchTimer);
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
    Object.assign(P, { open: true, mode: 'search', query: value, loading: true, index: -1 });
    renderPicker();
    searchTimer = setTimeout(async () => {
      const seq = ++P.seq;
      const r = await call('searchGames', { query: value }, { silent: true });
      if (seq !== P.seq || !P.open) return;
      P.results = r || [];
      P.loading = false;
      renderPicker();
    }, 350);
  }
  function onPlaceKey(e) {
    const rows = pickerRows();
    if (P.open && (e.key === 'ArrowDown' || e.key === 'ArrowUp') && rows.length) {
      e.preventDefault();
      P.index = (P.index + (e.key === 'ArrowDown' ? 1 : rows.length - 1) + (P.index < 0 && e.key === 'ArrowUp' ? 1 : 0)) % rows.length;
      renderPicker();
    } else if (e.key === 'Enter') {
      e.preventDefault();
      if (P.open && rows.length && (P.index >= 0 || P.mode === 'search')) pickGame(rows[Math.max(0, P.index)]);
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
    if ($('#place').value.trim() !== `https://www.roblox.com/games/${placeId}`) {
      $('#place').value = `https://www.roblox.com/games/${placeId}`;
      resolveGame();
    }
    $('#job').value = jobId;
    scheduleSaveInputs();
  }

  /** The game's public servers (like the Servers tab on its Roblox page): pick one to join it. */
  function openServers() {
    const g = S.game;
    if (!g?.placeId) return;
    const st = { fewest: false, items: [], next: null, loading: false, busy: false, seq: 0 };
    let api;
    const rows = () => st.items.map((s, i) => `<div class="srv-row">
        <div class="srv-fill" title="${t('srv.players', { n: s.playing, max: s.maxPlayers })}"><i style="transform:scaleX(${s.maxPlayers ? (s.playing / s.maxPlayers).toFixed(3) : 0})"></i></div>
        <b>${t('srv.players', { n: s.playing, max: s.maxPlayers })}</b><span class="srv-ping">${s.ping ? t('srv.ping', { ms: s.ping }) : ''}</span>
        <button class="btn sm" data-srv="${i}">${t('srv.pick')}</button></div>`).join('');
    const render = () => {
      const list = api.el.querySelector('#srv-list');
      list.innerHTML = st.items.length ? rows() + (st.next ? `<button class="btn ghost w" data-more ${st.loading ? 'disabled' : ''}>${t('srv.more')}</button>` : '')
        : st.loading ? '<div class="picker-empty"><span class="spin"></span></div>'
        : st.busy ? `<div class="picker-empty">${t('srv.busy')} <button class="btn sm" data-retry>${t('srv.retry')}</button></div>`
        : `<div class="picker-empty">${t('srv.none')}</div>`;
      api.el.querySelectorAll('[data-sort]').forEach(b => b.classList.toggle('on', (b.dataset.sort === 'fewest') === st.fewest));
    };
    const load = async more => {
      const seq = ++st.seq;
      st.loading = true;
      if (!more) { st.items = []; st.next = null; }
      render();
      const r = await call('servers', { placeId: g.placeId, fewest: st.fewest, cursor: more ? st.next : null });
      if (seq !== st.seq || !api.el.isConnected) return;
      st.loading = false;
      st.busy = !!r?.busy && !st.items.length;
      if (r && !r.busy) { st.items = more ? st.items.concat(r.servers) : r.servers; st.next = r.next; }
      render();
      // Roblox asked to slow down: try once more by itself a few seconds later.
      if (r?.busy && !st.retried) { st.retried = true; setTimeout(() => api.el.isConnected && seq === st.seq && load(more), 5000); }
    };
    api = modal({
      title: t('srv.title', { game: esc(g.name || `Place ${g.placeId}`) }),
      body: `<div class="srv-sort"><button class="chip on" data-sort="most">${t('srv.most')}</button><button class="chip" data-sort="fewest">${t('srv.fewest')}</button></div><div class="srv-list" id="srv-list"></div>`,
      actions: [{ text: t('common.close'), onClick: ({ close }) => close() }],
      onMount: ({ el, close }) => el.addEventListener('click', e => {
        const sort = e.target.closest('[data-sort]');
        if (sort) { st.fewest = sort.dataset.sort === 'fewest'; load(false); return; }
        if (e.target.closest('[data-more]')) { load(true); return; }
        if (e.target.closest('[data-retry]')) { st.retried = false; load(false); return; }
        const pick = e.target.closest('[data-srv]');
        if (pick) {
          useServer(g.placeId, st.items[+pick.dataset.srv].jobId);
          close();
          toast(t('srv.chosen'), 'ok');
        }
      }),
    });
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
    resolveTimer = setTimeout(resolveGame, 450);
  }
  function scheduleSaveInputs() {
    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      if (S.shell) call('setSettings', { lastPlaceInput: $('#place')?.value ?? '', lastJobId: $('#job')?.value ?? '' }, { silent: true });
    }, 900);
  }
  async function resolveGame() {
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
    renderDock();
  }

  /* ───────────────────────── actions ───────────────────────── */
  function selectOnly(id) {
    S.selected = new Set([id]);
    S.anchor = id;
    renderSelection();
  }
  function renderSelection() {
    renderGrid();
    renderHeader();
    renderDock();
  }
  function selectAllVisible() {
    visible().forEach(a => S.selected.add(a.id));
    renderSelection();
  }
  function onCardClick(id, e) {
    const ids = visible().map(a => a.id);
    if (e.shiftKey && S.anchor && ids.includes(S.anchor)) {
      const [i, j] = [ids.indexOf(S.anchor), ids.indexOf(id)].sort((a, b) => a - b);
      ids.slice(i, j + 1).forEach(x => S.selected.add(x));
    } else {
      if (S.selected.has(id)) S.selected.delete(id); else S.selected.add(id);
      S.anchor = id;
    }
    renderSelection();
  }

  async function launch(ids, appOnly, force = false) {
    if (!ids.length) return toast(t('t.selectFirst'), 'warn');
    const placeInput = $('#place')?.value.trim() || '';
    if (!appOnly && !placeInput) {
      toast(t('t.needGame'), 'warn');
      $('#place')?.focus();
      return;
    }
    const r = await call('launch', { ids, appOnly, force, placeInput, jobId: $('#job')?.value.trim() || '' });
    if (!r) return;
    if (r.needsConfirm) {
      if (await confirmBox(r.message, { title: t('t.confirmTitle'), ok: t('t.openAnyway') })) return launch(ids, appOnly, true);
      return;
    }
    if (r.errors?.length) {
      modal({
        title: t(r.launched ? 't.errsSome' : 't.errsAll'),
        body: `<ul class="errs">${r.errors.map(x => `<li>${esc(x)}</li>`).join('')}</ul>`,
        actions: [{ text: t('common.close'), kind: 'primary', onClick: ({ close }) => close() }],
      });
    }
    if (r.launched) toast(t(r.cancelled ? 't.launchedStopped' : 't.launched', { n: r.launched }), 'ok');
    else if (r.cancelled) toast(t('t.stopped'), 'info');
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
  /** The ways to add an account, each with one line on what it means. */
  const addOptions = () => `<div class="start-opts">
      <button class="start-opt" data-act="add-password">${icon('key')}<span><b>${t('start.pw')}</b><small>${t('start.pwSub')}</small></span>${icon('chevronRight')}</button>
      <button class="start-opt" data-act="add-phone">${icon('qr')}<span><b>${t('start.phone')}</b><small>${t('start.phoneSub')}</small></span>${icon('chevronRight')}</button>
      <button class="start-opt" data-act="add-browser">${icon('globe')}<span><b>${t('start.browser')}</b><small>${t('start.browserSub')}</small></span>${icon('chevronRight')}</button>
    </div>
    <div class="start-more">${t('start.more')} <button class="link" data-act="add-cookie">${t('start.cookie')}</button> · <button class="link" data-act="import-ram" title="${t('start.ramTip')}">${t('start.ram')}</button></div>`;

  function addChooser() {
    modal({ title: t('add.title'), body: `<p class="modal-text">${t('empty.text')}</p>${addOptions()}` });
  }
  /** Picking a way closes the chooser it was picked from. */
  const leaveChooser = el => el.closest('.overlay')?.querySelector('[data-x]')?.click();

  /* ───────────────────────── who's playing ───────────────────────── */
  const whoAtUnlock = () => store.get('rm.who', '1') === '1';
  const whoBusy = new Set();
  let whoDrawn = '';

  /** Steam-style "Who's playing?": pick an account and Roblox opens on its home screen, already signed in. */
  function openWho() {
    if ($('#who')) return;
    closeMenu();
    const el = document.createElement('div');
    el.id = 'who';
    el.className = 'who';
    el.setAttribute('role', 'dialog');
    el.setAttribute('aria-modal', 'true');
    el.setAttribute('aria-label', t('who.title'));
    el.innerHTML = `<div class="who-bg" id="who-bg" aria-hidden="true"></div>
      <header class="who-top"><img src="${LOGO}" alt=""><span>Bloxkeep</span>
        <button class="who-manage" data-who-close>${t('who.manage')}</button></header>
      <main class="who-main">
        <h1>${t('who.title')}</h1>
        <label class="search who-search" hidden>${icon('search')}<input id="who-q" placeholder="${t('acc.search')}" maxlength="100" aria-label="${t('acc.search')}"></label>
        <div class="who-grid" id="who-grid"></div>
      </main>
      <footer class="who-foot"><label class="who-ask"><span>${t('who.ask')}</span><span class="switch"><input type="checkbox" id="who-ask"><span></span></span></label></footer>`;
    $('#layer').append(el);
    whoDrawn = '';
    const ask = el.querySelector('#who-ask');
    ask.checked = whoAtUnlock();
    ask.onchange = () => store.set('rm.who', ask.checked ? '1' : '0');
    el.querySelector('#who-q').addEventListener('input', renderWho);
    el.addEventListener('click', e => {
      if (e.target.closest('[data-who-close]')) return closeWho();
      if (e.target.closest('[data-who-add]')) return addChooser();
      const tile = e.target.closest('[data-who]');
      if (tile) pickAccount(tile.dataset.who);
    });
    el.addEventListener('keydown', e => {
      if (e.key === 'Escape' && !document.querySelector('#layer .overlay:not(.out)')) {
        e.stopPropagation();
        closeWho();
      }
    });
    renderWho();
    whoBackdrop();
    setTimeout(() => el.querySelector('.who-tile')?.focus(), 30);
  }

  function closeWho() {
    const el = $('#who');
    if (!el) return;
    el.classList.add('out');
    setTimeout(() => el.remove(), 150);
  }

  function renderWho() {
    const grid = $('#who-grid');
    if (!grid) return;
    const all = accounts();
    const qBox = $('#who-q');
    const searchable = all.length > 12;
    qBox.closest('.search').hidden = !searchable;
    const q = searchable ? qBox.value.trim().toLowerCase() : '';
    const time = a => Date.parse(a.lastUsed || '') || 0;
    const list = all
      .filter(a => !q || [a.label, a.username, a.displayName, a.group].some(v => (v || '').toLowerCase().includes(q)))
      .sort((x, y) => time(y) - time(x) || x.label.localeCompare(y.label));
    // State arrives every few seconds; only redraw when something on screen changes.
    const sig = list.map(a => [a.id, a.label, a.username, a.avatarUrl, a.cookieValid, a.realm, isLocal(a), whoBusy.has(a.id)].join('|')).join(';') + q;
    if (sig === whoDrawn && grid.childElementCount) return;
    whoDrawn = sig;
    const focused = document.activeElement?.closest?.('[data-who]')?.dataset.who;
    grid.innerHTML = list.map(a => {
      const off = a.cookieValid === false;
      return `<button class="who-tile ${off ? 'off' : ''} ${whoBusy.has(a.id) ? 'busy' : ''}" data-who="${a.id}" title="${esc(a.label)}${a.username !== a.label ? ` (@${esc(a.username)})` : ''}">
          <span class="who-pic">${avatarInner(a)}${isLocal(a) ? `<i class="who-on">${t('who.open')}</i>` : ''}${a.realm === 'vng' ? '<i class="who-vng">VNG</i>' : ''}<span class="who-busy"><span class="spin"></span></span></span>
          <span class="who-name">${esc(a.label)}</span>${off ? `<span class="who-sub">${t('who.signIn')}</span>` : ''}
        </button>`;
    }).join('') + (q ? '' : `<button class="who-tile add" data-who-add title="${t('who.add')}"><span class="who-pic">${icon('plus')}</span><span class="who-name">${t('who.add')}</span></button>`);
    if (focused) grid.querySelector(`[data-who="${focused}"]`)?.focus();
  }

  /** A faint wall of game art behind the tiles, from the games these accounts play. */
  async function whoBackdrop() {
    const icons = new Set();
    const add = games => (games || []).forEach(g => { if (g?.iconUrl && icons.size < 24) icons.add(g.iconUrl); });
    add(S.app.settings?.recentGames);
    add(S.app.settings?.pinnedGames);
    if (icons.size < 12) {
      const lists = await call('gameLists', {}, { silent: true });
      if (lists) Object.values(lists).forEach(v => Array.isArray(v) && add(v));
    }
    if (!$('#who-bg') || icons.size < 6) return; // a handful repeated looks like a pattern, not a wall of games
    const cols = 8, rows = 4, size = 150, gap = 16;
    const art = [...icons];
    while (art.length < cols * rows) art.push(...art.slice(0, cols * rows - art.length));
    const load = src => new Promise(done => {
      const img = new Image();
      img.onload = () => done(img);
      img.onerror = () => done(null);
      img.src = src;
    });
    const imgs = (await Promise.all(art.map(load))).filter(Boolean);
    const bg = $('#who-bg');
    if (!bg || imgs.length < 6) return;
    // Blurred once into a still picture: a live CSS blur keeps costing CPU for as long as the screen is open.
    const canvas = document.createElement('canvas');
    canvas.width = cols * (size + gap);
    canvas.height = rows * (size + gap);
    const ctx = canvas.getContext('2d');
    ctx.filter = 'blur(14px)';
    imgs.forEach((img, n) => ctx.drawImage(img, (n % cols) * (size + gap), Math.floor(n / cols) * (size + gap), size, size));
    bg.replaceChildren(canvas);
  }

  async function pickAccount(id) {
    const a = accounts().find(x => x.id === id);
    if (!a || whoBusy.has(id)) return;
    if (a.cookieValid === false) return quickLogin(a.realm);
    whoBusy.add(id);
    renderWho();
    try {
      let r = await call('switchTo', { id });
      if (r?.needsConfirm) {
        r = await confirmBox(r.message, { title: t('who.switchTitle'), ok: t('who.switch') }) ? await call('switchTo', { id, force: true }) : undefined;
      }
      if (!r) return;
      if (r.errors?.length) return toast(r.errors.join('\n'), 'error');
      if (!r.focused) toast(t('who.opening', { name: a.label }), 'ok');
      closeWho();
    } finally {
      whoBusy.delete(id);
      renderWho();
    }
  }

  /**
   * Username + password sign-in: works on a PC with no phone and no roblox.com (the values go only to Roblox's/VNG's
   * official auth host, over HTTPS). Handles a 2-step code; a captcha can't be shown here, so it points to the browser.
   */
  function passwordLogin(forRealm) {
    const added = [];
    let realm = forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : '');
    let step = 'login', method = 'authenticator';
    const m = modal({
      title: t('pw.title'),
      body: `<div id="pw-body"></div>`,
      onClose: () => {
        if (!added.length) return;
        S.selected = new Set(added.map(a => a.id));
        S.anchor = added[added.length - 1].id;
        renderSelection();
      },
    });
    const body = () => m.el.querySelector('#pw-body');
    const err = msg => { const e = m.el.querySelector('#pw-err'); if (e) { e.textContent = msg; e.hidden = !msg; } };

    const realmTabs = () => `<div class="ql-realm" role="tablist">
        <button type="button" data-realm="" class="${realm === '' ? 'on' : ''}">Roblox</button>
        <button type="button" data-realm="vng" class="${realm === 'vng' ? 'on' : ''}">Roblox VNG</button>
      </div>`;

    function render() {
      if (step === 'captcha') {
        body().innerHTML = `<div class="pw-captcha">${icon('alert')}
          <p>${t('pw.captcha')}</p>
          <div class="pw-captcha-acts"><button type="button" class="btn" data-x2="phone">${t('start.phone')}</button>
          <button type="button" class="btn" data-x2="browser">${t('start.browser')}</button></div></div>`;
      } else if (step === 'twostep') {
        body().innerHTML = `<p class="pw-2fa-lead">${t('pw.twoLead')}</p>
          <div class="seg wide" id="pw-method">
            <button type="button" data-m="authenticator" class="${method === 'authenticator' ? 'on' : ''}">${t('pw.auth')}</button>
            <button type="button" data-m="email" class="${method === 'email' ? 'on' : ''}">${t('pw.email')}</button>
          </div>
          <form id="pw-two" autocomplete="off">
            <input class="field pw-code" id="pw-otp" inputmode="numeric" maxlength="6" autocomplete="one-time-code" placeholder="••••••" aria-label="${t('pw.code')}">
            <div class="form-error" id="pw-err" hidden></div>
            <button class="btn primary w" type="submit" id="pw-verify">${t('pw.verify')}</button>
          </form>`;
      } else {
        body().innerHTML = `${realmTabs()}
          <form id="pw-login" autocomplete="off" class="pw-form2">
            <input class="field" id="pw-user" maxlength="60" autocomplete="off" placeholder="${t('pw.userPh')}" aria-label="${t('pw.user')}">
            <div class="pw"><input class="field" type="password" id="pw-pass" maxlength="200" autocomplete="off" placeholder="${t('pw.passPh')}" aria-label="${t('pw.pass')}">
              <button type="button" class="icon-btn" id="pw-eye" title="${t('lock.showPw')}">${icon('eye')}</button></div>
            <div class="form-error" id="pw-err" hidden></div>
            <div class="pw-added" id="pw-added"></div>
            <button class="btn primary w" type="submit" id="pw-go">${t('pw.go')}</button>
          </form>
          <p class="pw-note">${t('pw.note')}</p>`;
      }
      wire();
    }

    function wire() {
      m.el.querySelectorAll('[data-realm]').forEach(b => b.onclick = () => {
        if (b.dataset.realm === realm) return;
        realm = b.dataset.realm;
        store.set('rm.qlRealm', realm);
        render();
      });
      m.el.querySelectorAll('[data-m]').forEach(b => b.onclick = () => { method = b.dataset.m; render(); });
      m.el.querySelectorAll('[data-x2]').forEach(b => b.onclick = () => {
        m.close();
        if (b.dataset.x2 === 'browser') addBrowser(null); else quickLogin();
      });
      const eye = m.el.querySelector('#pw-eye');
      if (eye) eye.onclick = () => { const i = m.el.querySelector('#pw-pass'); const show = i.type === 'password'; i.type = show ? 'text' : 'password'; eye.innerHTML = icon(show ? 'eyeOff' : 'eye'); };
      const loginForm = m.el.querySelector('#pw-login');
      if (loginForm) loginForm.onsubmit = e => { e.preventDefault(); submitLogin(); };
      const twoForm = m.el.querySelector('#pw-two');
      if (twoForm) twoForm.onsubmit = e => { e.preventDefault(); submitTwoStep(); };
      setTimeout(() => m.el.querySelector('#pw-user, #pw-otp')?.focus(), 30);
    }

    async function submitLogin() {
      const user = m.el.querySelector('#pw-user').value.trim();
      const pass = m.el.querySelector('#pw-pass').value;
      if (!user || !pass) return err(t('pw.need'));
      err('');
      let r;
      try { r = await busy(m.el.querySelector('#pw-go'), () => bridge.call('loginPassword', { username: user, password: pass, realm })); }
      catch (e) { return err(e.message); }
      handle(r);
    }

    async function submitTwoStep() {
      const code = m.el.querySelector('#pw-otp').value.trim();
      if (!code) return err(t('pw.needCode'));
      err('');
      let r;
      try { r = await busy(m.el.querySelector('#pw-verify'), () => bridge.call('loginTwoStep', { method, code })); }
      catch (e) { return err(e.message); }
      handle(r);
    }

    function handle(r) {
      if (!r) return;
      if (r.status === 'captcha') { step = 'captcha'; return render(); }
      if (r.status === 'twostep') { step = 'twostep'; return render(); }
      if (r.status === 'added') {
        const a = r.added;
        added.push(a);
        toast(t(a.merged ? 't.merged' : 't.added', { name: a.label }), 'ok');
        step = 'login';
        render();
        const note = m.el.querySelector('#pw-added');
        if (note) note.textContent = t('ql.addedList', { names: added.map(x => x.label).join(', ') });
      }
    }

    render();
  }

  /**
   * Quick Login: Roblox shows a code that the user confirms on any device already signed in (phone, or the Roblox app).
   * The code is renewed after each account, so several can be added in a row, like the browser sign-in.
   */
  function quickLogin(forRealm) {
    const added = [];
    let gen = 0, timer = 0, ticker = 0, seconds = 0, phase = '', expiredInRow = 0, closed = false;
    // Roblox VNG (Vietnam) accounts confirm the code in the Roblox VNG app and sign in on VNG's servers.
    let realm = forRealm ?? (store.get('rm.qlRealm', '') === 'vng' ? 'vng' : '');
    const m = modal({
      title: t('ql.title'),
      body: `<div class="ql-realm" role="tablist">
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
        if (my === gen && !closed) show('error', `<span>${esc(e.message)}</span>${newCodeBtn}`, 'bad');
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
          show('linked', `${r.picture ? `<img class="ql-pic" src="${esc(r.picture)}" alt="">` : '<span class="spin"></span>'}<span>${t('ql.linked', { name: esc(r.name || '') })}</span>`);
          break;
        case 'adding':
          show('adding', `<span class="spin"></span><span>${t('ql.adding')}</span>`);
          break;
        case 'cancelled':
          return show('cancelled', `<span>${t('ql.cancelled')}</span>${newCodeBtn}`, 'bad');
        case 'expired':
        case 'none':
          // Renew quietly a few times; after that stop asking Roblox until the user comes back.
          if (++expiredInRow <= 3) return start();
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
      ? { label: t('t.showSignedOut'), run: () => { setView('accounts'); S.filter = 'invalid'; update(); $('#scroll').scrollTop = 0; } } : null;
    toast(t(r.failed ? 't.checkedFail' : 't.checked', { ok: r.ok, bad: r.bad, f: r.failed }), r.bad ? 'warn' : 'ok', show);
  }

  async function closeIds(ids) {
    const n = await call('closeAccounts', { ids });
    if (n !== undefined) toast(n ? t('t.closing', { n }) : t('t.noneRunning'), n ? 'ok' : 'info');
  }

  async function closeAll() {
    if (!await confirmBox(t('t.closeAllQ'), { ok: t('t.closeAllOk'), danger: true })) return;
    const n = await call('closeAll');
    if (n !== undefined) toast(n ? t('t.closingAll', { n }) : t('t.noRoblox'), 'ok');
  }

  async function removeIds(ids) {
    const names = ids.map(byId).filter(Boolean).map(a => a.label);
    const list = names.slice(0, 5).join(', ') + (names.length > 5 ? ', …' : '');
    if (!await confirmBox(t('t.deleteQ', { n: ids.length, list }), { title: t('t.deleteTitle'), ok: t('t.deleteOk'), danger: true })) return;
    const n = await call('deleteAccounts', { ids });
    if (n !== undefined) {
      ids.forEach(id => S.selected.delete(id));
      renderSelection();
      toast(t('t.deleted', { n }), 'ok');
    }
  }

  async function copy(id, field) {
    const r = await call('copy', { id, field });
    if (r) toast(t(r.secret ? 't.copiedSecret' : 't.copied'), 'ok');
  }

  const confirmMulti = () => confirmBox(t('set.multiConfirm'), { title: t('set.multiConfirmTitle'), ok: t('set.multiOk') });

  async function importRam() {
    const r = await call('importRam');
    if (!r) return;
    if (!r.total) toast(t('t.importedNone'), 'warn');
    else toast(t('t.imported', { added: r.added, updated: r.updated }), 'ok');
  }

  /** The server an account is in: from its window's log here, or from Roblox's status when visible. */
  function serverOf(a) {
    if (!a) return null;
    if (isLocal(a) && a.session?.jobId && a.session.connection === 'ingame' && a.session.game?.placeId)
      return { placeId: a.session.game.placeId, jobId: a.session.jobId };
    if (a.presence?.type === 'ingame' && a.presence.jobId && a.presence.placeId) return { placeId: a.presence.placeId, jobId: a.presence.jobId };
    return null;
  }

  async function toggleRobloxConfig(ids) {
    const enable = !ids.every(i => byId(i)?.useRobloxConfig);
    const n = await call('setUseRobloxConfig', { ids, enabled: enable });
    if (n !== undefined) toast(t(enable ? 't.rcOn' : 't.rcOff', { n }), 'ok');
  }

  async function toggleRejoin(ids) {
    const enable = !ids.every(i => byId(i)?.autoRejoin);
    const n = await call('setAutoRejoin', { ids, enabled: enable });
    if (n !== undefined) toast(t(enable ? 't.rejoinOn' : 't.rejoinOff', { n }), 'ok');
  }

  /** Tiles all Roblox windows; with several monitors the user picks one first. */
  async function arrange(anchor) {
    const monitors = S.app.monitors || [];
    const run = async monitor => {
      const n = await call('arrangeWindows', { monitor });
      if (n !== undefined) toast(n ? t('t.arranged', { n }) : t('t.noWindows'), n ? 'ok' : 'info');
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
      '-',
      { icon: 'edit', text: t('m.edit'), hint: 'F2', disabled: many, run: () => editAccount(id) },
      { icon: 'refresh', text: t('m.check'), run: () => checkAccounts(ids) },
      { icon: 'server', text: t('m.follow'), disabled: many || !serverOf(a), run: () => { const s = serverOf(a); useServer(s.placeId, s.jobId); toast(t('srv.followed', { name: a.label }), 'ok'); } },
      { icon: 'repeat', text: t('m.autoRejoin'), checked: ids.every(i => byId(i)?.autoRejoin), run: () => toggleRejoin(ids) },
      ...(S.app.settings?.robloxConfig?.mode === 'marked'
        ? [{ icon: 'sliders', text: t('m.robloxConfig'), checked: ids.every(i => byId(i)?.useRobloxConfig), run: () => toggleRobloxConfig(ids) }] : []),
      { icon: 'external', text: t('m.profile'), disabled: many || !a.userId, run: () => call('openProfile', { userId: a.userId }) },
      '-',
      { icon: 'copy', text: t('m.copyUser'), disabled: many, run: () => copy(id, 'username') },
      { icon: 'key', text: t('m.copyPw'), disabled: many || !a.hasPassword, run: () => copy(id, 'password') },
      '-',
      { icon: 'trash', text: many ? t('m.deleteN', { n: ids.length }) : t('m.delete'), hint: 'Del', danger: true, run: () => removeIds(ids) },
    ], x, y);
  }

  function multiHelp() {
    const m = S.app.multi;
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
    modal({
      title: t('multi.title'),
      body,
      actions: [{ text: t('common.done'), kind: 'primary', onClick: ({ close }) => close() }],
      onMount: ({ el, close }) => el.addEventListener('click', async e => {
        const b = e.target.closest('[data-local]');
        if (!b) return;
        if (b.dataset.local === 'enable') {
          close();
          if (await confirmMulti()) call('setSettings', { multiInstance: true });
        } else if (b.dataset.local === 'close-all') {
          const n = await busy(b, () => call('closeAll'));
          if (n !== undefined) toast(n ? t('t.closingAll', { n }) : t('t.noRoblox'), 'ok');
        } else if (b.dataset.local === 'tray') {
          if ((await busy(b, () => call('disableTrayAutostart'))) !== undefined) {
            b.disabled = true;
            b.textContent = t('mh.s2done');
          }
        }
      }),
    });
  }

  function accountModal(a) {
    const isNew = !a;
    const groups = [...new Set(accounts().map(x => x.group).filter(Boolean))];
    const dirty = new Set();
    const head = isNew
      ? `<div class="callout">${icon('info')}<div>${t('am.callout')}</div></div>`
      : `<div class="acc-head"><div class="avatar">${avatarInner(a)}</div><div><b>${esc(a.label)}</b><div class="muted">@${esc(a.username)} · ID ${a.userId}</div></div></div>`;
    const reveal = name => `<button type="button" class="icon-btn" data-reveal="${name}" title="${t('common.show')}">${icon('eye')}</button>`;
    modal({
      title: t(isNew ? 'am.addTitle' : 'am.editTitle'),
      dismissable: false,
      body: `${head}
        <div class="form">
          <label>${t('am.alias')}<input class="field" name="alias" maxlength="60" placeholder="${t('am.aliasPh')}" value="${esc(a?.alias)}"></label>
          <label>${t('am.group')}<input class="field" name="group" maxlength="40" list="dl-groups" placeholder="${t('am.groupPh')}" value="${esc(a?.group)}"></label>
          <datalist id="dl-groups">${groups.map(g => `<option value="${esc(g)}">`).join('')}</datalist>
          <label><span>${t('am.pw')} <span class="muted">${t('am.pwHint')}</span></span>
            <div class="pw"><input class="field" type="password" name="password" maxlength="200" placeholder="${!isNew && a.hasPassword ? t('am.pwSaved') : t('am.pwNone')}">${reveal('password')}</div></label>
          ${isNew ? `<label>${t('am.cookie')}
            <div class="pw"><input class="field mono" type="password" name="cookie" maxlength="4096" placeholder="_|WARNING:-DO-NOT-SHARE-THIS…" spellcheck="false">${reveal('cookie')}</div></label>` : ''}
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
              return;
            }
            const args = { alias: val('alias'), group: val('group'), note: val('note') };
            if (isNew) Object.assign(args, { cookie: val('cookie'), password: val('password') });
            else {
              args.id = a.id;
              if (dirty.has('password')) args.password = val('password');
            }
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
            }
          },
        },
      ],
      onMount: ({ el, close }) => {
        el.querySelectorAll('input[name=password], input[name=cookie]').forEach(i => i.addEventListener('input', () => dirty.add(i.name)));
        el.addEventListener('click', async e => {
          if (e.target.closest('[data-local=browser]')) {
            close();
            addBrowser(null);
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
          if (!isNew && !dirty.has(input.name) && !input.value) {
            const v = await call('reveal', { id: a.id, field: input.name });
            if (typeof v === 'string') input.value = v;
          }
          input.type = 'text';
          b.innerHTML = icon('eyeOff');
        });
        setTimeout(() => el.querySelector(isNew ? '[name=cookie]' : '[name=alias]')?.focus(), 40);
      },
    });
  }
  const editAccount = id => { const a = byId(id); if (a) accountModal(a); };

  /* ───────────────────────── events ───────────────────────── */
  document.addEventListener('click', e => {
    const el = e.target.closest('[data-act]');
    if (!el || !S.app) return;
    const id = el.closest('.card')?.dataset.id;
    switch (el.dataset.act) {
      case 'card': return onCardClick(id, e);
      case 'play-one': return launch([id], false);
      case 'close-one': return closeIds([id]);
      case 'edit': return editAccount(id);
      case 'card-menu': {
        const r = el.getBoundingClientRect();
        return cardMenu(id, r.left, r.bottom + 6);
      }
      // The app answers with a state push in the new language.
      case 'lang': return el.dataset.lang !== lang && call('setLanguage', { language: el.dataset.lang });
      case 'view': return setView(el.dataset.view);
      case 'lock': return call('lock');
      case 'add-choose': return addChooser();
      case 'add-password': leaveChooser(el); return passwordLogin();
      case 'add-phone': leaveChooser(el); return quickLogin();
      case 'import-ram': leaveChooser(el); return importRam();
      case 'who': return openWho();
      case 'relogin': return quickLogin(accounts().find(x => x.id === id)?.realm);
      case 'add-browser': leaveChooser(el); return addBrowser(el);
      case 'add-cookie': leaveChooser(el); return accountModal(null);
      case 'add-menu':
        return menuAt(el, [
          { icon: 'qr', text: t('m.addCode'), run: () => quickLogin() },
          { icon: 'globe', text: t('m.addBrowser'), run: () => addBrowser(null) },
          { icon: 'cookie', text: t('m.addCookie'), run: () => accountModal(null) },
          '-',
          { icon: 'download', text: t('m.importRam'), run: importRam },
        ], 320);
      case 'tools-menu':
        return menuAt(el, [
          { icon: 'refresh', text: t('m.checkAll'), disabled: !accounts().length || S.app.checking, run: () => checkAccounts([]) },
          { icon: 'check', text: t('m.selectAll'), hint: 'Ctrl+A', disabled: !accounts().length, run: selectAllVisible },
          { icon: 'grid', text: t('m.arrange'), run: () => arrange($('[data-act=tools-menu]')) },
          { icon: 'download', text: t('m.importRam'), run: importRam },
          '-',
          { icon: 'power', text: t('m.closeAll'), danger: true, run: closeAll },
        ], 280);
      case 'layout':
        S.layout = el.dataset.l === 'list' ? 'list' : 'cards';
        store.set('rm.layout', S.layout);
        renderChips();
        renderGrid();
        return;
      case 'sort-menu':
        return menuAt(el, ['name', 'status', 'recent'].map(k => ({
          icon: 'sort', text: t(`sort.${k}`), checked: S.sort === k,
          run: () => {
            S.sort = k;
            store.set('rm.sort', k);
            renderChips();
            renderGrid();
          },
        })), 200);
      case 'refresh-presence':
        // refreshPresence replies null on success; call() yields undefined after showing an error.
        return busy(el, () => call('refreshPresence')).then(r => r !== undefined && toast(t('t.refreshed'), 'ok'));
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
      case 'cancel-launch': return call('cancelLaunch');
      case 'multi-help': return multiHelp();
      case 'select-all': return selectAllVisible();
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
      case 'check-updates': return busy(el, () => call('checkUpdates'));
      case 'whats-new': return call('whatsNew').then(list => showWhatsNew(list));
      // Bloxkeep closes and reopens as the new version; Roblox windows stay open.
      case 'apply-update': return call('applyUpdate');
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

  document.addEventListener('dblclick', e => {
    const card = e.target.closest('.card');
    if (card && !e.target.closest('button')) {
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
      if (e.key === 'Escape') return closeMenu();
      if (e.key === 'ArrowDown' || e.key === 'ArrowUp') {
        e.preventDefault();
        const items = [...menuEl.querySelectorAll('button:not(:disabled)')];
        const i = items.indexOf(document.activeElement);
        items[(i + (e.key === 'ArrowDown' ? 1 : items.length - 1)) % items.length]?.focus();
        return;
      }
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
      setView('accounts');
      $('#q').focus();
      return;
    }
    if (typing || S.view !== 'accounts') return;
    if (e.ctrlKey && key === 'a') {
      e.preventDefault();
      selectAllVisible();
    } else if (e.key === 'Delete' && S.selected.size) {
      removeIds([...S.selected]);
    } else if (e.key === 'F2' && S.selected.size === 1) {
      editAccount([...S.selected][0]);
    } else if (e.key === 'Enter' && S.selected.size && !e.target.closest('button')) {
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
    }
    S.app = st;
    if (st.locked) {
      $('#who')?.remove();
      S.selected.clear();
      S.metrics = { accounts: [], robloxCount: 0, totalMb: 0, systemMb: 0 };
      renderLock();
      return;
    }
    const ids = new Set(st.accounts.map(a => a.id));
    for (const id of [...S.selected]) if (!ids.has(id)) S.selected.delete(id);
    if (!S.shell) buildShell();
    update();
    renderWho();
    if (st.whatsNew && !S.whatsNewShown) {
      S.whatsNewShown = true;
      showWhatsNew(st.whatsNew.releases, () => call('seenWhatsNew'));
    }
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
  function saveView() {
    if (!S.shell) return;
    const view = { selected: [...S.selected], filter: S.filter, query: S.query, view: S.view, scroll: $('#scroll')?.scrollTop || 0 };
    bridge.call('keepView', { view: JSON.stringify(view) }).catch(() => {});
  }
  document.addEventListener('visibilitychange', () => { if (document.hidden) saveView(); });

  bridge.call('getState').then(first => {
    let saved = null;
    try { saved = first?.view ? JSON.parse(first.view) : null; } catch { saved = null; }
    if (saved && first.state && !first.state.locked) {
      S.selected = new Set(saved.selected || []);
      S.filter = saved.filter || 'all';
      S.query = saved.query || '';
      S.view = saved.view === 'settings' ? 'settings' : 'accounts';
      S.introShown = true;
    }
    applyState(first?.state);
    if (saved?.scroll && $('#scroll')) $('#scroll').scrollTop = saved.scroll;
  }, e => toast(e.message, 'error'));
})();
