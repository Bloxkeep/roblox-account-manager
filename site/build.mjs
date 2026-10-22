// Builds Bloxkeep's website into site/dist for GitHub Pages (https://bloxkeep.github.io/roblox-account-manager/): the landing
// page (pages.mjs) and the user guide made from docs/guide.md and docs/guide/*.md, in English and Vietnamese, with the
// screenshots, a sitemap and robots.txt. The guide's Markdown stays the one source: the site is rebuilt from it on every
// push (.github/workflows/pages.yml). Run: npm ci && node build.mjs, then serve site/dist.
import fs from 'node:fs';
import path from 'node:path';
import { Marked } from 'marked';
import { SITE, REPO, TOPICS, T, landing } from './pages.mjs';

const HERE = import.meta.dirname;
const ROOT = path.resolve(HERE, '..');
const DOCS = path.join(ROOT, 'docs');
const OUT = path.join(HERE, 'dist');

/** The newest regular version (not a beta), from the app's own release notes. */
const version = JSON.parse(fs.readFileSync(path.join(ROOT, 'RobloxManager', 'whatsnew.json'), 'utf8'))
  .map(r => r.version).find(v => !v.includes('-'));

const esc = s => String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;').replace(/"/g, '&quot;');
const plain = html => html.replace(/<[^>]+>/g, '').replace(/&quot;/g, '"').replace(/&#39;/g, "'").replace(/&lt;/g, '<')
  .replace(/&gt;/g, '>').replace(/&amp;/g, '&').replace(/\s+/g, ' ').trim();

/** A link from one page's URL path ("guide/x/") to another path, relative so the site works under any base. */
function rel(from, to) {
  const r = path.posix.relative('/' + from, '/' + to);
  return (r || '.') + (to.endsWith('/') || to === '' ? '/' : '');
}

// Heading ids as GitHub makes them, so the guide's #anchors keep working.
let seen = new Map();
function slug(text) {
  const base = text.toLowerCase().replace(/[^\p{L}\p{M}\p{N}\s_-]/gu, '').trim().replace(/\s/g, '-');
  const n = seen.get(base) ?? 0;
  seen.set(base, n + 1);
  return n ? `${base}-${n}` : base;
}
const marked = new Marked({ gfm: true });
marked.use({
  renderer: {
    heading({ tokens, depth }) {
      const inner = this.parser.parseInline(tokens);
      return `<h${depth} id="${slug(plain(inner))}">${inner}</h${depth}>\n`;
    },
  },
});

/** Where a file of the repository is on the site (a URL path), or its page on GitHub. */
function target(repoPath, lang, hash) {
  const p = repoPath.replace(/\\/g, '/');
  let m;
  if ((m = /^docs\/guide(\.vi)?\.md$/.exec(p)))
    return (m[1] ? 'vi/' : '') + 'guide/';
  if ((m = /^docs\/guide\/([a-z-]+?)(\.vi)?\.md$/.exec(p)) && TOPICS.includes(m[1]))
    return (m[2] ? 'vi/' : '') + `guide/${m[1]}/`;
  if (p.startsWith('docs/screenshots/'))
    return p.slice('docs/'.length);
  // The README's sections aren't on the site: a link into one goes to GitHub, a link to the README to the home page.
  if ((p === 'README.md' || p === 'README.vi.md') && !hash)
    return lang === 'vi' ? 'vi/' : '';
  return `${REPO}/blob/main/${p}`;
}

const ALERTS = ['note', 'tip', 'important', 'warning', 'caution'];

/** A guide page's HTML from its Markdown: links and pictures pointed at the site, GitHub's alerts as boxes. */
function guideHtml(file, url, lang) {
  let md = fs.readFileSync(file, 'utf8').replace(/\r\n/g, '\n');
  md = md.replace(/^\[(English|Tiếng Việt)\]\([^)]*\)\s*\n/, ''); // the site's header has the language switch
  seen = new Map();
  let html = marked.parse(md);
  html = html.replace(/<blockquote>\s*<p>\[!(\w+)\]\s*([\s\S]*?)<\/blockquote>/g, (all, kind, body) => {
    const k = kind.toLowerCase();
    return ALERTS.includes(k) ? `<div class="alert alert-${k}"><p class="alert-title">${T[lang].alerts[k]}</p><p>${body}</div>` : all;
  });
  html = html.replace(/<table>/g, '<div class="table-wrap"><table>').replace(/<\/table>/g, '</table></div>');
  html = html.replace(/<img /g, '<img loading="lazy" decoding="async" ');
  html = html.replace(/(href|src)="([^"]+)"/g, (all, attr, link) => {
    if (/^(https?:|mailto:|#|data:)/.test(link))
      return all;
    const [p, hash = ''] = link.split('#');
    const repoPath = path.relative(ROOT, path.resolve(path.dirname(file), decodeURIComponent(p)));
    const to = target(repoPath, lang, hash);
    const href = /^https?:/.test(to) ? to : rel(url, to);
    return `${attr}="${esc(href)}${hash ? '#' + hash : ''}"`;
  });
  const h1 = /<h1[^>]*>([\s\S]*?)<\/h1>/.exec(html);
  const title = h1 ? plain(h1[1]) : 'Bloxkeep';
  html = html.replace(/<h1[^>]*>[\s\S]*?<\/h1>\n?/, '');
  const first = /<p>([\s\S]*?)<\/p>/.exec(html);
  const description = first ? plain(first[1]).slice(0, 200) : '';
  return { title, description, body: html };
}

