# Localization guidelines

Bloxkeep speaks 14 languages. The bar for every one of them is the same: **a native speaker reads it and doesn't feel
it was translated.** Correct grammar and a faithful meaning are the starting point, not the goal.

These rules are binding for every new feature: no new English word ships without its words in every language,
written to this guide.

## Where the words live

| What | File | Notes |
|---|---|---|
| English and Vietnamese interface words | `ui-gpui/assets/words.json`, `ui-gpui/assets/i18n.json` | `{"en": {key: text}, "vi": {key: text}}`; i18n.json is one minified line |
| Other languages' interface words | `ui-gpui/assets/lang/<code>.json` → `"words"` | same keys as English, plus `country.XX` |
| Sentences the core says (errors, notices, toasts) | `ui-gpui/assets/lang/<code>.json` → `"core"`; Vietnamese in `ui-gpui/src/core/l.rs` (`PAIRS`) | keyed by the exact English sentence: changing an English sentence drops all its translations |
| Language list, number and date rules, plural rules | `ui-gpui/src/lang.rs` | CLDR plural categories per language |

A key missing in a language shows English. `node`/`python` checks below find those.

## Writing for a place, not for a dictionary

Before writing a string, find where it shows (`grep -rn '"the.key' ui-gpui/src`) and read the code around it. The same
English word can need different words in a button, a heading and a sentence.

| Place | How it reads | Example (en) |
|---|---|---|
| Button | The action, as that language's buttons say it (imperative, infinitive or noun, per locale) | Save · Open Roblox · Retry |
| Tab, nav item, chip | A short noun; must fit about the English width (Settings nav at 960 px truncates) | Accounts · Security |
| Heading | What the page or block is, in sentence case | Who can join you |
| Description, hint | One plain sentence: what it does for the user, not how it's built | Joins them again when a game drops them. |
| Tooltip | One short phrase; no period unless it is a full sentence | Refresh online status |
| Toast, notice | What happened, in past or present, no drama | 3 accounts signed in. |
| Error | What went wrong + what to do next; never blame the user | Can't reach roblox.com. Check your connection, then press Retry. |
| Empty state | What will be here and the one step that starts it | No presets yet. Make one from the accounts you play with. |
| Confirmation | The question names the thing and the result; the button repeats the verb | Delete "Farm 2"? · Delete |
| Placeholder | An example or a short prompt, never instructions | Search accounts, groups, notes… |

## The voice

- Modern, plain, clear, professional. Friendly, never chummy. Short, never clipped.
- Speak about what the user gets ("Joins again when a game drops you"), not about Bloxkeep's internals.
- No bureaucratic, legal or academic register; no marketing words ("seamless", "powerful", "effortless"); no forced jokes.
- No word-for-word calques. Don't carry English sentence structure, English Title Case, or English punctuation into
  other languages. Each language may say it differently if the meaning and purpose stay the same.
- Don't rewrite a string that is already natural just to make a change.
- Think of how Discord, Steam and Linear write in that language: that is the register, not their exact words.

## Terms

One concept, one word, everywhere in a language. The glossaries per language are below; the concepts are:

| Concept | English | Notes |
|---|---|---|
| A Roblox login kept in Bloxkeep | account | never "alt"/"acc" in the interface (users say it, the app doesn't) |
| A saved set of accounts + game + server | preset | Vietnamese keeps "Preset" |
| The encrypted store of accounts | vault | |
| The password that opens the vault | master password | |
| Joining again after a drop | auto-rejoin | |
| Keeping idle accounts in game | anti-AFK | |
| Recorded inputs played back | macro | |
| A running Roblox game instance | server; a paid one: private server | Roblox's own words where Roblox localizes them |
| Starting Roblox for an account | open / launch | the main button is Roblox's own "Play" in that language |
| A Roblox game | game (Roblox says "experience" in some locales) | use what players in that language say |

Never translate: Bloxkeep, Roblox, Robux, Discord, Windows, Roblox Studio, Fast Flags, product names of themes
(macOS, Ubuntu, KDE Plasma…).

## Safety rules for translators

- Keep keys, placeholders (`{n}`, `{0}`, `{name}`…), markup (`<button …>`, `<b>`) and `\n` exactly.
- Counts go through the plural rules: `key1` is the words for exactly 1 (may be "one account", "See its window"),
  `key#one` / `key#few` / `key#many` are CLDR forms (Russian and Polish need all three; French and Portuguese use
  `#one` for 0 and 1). A count not routed through them must be phrased count-neutral ("Accounts: {n}").
- Numbers, sizes, dates and times are formatted by `lang.rs`; write only the words around them.
- Thai and Lao have no spaces between words; the app breaks their lines with a dictionary segmenter. Use spaces only
  where those languages put them (between phrases).
- Korean particles follow the sound of the word before: Bloxkeep (블록스킵) takes 이/을/은/으로/과/이나; Roblox, Discord,
  Windows take 가/를/로.

## Checks before a change ships

1. Every key in English exists in every language (no English leaking through), and placeholders and markup match.
2. Labels for tabs, nav, chips and buttons are not much longer than English (about 1.7× or +8 characters at most).
3. Shots of the touched pages in the long and non-Latin languages (de, fr, ru, pl, th, lo) at 960 px wide.
4. For each string, the native test:
   - Would a native write it this way?
   - Does it sound translated from another language?
   - Does it fit its place (button, tooltip, error…)?
   - Is it understood on first read?
   - Is there a more natural way that keeps the meaning?
   - Does it feel like a cared-for product?

Strings no one could confirm with a native speaker are listed under **Needs native review** in each language's section.
Treat that list as open work: a native speaker's correction wins over this guide.

## Languages

### English (source) (`en`)

#### Style

- Sentence case everywhere ("Profile song", "Social media"); only brand/proper names capitalised.
- US spelling: color, favorites, customize, gray, harbor, behavior, anymore. (Was mixed with UK in about 12 strings.)
- Contractions are used ("Couldn't", "isn't"); no "Could not".
- Apostrophes: words.json uses straight `'`, i18n.json uses curly `’`. Keep each file's own.
- Buttons are verbs, tabs/nav are short nouns, errors say what happened ("Couldn't load X (why)") and what to do when there is something to do.
- One term per idea: sign in / sign out (not log in / log out), account, preset, vault, master password, auto-rejoin, anti-AFK, Multi-Roblox, drop (account leaves a game), "needs a look" (warning chip).
- Counts: "{n} <plural noun>" always has a `key1`; if the number can be 1 and a form is awkward, word it count-neutral ("Passes: {k}").
- Em dashes appear in about 20 strings (rbx.outdated*, ru.closed, mh.off, launch.*, rw.*). Not wrong; colons are the app's usual join. Left alone except where the string was changed anyway.

#### Glossary

Sign in / signed out (replaces Log out, Logged out) · account (not acc/profile) · vault (the .rmv file + master password) · master password · preset · macro · auto-rejoin · anti-AFK · Multi-Roblox · drop / dropped (leaves a game unwillingly) · kick (game removed you) · launch / open (Roblox windows: "open") · Fast Flags · private server · Job ID · Place ID · Marketplace · Favorites (US) · color (US) · locate (a server's country; replaces "place") · Refresh (replaces "Read again") · Sign out others · per server / per day (replaces "a server", "a day").

#### Needs native review

- `mods.failed` "Couldn't: {why}" shows "Couldn't: GameBanana" when no reason is known (mods.rs:176, 219). Needs a code-side fix (an action word) rather than a wording one.
- `mon.remote.hint`: "Anyone with the link can see the Roblox windows you can watch." The meaning of "you can watch" is unclear to me; owner should confirm.
- `vis.FriendsFollowingAndFollowers` is a dropdown option; "Friends, following, followers" is longer than before.
- Unused keys found while tracing (no code reference): `games.items`, `wiki.search`, `cm.sum`, `cm.altsN`, `sys.moreTip`, `macro.e.scroll`, `macro.e.atMost`. Not proposed; may be dead.
- Core sentence `Vietnamese typing is on` (macro status) is an IME state for any language; core text, not touched.
- `set.rejoinCount` "{n}/{total} accounts" reads "0/1 accounts" when there is one account. Minor; left.

#### Layout risks

- `vis.FriendsFollowingAndFollowers` (+9 chars) in a select/segment.
- `rs.logOutAll` stays the same length; `rs.loggedOut` is 1 char longer.
- `own.nRenew` "{n} renewing" is 3 chars longer than "{n} renew" in the Private servers summary line (own.nRunning · own.nRenew · own.nSoon).
- `pal.more` and `cm.youLine` change shape but not length much.
- `con.note` is a bit longer (header sub-line of Statistics › Console).
- Long tooltips/descriptions (`set.antiAfkEveryDesc`, `gpv.note`) are about the same length as before; no new overflow expected, but the antiAfkEveryDesc is ~20 characters longer.

### Tiếng Việt (`vi`)

#### Style

- Overall the existing Vietnamese was already natural, so this was a consistency and de-stiffening pass, not a rewrite.
- Address: "bạn" only where it carries meaning ("của bạn", "bạn bè"); errors for a missing input drop it ("Chưa nhập mật khẩu chủ.", not "Bạn chưa nhập...").
- No "Hãy" at the start of a sentence: plain imperative ("Chọn ít nhất một tài khoản.", "Đăng nhập lại."). ", hãy" mid-sentence stays. No "vui lòng", "thực hiện", "tiến hành" anywhere.
- Sentence case. "Preset" always capitalised and kept (never "cấu hình sẵn"); "tài khoản" always (no "acc"). "Chủ đề", not "theme".
- Short labels (1-3 words), verbs for buttons ("Chơi", "Sao chép", "Hủy"), short nouns for nav/tabs.
- Gamer loanwords people really say stay: server, game, link, file, kick, ban, văng, outfit, emote, cookie, macro, vault, Fast Flags, FPS, ping, online/offline.
- Diacritics: modern style on the oa/uy words (óa, ủy, ùy): Xóa, Hủy, Khóa, Tùy.
- Numbers: Vietnamese has no plural; the exactly-one keys (`key1`) now exist and read the same as the base.
- Placeholders, `<b>`/`<button>` markup and `\n` untouched (checked en vs vi for every key: 0 mismatches except the known `tf.monthYear`, where the page passes `{n}` too).

#### Glossary

- account -> tài khoản
- preset -> Preset
- vault -> vault
- master password -> mật khẩu chủ (one stray "mật khẩu chính" replaced; the user guide docs use "chủ")
- auto-rejoin -> Tự vào lại; rejoin -> vào lại (replaced "rejoin")
- anti-AFK -> Chống AFK
- macro -> macro
- Play button -> Chơi (Roblox VN's own word; replaced "Vào game" where it meant Play)
- join (a game/server) -> vào / vào cùng
- server -> server (replaced "máy chủ" for game servers; "máy chủ cập nhật / góp ý" for real web services stays)
- private server -> server riêng
- community -> cộng đồng (replaced "nhóm" for Roblox communities; "nhóm" stays for account groups and chat groups)
- game pass -> game pass (replaced "gamepass")
- body (avatar) -> Cơ thể (replaced "Thân"; Roblox VN's avatar category)
- emote -> Emote (kept the loanword in both places)
- outfit -> outfit / trang phục
- theme -> chủ đề (replaced "theme")
- customize -> Tùy chỉnh (replaced "Tuỳ biến")
- file -> file (replaced "tệp")
- copy -> Sao chép (replaced "Chép" / "Đã chép" on buttons and toasts)
- link -> link (replaced "liên kết")
- code (redeem / private server) -> code; verification/sign-in code -> mã
- Roblox settings -> cài đặt Roblox (replaced "cấu hình Roblox")
- delete -> Xóa; cancel -> Hủy; lock -> khóa
- drop / disconnect -> bị văng; kicked by game -> bị game kick; banned -> bị ban
- game -> game (replaced "trò chơi")
- select / tick -> chọn / bấm ("Tick" replaced by "chọn")

#### Needs native review

- `theme.keep` (words.json) "Thành lúc hoàng hôn" (English "Keep at dusk"): not changed, meaning unclear; probably "Giữ lúc hoàng hôn"? Check where it shows first.
- `gc.title` "Code" as a panel heading (was "Giftcode"): maybe "Mã quà" or "Giftcode" is what the audience says.
- Play = "Chơi" everywhere (Roblox VN's word) instead of the old "Vào game": confirm the owner agrees for the account card button and tray.
- `wiki.oddsUnlucky` "Bạn xui thật." and `wiki.oddsNormal` "Cố thêm chút.": playful tone, check it is not too chummy.
- `cm.altsN` "{n} alt", `cm.board` "Treo nhiều alt nhất": the guidelines say the app never says "alt"; the English also says "alts", so left as it is.
- `in.tr.cancel` "Hủy lời mời" for "Take it back" (withdraw a trade): "Rút lại" may be more exact.
- `fr.unfriend` "Hủy kết bạn": fine, but Roblox VN may say "Hủy kết bạn" too; unverified.
- "Thử lại nhé" (9 uses, friendly softener in errors): kept; drop if too chummy.
- `av.shelf.background` "Hình nền" for Roblox avatar backgrounds: Roblox VN may say "Nền".
- `rs.statusHeld` "Bị Roblox xử lý": vague; check against the Roblox VN moderation wording.

#### Layout risks

- `gp.copy`, `ps.copy`, `set.phoneCopy`: "Chép" (4) -> "Sao chép" (8): small buttons next to a code/link/topic; check they do not wrap.
- `pf.groups` "Cộng đồng" (9) in the profile section label; `home.tryPlay` and `card.play` got shorter ("Chơi").
- `gp.votes` "{up} thích · {down} không thích" is 8 characters longer; check the game page's vote row.
- `ct.fromThis` "Tùy chỉnh chủ đề này" is about 3 characters longer than before.
- `es.matters` "Ưu tiên" is 3 characters shorter; no risk.
- Nav and tab labels were not lengthened anywhere.

### Español (Latinoamérica) (`es`)

#### Style

- Informal "tú" everywhere (Presiona, Elige, Agrega); no vosotros, no Spain-only forms.
- Quotes «…»; the Roblox "Jugar" button word; sentence case, not English Title Case.
- Neutral LatAm lexicon: agregar (not añadir), verificar (not comprobar), presionar (not pulsar), mouse (not ratón), respaldo (not copia de seguridad), predeterminado, hacia adelante, "empieza con", promedio (not medio).
- Buttons are infinitives (Agregar cuenta, Guardar), tabs/nav are short nouns, errors say what happened plus what to do.
- "PC" kept for the computer; "ícono"; "video"; "celular" is not used (teléfono).
- Roblox community words: Insignias, Comunidad, Emotes, Trajes, Amigos, Pases, Servidor privado.

#### Glossary

account → cuenta; add → agregar (was añadir); check / verify → verificar (was comprobar); press → presionar (was pulsar); backup → respaldo (was copia de seguridad); mouse → mouse (was ratón); friend → amigo (was amistad); emote → Emote (was Gesto); outfit → traje (was atuendo in the item-swap dialog); preset → preset; vault → bóveda; master password → contraseña maestra; auto-rejoin → Reingreso automático; anti-AFK → Anti-AFK; macro → macro (f.); server / private server → servidor / servidor privado; launch/open → abrir; Settings › Launching → Configuración › Apertura (was Inicio, clashed with nav Home); sign in → iniciar sesión; drop / disconnected → caída / desconectada; kicked → expulsado; banned → baneado; Backup & data → Respaldo y datos; default → predeterminado; average → promedio; "board" (Discord status) → Estado.

#### Needs native review

- set.launch "Apertura" (new tab name for Launching; alternatives: Arranque, Lanzamiento).
- m.friends "Hacerlas amigas entre sí" and the core "Amistades: {0} nuevas, {1} ya eran amigas…" (kept "amigas" for accounts).
- sys.mood.drop / act.kicked / mon.st.bad "Desconectada" (feminine, refers to the account).
- home.needN "{n} te necesitan"; home.for "por {t}"; gp.items "Objeto"; ui.lastOpen "Última apertura" (source not found in code, context guessed).
- news.liveTip "Apariciones de jefes, stock" (game jargon "stock").
- gp.votes "a favor / en contra"; av3.small "Reducir".
- Keys with no literal use found in src (translated from English meaning alone): home.for, rj.k, ui.playBox, gp.items, ui.note, sys.window1, gp.now, card.dayDrops, es.matters, rc.level, snap.sizes, mh.windowedDesc, ps.schNoteTitle, ps.tip, act.inactive.
- Plurals: no "1"-forms added beyond those English already has (sys.moreTip, games.items, wiki.pages, cm.altsN, srv.players would read "1 ventanas más" if n=1).

#### Layout risks

- set.launch "Apertura" (8 chars, fine); nav "Respaldo y datos" unchanged.
- bk.on "Respaldo en una carpeta" shorter than before; bk.now "Hacer respaldo ahora" shorter.
- act.signedOut "Requiere iniciar sesión" is longer than English "Needs sign-in" (chip/status label).
- rs.onlineIs.* and rs.joinIs.* are full sentences (descriptions, not chips).
- dock.stepGo "Jugar" fine; mon.closeAsk "¿Cerrar {n}?" fine.
- m.friends "Hacerlas amigas entre sí" is longer than the English menu item.

### Português (Brasil) (`pt`)

#### Style

- Address: "você" (never "tu"/"teu"), imperative in "você" form (Escolha, Cole, Digite). pt-BR only.
- Tone: friendly, short, like Discord/Steam in Brazil. No bureaucratic "tem de"; use "precisa ter".
- Quotes “ ” (straight quotes fixed). Ellipsis …, "·" kept as separator. Percent without space: 50%.
- Gerund for live states (jogando, abrindo, carregando), not "a fazer".
- Buttons are verbs in the infinitive/imperative ("Salvar", "Copiar"); tabs/nav are short nouns.
- "Robux" is written out; the "R$" sign (Brazilian reais) is never used.
- Roblox community terms kept: Play = "Jogar", alts, farm, Fast Flags, Place ID, Job ID, AFK, preset, macro.
- Plurals: pt "one" covers 0 and 1; existing key / key#one / key1 pattern kept.

#### Glossary

account = conta; sign in = entrar (noun "login"); sign out = desconectar / sair; vault = cofre; master password = senha mestra;
preset = preset; auto-rejoin = reentrada automática (verb: entrar de novo); anti-AFK = Anti-AFK; macro = macro; step = etapa;
server = servidor; private server = servidor privado; launch/open = abrir; play = Jogar; window = janela; game = jogo;
experience = jogo; friend = amigo(a); follow = seguir; community (group) = comunidade; badge = emblema; outfit = visual/traje;
backup = backup; settings = Configurações; tray = bandeja; hotkey = atalho; drop/disconnect = queda / cair; kick = expulsar;
teleport = teletransporte (unified; was teleporte/teleportando); webhook = webhook; cookie = cookie; captcha = quebra-cabeça;
game box = caixa/quadro do jogo; theme = tema; label = rótulo; area = área; "Developer Mode" = Modo de desenvolvedor.

#### Needs native review

- ql.step2 "Login rápido" (Roblox mobile's pt-BR name for Quick Log In; I believe so, not verified).
- card.appeal "Recorrer" (Roblox might say "Apelar").
- gp.votes "curtidas / não curtidas" vs Roblox's own thumbs wording.
- kick.* "teletransporte" vs Roblox's own pt-BR error text (could be "teleporte").
- dc.open "Entrar" (assumed a join button).
- ps.active "Servidor" (row label of the on/off switch; "Ativo" may be better).
- macro.e.* sentence fragments (built from several pieces; word order depends on the code).
- rs.tp.Following "Quem eu sigo", wiki.read "leitura de {t}" (context assumed).
- Keys with no code reference (dead old-UI keys): translated by meaning only.

#### Layout risks

- nav.monitor ("Monitor") shortened deliberately; nav.shop "Mercado", nav.inbox "Caixa de entrada" are the longest nav labels.
- set.nav.* and chips are short. Longest buttons: "Reiniciar para atualizar", "Desativar início automático", "Selecionar resultados · {n}".
- ps.schNote, set.antiAfkEveryDesc, mon.remote.hint are long paragraphs (wrap fine, check height).
- dock.stepGo "Jogar" in a narrow step pill.

### Bahasa Indonesia (`id`)

#### Style

- Address: "kamu" / "-mu" (casual, gamer audience). Never Anda. Self-reference in Roblox privacy rows stays "saya" (as Roblox does).
- Sentence case everywhere; no English Title Case (fixed "Kualitas Grafik", "Kotak Masuk").
- Short colon style for explanations ("Mati: hanya jalan saat..."), no formal connectors (tersebut, merupakan, melakukan: none present).
- Units: dtk / mnt / j; "jam" and "menit" in full sentences; gp.billion "M", gp.million "jt".
- Roblox's own word "Main" for Play; "Gabung" for join; game words kept English (server, ping, kick, ban, alt, preset, makro).
- No plural forms; `key1` used for "Sekali"/"1 ..." wording.
- Quotes: curly “ ”; separator " · ".

#### Glossary

account=akun; sign in=masuk; sign in again=masuk ulang; auto-rejoin=masuk ulang otomatis; rejoin now=masuk lagi; anti-AFK=Anti-AFK; preset=preset; macro=makro; vault=brankas; master password=kata sandi utama; server=server; private server=server pribadi; launch/open=buka; play=Main; join=gabung; link=link (was mixed tautan/link); channel=channel (Discord's own menu path: Saluran); webhook=webhook; window=jendela; kick=di-kick; ban=di-ban; dropped=terputus; main account=akun utama; backup=cadangan; theme=tema; badge=lencana; community=komunitas; marketplace=Marketplace; inventory=inventaris; follow=ikuti; friends=teman; trusted friends=teman tepercaya; settings=Pengaturan; tray=tray; Multi-Roblox=Multi-Roblox.

#### Needs native review

- "Masuk ulang" is used both for sign in again (card.relogin, "perlu masuk ulang") and for rejoin a game ("Masuk ulang otomatis"). Could confuse; "Gabung ulang otomatis" would separate them, but it was left (established across ~100 strings).
- home.community "Sedang main di Bloxkeep" (English "Bloxkeep is playing"; key not traced in src, only the element id).
- rc.hint.marked "{n} dipilih — pilih akun lewat menu ⋯ masing-masing."
- rs.tp.Following "Yang saya ikuti" (self-voice kept as in rs.* rows).
- pf.hiAfternoon "Selamat siang" (could be "sore" depending on hour).
- cf.hue "Rona"; dock.stepGo "Main"; ui.playBox "Gabung ke game" (several of the new keys have no src usage found, so context was guessed from the English).
- news.liveTip "Boss muncul, stok, dan update".

#### Layout risks

- nav.monitor "Pantau game" (11 chars vs 12 English) fine; nav.servers "Server pribadi" (14 vs 15 English).
- rs.joinIs.* / rs.onlineIs.* are descriptive lines, longer than English by a few chars ("Orang yang kamu ikuti dan temanmu bisa menyusulmu").
- acc.selectedHidden and rw.* chips are slightly longer than English; should still fit.

### Français (`fr`)

#### Style

- Vous, friendly but not chummy; imperative for buttons ("Enregistrer", "Lancer"), infinitive-free short nouns for tabs.
- Sentence case everywhere; no English Title Case.
- Quotes « » with a no-break space inside; no-break space (U+00A0) before : ; ? ! % and before », and in numbers (1 000, 100 000). The file had none of these (all plain spaces): applied to every value (words and core). U+00A0 chosen over U+202F to avoid missing-glyph risk.
- Apostrophe is the typographic ’ (already the file's convention).
- fr plural: "one" covers 0 and 1; "key1" kept for exactly 1.
- Roblox's own words: Jouer, Abonnés, Abonnements, Communauté, Marketplace, Badges, Tenue.
- Loanwords gamers use are kept: preset, alts, farm, ping, FPS, RAM, bug, Fast Flags, Job ID, Place ID.

#### Glossary

account=compte; preset=preset; vault=coffre; master password=mot de passe principal; auto-rejoin=reconnexion auto; anti-AFK=anti-AFK; macro=macro (Étape/Zone/Fonction); server=serveur; private server=serveur privé; launch/open=lancer (Roblox) / ouvrir (fenêtre); sign in=se connecter; signed out=déconnecté; dropped=déconnecté/déconnexion; kicked=expulsé; schedule=planification (was also "planning": unified); app=appli (was also "app": unified); game box=zone du jeu (was also "case"/"champ"); window=fenêtre; tray=zone de notification; update=mise à jour; backup=sauvegarde; theme=thème; outfit=tenue; friends=amis; followers=abonnés; following=abonnements; community=communauté; webhook=webhook; channel (Discord)=salon; cookie=cookie; puzzle=puzzle; Multi-Roblox=Multi-Roblox; Save RAM=Économiser la RAM.

#### Needs native review

- gp.votes "{up} pour · {down} contre" (maybe use thumbs wording)
- home.community "En jeu avec Bloxkeep" (English "Bloxkeep is playing")
- wiki.read "lu {t}" (context unknown, key unused in src)
- rc.hint.marked "{n} choisis. Choisissez les comptes dans leur menu ⋯." (English odd)
- rs.tp.Following "Mes abonnements"
- dock.stepGo "Go"
- wiki.winsOf "lignes meilleures", pf.deco.r.mai "Fleurs d’abricotier" (not changed)
- mon.load.first "sature en premier" vs cap.tip/es.why "s’épuise en premier" (not unified)
- U+00A0 vs U+202F before ; ? ! (chosen U+00A0 for font safety)

#### Layout risks

- nav.inbox "Boîte de réception" (capsule tooltip only), dock.stepGo, srv.privateOnly, rw.h.* (Multi-Roblox …) chips, acc.selectedHidden ("masqués : {h}") in the account bar, mon.closeAsk "Fermer {n} fenêtres ?" (longer than "Close {n}?"), sys.moreTip.
- Settings nav labels were not lengthened.

### Deutsch (`de`)

#### Style

- Informal "du", imperative in du-form ("Wähle", "Füge ein"); no "Sie".
- Quotes „…“ (core Discord name sentence fixed from straight quotes); keys as Roblox/Windows say them (Strg, not Ctrl).
- Sentence case, nouns capitalised as German requires; no English Title Case.
- Buttons: infinitive/short verb ("Speichern", "Alle schließen"); tooltips: one short phrase; errors: what happened + what to do.
- Gamer loanwords kept where natives use them: Preset, Alts, Server, Ping, Item, Follower, Auto-Rejoin, Anti-AFK, Fast Flags.
- "Spielen" is Roblox's Play button; "Spielfeld" = the game box.

#### Glossary

account Konto (not Account) | preset Preset | vault Tresor | master password Master-Passwort | auto-rejoin Auto-Rejoin (was "Automatisch erneut beitreten", too long for chips) | rejoin erneut beitreten | anti-AFK Anti-AFK | macro Makro | server Server | private server Privater Server | Job ID Job-ID | sign in anmelden / signed out abgemeldet | dropped rausgeflogen / disconnected getrennt | kicked gekickt | banned gebannt (game) / gesperrt (Roblox) | window Fenster | tray Infobereich | game box Spielfeld | friends Freunde | followers Follower | trusted friends vertraute Freunde | theme Design | mod Mod | wiki Wiki | item Item (in-game) / Artikel (catalog) | multi-Roblox Multi-Roblox | launch/open öffnen / starten | play Spielen | board Tafel/Übersicht | alts Alts | cookie Cookie | webhook Webhook

#### Needs native review

- rs.tp.Following "Leute, denen ich folge" (width in a selector).
- gp.items "Item" vs "Artikel" consistency (Item = in-game, Artikel = catalog).
- gp.votes "Likes/Dislikes" (Roblox DE shows thumbs).
- srv.pick "Wählen" (button on server row).
- dock.stepGo "Los".
- core "Roblox-Version" setting name and "Schließen, aktualisieren, neu öffnen" (button name guessed, not in de words).
- set.clientDesc: English text in words.json looks shorter than the German (German is the longer older version); left as is.
- Many new keys (ui.*, auto.sure, sys.*, ru.*, rw.*) are unused in the GPUI code (old web UI), translated without visual context.

#### Layout risks

- nav.servers "Private Server", nav.monitor "Spielmonitor" (slightly longer than English).
- rs.tp.Following "Leute, denen ich folge", rs.onlineIs.FriendsFollowingAndFollowers (long, description line).
- mon.st.signedout "Abgemeldet", act.signedOut "Anmeldung nötig".
- gp.votes "{up} Likes · {down} Dislikes" is longer than "up/down".
- pv.optRejoinOn/Off now shorter than before.

### Русский (`ru`)

#### Style

- Address: ты (informal), imperative buttons/instructions ("Выбери", "Нажми"). Removed the lone "Вы" (news.following, pf.followed -> "Подписан").
- Quotes: «ёлочки»; UI paths written "Настройки › Запуск" (no quotes).
- Buttons: infinitive ("Добавить", "Сохранить"); Roblox's own "Играть" for Play; tabs/nav: short nouns.
- Sentence case only; no English Title Case. Errors: what happened + what to do.
- Plurals: base = many (5 аккаунтов), #few (2-4), #one (21), key1 exact 1. Core plural objects keep one/few/many.
- Tone: friendly gamer register ("кик", "вылет", "альты"), no bureaucratese.
- Numbers after colon ("Аккаунтов: N") where agreement is impossible.

#### Glossary

account аккаунт | preset пресет | vault хранилище | master password мастер-пароль | auto-rejoin автоперезаход (was fine) | anti-AFK анти-AFK | macro макрос | server сервер | private server частный сервер (was приватный; Roblox RU says "частные серверы") | arrange windows расставить окна (was mixed with "упорядочить") | launch/open открыть / запустить | sign in войти (вход) | sign out выход | drop (disconnect) вылет | kick кик / исключён | window окно | Fast Flags Fast Flags | Multi-Roblox Multi-Roblox | Save RAM Экономия RAM | main account основной аккаунт | alt альт | webhook вебхук | cookie cookie (m.) | puzzle/captcha головоломка / капча | community сообщество | friends/following/followers друзья / подписки / подписчики | wiki вики | Marketplace Маркетплейс | outfit образ | game pass пропуск | Play Играть | Place ID Place ID | Job ID Job ID | badge значок

#### Needs native review

- card.botCheck "Пройти" (Verify button; very short, maybe "Проверка").
- tray.playAs "Играть за" (maybe "Играть как").
- dock.stepGo "Вперёд", home.for "уже {t}", cm.sum, ui.cpu "ЦП" vs "CPU" (usage unclear, keys not found in src).
- macro.line.recovering / panel.unstuck "выходит из тупика".
- set.antiAfkEveryDesc ("в 2-секундную паузу при наборе текста") is stiff; left as is.
- Mix "кик" (core) vs "исключён" (kick.* words).
- gp.votes "{up} за · {down} против" (likes/dislikes).

#### Layout risks

- acc.summary (HTML row) is longer than English; sys.mood/chips fine.
- rw.h.* ("Multi-Roblox, скорее всего, заблокирован") longer than English.
- rs.tp.Following "Те, на кого я подписан" in a dropdown.
- card.mainSet/mainTip tooltips long (already were).
- Settings nav labels unchanged (none edited).

### 日本語 (`ja`)

#### Style

- Sentences: polite です/ます. Buttons, tabs and labels: short nouns or verb stems (保存, 削除, 開く, 再参加).
- No 「あなた」/「あなたの」. The user is implied, or the wording is 自分の… ("own"), or ご本人 in a security prompt.
- Quotes 「」, sentence end 。, and a half-width colon plus space for "label: value" (the existing style, kept).
- Counters: アカウント for accounts (not 件), 個 for windows, servers and presets. For chips and counts the number goes after the label (プレイ中 {n}, 切断 {n}).
- Katakana only for loanwords Japanese apps really use (サーバー, ウィンドウ, プリセット, マクロ, フレンド). Brand names stay as they are.
- Tone is friendly but not chummy, like Discord or Steam Japan. No Title Case.
- The file keeps no "1" plural keys, because Japanese has no plural. Sentences with exactly-one wording are kept only where the wording differs.

#### Glossary

| English | Chosen | Replaced |
|---|---|---|
| Play (Roblox JP) | プレイ | – |
| friend | フレンド | 友達 (Steam, Discord and Roblox JP all say フレンド) |
| friend request | フレンドリクエスト / フレンド申請 | 友達リクエスト |
| unfriend | フレンド解除 | 友達解除 |
| account | アカウント | – |
| alt (account) | サブ垢 | kept |
| preset | プリセット | – |
| vault | 保管庫 | – |
| master password | マスターパスワード | – |
| auto-rejoin | 自動再参加 | – |
| anti-AFK | 放置防止 | Anti-AFK (left in English) |
| macro | マクロ | – |
| server / private server | サーバー / プライベートサーバー | – |
| launch / open Roblox | 起動 / 開く | – |
| sign in | ログイン | – |
| signed out | ログアウト | – |
| drop (disconnect) | 切断 | 落ちる (kept only in the Discord-style phrase) |
| kick / ban | キック / BAN | 停止中 for ban (one place) |
| community | コミュニティ | – |
| trusted friends | 信頼できるフレンド | – |
| cookie | Cookie | クッキー |
| Team Create | チームクリエイト | Team Create |
| outfit | コーデ | kept |
| Multi-Roblox | Multi-Roblox | – |
| window | ウィンドウ | – |
| Fast Flags | Fast Flags | – |
| Quick Log In | クイックログイン | – |
| Marketplace | マーケットプレイス | – |
| No one | なし | 誰もいない |
| Statistics | 統計 | – |
| Activity | アクティビティ | – |
| Watch remotely | リモートで見る | – |
| secret/own | 自分の… | あなたの… |

#### Needs native review

- cm.alts, cm.altsN, cm.youLine, cm.board: サブ垢 is internet slang. A friendlier or more neutral option is サブアカ or サブアカウント.
- av.lookName, av.saveLook and the outfit strings (コーデ): Roblox JP may use コスチューム. コーデ is the gamer wording.
- gp.million "M", gp.billion "B": left as in English. Japanese reads 万 and 億, but the code divides by 10^6 so a swap needs a code change. Neither helps unless gp.thousand is also set.
- ps.none: this key is shared by preset_menu.rs ("No presets yet") and servers.rs:940 (no private servers). English shows the preset text on both screens, and so does ja now. This is a source bug: the servers screen needs its own key.
- friends/フレンド: if the owner prefers 友達 (casual), revert the glossary replacement. Roblox's own JP UI says フレンド.
- 放置防止: if you want the English term AFK in the label, 「AFK対策」 is the alternative.
- macro.tile.record 「1回プレイすればまねします」: casual. Alternative: 操作を記録して再現します.
- pf.hiMorning, pf.hiAfternoon and pf.hiEvening: a greeting followed directly by the name. A suffix such as さん may be needed in code.
- macro.why.imeOn: the English says "Vietnamese typing is on". The ja text says IME on (日本語入力など).
- Missing-key batch: mh.windowedDesc, dock.serversTip, news.live and news.liveTip are free paraphrases.

#### Layout risks

- set.p.security (nav): 保管庫とセキュリティ is 9 characters, still shorter than the English.
- acc.selectedHidden: it has a button inside, so the long form 選択中 · 非表示{h} · 解除 may wrap at narrow widths.
- cap.tip, mon.remote.hint, card.mainTip and onl.desc are very long tooltips and descriptions (unchanged).
- bw.newTip and ps.opt.restartNote are long for a tooltip.
- nav.servers (プライベートサーバー, 10 characters) is longer than most nav labels; English is 15, so it fits.

### 한국어 (`ko`)

#### Style

- Sentences in 해요체 (-어요/-아요); instructions as -세요; no 당신/귀하, no 합니다체 (one 감사합니다 changed to 고마워요).
- Buttons and tabs are short nouns or noun+verb stems (저장, 삭제, 열기, 모두 닫기).
- Particles: Bloxkeep takes 이/을/은/으로/과 (Bloxkeep을, Bloxkeep이); Roblox/Discord/Windows take 가/를/로. Placeholders use 이(가)/을(를)/(으)로 where the ending is unknown.
- No Title Case, no trailing period on labels; full sentences end in a period.
- Numbers with counters: 계정 {n}개, PC {n}대, 창 {n}개, {n}회, {n}분.
- Roblox's Korean terms kept: 플레이, 팔로우/팔로잉/팔로워, 사용자 이름, 신뢰하는 친구, 커뮤니티.

#### Glossary

account 계정 | preset 프리셋 | vault 보관함 | master password 마스터 비밀번호 | auto-rejoin 자동 재참가 | join 참가 | play 플레이 | launch 실행 | open 열기 | server 서버 | private server 비공개 서버 | Job ID Job ID | window 창 | macro 매크로 | anti-AFK Anti-AFK (idle = 잠수) | kicked/dropped 튕김 | disconnected 연결 끊김 | signed out 로그아웃됨 | sign in 로그인 | alts 부계정 | preview 미리보기 | webhook 웹후크 | notification 알림 | game 게임 | wiki 위키 | news 소식 | labels 라벨 | main account 메인 계정 | statistics 통계 | settings 설정 | theme 테마 | restore 복원 | backup 백업 | tab 탭

#### Needs native review

- rs.onlineSub "아무도 안 봄: 모두에게 오프라인으로 보여요" (stiff?)
- home.community "지금 Bloxkeep 플레이 중" (heading wording)
- mf.later, wn.star ("스타를 눌러 주면") tone
- ps.none key exists in both words.json and i18n.json with different English; ko follows words.json (private servers). Source issue.
- Many added keys (es.*, ui.*, win.*, wiki.*, bw.*) may be unused by the GPUI code (old web UI), so context was by meaning only.

#### Layout risks

- nav.servers "비공개 서버", rs.onlineIs.FriendsFollowingAndFollowers (long line), dock.serversTip, mon.remote.hint (long, paragraph only).
- Nav/tab/button labels are all at or below English length.

### Türkçe (`tr`)

#### Style

- Casual "sen" everywhere (imperatives: seç, aç, bas), no "siz" found.
- Sentence case; no English Title Case.
- Apostrophe after brand names/IDs with correct harmony (Roblox'u, Bloxkeep'e, Discord'a, PID'i). Straight apostrophes are used throughout the file; I kept that (one exception: sys.ramPct uses ’ because of the %{pct} pattern, harmless).
- No plural after numbers ({n} hesap, {n} pencere). Plural only in nav/headings (Hesaplar, Presetler).
- Button = bare imperative (Kapat, Kaydet, Yeniden dene). Status = present tense (Oynuyor, Katılıyor).
- Errors: what happened + what to do, short ("... yüklenemedi.").
- Number-then-suffix risks avoided by rewording (home.ofRam "%{pct} kadarı", con.again "{t} saatine kadar").
- The existing file was already natural and consistent; I only filled gaps and made one wording fix.

#### Glossary

account hesap · alt yan hesap · preset preset · vault kasa · master password ana parola · password parola · auto-rejoin otomatik yeniden katılma · rejoin yeniden katıl · anti-AFK Anti-AFK · macro makro · server sunucu · private server özel sunucu · Play Oyna (Roblox's TR word) · launch/open aç (başlat for launcher) · sign in giriş yap · signed out çıkış yapıldı / giriş gerekli · online çevrimiçi · offline çevrimdışı · dropped/disconnected düştü / bağlantı koptu · banned yasaklı · block engelle · friend arkadaş · follower takipçi · cookie çerez · window pencere · tab sekme · theme tema · notification bildirim · wiki Wiki · item öğe (Roblox items: eşya) · Multi-Roblox, Fast Flags, FPS, RAM, CPU kept.

#### Needs native review

- con.again "{t} saatine kadar" (t may not always be a clock time).
- mods.updatedAgo "{ago} güncellendi" (assumes {ago} already contains "önce").
- home.putGame "{name} oyununu oyun kutusuna koy" (assumes {name} is a game).
- es.matters "Önceliğin ne?"; dock.stepGo "Başla"; err.draw "Bir şey çizilemedi."; ui.playBox "Bir oyuna katıl".
- rs.tv.* "Düşük/Orta/Yüksek" (voice/video setting level, unseen context); gp.items "Öğe".
- wiki.read "{t} okuma" (reading time).
- Many of these keys are not referenced in src by literal; context was inferred from English.

#### Layout risks

- dock.serversTip, srv.privateNone, mh.windowedDesc are longer than English but in descriptions/tooltips.
- rw.h.gone "Multi-Roblox büyük olasılıkla engellendi" slightly longer than English; short fallback "Muhtemelen engellendi" if truncated.
- nav.servers "Özel sunucular" equal length to English.

### ไทย (`th`)

#### Style

- Address: คุณ (second person), dropped whenever the sentence works without it. No ครับ/ค่ะ/คะ, no ท่าน, no กรุณา/โปรด.
- No spaces between words. A space only separates phrases or sentences (Thai has no sentence period, so a space stands in for it). No trailing full stop.
- Questions end with ไหม?. Confirm dialogs read "ลบ … ไหม?".
- Tone: casual-neutral like Discord/Steam TH. "ถูก…" passive only where Thai needs it (ถูกแบน, ถูกเตะ).
- Avoid calqued possessives: "ของมัน" (its) replaced by the noun or dropped.
- Keep loanwords as gamers say them: บัญชี, เซิร์ฟเวอร์, พรีเซ็ต, มาโคร, ออนไลน์/ออฟไลน์, แท็บ, ธีม, คุกกี้, ปิง, ไอเท็ม.
- Nav/tab/chip labels stay 1–2 words; Settings nav "ความปลอดภัย" (set.p.security) kept short as asked.
- Brand names and technical tokens untouched (Bloxkeep, Roblox, Discord, Multi-Roblox, Anti-AFK, Fast Flags, PID, Job ID).

#### Glossary

account → บัญชี; alt → บัญชีรอง; preset → พรีเซ็ต; macro → มาโคร; vault → ตู้นิรภัย; master password → รหัสผ่านหลัก; sign in → เข้าสู่ระบบ; sign out → ออกจากระบบ; server → เซิร์ฟเวอร์; private server → เซิร์ฟเวอร์ส่วนตัว; auto-rejoin → เข้าใหม่อัตโนมัติ; rejoin → เข้าใหม่; anti-AFK → Anti-AFK (kept Latin); window → หน้าต่าง; launch/open → เปิด; play → เล่น; join → เข้าร่วม / เข้าเกม; drop/disconnected → หลุด / ขาดการเชื่อมต่อ; kicked → ถูกเตะ; banned → ถูกแบน; block → บล็อก (was บล็อค); cart → ตะกร้า (was รถเข็น); community → ชุมชน; wiki → วิกิ; cookie → คุกกี้; settings → การตั้งค่า; privacy → ความเป็นส่วนตัว; backup → สำรองข้อมูล; update → อัปเดต; theme → ธีม; badge → ตรา; inventory → คลังไอเท็ม; friends → เพื่อน; trusted friends → เพื่อนสนิท; followers → ผู้ติดตาม; webhook → webhook; console → คอนโซล; status bar pill → หน้าต่าง/แท็บ as per key; PC → พีซี; tray → ถาด.

#### Needs native review

- ui.playBox / dock.stepGo "เข้าเกม" vs "เล่น": button/step wording for "Go".
- home.for "มาแล้ว {t}" (elapsed time "for {t}"): context was a Home pill; may need "มา {t}".
- home.needN "{n} ต้องดู" (accounts needing a look): maybe "{n} ต้องตรวจ" to match acc.sum.look.
- mh.windowedDesc "เปิดหลายหน้าต่างแล้วจะตีกัน": colloquial, check tone.
- rs.onlineSub "ไม่มีใคร: ทุกคนเห็นคุณออฟไลน์" and the rs.onlineIs/joinIs set.
- set.p.security "ความปลอดภัย" is generic (the key's English is "Vault & security"); shortened as requested.
- wiki.mentioned "กล่าวถึงใน" ("Named in"): a bit literal.
- gp.evGoing "ไป" ("Going"): short but terse; unchanged.
- Strength labels "แข็งแรง/อ่อน" (password): common but could be แข็งแกร่ง.

#### Layout risks

- Thai strings are usually shorter or equal to English. Watch: set.p.* nav (kept), rw.* / multi.watchWarn (the first is long in Thai), mon.closeAsk1 "ปิด 1 หน้าต่างไหม?", lock.pw6, home.w.most "เล่นมากสุดบน Bloxkeep" (widget title), wn.older "และใน {n} เวอร์ชันก่อนหน้า".
- Thai text has no spaces, so long unbroken strings (webhook/URL hints) may not wrap; line breaking relies on dictionary breaking in the app.

### ລາວ (`lo`)

#### Style

- Informal-neutral "ເຈົ້າ" for "you"; no polite particles (ກະລຸນາ, ເດີ້, ຄັບ) in labels or messages.
- Lao script only, no spaces between words; a space separates clauses; sentences end with "." (no Thai-style spacing).
- Buttons: bare verb (ບັນທຶກ, ລຶບ, ເປີດ, ປິດ). Questions end in "ບໍ" (ລຶບບັນຊີບໍ?). Progress "ກຳລັງ…".
- Nav, tabs and chips: one short noun. Settings nav "ຄວາມປອດໄພ" (kept short).
- Keep English tech terms people really say: RAM, CPU, FPS, Fast Flags, webhook, Place ID, Job ID, PID, QR.
- Quotes “ ”; modern spelling ຫຼ and ໝ (not ຫລ, ຫມ); digits stay Western.

#### Glossary

| English | Lao | Notes |
|---|---|---|
| account | ບັນຊີ | |
| alt | ບັນຊີສຳຮອງ | |
| preset | ພຣີເຊັດ | transliteration, used everywhere |
| vault | ຕູ້ນິລະໄພ | |
| master password | ລະຫັດຜ່ານຫຼັກ | |
| auto-rejoin | ເຂົ້າຄືນອັດຕະໂນມັດ | |
| rejoin | ເຂົ້າຄືນ | |
| anti-AFK | ກັນ AFK | |
| macro | ມາໂຄຣ | |
| server / private server | ເຊີບເວີ / ເຊີບເວີສ່ວນຕົວ | |
| sign in / sign out | ເຂົ້າສູ່ລະບົບ / ອອກຈາກລະບົບ | |
| play | ຫຼິ້ນ | button word |
| open / launch | ເປີດ | |
| window | ໜ້າຕ່າງ | |
| cookie | ຄຸກກີ້ | |
| browser | ບຣາວເຊີ | |
| schedule | ຕາຕະລາງເວລາ | replaced ກຳນົດເວລາ |
| drop (disconnect) | ຫຼຸດ | |
| kick | ເຕະ | |
| ban | ແບນ | |
| block | ບລັອກ | |
| theme | ທີມ | |
| settings | ການຕັ້ງຄ່າ | |
| privacy | ຄວາມເປັນສ່ວນຕົວ | |
| friends / community | ໝູ່ / ຊຸມຊົນ | |
| statistics | ສະຖິຕິ | |
| notification / alert | ການແຈ້ງເຕືອນ | |
| backup / restore | ສຳຮອງ / ກູ້ຄືນ | |
| copy | ສຳເນົາ | |
| item | ໄອເທັມ | |
| Robux | Robux | unchanged |

#### Needs native review

- The 237 added keys, especially cf.hue "ເສດສີ", news.liveTip, rs.onlineIs.*, rs.joinIs.*, mh.windowedDesc, bw.saveTip.
- theme.fireflies "ແມງໄຟ": is this the firefly word? (alternative ແມງຫິງຫ້ອຍ)
- theme.flametree "ຕົ້ນຟ້າງ": unsure this names the flame tree (phuong vi).
- pf.deco.r.mai "ດອກໄມ້ເຫຼືອງ" for Tet apricot blossom: generic.
- theme.dim "ມົວ": reads as "blurry", not "dim".
- "ຂັດຂ້ອງ" (9 core strings) for "Roblox crashed or hangs": real word, but would natives say ຄຣາດ / ແຄຣດ?
- country.PT ປອກຕຸຍການ and country.MA ໂມຣັອກໂຄ (stray ັ kept from CLDR).
- rx.up-and-coming "ດາວຮຸ່ງ"; theme.midautumn "ໄຫວ້ພະຈັນ"; theme.lunar/pf.deco.t.tet "ກຸດຈີນ" for Lunar New Year.
- av.lookName "ລຸກ" (outfit look, borrowed) and "ພຣີເຊັດ" as the app-wide word for preset.
- ການແຈ້ງເຕືອນ vs ການເຕືອນ both used for alert (set.dc.tab.alerts, set.nav.alerts): pick one.
- pf.hi* greetings "ສະບາຍດີຕອນເຊົ້າ/ບ່າຍ/ແລງ" (trailing space kept).

#### Layout risks

- Lao text runs 1.5–2× English by code points (marks add no width), so visible width is about 1.2–1.4×.
- Check at 960 px: gpv.chip "ຄວາມເປັນສ່ວນຕົວໃນເກມເປີດຢູ່", cm.board "ບັນຊີສຳຮອງຫຼາຍສຸດຕອນນີ້", alog.g.signin "ການເຂົ້າສູ່ລະບົບ", alog.g.schedule "ຕາຕະລາງເວລາ", in.ts.Countered "ຍື່ນຂໍ້ສະເໜີກັບແລ້ວ", set.p.backup "ສຳຮອງ ແລະ ຂໍ້ມູນ", card.relogin and pw.go "ເຂົ້າສູ່ລະບົບ".
- Settings nav (set.p.*) is short: ຄວາມປອດໄພ, ເວີຊັນ Roblox, Bloxkeep.

### Polski (`pl`)

#### Style

- Informal "ty" addressed with capitalised Ty/Twój/Cię (the file's existing convention, same as Steam/Discord Polish); impersonal forms for buttons and toasts ("Zapisano", "Dodaj konto").
- Quotes „…” (low-high), « › » path separator "Ustawienia › Uruchamianie".
- Buttons are imperatives (Graj, Zapisz); tabs and nav are short nouns; sentence case, no English Title Case.
- Plurals one/few/many (1 konto / 2–4 konta / 5 kont). Base key = many, #few, #one, key1 = exactly 1.
- Roblox accounts are neuter ("konto"), so statuses are neuter (Zbanowane, Wyrzucone, Rozłączone). Avoided gendered verbs about people (wysłał(a) and the like), using impersonal wording.
- Discord is declined like a Polish noun in running text ("na Discordzie", "z Discorda", "webhook Discorda"); other brands stay unchanged.
- Settings nav labels kept short ("Kopie i dane", "Bezpieczeństwo").

#### Glossary

account konto; add Dodaj; play button Graj; auto-rejoin Automatyczny powrót; anti-AFK Anty-AFK; vault sejf; master password hasło główne; preset presety/presetów; macro makro; server serwer; private server serwer prywatny; Place ID/Job ID unchanged; sign in zaloguj się / logowanie; sign out wylogować; drop / dropped rozłączenie / rozłączone; kick / kicked wyrzucenie / wyrzucone; ban ban / zbanowane; inventory Ekwipunek (was Wyposażenie); outfit strój; badge odznaka; community społeczność; friends znajomi; follow obserwuj; trade wymiana; marketplace Marketplace; theme motyw; schedule harmonogram; window okno; tray zasobnik; backup kopia zapasowa; update aktualizacja; launcher launcher; puzzle/bot check łamigłówka/weryfikacja; cookie cookie/cookies; limit "Pierwszy limit: {by}" (was awkward "Pierwszy skończy się").

#### Needs native review

- av.inventory / inv.title "Ekwipunek": Roblox's own Polish might say something else.
- gp.dislike "Nie lubię" / gp.like "Lubię" (kept): Roblox's Polish label unverified.
- shop.* "Marketplace", nav.shop: Roblox pl may use "Sklep".
- bd.cakewalk "Bułka z masłem" (badge difficulty names kept).
- pw.captcha "łamigłówka" vs Roblox's "zagadka".
- home.needN "Czeka na Ciebie: {n}", sys.windows/sys.window1 ("okien"/"okno" as unit), wiki.read "czytanie: {t}", gp.votes "Lubi: … · Nie lubi: …": keys with no Rust usage found, meaning guessed from English.
- dc.dc* Discord declension choices (Discord vs Discorda in labels).
- kick.* neuter statuses read oddly if shown next to a player name.
- rs.logOut* "miejsc" for sign-in places (could be "sesji").

#### Layout risks

- Settings nav: "Kopie i dane" (12) ok; "Bezpieczeństwo" (14) is the longest.
- Buttons: m.* long ones "Kopiuj kod 2FA", "Dodaj nawzajem do znajomych" (27, was 24), "t.openRefused" "Otwórz roblox.com na tych kontach" (32).
- status chips "Wymaga logowania", "Niezweryfikowane"; tray/pill "Niegotowe".
- dc.remoteAsk and ps.schNote are long paragraphs (fine in dialogs).
- country names such as "Francuskie Terytoria Południowe i Antarktyczne" are long (CLDR standard).

### Filipino (`fil`)

#### Style

- Casual Taglish, as Filipino players write: "mo" / "ka" for you, never "po" or "ninyo". Common English gaming and tech words stay English (account, password, server, window, cookie, hotkey, macro, update, link, browser).
- Buttons are bare verbs: Maglaro, Sumali, I-save, Burahin, Buksan, Isara, Kanselahin. English verbs take the hyphen prefix (i-click, mag-sign in, na-kick, mag-rejoin).
- Sentences put the verb first ("Hindi ma-save ang file"); progress is the repeated-syllable form ("Binubuksan…", "Sinusuri…").
- No plural forms after a count ("1 account", "5 account"); the plural marker "mga" only without a number. `key1` used for "1 …" wording.
- Units: seg / min / oras / araw / buwan / taon; time 2:30 PM, dates month first (11/25/2026), as Windows writes fil-PH.

#### Glossary

account=account; sign in / sign out=mag-sign in / mag-sign out; rejoin=mag-rejoin; auto-rejoin=auto-rejoin; dropped=natanggal; kicked=na-kick; banned=na-ban; play=Maglaro; join=Sumali; settings=Mga Setting; friends=Mga kaibigan; games=Mga laro; preset=preset; vault=vault; master password=master password; private server=private server; step=hakbang; area=area; If=Kung; repeat=repeat; stuck=na-stuck; theme=tema; schedule=iskedyul; backup=backup.

#### Needs native review

- How much English to keep: labels follow Taglish (Mga Setting, private server, master password); a stricter Tagalog reader may want more native words.
- pf.hi* greetings "Magandang umaga/hapon/gabi" (trailing space kept).
- Macro step names mix Tagalog and English as players type them (Kung…, Kung hindi, Tapusin ang if, Tapusin ang repeat, Kung na-stuck…); a native reader may prefer all-Tagalog names.

#### Layout risks

- Filipino runs about 1.2–1.4× English; verb-first sentences with "ang"/"ng" add short words.
- Check at 960 px: Settings nav, status chips ("Kailangang mag-sign in ulit"), Home buttons and the Play row.

### Bahasa Melayu (`ms`)

#### Style

- Malaysian Malay (ms-MY), not Indonesian: "anda" for you; Malaysian words where the two differ (tetingkap, kata laluan, pelayan, muat turun, kemas kini, akaun, Tetapan, kuki).
- Buttons are bare verbs: Main, Sertai, Simpan, Padam, Buka, Tutup, Batal. Statuses use di- passives (dikunci, diharamkan, ditendang).
- No plural forms after a count ("1 akaun", "5 akaun"); `key1` used for "1 …" wording.
- Units: s / min / j (jam) / hari; time 2:30 PTG (PG/PTG), dates day first (25/11/2026), as Windows writes ms-MY.

#### Glossary

account=akaun; password=kata laluan; master password=kata laluan induk; sign in / sign out=log masuk / log keluar; rejoin=sertai semula; auto-rejoin=sertai semula automatik; dropped=terputus; kicked=ditendang; banned=diharamkan; window=tetingkap; server=pelayan; private server=pelayan peribadi; settings=Tetapan; play=Main; join=Sertai; friends=Rakan; games=Permainan; update=kemas kini; download=muat turun; cookie=kuki; on/off=Hidup/Mati; macro=makro; tray=dulang sistem; vault=vault; preset=preset; step=langkah; area=kawasan; If=Jika.

#### Needs native review

- "diharamkan" for banned (Roblox Malaysia players often just say "kena ban").
- Lunar New Year theme "Tahun Baru Cina", as Malaysia says it.
- Macro step names (Jika, Jika tidak, Tamat jika, Tamat ulang) read as plain words; check them in the step list.

#### Layout risks

- Malay runs about 1.2–1.3× English ("Tetapan › Pelancaran › Buka Roblox dengan").
- Check at 960 px: Settings nav, status chips ("Perlu log masuk semula"), Home buttons and the Play row.
