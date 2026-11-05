// Game news cards (worker/feeds.js cardsOf): every kind of thing Discord lets another game's bot post becomes blocks the
// app draws, in Discord's order. No worker needed: node cards.mjs
import { cardsOf, md } from '../worker/feeds.js';

let failures = 0;
const check = (name, ok, detail = '') => { console.log(`${ok ? 'PASS' : 'FAIL'}  ${name}${ok ? '' : `  [${String(detail).slice(0, 400)}]`}`); if (!ok) failures++; };
const AT = Date.parse('2026-11-05T12:00:00Z');
const PIC = 'https://media.discordapp.net/attachments/1/2/boss.png';
const kinds = c => c.blocks.map(b => b.t).join(',');

// Markdown kept for the app to draw; pings and mentions out.
check('markdown kept, pings out', md('<@&123456789012> **Boss** spawned @everyone in <#123456789012>, use </join:123456789012>\n> quoted\n```\ncode\n```')
  === '**Boss** spawned  in , use /join\n> quoted\n```\ncode\n```', md('<@&123456789012> **Boss** spawned @everyone in <#123456789012>, use </join:123456789012>'));

// An embed with everything an embed can have.
const embed = cardsOf({
  content: '<@&123456789012> new boss!', author: { id: '123456789012', name: 'BossBot', avatar: null }, flags: 0,
  embeds: [{
    title: 'Dough King', url: 'https://example.com/boss', description: '**Spawned** in *Sea 3*\n- Island: Castle\n- HP: `5,000,000`',
    color: 0xff8800, author: { name: 'Boss Alerts', icon_url: 'https://cdn.discordapp.com/icons/1/a.png', url: 'https://example.com' },
    thumbnail: { url: 'https://x.test/t.png', proxy_url: 'https://images-ext-1.discordapp.net/external/t.png' },
    fields: [{ name: 'Server', value: 'EU', inline: true }, { name: 'Players', value: '12/12', inline: true }, { name: 'Ends', value: '<t:4102444800:R>', inline: false }],
    image: { url: PIC, proxy_url: PIC, width: 800, height: 400 },
    footer: { text: 'Boss Alerts v2', icon_url: 'https://cdn.discordapp.com/icons/1/b.png' }, timestamp: '2026-11-05T11:59:00Z',
  }],
  components: [{ type: 1, components: [{ type: 2, style: 5, label: 'Join', url: 'https://www.roblox.com/games/2753915549', emoji: { name: '🎮' } }, { type: 2, style: 1, label: 'Remind me', custom_id: 'x' }] }],
}, AT);
const e = embed[0];
check('embed: one card, its title, author, colour, side picture', embed.length === 1 && e.title === 'Dough King' && e.author?.name === 'Boss Alerts' && e.color === 0xff8800
  && e.picture === 'https://images-ext-1.discordapp.net/external/t.png' && e.link === 'https://example.com/boss', JSON.stringify(e));
check('embed: blocks in order (text, fields, picture, footer, buttons)', kinds(e) === 'md,md,fields,gallery,footer,buttons', kinds(e));
check('embed: the ping line, without the ping', e.blocks[0].md === 'new boss!', e.blocks[0].md);
check('embed: description keeps its markdown', e.blocks[1].md.startsWith('**Spawned** in *Sea 3*\n- Island'), e.blocks[1].md);
check('embed: fields side by side or on their own line', e.blocks[2].items.map(f => f.inline).join() === 'true,true,false', JSON.stringify(e.blocks[2]));
check('embed: footer with its time', e.blocks[4].md === 'Boss Alerts v2' && e.blocks[4].at === Date.parse('2026-11-05T11:59:00Z') && e.blocks[4].icon === 'https://cdn.discordapp.com/icons/1/b.png', JSON.stringify(e.blocks[4]));
check('buttons: a link opens, another bot\'s button only shows', e.blocks[5].items[0].url === 'https://www.roblox.com/games/2753915549' && e.blocks[5].items[0].emoji === '🎮' && e.blocks[5].items[1].url === null, JSON.stringify(e.blocks[5]));
check('until from a field\'s time', e.until === 4102444800000, e.until);

