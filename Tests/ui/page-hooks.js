// Runs in the preview page before any of its own scripts (Page.addScriptToEvaluateOnNewDocument), as window.__ui:
//   - the same clock and "random" numbers on every run, so relative times ("3 min ago") and the mock's numbers repeat;
//   - every command the page sends to the (fake) app, recorded;
//   - settle(): waits until no command is pending and nothing on the page changed for a moment;
//   - snap(): the page as text (what is shown, how it nests, its labels and states) to compare with a saved copy;
//   - find(): an element by its text in the interface's language (an i18n key), so a step works whatever builds the page.
(() => {
  // 16 October 2026, 12:00 in Vietnam (the run sets that timezone); the clock runs from there.
  const FIXED = Date.UTC(2026, 9, 16, 5, 0, 0);
  const RealDate = Date, t0 = performance.now();
  const now = () => Math.floor(FIXED + (performance.now() - t0));
  class FixedDate extends RealDate {
    constructor(...a) { if (a.length) super(...a); else super(now()); }
    static now() { return now(); }
  }
  window.Date = FixedDate;
  Math.random = () => 0.5;

  const ui = window.__ui = { calls: [], pending: 0, lastChange: performance.now(), lang: 'en' };
  // Short timers the page has set and that haven't run yet ("remove the intro class in 800 ms"): settle() waits for
  // them too. Longer ones (a minute's refresh) aren't waited for.
  const realSetTimeout = window.setTimeout.bind(window), realClearTimeout = window.clearTimeout.bind(window);
  const timers = new Set();
  window.setTimeout = (fn, ms, ...rest) => {
    const id = realSetTimeout((...a) => { timers.delete(id); ui.lastChange = performance.now(); return typeof fn === 'function' ? fn(...a) : undefined; }, ms, ...rest);
    if (!(ms > 3000)) timers.add(id);
    return id;
  };
  window.clearTimeout = id => { timers.delete(id); realClearTimeout(id); };
  new MutationObserver(() => { ui.lastChange = performance.now(); })
    .observe(document, { subtree: true, childList: true, attributes: true, characterData: true });

  // The fake bridge (dev/mock.js) sets window.__mockBridge: its calls are recorded on the way through.
  let bridge;
  Object.defineProperty(window, '__mockBridge', {
    configurable: true,
    get: () => bridge,
    set(v) {
      const call = v.call.bind(v);
      v.call = async (cmd, args) => {
        ui.calls.push({ cmd, args: args ?? null });
        ui.pending++;
        try { return await call(cmd, args); } finally { ui.pending--; ui.lastChange = performance.now(); }
      };
      bridge = v;
    },
  });

  const sleep = ms => new Promise(r => realSetTimeout(r, ms));
  ui.settle = async (quiet = 350, max = 8000) => {
    const end = performance.now() + max;
    await document.fonts?.ready;
    while (performance.now() < end) {
      const busy = document.getAnimations().some(a => a.playState === 'running' && a.effect?.getTiming().iterations !== Infinity);
      if (!ui.pending && !timers.size && !busy && performance.now() - ui.lastChange >= quiet) return true;
      await sleep(40);
    }
    return false;
  };

  const visible = el => el.checkVisibility ? el.checkVisibility({ checkVisibilityCSS: true }) : el.getClientRects().length > 0;
  const norm = s => (s || '').replace(/\s+/g, ' ').trim();
  const hash = s => { let h = 2166136261; for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619); return (h >>> 0).toString(36); };

  /**
   * The page as indented lines, one per shown element: tag.classes [attributes] "own text". Implementation details
   * (data-* attributes, inline styles, event handlers) are left out; icons are their drawing's hash. With layout, each
   * line ends with its box (x,y w×h).
   */
  ui.snap = ({ layout = false } = {}) => {
    const out = [];
    const KEEP = ['id', 'role', 'type', 'name', 'placeholder', 'title', 'alt', 'href', 'for', 'tabindex', 'autocomplete', 'maxlength', 'min', 'max', 'step', 'open', 'readonly', 'multiple'];
    const walk = (el, depth) => {
      if (el.nodeType !== 1 || !visible(el)) return;
      const tag = el.tagName.toLowerCase();
      if (tag === 'script' || tag === 'style' || tag === 'template') return;
      const pad = '  '.repeat(depth);
      const cls = [...el.classList].sort().join('.');
      if (tag === 'svg') { out.push(`${pad}svg${cls ? '.' + cls : ''} #${hash(el.innerHTML)}`); return; }
      const attrs = [];
      for (const a of el.attributes)
        if (KEEP.includes(a.name) || a.name.startsWith('aria-')) attrs.push(`${a.name}=${JSON.stringify(a.value)}`);
      if (tag === 'img') attrs.push(`src=${el.getAttribute('src')?.startsWith('data:') ? '#' + hash(el.getAttribute('src')) : JSON.stringify(el.getAttribute('src'))}`);
      if (tag === 'input' || tag === 'textarea' || tag === 'select') {
        if (el.type === 'checkbox' || el.type === 'radio') attrs.push(`checked=${el.checked}`);
        else if (el.type !== 'password') attrs.push(`value=${JSON.stringify(el.value)}`);
        else attrs.push(`value=${el.value ? '"•••"' : '""'}`);
      }
      if (el.disabled) attrs.push('disabled');
      if (document.activeElement === el && el !== document.body) attrs.push('FOCUSED');
      let text = '';
      for (const n of el.childNodes) if (n.nodeType === 3) text += n.nodeValue;
      text = norm(text);
      let line = `${pad}${tag}${cls ? '.' + cls : ''}${attrs.length ? ' [' + attrs.join(' ') + ']' : ''}${text ? ' ' + JSON.stringify(text) : ''}`;
      if (layout) {
        const r = el.getBoundingClientRect();
        line += ` @${Math.round(r.x)},${Math.round(r.y)} ${Math.round(r.width)}x${Math.round(r.height)}`;
        if (el.scrollTop) line += ` scroll=${Math.round(el.scrollTop)}`;
      }
      out.push(line);
      for (const c of el.children) walk(c, depth + 1);
    };
    for (const sel of ['#app', '#layer', '#toasts']) {
      const root = document.querySelector(sel);
      if (root) { out.push(sel); for (const c of root.children) walk(c, 1); }
    }
    return out.join('\n');
  };

  const CLICKABLE = 'button, a, summary, label, input, textarea, select,[role=button], [role=menuitem], [role=menuitemradio], [role=menuitemcheckbox], [role=tab], [role=option], [role=radio], [role=switch], [role=checkbox], [tabindex]';
  /** The interface's words for an i18n key ({placeholders} match anything). */
  const words = key => {
    const d = window.I18N || {};
    const s = (d[ui.lang] && d[ui.lang][key]) ?? d.en?.[key];
    if (s == null) throw new Error(`no such text key: ${key}`);
    return s;
  };
  // The words, then maybe a count ("Playing 3", "Play 2").
  const matcher = s => new RegExp('^' + s.split(/\{[^}]+\}/).map(p => p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('.*') + '(\\s*\\d+)?$', 'i');

  /**
   * An element to act on: { css }, or { key } (an i18n key: an element whose text, aria-label or title is those words)
   * or { text } (the words themselves), optionally { within: css } and { nth } when several match. Throws when none does.
   */
  ui.find = spec => {
    const scope = spec.within ? document.querySelector(spec.within) : document;
    if (!scope) throw new Error(`nothing matches ${spec.within}`);
    let list;
    if (spec.css) list = [...scope.querySelectorAll(spec.css)].filter(visible);
    else {
      const re = matcher(spec.key ? words(spec.key) : spec.text);
      const sel = spec.css2 || CLICKABLE;
      list = [...scope.querySelectorAll(sel)].filter(el => visible(el) &&
        [el.textContent, el.getAttribute('aria-label'), el.getAttribute('title'), el.getAttribute('placeholder')].some(v => re.test(norm(v))));
      // The innermost match: a button inside a matching menu row is the one to press.
      list = list.filter(el => !list.some(o => o !== el && el.contains(o)));
    }
    const el = list[spec.nth || 0];
    if (!el) throw new Error(`not found: ${JSON.stringify(spec)}`);
    return el;
  };
  /** Its centre in the viewport, scrolled into view first (for a real mouse click). */
  ui.point = spec => {
    const el = ui.find(spec);
    el.scrollIntoView({ block: 'nearest', inline: 'nearest' });
    const r = el.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  };
})();