/** The page around the content: head with what search engines and link previews read, header, footer. */
function page({ lang, url, alt, title, description, body, bodyClass = '', jsonLd = null }) {
  const t = T[lang];
  const home = lang === 'vi' ? 'vi/' : '';
  const guide = home + 'guide/';
  const abs = u => SITE + u;
  return `<!doctype html>
<html lang="${lang}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(title)}</title>
<meta name="description" content="${esc(description)}">
<link rel="canonical" href="${abs(url)}">
<link rel="alternate" hreflang="${lang}" href="${abs(url)}">
<link rel="alternate" hreflang="${lang === 'vi' ? 'en' : 'vi'}" href="${abs(alt)}">
<link rel="alternate" hreflang="x-default" href="${abs(lang === 'vi' ? alt : url)}">
<meta property="og:type" content="website">
<meta property="og:site_name" content="Bloxkeep">
<meta property="og:title" content="${esc(title)}">
<meta property="og:description" content="${esc(description)}">
<meta property="og:url" content="${abs(url)}">
<meta property="og:image" content="${abs('og.png')}">
<meta property="og:locale" content="${lang === 'vi' ? 'vi_VN' : 'en_US'}">
<meta name="twitter:card" content="summary_large_image">
<meta name="color-scheme" content="dark light">
<link rel="icon" href="${rel(url, 'logo.svg')}" type="image/svg+xml">
<link rel="apple-touch-icon" href="${rel(url, 'icon-128.png')}">
<link rel="stylesheet" href="${rel(url, 'style.css')}">
${jsonLd ? `<script type="application/ld+json">${JSON.stringify(jsonLd)}</script>\n` : ''}</head>
<body class="${bodyClass}">
<header class="top">
  <div class="wrap top-in">
    <a class="brand" href="${rel(url, home)}"><img src="${rel(url, 'logo.svg')}" width="28" height="28" alt="">Bloxkeep</a>
    <nav>
      <a href="${rel(url, guide)}">${t.nav.guide}</a>
      <a href="${rel(url, home)}#download">${t.nav.download}</a>
      <a href="${REPO}">GitHub</a>
      <a class="lang" href="${rel(url, alt)}" hreflang="${lang === 'vi' ? 'en' : 'vi'}" lang="${lang === 'vi' ? 'en' : 'vi'}">${t.nav.other}</a>
    </nav>
  </div>
</header>
<main>
${body}
</main>
<footer class="foot">
  <div class="wrap">
    <p>${t.foot.disclaimer}</p>
    <p><a href="${REPO}">GitHub</a> · <a href="${REPO}/releases">${t.foot.releases}</a> · <a href="${REPO}/blob/main/PRIVACY.md">${t.foot.privacy}</a> · <a href="${REPO}/blob/main/docs/CODE_SIGNING_POLICY.md">${t.foot.signing}</a> · <a href="${REPO}/blob/main/LICENSE">${t.foot.license}</a></p>
  </div>
</footer>
</body>
</html>
`;
}

function write(url, html) {
  const file = path.join(OUT, url, url.endsWith('.html') ? '' : 'index.html');
  fs.mkdirSync(path.dirname(file), { recursive: true });
  fs.writeFileSync(file, html);
}

fs.rmSync(OUT, { recursive: true, force: true });
fs.mkdirSync(OUT, { recursive: true });
const urls = [];

