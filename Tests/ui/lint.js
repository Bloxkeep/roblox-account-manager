// The page measured for flaws a person sees and the text snapshots miss (run.mjs --lint): run in the page, it gives a line
// per flaw. Every check is geometry read from the live page, so it holds for every theme, language and window size.
//   align   things on one line whose middles disagree: a pill, an icon or a smaller text beside a title, measured on the
//           letters themselves (the capital height, from the font), not the line box
//   clip    text cut off by its own box with no ellipsis (a word or a line that doesn't show)
//   spill   text running out of its box (past a button's or a chip's edge)
//   overlap two boxes side by side in the page's flow lying over each other
//   scroll  the page itself scrolling sideways
//   trunc   text cut with an ellipsis and no title to read it whole
//   slack   a panel stretched by its row or column far past its content (a big empty part of the panel)
(() => {
  const out = [];
  const W = innerWidth, H = innerHeight;
  // Off by more than this reads as crooked: the owner saw a pill 1 px high next to a title at 150 % (1.5 screen pixels).
  const TOL = 0.75;
  const name = el => {
    if (!el || el.nodeType !== 1) return '?';
    const cls = typeof el.className === 'string' ? el.className.trim().split(/\s+/).filter(Boolean).slice(0, 2).join('.') : '';
    const own = el.id ? '#' + el.id : el.tagName.toLowerCase() + (cls ? '.' + cls : '');
    const p = el.parentElement;
    const up = p && p !== document.body ? (p.id ? '#' + p.id : p.tagName.toLowerCase() + (typeof p.className === 'string' && p.className.trim() ? '.' + p.className.trim().split(/\s+/)[0] : '')) : '';
    return (up ? up + ' > ' : '') + own;
  };
  // Seen and meant: { selector, kind, why }.
  const meant = [
    { sel: '.day > small', kind: 'spill', why: "a day's label is wider than its bar and centred under it" },
    { sel: '#mon-area', kind: 'slack', why: 'with no live pictures yet the whole area is the dashed place they will show in' },
    { sel: '.mon-pic', kind: 'slack', why: "a minimised window's tile keeps a picture's size so the grid doesn't jump" },
  ];
  const say = (kind, el, what) => { if (!meant.some(m => m.kind === kind && el.matches?.(m.sel))) out.push(`${kind.padEnd(7)} ${name(el)}: ${what}`); };
  const shown = el => {
    const r = el.getBoundingClientRect();
    if (r.width < 1 || r.height < 1 || r.bottom < 0 || r.right < 0 || r.top > H || r.left > W) return false;
    return el.checkVisibility({ checkOpacity: true, checkVisibilityCSS: true });
  };
  // Clipped away by a scrolling box around it (a row scrolled out of its list isn't on screen).
  const inView = el => {
    const r = el.getBoundingClientRect();
    for (let a = el.parentElement; a && a !== document.documentElement; a = a.parentElement) {
      const cs = getComputedStyle(a);
      if (!/(hidden|auto|scroll|clip)/.test(cs.overflowX + cs.overflowY)) continue;
      const b = a.getBoundingClientRect();
      if (r.bottom <= b.top + 1 || r.top >= b.bottom - 1 || r.right <= b.left + 1 || r.left >= b.right - 1) return false;
    }
    return true;
  };

  // ── font metrics: the capital height and the x-height of a font, in px, cached ──
  const ctx = document.createElement('canvas').getContext('2d');
  const metricCache = new Map();
  const metrics = cs => {
    const font = `${cs.fontStyle} ${cs.fontWeight} ${cs.fontSize} ${cs.fontFamily}`;
    let m = metricCache.get(font);
    if (!m) {
      ctx.font = font;
      const H_ = ctx.measureText('H'), x = ctx.measureText('x'), f = ctx.measureText('Hg');
      m = { cap: H_.actualBoundingBoxAscent, x: x.actualBoundingBoxAscent, asc: f.fontBoundingBoxAscent, desc: f.fontBoundingBoxDescent };
      metricCache.set(font, m);
    }
    return m;
  };
  // A text node's first line: where its letters sit (baseline, middle of the capitals, middle of the x-height).
  const textLine = (node, cs) => {
    const rg = document.createRange();
    rg.selectNodeContents(node);
    const rects = [...rg.getClientRects()].filter(r => r.width > 0.5);
    if (!rects.length) return null;
    const r = rects[0], m = metrics(cs);
    const base = r.top + (r.height - (m.asc + m.desc)) / 2 + m.asc;
    const lines = new Set(rects.map(q => Math.round(q.top))).size, last = rects[rects.length - 1];
    return { left: r.left, right: r.right, top: r.top, bottom: lines > 1 ? last.bottom : r.bottom, base, capMid: base - m.cap / 2, xMid: base - m.x / 2,
      size: parseFloat(cs.fontSize), lines, blockMid: (r.top + last.bottom) / 2 };
  };
  const ownText = el => [...el.childNodes].filter(n => n.nodeType === 3 && n.textContent.trim());
  const painted = cs => (cs.backgroundColor !== 'rgba(0, 0, 0, 0)' && cs.backgroundColor !== 'transparent') || parseFloat(cs.borderTopWidth) > 0 || parseFloat(cs.borderLeftWidth) > 0;

  // ── align ──
  // Each parent's line items: its own text nodes, and its children that are small (a line's worth tall). Each item gets
  // the middle a reader's eye uses: a box that is painted (a pill, a chip, a button, an icon) its box's middle; plain text
  // its letters. Items on one line (their boxes share most of their height) are compared to the line's text.
  const parents = new Set();
  for (const el of document.querySelectorAll('body *')) {
    if (!(el instanceof HTMLElement || el instanceof SVGSVGElement)) continue;
    if (el.closest('svg') && el.tagName.toLowerCase() !== 'svg') continue;
    if (el.parentElement) parents.add(el.parentElement);
  }
  for (const p of parents) {
    if (!shown(p) || !inView(p) || p.closest('svg, script, style, template, [aria-hidden="true"]')) continue;
    const pcs = getComputedStyle(p);
    if (/^(grid)$/.test(pcs.display)) continue; // a grid's cells line up by its tracks, checked as boxes elsewhere
    const items = [];
    for (const n of ownText(p)) {
      const t = textLine(n, pcs);
      if (t) items.push({ kind: 'text', el: p, ...t });
    }
    for (const c of p.children) {
      if (!shown(c)) continue;
      const cs = getComputedStyle(c);
      if (cs.position === 'absolute' || cs.position === 'fixed' || cs.float !== 'none') continue;
      const r = c.getBoundingClientRect();
      if (r.height > 64) continue;
      const tag = c.tagName.toLowerCase();
      if (tag === 'svg' || tag === 'img' || (painted(cs) && r.height <= 40)) {
        // A painted box with its own text: compared by its box's middle, and its text by its baseline.
        // Its first text, however deep (a button's label in a span): a box whose text shares the line's baseline is right.
        const walk = document.createTreeWalker(c, NodeFilter.SHOW_TEXT, { acceptNode: n => (n.textContent.trim() && shown(n.parentElement) ? NodeFilter.FILTER_ACCEPT : NodeFilter.FILTER_SKIP) });
        const inner = tag === 'svg' ? null : walk.nextNode();
        // A button holding only an icon is read by its icon.
        const only = c.children.length === 1 && c.firstElementChild.tagName.toLowerCase() === 'svg' && !inner ? c.firstElementChild.getBoundingClientRect() : null;
        if (only) { items.push({ kind: 'icon', el: c, top: r.top, bottom: r.bottom, left: r.left, right: r.right, mid: (only.top + only.bottom) / 2, inner: null }); continue; }
        items.push({ kind: tag === 'svg' || tag === 'img' ? 'icon' : 'box', el: c, top: r.top, bottom: r.bottom, left: r.left, right: r.right, mid: (r.top + r.bottom) / 2, inner: inner ? textLine(inner, getComputedStyle(inner.parentElement)) : null });
      } else {
        const inner = ownText(c);
        if (inner.length && c.children.length === 0) {
          const t = textLine(inner[0], cs);
          if (t) items.push({ kind: 'text', el: c, ...t });
        }
      }
    }
    if (items.length < 2) continue;
    // Only the items on one line with a text to measure against (the line's largest text).
    const texts = items.filter(i => i.kind === 'text');
    if (!texts.length) continue;
    const ref = texts.reduce((a, b) => (b.size > a.size ? b : a));
    for (const it of items) {
      if (it === ref) continue;
      const overlapY = Math.min(it.bottom, ref.bottom) - Math.max(it.top, ref.top);
      if (overlapY < Math.min(it.bottom - it.top, ref.bottom - ref.top) * 0.5) continue; // another line
      const gap = it.left >= ref.right ? it.left - ref.right : ref.left >= it.right ? ref.left - it.right : 0;
      if (gap > 48) continue; // far apart on the row: not read together
      if (it.kind === 'text') {
        if (ref.lines > 1 || it.lines > 1) continue; // wrapped text: its other lines have their own baselines
        const ics = getComputedStyle(it.el);
        if (it.el !== ref.el && ics.display === 'inline' && ics.verticalAlign === 'baseline') continue; // on the line's baseline by construction
        // Texts on one line share a baseline (a smaller one beside a bigger one too).
        const d = it.base - ref.base;
        if (Math.abs(d) > TOL) say('align', it.el, `text ${d > 0 ? 'lower' : 'higher'} than ${name(ref.el)}'s by ${Math.abs(d).toFixed(1)}px (baselines)`);
      } else {
        // A box or an icon sits on the text's middle: its capitals' middle, or its x-height's for an icon.
        // Beside text on several lines, a box sits on the block's middle.
        const d = it.mid - ref.capMid;
        // Beside text on several lines a box sits on the first line, or on the block's middle.
        if (ref.lines > 1) {
          const db = it.mid - ref.blockMid;
          if (Math.abs(db) > 2 && Math.abs(d) > TOL && (it.kind !== 'icon' || Math.abs(it.mid - ref.xMid) > TOL)) say('align', it.el, `${it.kind} neither on the first line nor the middle of ${name(ref.el)}'s ${ref.lines} lines (${d.toFixed(1)}px, ${db.toFixed(1)}px)`);
          continue;
        }
        const dx = it.mid - ref.xMid;
        const sameBase = it.inner && Math.abs(it.inner.base - ref.base) <= 0.5;
        if (Math.abs(d) > TOL && (it.kind !== 'icon' || Math.abs(dx) > TOL) && !sameBase) say('align', it.el, `${it.kind} ${d > 0 ? 'lower' : 'higher'} than ${name(ref.el)}'s letters by ${Math.abs(d).toFixed(1)}px`);
      }
    }
  }

  // ── clip, spill, trunc ──
  for (const el of document.querySelectorAll('body *')) {
    if (!(el instanceof HTMLElement) || !shown(el) || !inView(el)) continue;
    const texts = ownText(el);
    if (!texts.length) continue;
    const cs = getComputedStyle(el);
    const r = el.getBoundingClientRect();
    const hidden = /(hidden|clip)/.test(cs.overflowX + cs.overflowY);
    const ellipsis = cs.textOverflow === 'ellipsis';
    const clamp = cs.webkitLineClamp && cs.webkitLineClamp !== 'none';
    const forReaders = /inset\(50%\)/.test(cs.clipPath) || (r.width <= 1 && r.height <= 1); // only for screen readers
    if (forReaders) continue;
    if (hidden && (el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 2)) {
      if (ellipsis || clamp) {
        if (!el.title && !el.closest('[title]') && !el.getAttribute('aria-label')) say('trunc', el, `"${el.textContent.trim().slice(0, 40)}" cut with … and no title`);
      } else if (!/^(textarea|input|select)$/i.test(el.tagName)) {
        say('clip', el, `"${el.textContent.trim().slice(0, 40)}" cut off (${el.scrollWidth}×${el.scrollHeight} in ${el.clientWidth}×${el.clientHeight})`);
      }
      continue;
    }
    // Text drawn outside the box it belongs to.
    for (const n of texts) {
      const rg = document.createRange();
      rg.selectNodeContents(n);
      for (const q of rg.getClientRects()) {
        if (q.width < 1) continue;
        // The font's own box is taller than a tight line height: only what goes past the line itself counts.
        const lh = parseFloat(cs.lineHeight) || parseFloat(cs.fontSize) * 1.2, slack = Math.max(0, (q.height - lh) / 2);
        const outX = Math.max(0, r.left - q.left, q.right - r.right), outY = Math.max(0, r.top - q.top, q.bottom - r.bottom) - slack;
        if (outX > 1.5 || outY > 2) { say('spill', el, `"${n.textContent.trim().slice(0, 40)}" runs ${outX > 1.5 ? `${outX.toFixed(0)}px sideways` : `${outY.toFixed(0)}px up/down`} out of its box`); break; }
      }
    }
  }

  // ── overlap: in-flow siblings lying over each other ──
  for (const p of parents) {
    if (!shown(p) || !inView(p)) continue;
    const kids = [...p.children].filter(c => {
      if (!shown(c) || !inView(c)) return false;
      const cs = getComputedStyle(c);
      return cs.position !== 'absolute' && cs.position !== 'fixed' && cs.display !== 'contents' && !c.matches('svg *')
        && !/^table-|^(table)$/.test(cs.display) && cs.display !== 'inline' && !c.matches('colgroup, col, thead, tbody, tfoot, tr, td, th');
    });
    for (let i = 0; i < kids.length; i++) for (let j = i + 1; j < kids.length; j++) {
      const a = kids[i].getBoundingClientRect(), b = kids[j].getBoundingClientRect();
      const x = Math.min(a.right, b.right) - Math.max(a.left, b.left), y = Math.min(a.bottom, b.bottom) - Math.max(a.top, b.top);
      // Laid over each other on purpose: a negative margin (avatars in a stack), a box pulled up over the one before.
      const pulled = [kids[i], kids[j]].some(k => { const cs = getComputedStyle(k); return parseFloat(cs.marginLeft) < 0 || parseFloat(cs.marginTop) < 0 || parseFloat(cs.marginRight) < 0; });
      if (x > 2 && y > 2 && !pulled) say('overlap', kids[j], `over ${name(kids[i])} by ${x.toFixed(0)}×${y.toFixed(0)}px`);
    }
  }

  // ── an icon alone in its box (an icon button, a badge): on the box's middle ──
  for (const svg of document.querySelectorAll('svg')) {
    const box = svg.parentElement;
    if (!box || !shown(svg) || !inView(svg) || svg.closest('[aria-hidden="true"]') !== svg.closest('svg')) continue;
    // A label scrolled out of sight (a row of tabs) is still there: laid out, only clipped.
    const laid = n => getComputedStyle(n).display !== 'none' && n.getBoundingClientRect().width > 1;
    const others = [...box.childNodes].filter(n => n !== svg && (n.nodeType === 3 ? n.textContent.trim() : n.nodeType === 1 && laid(n) && getComputedStyle(n).position !== 'absolute'));
    if (others.length) continue;
    const bcs = getComputedStyle(box);
    if (!painted(bcs) && !box.matches('button, [role="button"], a')) continue;
    const a = svg.getBoundingClientRect(), b = box.getBoundingClientRect();
    const dy = (a.top + a.bottom) / 2 - (b.top + b.bottom) / 2, dx = (a.left + a.right) / 2 - (b.left + b.right) / 2;
    if (Math.abs(dy) > TOL || Math.abs(dx) > TOL) say('center', svg, `icon off its box's middle by ${dx.toFixed(1)}, ${dy.toFixed(1)}px (${box.getAttribute('aria-label') || box.title || box.className})`);
  }

  // ── slack: a panel stretched by its row or column far past what it holds (Settings' page list on a picture theme
  // once ran down the whole window, mostly empty). A panel is a box with rounded corners, a border or a shadow: a plain
  // fill without them is a region of the page (a view's background) and is meant to reach the window's edge. Cards in a
  // row all as tall as the fullest one are meant too. ──
  const stretched = (el, cs, p) => {
    const pcs = getComputedStyle(p);
    const self = cs.alignSelf === 'auto' ? pcs.alignItems : cs.alignSelf;
    if (/grid/.test(pcs.display)) return /^(normal|stretch)$/.test(self);
    if (!/flex/.test(pcs.display)) return false;
    return /column/.test(pcs.flexDirection) ? parseFloat(cs.flexGrow) > 0 : /^(normal|stretch)$/.test(self);
  };
  const filledTo = el => {
    let bottom = -Infinity;
    for (const n of el.childNodes) {
      // Laid out counts, on screen or not: a panel's rows scrolled past the window's bottom still fill it.
      if (n.nodeType === 1 && n.checkVisibility({ checkVisibilityCSS: true }) && n.getBoundingClientRect().height > 0 && !/^(absolute|fixed)$/.test(getComputedStyle(n).position)) bottom = Math.max(bottom, n.getBoundingClientRect().bottom + Math.max(0, parseFloat(getComputedStyle(n).marginBottom) || 0));
      else if (n.nodeType === 3 && n.textContent.trim()) { const rg = document.createRange(); rg.selectNodeContents(n); bottom = Math.max(bottom, rg.getBoundingClientRect().bottom); }
    }
    return bottom;
  };
  const slackOf = el => {
    const cs = getComputedStyle(el), r = el.getBoundingClientRect(), to = filledTo(el);
    return to === -Infinity ? 0 : r.bottom - parseFloat(cs.borderBottomWidth) - parseFloat(cs.paddingBottom) - to;
  };
  const panel = cs => painted(cs) && (parseFloat(cs.borderTopLeftRadius) > 0 || cs.boxShadow !== 'none' || parseFloat(cs.borderTopWidth) > 0);
  for (const el of document.querySelectorAll('body *')) {
    const p = el.parentElement;
    if (!(el instanceof HTMLElement) || !p || !shown(el) || !inView(el)) continue;
    const cs = getComputedStyle(el);
    if (!panel(cs) || !stretched(el, cs, p) || el.scrollHeight > el.clientHeight + 1) continue;
    const r = el.getBoundingClientRect(), slack = slackOf(el);
    if (slack <= Math.max(48, r.height * 0.25) || parseFloat(cs.minHeight) >= r.height - 1) continue;
    const row = [...p.children].filter(s => s !== el && shown(s) && Math.abs(s.getBoundingClientRect().top - r.top) < 2 && panel(getComputedStyle(s)));
    if (row.some(s => slackOf(s) < 16)) continue;
    say('slack', el, `panel ${Math.round(r.height)}px tall, its content ends ${Math.round(slack)}px above its bottom (stretched by ${name(p)})`);
  }

  // ── scroll: the page itself ──
  const se = document.scrollingElement;
  if (se.scrollWidth > se.clientWidth + 1) say('scroll', document.body, `page scrolls sideways by ${se.scrollWidth - se.clientWidth}px`);

  return [...new Set(out)];
})()
