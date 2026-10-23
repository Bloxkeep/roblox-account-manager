# Privacy policy

Bloxkeep runs on your PC and has no accounts or analytics of its own. It sends nothing about you or your Roblox
accounts to its developer. What reaches the developer is a feedback message you choose to send and, while Bloxkeep is
open, an anonymous "open now" note so they can see how many people use it right now. You can turn that note off in
**Settings › Bloxkeep updates › Privacy › Add me to the online count**.

## What stays on your PC

Your accounts (cookies, passwords if you saved them, authenticator keys) are kept in `vault.rmv`, encrypted with your
master password (AES-256-GCM, key from PBKDF2-HMAC-SHA256 with 600,000 iterations). The master password is never
stored. The other files in Bloxkeep's data folder, and how they're protected, are listed in
[Data and security](README.md#data-and-security).

## What leaves your PC, and when

| Where | When, and what |
|---|---|
| Roblox's API hosts (`*.roblox.com`, and `*.robloxapp.vnggames.com` for Roblox VNG accounts) | Only for accounts you added: signing in (the password only goes to `auth.roblox.com`), session and ban checks, saving the new sign-in Roblox sends when it renews one, status every 30 seconds, game, server, friend and player lookups, join tickets, the newest Roblox version |
| `www.roblox.com` | The browser sign-in window, profiles, and the join link handed to Roblox |
| `*.rbxcdn.com` | Avatars and game pictures |
| Roblox's captcha providers (Arkose Labs, HUMAN Security) | Only in the puzzle window, when Roblox asks for a captcha during password sign-in |
| GitHub (`github.com`, `raw.githubusercontent.com`) | Checking for and downloading Bloxkeep updates (can be turned off in **Settings › Bloxkeep updates**), and reading the Roblox watch's findings about Roblox builds (`multi-status.json`). No account data is sent |
| Microsoft (`go.microsoft.com`) | Only when WebView2 has to be installed |
| Bloxkeep's feedback service (a Cloudflare Worker that forwards to the developer's Discord) | Only when you press **Send** in **Send feedback**: your message, a contact if you give one, Bloxkeep's and Windows' versions, the app's language and, if ticked, the end of `error.log` with account names, ids, passwords, cookies and similar details removed. **See exactly what will be sent** shows all of it first ([details](docs/feedback.md)) |
| Bloxkeep's Discord bot (`bloxkeep-bot.bloxkeep-app.workers.dev`, a Cloudflare Worker) | While Bloxkeep is open, unless **Settings › Bloxkeep updates › Privacy › Add me to the online count** is off: every minute (further apart when many people have Bloxkeep open), and once as it closes, an id made from your PC (Windows' id for the PC, scrambled one way so it can't be traced back; linked to no account, vault, Roblox or Discord, and the same after you reinstall Bloxkeep, so a reinstall isn't counted twice), Bloxkeep's version and the app's language. Nothing else. The bot keeps an id only while that Bloxkeep counts as open (until a while after its last note, or until it says it's closing), then deletes it. It never keeps your IP address and keeps no history of who was open when: only, for each day, the highest number open at once and how many different PCs had Bloxkeep open. To count those without keeping ids, each id is scrambled one way with a random key made for that day, and the day's list of scrambled ids and its key are deleted when the day is over (Vietnam time). Turning the switch off sends one last note that you've gone, and nothing after |
| Your Discord webhook (`discord.com`) | Only when you turn on **Settings › Discord**: account names (or **Account 1**, **Account 2**… when hidden), game names, play time, sessions, PID, RAM, and links to Roblox's own pictures of the games and of the accounts' avatars (no avatar when names are hidden). Never cookies or passwords |
| The backup folder you choose | A copy of the encrypted vault; a OneDrive or Google Drive app may sync it |

**Bloxkeep's Discord bot** (a Cloudflare Worker) runs apart from the app and knows nothing about your accounts. Apart
from the "open now" notes above, the app talks to it in one way only: its Discord button opens the bot's address in your
browser, which sends you on to the community server's current invite, and sends nothing itself. How many people have
Bloxkeep open shows in the bot's status in Discord; its developers also see it by version and language. In the
servers that add it, Discord tells it who posted a message in which channel, but not what the message says (the bot
doesn't ask for message content). It uses that for one thing only: in a server that set up a trap channel with
`/setup`, whoever posts there is banned. It keeps none of it, only what it needs to stay connected (its own session,
and when its connection started and ended), each server's choices made with `/notify`, `/board`, `/setup` and
`/verify`, and a note of each ban for that server's moderators. In a server that uses `/verify`, pressing **Verify**
shows you a captcha; the bot remembers its code (for five minutes) and when you asked (for ten, to limit tries) with
your Discord id, in memory only, and gives you the server's role once you type the code right. The chart in `/online` (for Bloxkeep's developers) is drawn by [QuickChart](https://quickchart.io) from the last seven days' highest numbers of Bloxkeep apps open at once, and nothing else.

Each of these services has its own privacy policy: [Roblox](https://www.roblox.com/info/privacy),
[GitHub](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement),
[Microsoft](https://privacy.microsoft.com/privacystatement), [Cloudflare](https://www.cloudflare.com/privacypolicy/),
[Discord](https://discord.com/privacy).

## Removing everything

Uninstalling Bloxkeep removes the program. Your accounts and settings are deleted too when you tick **Also delete my
accounts and settings** in the uninstaller; backups in your backup folder are never touched. See
[Uninstall](README.md#uninstall).

## Questions

Open an issue at https://github.com/Bloxkeep/roblox-account-manager/issues.