// Pictures from anywhere but Discord's own servers are left out.
const away = cardsOf({ content: 'x', embeds: [{ title: 'A', image: { url: 'https://evil.test/a.png' }, thumbnail: { url: 'https://evil.test/b.png' } }] }, AT)[0];
check('pictures only through Discord', away.picture === null && !away.blocks.some(b => b.t === 'gallery'), JSON.stringify(away));

// Embeds sharing a link, each with one picture: one gallery.
const grid = cardsOf({ embeds: [1, 2, 3].map(i => ({ url: 'https://example.com/p', ...(i === 1 ? { title: 'Fruits' } : {}), image: { url: `https://media.discordapp.net/a/${i}.png` } })) }, AT);
check('embeds with one link: one card, one gallery of 3', grid.length === 1 && grid[0].blocks.find(b => b.t === 'gallery')?.items.length === 3, JSON.stringify(grid));

// Discord's newer layout: a box with a section beside a picture, a gallery, a file, a line, a select menu.
const v2 = cardsOf({
  flags: 1 << 15, author: { id: '123456789012', name: 'SpawnBot', avatar: 'a_0123456789abcdef0123456789abcdef' },
  attachments: [{ name: 'map.txt', url: 'https://cdn.discordapp.com/attachments/1/2/map.txt', size: 2048, type: 'text/plain' }],
  components: [{
    type: 17, accent_color: 0x3388ff, spoiler: true, components: [
      { type: 10, content: '## Mirage Island\nSpawned <t:1762340000:R>' },
      { type: 9, components: [{ type: 10, content: '**Server** EU-West' }], accessory: { type: 11, media: { url: PIC, proxy_url: PIC } } },
      { type: 9, components: [{ type: 10, content: 'Join fast' }], accessory: { type: 2, style: 5, label: 'Open', url: 'https://example.com/j' } },
      { type: 14, divider: true, spacing: 2 },
      { type: 12, items: [{ media: { url: PIC, proxy_url: PIC }, description: 'the island', spoiler: true }, { media: { url: 'https://cdn.discordapp.com/attachments/1/2/clip.mp4', content_type: 'video/mp4' } }] },
      { type: 13, file: { url: 'attachment://map.txt' } },
      { type: 1, components: [{ type: 3, placeholder: 'Pick a server', custom_id: 's' }] },
    ],
  }],
}, AT);
const b = v2[0];
check('box: one card, its colour, hidden as a spoiler', v2.length === 1 && b.color === 0x3388ff && b.spoiler === true, JSON.stringify(v2).slice(0, 400));
check('box: its first line (a Discord heading) is the title', b.title === 'Mirage Island' && b.blocks[0].md === 'Spawned <t:1762340000:R>', JSON.stringify(b.blocks[0]) + b.title);
check('box: blocks in order', kinds(b) === 'md,section,section,sep,gallery,file,buttons', kinds(b));
check('section with its picture', b.blocks[1].thumb === PIC && b.blocks[1].md === '**Server** EU-West', JSON.stringify(b.blocks[1]));
check('section with a link button', b.blocks[2].button?.url === 'https://example.com/j' && b.blocks[2].button.label === 'Open', JSON.stringify(b.blocks[2]));
check('a big line between', b.blocks[3].line && b.blocks[3].big, JSON.stringify(b.blocks[3]));
check('gallery: picture with its words and spoiler, and a video', b.blocks[4].items[0].alt === 'the island' && b.blocks[4].items[0].spoiler && b.blocks[4].items[1].video, JSON.stringify(b.blocks[4]));
check('file: its name and size', b.blocks[5].name === 'map.txt' && b.blocks[5].size === 2048, JSON.stringify(b.blocks[5]));
check('a select menu shows as what it says', b.blocks[6].items[0].menu && b.blocks[6].items[0].label === 'Pick a server', JSON.stringify(b.blocks[6]));
check('the poster\'s picture (animated)', b.author.icon === 'https://cdn.discordapp.com/avatars/123456789012/a_0123456789abcdef0123456789abcdef.webp?size=64', b.author.icon);

