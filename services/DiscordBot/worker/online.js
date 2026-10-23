// How many people have Bloxkeep open right now. While it is open, the app says so every minute, further apart once
// many have it open (POST /online/ping: an id made from that PC, scrambled so it leads back to nothing, the same after
// Bloxkeep is reinstalled; its version and its language), and says goodbye as it closes. Whoever said so within one and
// a half of those gaps and a minute more is online; the id is forgotten once they aren't. The bot keeps no IP address and no history of who was online when: only the day's peak,
// one number a day.
// Bloxkeep's developers see it with /online (worker.js), on a page that link opens (dashboardPage), and anyone sees how
// many are online in the bot's custom status (texts.js presenceLines).

/** How often the app says it's open while few have it open (the app does what each answer says: changed here alone). */
export const BEAT_SECONDS = 60;
/**
 * Beats a day the bot takes at most: with more people online they come further apart (beatEvery), so that they and the
 * bot's own work stay within Cloudflare's free plan (100,000 requests, and as many rows written, a day for the whole
 * account). Past that, the bot itself would stop answering (slash commands, the app's Discord button) until the next day.
 */
export const BEAT_BUDGET = 40_000;
/** The longest the app is told to wait between beats (about 10,000 online); it never waits longer, whatever it's told. */
export const MAX_BEAT_SECONDS = 6 * 3600;
/** Online until one and a half gaps and a minute after a beat: one that comes late, or once tried again, still counts. */
export const GRACE = 1.5;
export const GRACE_SLACK_SECONDS = 60;

/** Seconds between beats with this many online: a minute up to about 27 online, then further apart. */
export const beatEvery = (online, budget = BEAT_BUDGET) => Math.min(MAX_BEAT_SECONDS, Math.max(BEAT_SECONDS, Math.ceil(online * 86_400 / budget)));
/** Days run on Vietnam's time (UTC+7, no summer time), the developer's. */
const DAY_OFFSET = 7 * 3600_000;
/** Days the dashboard shows. */
export const DAYS_SHOWN = 30;

/** Day's date (YYYY-MM-DD, Vietnam time) of a moment. */
export const dayOf = ms => new Date(ms + DAY_OFFSET).toISOString().slice(0, 10);

/** What the app sent, checked: { id, app, lang } or { id, gone: true }; null for anything else. */
export function readBeat(body) {
  const id = typeof body?.id === 'string' ? body.id.toLowerCase() : '';
  if (!/^[0-9a-f]{32}$/.test(id))
    return null;
  if (body.gone === true)
    return { id, gone: true };
  const app = typeof body.app === 'string' && /^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(body.app) && body.app.length <= 30 ? body.app : null;
  if (!app)
    return null;
  return { id, app, lang: body.language === 'vi' ? 'vi' : 'en' };
}

