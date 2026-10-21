# Privacy policy

Bloxkeep runs on your PC and has no accounts, servers or analytics of its own. It collects no telemetry and sends
nothing about you or your Roblox accounts to its developer, except a feedback message you choose to send.

## What stays on your PC

Your accounts (cookies, passwords if you saved them, authenticator keys) are kept in `vault.rmv`, encrypted with your
master password (AES-256-GCM, key from PBKDF2-HMAC-SHA256 with 600,000 iterations). The master password is never
stored. The other files in Bloxkeep's data folder, and how they're protected, are listed in
[Data and security](README.md#data-and-security).

## What leaves your PC, and when

| Where | When, and what |
|---|---|
| Roblox's API hosts (`*.roblox.com`, and `*.robloxapp.vnggames.com` for Roblox VNG accounts) | Only for accounts you added: signing in (the password only goes to `auth.roblox.com`), session and ban checks, status every 30 seconds, game, server, friend and player lookups, join tickets, the newest Roblox version |
| `www.roblox.com` | The browser sign-in window, profiles, and the join link handed to Roblox |
| `*.rbxcdn.com` | Avatars and game pictures |
| Roblox's captcha providers (Arkose Labs, HUMAN Security) | Only in the puzzle window, when Roblox asks for a captcha during password sign-in |
| GitHub (`github.com`, `raw.githubusercontent.com`) | Checking for and downloading Bloxkeep updates (can be turned off in **Settings › Bloxkeep updates**), and reading the Roblox watch's findings about Roblox builds (`multi-status.json`). No account data is sent |
| Microsoft (`go.microsoft.com`) | Only when WebView2 has to be installed |
| Bloxkeep's feedback service (a Cloudflare Worker that forwards to the developer's Discord) | Only when you press **Send** in **Send feedback**: your message, a contact if you give one, Bloxkeep's and Windows' versions, the app's language and, if ticked, the end of `error.log` with account names, ids, passwords, cookies and similar details removed. **See exactly what will be sent** shows all of it first ([details](docs/feedback.md)) |
| Your Discord webhook (`discord.com`) | Only when you turn on **Settings › Discord**: account names (or **Account 1**, **Account 2**… when hidden), game names, play time, sessions, PID, RAM, and links to Roblox's own pictures of the games and of the accounts' avatars (no avatar when names are hidden). Never cookies or passwords |
| The backup folder you choose | A copy of the encrypted vault; a OneDrive or Google Drive app may sync it |

**Bloxkeep's Discord bot** (a Cloudflare Worker) runs apart from the app and knows nothing about your accounts. In the
servers that add it, Discord tells it who posted a message in which channel, but not what the message says (the bot
doesn't ask for message content). It uses that for one thing only: in a server that set up a trap channel with
`/setup`, whoever posts there is banned. It keeps none of it, only what it needs to stay connected (its own session,
and when its connection started and ended), each server's choices made with `/notify`, `/board`, `/setup` and
`/verify`, and a note of each ban for that server's moderators. In a server that uses `/verify`, pressing **Verify**
shows you a captcha; the bot remembers its code (for five minutes) and when you asked (for ten, to limit tries) with
your Discord id, in memory only, and gives you the server's role once you type the code right.

Each of these services has its own privacy policy: [Roblox](https://www.roblox.com/info/privacy),
[GitHub](https://docs.github.com/site-policy/privacy-policies/github-general-privacy-statement),
[Microsoft](https://privacy.microsoft.com/privacystatement), [Cloudflare](https://www.cloudflare.com/privacypolicy/),
[Discord](https://discord.com/privacy).

## Removing everything

Uninstalling Bloxkeep removes the program. Your accounts and settings are deleted too when you tick **Also delete my
accounts and settings** in the uninstaller; backups in your backup folder are never touched. See
[Uninstall](README.md#uninstall).

## Questions

Open an issue at https://github.com/minhhoang0122/bloxkeep/issues.
