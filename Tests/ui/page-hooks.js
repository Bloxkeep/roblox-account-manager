// Runs in the preview page before any of its own scripts (Page.addScriptToEvaluateOnNewDocument), as window.__ui:
//   - the same clock and "random" numbers on every run, so relative times ("3 min ago") and the mock's numbers repeat;
//   - every command the page sends to the (fake) app, recorded;
//   - settle(): waits until no command is pending and nothing on the page changed for a moment;
//   - snap(): the page as text (what is shown, how it nests, its labels and states) to compare with a saved copy;
//   - find(): an element by its text in the interface's language (an i18n key), so a step works whatever builds the page.
(() => {
  // 16 October 2026, 12:00 in Vietnam (the run sets that timezone), and it stays that time: countdowns ("Resend in
  // 59 s") and "checked at" read the same however long a step takes. Timers (setTimeout) still run.
  const FIXED = Date.UTC(2026, 9, 16, 5, 0, 0);
  const RealDate = Date;
  const now = () => FIXED;
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
  // run.mjs --jank sets __uiSlow: waits of the fake app (ui/dev/mock.js answers in 0.1–0.8 s) and the page's short timers
  // take that many times longer, as over a slow connection, so content arriving late is seen arriving late.
  const slow = window.__uiSlow || 1;
  // A countdown sets its next tick from the tick before, again and again; under the fixed clock it never reaches zero.
  // A timer set by a timer set by a timer… past CHAIN deep is that, and isn't waited for (it would hold every step up
  // to settle()'s limit); the first ticks still are.
  const CHAIN = 3;
  let chain = 0; // how deep in such a chain the timer running now is (0: none is)
  window.setTimeout = (fn, ms, ...rest) => {
    if (slow > 1 && ms >= 100 && ms <= 3000) ms *= slow;
    const depth = chain + 1;
    const id = realSetTimeout((...a) => {
      timers.delete(id);
      ui.lastChange = performance.now();
      const was = chain;
      chain = depth;
      try { return typeof fn === 'function' ? fn(...a) : undefined; } finally { chain = was; }
    }, ms, ...rest);
    if (!(ms > 3000) && depth <= CHAIN) timers.add(id);
    return id;
  };
  window.clearTimeout = id => { timers.delete(id); realClearTimeout(id); };
  new MutationObserver(() => { ui.lastChange = performance.now(); })
    .observe(document, { subtree: true, childList: true, attributes: true, characterData: true });

  // Layout shifts (the browser's own measure of things on screen jumping): which element moved, from where to where,
  // and whether it came right after a click or key (the browser's hadRecentInput: half a second) or later, when
  // something loaded. The later ones are what reads as the page "jerking".
  ui.shifts = [];
  const describe = n => {
    if (!n || n.nodeType !== 1) return n?.nodeType === 3 ? 'text' : '?';
    const cls = [...n.classList].slice(0, 3).join('.');
    return `${n.tagName.toLowerCase()}${n.id ? '#' + n.id : ''}${cls ? '.' + cls : ''}`;
  };
  try {
    new PerformanceObserver(list => {
      for (const e of list.getEntries())
        ui.shifts.push({ at: Math.round(e.startTime), value: e.value, recent: e.hadRecentInput,
          sources: (e.sources || []).map(s => ({ node: describe(s.node), from: [s.previousRect.y, s.previousRect.height].map(Math.round), to: [s.currentRect.y, s.currentRect.height].map(Math.round) })) });
    }).observe({ type: 'layout-shift', buffered: true });
  } catch { /* no Layout Instability API */ }

  // The fake bridge (ui/dev/mock.js) sets window.__mockBridge: its calls are recorded on the way through.
  let bridge;
  Object.defineProperty(window, '__mockBridge', {
    configurable: true,
    get: () => bridge,
    set(v) {
      const call = v.call.bind(v), attach = v.attach.bind(v);
      // The page's own event handler, so a scenario can send it what the app would (__ui.emit('state', …)).
      v.attach = fn => { ui.emit = (name, data) => fn(name, data); return attach(fn); };
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
    // data-* attributes are how the page finds its own elements, so they're left out, except those the stylesheet
    // selects on (they change how the page looks).
    const styled = new Set();
    for (const sheet of document.styleSheets) {
      let rules;
      try { rules = sheet.cssRules; } catch { continue; }
      const scan = list => { for (const r of list) { if (r.selectorText) for (const m of r.selectorText.matchAll(/\[(data-[\w-]+)/g)) styled.add(m[1]); if (r.cssRules) scan(r.cssRules); } };
      scan(rules);
    }
    const inline = el => [...el.style].sort().map(p => `${p}:${el.style.getPropertyValue(p)}`).join(';');
    const rank = name => (KEEP.includes(name) ? KEEP.indexOf(name) : name.startsWith('aria-') ? 100 : 200);
    const html = document.documentElement;
    out.push(`html [${[...html.attributes].filter(a => a.name !== 'style').map(a => `${a.name}=${JSON.stringify(a.value)}`).sort().join(' ')}]${inline(html) ? ` style=${JSON.stringify(inline(html))}` : ''}`);
    const walk = (el, depth) => {
      if (el.nodeType !== 1 || !visible(el)) return;
      const tag = el.tagName.toLowerCase();
      if (tag === 'script' || tag === 'style' || tag === 'template') return;
      const pad = '  '.repeat(depth);
      const cls = [...el.classList].sort().join('.');
      if (tag === 'svg') { out.push(`${pad}svg${cls ? '.' + cls : ''} #${hash(el.innerHTML)}`); return; }
      // In one fixed order, not the order they were set in (React sets them in its own order; it doesn't change the page).
      const attrs = [...el.attributes].filter(a => KEEP.includes(a.name) || a.name.startsWith('aria-') || styled.has(a.name))
        .sort((x, y) => rank(x.name) - rank(y.name) || (x.name < y.name ? -1 : x.name > y.name ? 1 : 0))
        .map(a => `${a.name}=${JSON.stringify(a.value)}`);
      const st = inline(el);
      if (st) attrs.push(`style=${JSON.stringify(st)}`);
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
      // A list's choices aren't shown until it is opened, but what they say (and which can't be picked) is the page's.
      if (tag === 'select') {
        for (const o of el.options) {
          const flags = [`value=${JSON.stringify(o.value)}`, o.disabled && 'disabled', o.hidden && 'hidden'].filter(Boolean);
          out.push(`${pad}  option [${flags.join(' ')}] ${JSON.stringify(norm(o.text))}`);
        }
        return;
      }
      for (const c of el.children) walk(c, depth + 1);
    };
    for (const sel of ['#app', '#layer', '#toasts', '#tray']) { // #tray: the tray's page (tray-preview.html)
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
  const matcher = (s, partial) => new RegExp('^' + s.split(/\{[^}]+\}/).map(p => p.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('.*') + (partial ? '' : '(\\s*\\d+)?$'), 'i');

  /**
   * An element to act on: { css }, or { key } (an i18n key: an element whose text, aria-label or title is those words)
   * or { text } (the words themselves), optionally { within: css } and { nth } when several match, { partial: true } when
   * more follows the words (a second line, a key hint). Throws when none does.
   */
  ui.find = spec => {
    const scope = spec.within ? document.querySelector(spec.within) : document;
    if (!scope) throw new Error(`nothing matches ${spec.within}`);
    let list;
    if (spec.css) list = [...scope.querySelectorAll(spec.css)].filter(visible);
    else {
      const re = matcher(spec.key ? words(spec.key) : spec.text, spec.partial);
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
