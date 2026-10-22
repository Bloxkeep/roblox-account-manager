# How Bloxkeep's Discord messages look

Every message Bloxkeep posts to Discord follows this, whoever sends it: the app (Settings › Discord), the Roblox watch
(`RobloxWatch/watch.py`), the feedback Worker (`Feedback/worker`) and the domain watch. The model is a good status bot:
a reader gets the news from the title and the coloured bar, the facts from short fields side by side, and nothing they
have to read twice. Look at a message in `dev/discord-preview.html?src=<payloads.json>` before it ships.

## Sender

- `username` **Bloxkeep** and `avatar_url` the logo,
  `https://raw.githubusercontent.com/Bloxkeep/roblox-account-manager/main/docs/brand/bloxkeep.png` (128 px). In the app the
  person can set their own name and picture; these are the defaults.

## The embed

One embed per event. A message about several events at once (a launch of six accounts) is still one embed, with a
field per item.

| Part | Rule |
|---|---|
| Colour | What it means, Discord's own colours: green `0x57F287` good or done, yellow `0xFEE75C` needs a look, red `0xED4245` broken or blocked, blurple `0x5865F2` news, grey `0x4E5058` nothing to do |
| Author | Optional. The subject when the title is about something else: the account's name with its avatar (`icon_url`), or the game |
| Title | The news in a few words, at most ~60 characters, no full stop, no emoji. "Roblox 0.744 is live", "NoobFarm2 was kicked", "Play time this week". Links (`url`) to where to read more |
| Description | One or two short, plain sentences: what it means for the reader and what to do. No jargon, no internal names. Leave it out when the title and fields say it all |
| Fields | The facts, short and **inline**: Discord puts three to a row, beside a thumbnail too, so give them in threes (3 or 6) or the last row has one alone. Field names of one or two words, values of a few. Versions, ids and builds in `code`. Times as Discord timestamps (`<t:unix:f>`, or `<t:unix:R>` for "3 minutes ago"), which each reader sees in their own time zone and language, never as UTC text. A field that is a list (several accounts) is not inline. At most about eight fields |
| Thumbnail | The subject's picture: the game's icon, the account's avatar; the Bloxkeep logo when there is no subject |
| Image | Only for something to look at: the play-time summary's chart |
| Footer | `Bloxkeep · <what sent it>` ("Roblox watch", "Feedback", "Alerts", "Summary") with the logo as `icon_url`, and the embed `timestamp` set to when it happened |

## The Roblox update card

The bot's Roblox update (`DiscordBot/worker/texts.js` `buildMessage`) is a card in Discord's newer layout (components,
flag `1 << 15`) instead of an embed, so it can carry a heading, a list and buttons:

- A container in the verdict's colour. Beside its picture, a `##` heading ("Roblox 0.744 is live", the weekly release
  only), "Multi-Roblox · **No change**" with the watch's sentence, and a small line with the build, its hash and when it
  went live (or "on Roblox's test channel").
- The picture is the verdict's icon (`docs/brand/bot/update-*.png`, drawn by `make-icons.py`): an update arrow around a
  check, `!`, `×`, a dot (first build seen) or three dots (not checked yet). Not the logo: the bot's avatar is the logo
  already.
- A divider, then "What's new" from Roblox's own notes: up to three improvements and three fixes, those a player notices
  first (not about Studio, Luau or the API), each cut near 110 characters, and "6 of 33 changes". Until Roblox posts
  its notes (usually after the build is live), a small line says they will show up; the bot edits them into each post
  once they're out.
- Link buttons: Roblox's notes (once out) and how the check works. The role, when a server pings one, is a line above
  the card.

## Words

- Short and plain, the way the reader would say it. "Multi-Roblox: no change", not "No change to Roblox's
  single-window lock or its code names".
- Bold (`**…**`) for the one thing that matters in a description, never for whole sentences.
- `-#` small text for a side note (a hint, "sent late"), at the end.
- Names people typed are escaped (`DiscordWebhook.Escape`), so their `*` or `@` stay text. Mentions are off unless the
  person chose who gets pinged.
- The app writes in the language chosen for Discord (English or Vietnamese); the Roblox watch and the feedback Worker
  write in English.

## Don't

- Walls of text, a sentence per field, the same fact twice (title and a field).
- Raw ISO or UTC times, internal ids nobody needs, technical names of Roblox's code.
- Emoji as decoration; colour already says how it went.
- More than one message for one event.