for (const lang of ['en', 'vi']) {
  const t = T[lang];
  const home = lang === 'vi' ? 'vi/' : '';
  const other = lang === 'vi' ? '' : 'vi/';
  const suffix = lang === 'vi' ? '.vi' : '';

  // The landing page.
  const l = landing(lang, { version, rel: to => rel(home, to) });
  write(home, page({ lang, url: home, alt: other, title: l.title, description: l.description, body: l.body, bodyClass: 'home', jsonLd: l.jsonLd }));
  urls.push([home, other]);

  // The guide's front page, then one page per topic.
  const index = guideHtml(path.join(DOCS, `guide${suffix}.md`), home + 'guide/', lang);
  write(home + 'guide/', page({
    lang, url: home + 'guide/', alt: other + 'guide/', title: `${index.title} · Bloxkeep`, description: index.description,
    body: `<article class="wrap doc"><h1>${esc(index.title)}</h1>\n${index.body}</article>`, bodyClass: 'guide-index',
  }));
  urls.push([home + 'guide/', other + 'guide/']);

  for (const topic of TOPICS) {
    const url = `${home}guide/${topic}/`;
    const g = guideHtml(path.join(DOCS, 'guide', `${topic}${suffix}.md`), url, lang);
    const list = TOPICS.map(x => `<li><a href="${rel(url, `${home}guide/${x}/`)}"${x === topic ? ' aria-current="page"' : ''}>${esc(t.topics[x])}</a></li>`).join('');
    write(url, page({
      lang, url, alt: `${other}guide/${topic}/`, title: `${g.title} · ${t.guideTitle}`, description: g.description, bodyClass: 'guide',
      body: `<article class="wrap doc">
<p class="crumbs"><a href="${rel(url, home + 'guide/')}">${t.guideTitle}</a> › ${esc(g.title)}</p>
<h1>${esc(g.title)}</h1>
${g.body}
<nav class="topics" aria-label="${esc(t.allTopics)}"><h2>${t.allTopics}</h2><ul>${list}</ul></nav>
</article>`,
    }));
    urls.push([url, `${other}guide/${topic}/`]);
  }
}

// A page GitHub Pages shows for any address it doesn't have: absolute links, since it can be served at any depth.
fs.writeFileSync(path.join(OUT, '404.html'), `<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Not found · Bloxkeep</title><meta name="robots" content="noindex"><link rel="stylesheet" href="/roblox-account-manager/style.css"></head><body><main class="wrap doc"><h1>This page doesn't exist</h1><p><a href="/roblox-account-manager/">Bloxkeep's home page</a> · <a href="/roblox-account-manager/guide/">User guide</a></p><p lang="vi"><a href="/roblox-account-manager/vi/">Trang chủ tiếng Việt</a></p></main></body></html>\n`);

const today = new Date().toISOString().slice(0, 10);
fs.writeFileSync(path.join(OUT, 'sitemap.xml'), `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9" xmlns:xhtml="http://www.w3.org/1999/xhtml">
${urls.map(([u, alt]) => {
  const vi = u.startsWith('vi/');
  return `<url><loc>${SITE}${u}</loc><lastmod>${today}</lastmod>` +
    `<xhtml:link rel="alternate" hreflang="${vi ? 'vi' : 'en'}" href="${SITE}${u}"/>` +
    `<xhtml:link rel="alternate" hreflang="${vi ? 'en' : 'vi'}" href="${SITE}${alt}"/></url>`;
}).join('\n')}
</urlset>
`);
fs.writeFileSync(path.join(OUT, 'robots.txt'), `User-agent: *\nAllow: /\n\nSitemap: ${SITE}sitemap.xml\n`);

// Files as they are: screenshots, logo, the picture link previews show, the stylesheet, Search Console's file.
fs.cpSync(path.join(DOCS, 'screenshots'), path.join(OUT, 'screenshots'), { recursive: true });
fs.copyFileSync(path.join(ROOT, 'RobloxManager', 'wwwroot', 'logo.svg'), path.join(OUT, 'logo.svg'));
fs.copyFileSync(path.join(DOCS, 'brand', 'social-preview.png'), path.join(OUT, 'og.png'));
fs.copyFileSync(path.join(DOCS, 'brand', 'bloxkeep.png'), path.join(OUT, 'icon-128.png'));
fs.copyFileSync(path.join(HERE, 'style.css'), path.join(OUT, 'style.css'));
fs.cpSync(path.join(HERE, 'static'), OUT, { recursive: true });

console.log(`Built ${urls.length} pages (Bloxkeep ${version}) into ${path.relative(ROOT, OUT)}`);