// A plain message: text, pictures, a file, a sticker, a poll.
const plainMsg = cardsOf({
  content: 'Full moon tonight 🌕\nGet ready', author: { name: 'MoonBot' },
  attachments: [
    { name: 'moon.png', url: 'https://cdn.discordapp.com/attachments/1/2/moon.png', proxy: 'https://media.discordapp.net/attachments/1/2/moon.png', type: 'image/png', w: 512, h: 512 },
    { name: 'SPOILER_x.png', url: 'https://cdn.discordapp.com/attachments/1/2/SPOILER_x.png', type: 'image/png' },
    { name: 'log.zip', url: 'https://cdn.discordapp.com/attachments/1/2/log.zip', type: 'application/zip', size: 10_000_000 },
  ],
  stickers: [{ id: '749054660769218631', name: 'Wave', format: 1 }, { id: '749054660769218632', name: 'Lottie', format: 3 }],
  poll: { question: { text: 'Which fruit?' }, answers: [{ answer_id: 1, poll_media: { text: 'Kitsune', emoji: { name: '🦊' } } }, { answer_id: 2, poll_media: { text: 'Dragon' } }],
    expiry: '2026-11-06T12:00:00Z', results: { is_finalized: false, answer_counts: [{ id: 1, count: 7 }] } },
}, AT);
const p = plainMsg[0];
check('plain message: first line as title, the rest as text', p.title === 'Full moon tonight 🌕' && p.blocks[0].md === 'Get ready', JSON.stringify(p).slice(0, 300));
check('plain message: blocks in order', kinds(p) === 'md,gallery,file,sticker,poll', kinds(p));
check('pictures through Discord\'s copy, spoiler marked', p.blocks[1].items[0].url === 'https://media.discordapp.net/attachments/1/2/moon.png' && p.blocks[1].items[1].spoiler, JSON.stringify(p.blocks[1]));
check('sticker (a Lottie one is left out)', p.blocks[3].url === 'https://media.discordapp.net/stickers/749054660769218631.webp?size=160' && kinds(p).split('sticker').length === 2, JSON.stringify(p.blocks[3]));
check('poll with its answers and votes', p.blocks[4].q === 'Which fruit?' && p.blocks[4].answers[0].votes === 7 && p.blocks[4].answers[0].emoji === '🦊' && p.blocks[4].answers[1].votes === 0, JSON.stringify(p.blocks[4]));

// A stock box split at its headings, items as tiles (Vulcan's way), still the same for the alerts.
const stock = cardsOf({
  flags: 1 << 15, components: [{ type: 17, components: [
    { type: 10, content: '**Normal Stock:**\n<:flame:123456789012> **Flame • <:money:223456789012>`250,000`**\n**Mirage Stock:**\n🍌 **Banana • `1,000`**\nChanges <t:4102444800:R>' },
  ] }],
}, AT);
check('stock: two cards, items as side-by-side fields', stock.length === 2 && stock[0].title === 'Normal Stock' && kinds(stock[0]) === 'fields' && stock[0].blocks[0].items[0].inline
  && stock[1].title === 'Mirage Stock' && kinds(stock[1]) === 'fields,md' && stock[1].until === 4102444800000, JSON.stringify(stock));

// Something Discord adds later, with words: still shown as text.
const later = cardsOf({ flags: 1 << 15, components: [{ type: 99, content: 'A new kind of thing' }] }, AT);
check('a kind not known yet: its words kept', later[0]?.title === 'A new kind of thing', JSON.stringify(later));

console.log(failures ? `\n${failures} FAILED` : '\nall passed');
process.exit(failures ? 1 : 0);
