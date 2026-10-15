// Takes "Send feedback" from Bloxkeep (an idea or a bug report, RobloxManager/AppController.Feedback.cs) and posts it to
// the developer's Discord channel. The webhook stays here (secret FEEDBACK_WEBHOOK), out of the app, so nobody can
// post to the channel directly. What anyone can send here is kept small: one JSON body of a few kilobytes, a few sends
// a minute from one address and a few dozen from everyone, and no pings whatever the text says.
const MAX_BODY = 40_000;
const MAX_TEXT = 2000;
const MAX_CONTACT = 100;
const MAX_LOG = 16_000;
// How it looks: docs/discord-style.md.
const LOGO = 'https://raw.githubusercontent.com/minhhoang0122/bloxkeep/main/docs/brand/bloxkeep.png';
const COLORS = { bug: 0xED4245, idea: 0x5865F2 };

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname !== '/feedback')
      return reply({ error: 'not found' }, 404);
    if (request.method !== 'POST')
      return reply({ error: 'POST only' }, 405);
    const ip = request.headers.get('CF-Connecting-IP') || 'unknown';
    // Rate limiting bindings (wrangler.toml); missing ones (a local run) don't limit.
    if (env.LIMITER && !(await env.LIMITER.limit({ key: ip })).success)
      return reply({ error: 'rate' }, 429);
    if (env.LIMITER_ALL && !(await env.LIMITER_ALL.limit({ key: 'all' })).success)
      return reply({ error: 'rate' }, 429);
    if (Number(request.headers.get('content-length') || 0) > MAX_BODY)
      return reply({ error: 'too large' }, 413);
    const raw = await request.text();
    if (raw.length > MAX_BODY)
      return reply({ error: 'too large' }, 413);
    let body;
    try {
      body = JSON.parse(raw);
    } catch {
      return reply({ error: 'not JSON' }, 400);
    }
    const kind = body?.kind === 'bug' || body?.kind === 'idea' ? body.kind : null;
    const text = typeof body?.text === 'string' ? body.text.trim() : '';
    if (!kind || text.length < 3 || text.length > MAX_TEXT)
      return reply({ error: 'text' }, 400);
    const short = (value, max) => typeof value === 'string' ? value.trim().slice(0, max) : '';
    const contact = short(body.contact, MAX_CONTACT);
    const app = /^\d+\.\d+\.\d+$/.test(body.app) ? body.app : '?';
    const windows = short(body.windows, 80) || '?';
    const language = body.language === 'vi' ? 'Tiếng Việt' : 'English';
    const log = short(body.log, MAX_LOG);

    const fields = [
      { name: 'Bloxkeep', value: app === '?' ? '?' : `\`${app}\``, inline: true },
      { name: 'Windows', value: windows.replace(/^Windows /, ''), inline: true },
      { name: 'Language', value: language, inline: true },
    ];
    if (contact)
      fields.push({ name: 'Reply to', value: contact, inline: true });
    if (log)
      fields.push({ name: 'Error log', value: 'Attached', inline: true });
    const payload = {
      username: 'Bloxkeep',
      avatar_url: LOGO,
      embeds: [{
        title: kind === 'bug' ? 'Bug report' : 'Idea',
        description: text,
        color: COLORS[kind],
        fields,
        thumbnail: { url: LOGO },
        footer: { text: 'Bloxkeep · Feedback', icon_url: LOGO },
        timestamp: new Date().toISOString(),
      }],
      allowed_mentions: { parse: [] },
    };
    let init;
    if (log) {
      const form = new FormData();
      form.append('payload_json', JSON.stringify(payload));
      form.append('files[0]', new Blob([log], { type: 'text/plain' }), 'error-log.txt');
      init = { method: 'POST', body: form };
    } else {
      init = { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload) };
    }
    const sent = await fetch(env.FEEDBACK_WEBHOOK, init);
    if (!sent.ok)
      return reply({ error: 'discord' }, 502);
    return reply({ ok: true }, 200);
  },
};

function reply(value, status) {
  return new Response(JSON.stringify(value), { status, headers: { 'Content-Type': 'application/json' } });
}