/** The dashboard's key, made from the bot's token: only the link /online gives opens it, and a new token changes it. */
export async function dashboardKey(token) {
  if (!token)
    return null;
  const key = await crypto.subtle.importKey('raw', new TextEncoder().encode(token), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  const mac = new Uint8Array(await crypto.subtle.sign('HMAC', key, new TextEncoder().encode('bloxkeep-online')));
  return btoa(String.fromCharCode(...mac.slice(0, 18))).replace(/\+/g, '-').replace(/\//g, '_');
}

const WEEKDAYS = ['Sun', 'Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat'];

/**
 * The last 7 days' peaks as a bar chart, drawn by QuickChart (quickchart.io, Chart.js) from those seven numbers alone:
 * Discord's dark grey behind, each day's peak over its bar, today's bar the bright green.
 */
export function weekChart(days, today) {
  const week = days.slice(-7);
  const label = d => (d.day === today ? 'Today' : `${WEEKDAYS[new Date(`${d.day}T00:00:00Z`).getUTCDay()]} ${Number(d.day.slice(8))}`);
  const chart = {
    type: 'bar',
    data: {
      labels: week.map(label),
      datasets: [{ data: week.map(d => d.peak), backgroundColor: week.map(d => (d.day === today ? '#23a55a' : '#3f6f52')), borderRadius: 6, barPercentage: 0.62 }],
    },
    options: {
      layout: { padding: { top: 26, left: 8, right: 8, bottom: 4 } },
      plugins: { legend: { display: false }, datalabels: { anchor: 'end', align: 'end', color: '#f2f3f5', font: { size: 13, weight: 'bold' } } },
      scales: { x: { grid: { display: false }, border: { display: false }, ticks: { color: '#949ba4', font: { size: 12 } } }, y: { display: false, beginAtZero: true } },
    },
  };
  return `https://quickchart.io/chart?v=4&w=640&h=240&devicePixelRatio=2&bkg=${encodeURIComponent('#2b2d31')}&c=${encodeURIComponent(JSON.stringify(chart))}`;
}

/**
 * /online's answer, a card: how many use Bloxkeep now as its heading with today's peak under it, the last 7 days'
 * peaks as a chart, the versions in use, and the dashboard's link.
 */
export function onlineMessage(o, link) {
  const time = at => `<t:${Math.floor(at / 1000)}:t>`;
  const text = content => ({ type: 10, content });
  const peak = o.today.peak ? `Peak today **${o.today.peak}**, at ${time(o.today.at)}` : 'Nobody yet today';
  const versions = o.apps.map(r => `${r.name} **${r.n}**`).join(' · ');
  return {
    flags: 1 << 15,
    components: [{
      type: 17, accent_color: o.now ? 0x57F287 : 0x4E5058, components: [
        text(`## ${o.now} using Bloxkeep now\n${peak}`),
        // No chart of seven empty days.
        ...(o.days.slice(-7).some(d => d.peak)
          ? [{ type: 12, items: [{ media: { url: weekChart(o.days, o.today.day) }, description: "Each day's peak, the last 7 days" }] }] : []),
        text([...(versions ? [`**Versions** ${versions}`] : []),
          `-# The chart: each day's highest, the last 7 days. An app counts while it said it's open within ${o.forMinutes} minutes.`].join('\n')),
        ...(link ? [{ type: 1, components: [{ type: 2, style: 5, label: 'Dashboard', url: link }] }] : []),
      ],
    }],
  };
}

/**
 * The dashboard: online now, today's peak, the peak of each of the last 30 days, and who is online by version and
 * language. It reads /online.json with the same key every 30 seconds. English, or Vietnamese for a Vietnamese browser.
 */
export function dashboardPage() {
  return `<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta name="robots" content="noindex">
<title>Bloxkeep online</title>
<style>
:root {
  color-scheme: light;
  --page: #f4f4f2; --surface: #fcfcfb; --line: #e3e2de; --grid: #ecebe7;
  --text: #0b0b0b; --text-2: #52514e; --muted: #7a7974; --series: #2a78d6; --tip: #0b0b0b; --tip-text: #f0efec;
}
@media (prefers-color-scheme: dark) {
  :root:not([data-theme="light"]) {
    color-scheme: dark;
    --page: #111110; --surface: #1a1a19; --line: #2e2d2b; --grid: #262624;
    --text: #f0efec; --text-2: #c3c2b7; --muted: #8f8e88; --series: #3987e5; --tip: #f0efec; --tip-text: #0b0b0b;
  }
}
:root[data-theme="dark"] {
  color-scheme: dark;
  --page: #111110; --surface: #1a1a19; --line: #2e2d2b; --grid: #262624;
  --text: #f0efec; --text-2: #c3c2b7; --muted: #8f8e88; --series: #3987e5; --tip: #f0efec; --tip-text: #0b0b0b;
}
* { box-sizing: border-box; }
body { margin: 0; background: var(--page); color: var(--text); font: 14px/1.45 "Segoe UI", system-ui, sans-serif; }
main { max-width: 880px; margin: 0 auto; padding: 24px 16px 40px; }
h1 { font-size: 15px; font-weight: 600; margin: 0 0 16px; color: var(--text-2); }
.tiles { display: grid; grid-template-columns: repeat(auto-fit, minmax(200px, 1fr)); gap: 12px; margin-bottom: 12px; }
.card { background: var(--surface); border: 1px solid var(--line); border-radius: 8px; padding: 16px; }
.label { color: var(--text-2); font-size: 13px; }
.big { font-size: 44px; font-weight: 600; line-height: 1.1; margin-top: 4px; font-variant-numeric: tabular-nums; }
.sub { color: var(--muted); font-size: 12px; margin-top: 2px; }
h2 { font-size: 13px; font-weight: 600; margin: 0 0 12px; color: var(--text-2); }
.chart { position: relative; height: 180px; margin-left: 28px; border-bottom: 1px solid var(--muted);
  display: flex; align-items: flex-end; gap: 2px; }
.gridline { position: absolute; left: 0; right: 0; border-top: 1px solid var(--grid); }
.tick { position: absolute; left: -28px; width: 22px; text-align: right; font-size: 11px; color: var(--muted); transform: translateY(-50%); }
.slot { flex: 1; height: 100%; display: flex; align-items: flex-end; justify-content: center; position: relative; }
.bar { width: 100%; max-width: 20px; background: var(--series); border-radius: 4px 4px 0 0; }
.slot:hover .bar, .slot:focus .bar { opacity: .8; }
.slot:focus { outline: none; }
.xaxis { display: flex; justify-content: space-between; margin: 6px 0 0 28px; font-size: 11px; color: var(--muted); }
.tip { position: absolute; bottom: calc(100% + 6px); left: 50%; transform: translateX(-50%); background: var(--tip); color: var(--tip-text);
  font-size: 12px; padding: 4px 8px; border-radius: 6px; white-space: nowrap; pointer-events: none; display: none; z-index: 1; }
.slot:hover .tip, .slot:focus .tip { display: block; }
.slot:first-child .tip { left: 0; transform: none; }
.slot:last-child .tip { left: auto; right: 0; transform: none; }
.lists { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 12px; margin-top: 12px; }
table { width: 100%; border-collapse: collapse; font-variant-numeric: tabular-nums; }
td { padding: 5px 0; border-top: 1px solid var(--grid); }
td:last-child { text-align: right; font-weight: 600; }
tr:first-child td { border-top: 0; }
.empty { color: var(--muted); }
details { margin-top: 12px; color: var(--text-2); font-size: 13px; }
details table { margin-top: 8px; max-width: 320px; }
footer { color: var(--muted); font-size: 12px; margin-top: 16px; }
.error { color: var(--text-2); }
</style>
</head>
<body>
<main>
  <h1 id="title">Bloxkeep · online</h1>
  <div class="tiles">
    <div class="card"><div class="label" data-t="now">Online now</div><div class="big" id="now">–</div><div class="sub" id="nowSub"></div></div>
    <div class="card"><div class="label" data-t="peak">Peak today</div><div class="big" id="peak">–</div><div class="sub" id="peakSub"></div></div>
  </div>
  <div class="card">
    <h2 data-t="days">Peak online, day by day</h2>
    <div class="chart" id="chart" role="img"></div>
    <div class="xaxis"><span id="from"></span><span id="to"></span></div>
    <details><summary data-t="table">As a table</summary><table id="dayTable"></table></details>
  </div>
  <div class="lists">
    <div class="card"><h2 data-t="byApp">Online now, by version</h2><table id="apps"></table></div>
    <div class="card"><h2 data-t="byLang">Online now, by language</h2><table id="langs"></table></div>
  </div>
  <footer id="foot"></footer>
</main>
<script>
const vi = (navigator.language || '').toLowerCase().startsWith('vi');
const T = vi ? {
  now: 'Đang online', peak: 'Cao nhất hôm nay', days: 'Số online cao nhất mỗi ngày', table: 'Xem dạng bảng', byApp: 'Đang online, theo phiên bản',
  byLang: 'Đang online, theo ngôn ngữ', nobody: 'Chưa có ai', at: t => 'lúc ' + t, none: 'Chưa có ai trong hôm nay', day: 'Ngày', people: 'Số người',
  foot: m => 'Tính các app Bloxkeep đã báo đang mở trong ' + m + ' phút qua. Tự cập nhật mỗi 30 giây.', error: 'Không tải được số liệu. Thử lại sau ít phút.',
  langs: { vi: 'Tiếng Việt', en: 'Tiếng Anh' }, updated: t => 'Cập nhật lúc ' + t,
} : {
  now: 'Online now', peak: 'Peak today', days: 'Peak online, day by day', table: 'As a table', byApp: 'Online now, by version',
  byLang: 'Online now, by language', nobody: 'Nobody', at: t => 'at ' + t, none: 'Nobody today yet', day: 'Day', people: 'People',
  foot: m => "Counts the Bloxkeep apps that said they're open in the last " + m + ' minutes. Updates every 30 seconds.',
  error: "Couldn't load the numbers. Try again in a few minutes.", langs: { vi: 'Vietnamese', en: 'English' }, updated: t => 'Updated ' + t,
};
if (vi) document.documentElement.lang = 'vi';
document.querySelectorAll('[data-t]').forEach(e => { e.textContent = T[e.dataset.t]; });
const $ = id => document.getElementById(id);
const clock = ms => new Date(ms).toLocaleTimeString(vi ? 'vi-VN' : 'en-GB', { hour: '2-digit', minute: '2-digit' });
const shortDay = d => new Date(d + 'T00:00:00Z').toLocaleDateString(vi ? 'vi-VN' : 'en-GB', { day: 'numeric', month: 'short', timeZone: 'UTC' });
const esc = s => String(s).replace(/[&<>"]/g, c => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));
const rows = (list, name) => list.length ? list.map(r => '<tr><td>' + esc(name(r.name)) + '</td><td>' + r.n + '</td></tr>').join('') : '<tr><td class="empty">' + T.nobody + '</td></tr>';
function draw(o) {
  $('now').textContent = o.now;
  $('nowSub').textContent = T.updated(clock(Date.parse(o.at)));
  $('peak').textContent = o.today.peak;
  $('peakSub').textContent = o.today.peak ? T.at(clock(o.today.at)) : T.none;
  const max = Math.max(1, ...o.days.map(d => d.peak));
  const step = max <= 5 ? 1 : Math.ceil(max / 4);
  const top = Math.ceil(max / step) * step;
  let html = '';
  for (let v = step; v <= top; v += step)
    html += '<div class="gridline" style="bottom:' + (v / top * 100) + '%"></div><div class="tick" style="bottom:' + (v / top * 100) + '%;transform:translateY(50%)">' + v + '</div>';
  html += '<div class="tick" style="bottom:0;transform:translateY(50%)">0</div>';
  html += o.days.map(d => '<div class="slot" tabindex="0"><div class="bar" style="height:' + (d.peak / top * 100) + '%"></div><div class="tip">'
    + shortDay(d.day) + ': ' + d.peak + (d.peak ? ' · ' + clock(d.at) : '') + '</div></div>').join('');
  $('chart').innerHTML = html;
  $('chart').setAttribute('aria-label', T.days);
  $('from').textContent = shortDay(o.days[0].day);
  $('to').textContent = shortDay(o.days[o.days.length - 1].day);
  $('dayTable').innerHTML = '<tr><td>' + T.day + '</td><td>' + T.people + '</td></tr>'
    + o.days.slice().reverse().map(d => '<tr><td>' + shortDay(d.day) + '</td><td>' + d.peak + '</td></tr>').join('');
  $('apps').innerHTML = rows(o.apps, n => 'Bloxkeep ' + n);
  $('langs').innerHTML = rows(o.languages, n => T.langs[n] || n);
  $('foot').textContent = T.foot(o.forMinutes);
}
async function load() {
  try {
    const res = await fetch('online.json' + location.search, { cache: 'no-store' });
    if (!res.ok) throw new Error(res.status);
    draw(await res.json());
  } catch {
    $('foot').innerHTML = '<span class="error">' + T.error + '</span>';
  }
}
load();
setInterval(load, 30000);
</script>
</body>
</html>`;
}
