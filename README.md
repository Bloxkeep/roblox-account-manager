<p align="center">
  <img src="RobloxManager/wwwroot/logo.svg" width="88" alt="Bloxkeep logo">
</p>
<h1 align="center">Bloxkeep</h1>
<p align="center">Manage and launch several Roblox accounts on Windows. Accounts are encrypted and stay on your PC.</p>
<p align="center">English | <a href="README.vi.md">Tiếng Việt</a></p>
<p align="center">
  <a href="https://github.com/Bloxkeep/roblox-account-manager/releases/latest"><img src="https://img.shields.io/github/v/release/Bloxkeep/roblox-account-manager?label=release" alt="Newest release"></a>
  <a href="https://github.com/Bloxkeep/roblox-account-manager/releases"><img src="https://img.shields.io/endpoint?url=https%3A%2F%2Fbloxkeep-bot.bloxkeep-app.workers.dev%2Fbadge%2Fdownloads" alt="Downloads of the install files"></a>
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20(64--bit)-0078D4" alt="Windows 10 or 11, 64-bit">
  <img src="https://img.shields.io/badge/.NET-10-512BD4" alt="Built with .NET 10">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
  <a href="https://bloxkeep-bot.bloxkeep-app.workers.dev/discord"><img src="https://img.shields.io/badge/Discord-join_the_server-5865F2" alt="Join Bloxkeep's Discord server"></a>
</p>
<p align="center"><a href="https://github.com/Bloxkeep/roblox-account-manager/releases/latest/download/Bloxkeep-Setup.exe"><img src="https://img.shields.io/badge/Download_for_Windows-Bloxkeep--Setup.exe-2ea44f?style=for-the-badge" height="36" alt="Download Bloxkeep for Windows (Bloxkeep-Setup.exe, the newest version)"></a> <a href="https://bloxkeep-bot.bloxkeep-app.workers.dev/discord"><img src="https://img.shields.io/badge/Discord-Join_the_server-5865F2?style=for-the-badge&logo=discord&logoColor=white" height="36" alt="Join Bloxkeep's Discord server"></a></p>
<p align="center"><a href="#download-and-install">Download</a> · <a href="#getting-started">Getting started</a> · <a href="https://bloxkeep.github.io/roblox-account-manager/guide/">User guide</a> · <a href="https://bloxkeep.github.io/roblox-account-manager/">Website</a> · <a href="https://bloxkeep.github.io/roblox-account-manager/whats-new/">What's new</a> · <a href="#community">Community</a> · <a href="#data-and-security">Security</a> · <a href="#faq">FAQ</a> · <a href="PRIVACY.md">Privacy</a></p>

<p align="center"><img src="docs/screenshots/multi-roblox-real.jpg" width="860" alt="A real screenshot of one Windows PC: six Roblox windows open at once, tiled side by side by Bloxkeep, all in the same game, titled Roblox - Account 1 to Roblox - Account 7"></p>

Keep your Roblox accounts in one place, pick a game, pick accounts, press **Play**. Everything stays on your PC, encrypted with your master password. Bloxkeep has no server of its own.

This repository holds Bloxkeep's source code, its releases and the issue tracker. The user guide lives on the [website](https://bloxkeep.github.io/roblox-account-manager/).

*Bloxkeep is an independent project, not affiliated with or endorsed by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*

## Features

<img src="docs/screenshots/hero.png" width="860" alt="Bloxkeep main window: account cards with two accounts chosen, the game bar at the bottom, and Multi-Roblox turned on">

<table>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/accounts/"><img src="docs/screenshots/add-account.png" width="420" alt="Add accounts"></a><br><b>Add accounts</b><br>With your phone (code or QR), username and password (2-step verification and captchas work), a browser window, a cookie or another Bloxkeep file. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/accounts/">Accounts</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/accounts/"><img src="docs/screenshots/accounts-list.png" width="420" alt="See every account"></a><br><b>See every account</b><br>Online, in game here or on another device, why it disconnected, RAM, play time, needs sign-in, banned. Search, groups, cards or list. Mark your main account, and make your accounts Roblox friends. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/accounts/">Accounts</a></td>
</tr>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/playing/"><img src="docs/screenshots/demo-servers.webp" width="420" alt="Play"></a><br><b>Play</b><br>Pick a game by name, link or Place ID, and a server: most or fewest players, lowest ping, room for N accounts, your private servers, a friend's or any player's server. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/playing/">Playing</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/multi-roblox/"><img src="docs/screenshots/multi-roblox-real-detail.jpg" width="420" alt="Multi-Roblox"></a><br><b>Multi-Roblox</b><br>Several Roblox windows at once. Off until you turn it on; every new Roblox build is checked. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/multi-roblox/">Multi-Roblox</a></td>
</tr>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/presets/"><img src="docs/screenshots/demo-presets.webp" width="420" alt="Presets"></a><br><b>Presets</b><br>Accounts, game and server saved for one click, with a schedule to start and stop. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/presets/">Presets</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/staying-in-game/"><img src="docs/screenshots/demo-rejoin.webp" width="420" alt="Stay in game"></a><br><b>Stay in game</b><br>Anti-AFK against idle kicks, auto-rejoin after a disconnect. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/staying-in-game/">Staying in game</a></td>
</tr>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/macros/"><img src="docs/screenshots/macros.png" width="420" alt="Macros (Beta)"></a><br><b>Macros (Beta)</b><br>Steps written as plain sentences: press keys, click, wait for a picture, a colour or text, repeat, If. Record one or start from a template, try it, then move its points and areas right on the game. Each macro gets a small panel beside the game with tabs, switches, lists to tick and points the player sets. Stuck? It runs your recovery steps. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/macros/">Macros</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/windows-and-stats/"><img src="docs/screenshots/game-monitor-real.jpg" width="420" alt="Windows"></a><br><b>Windows</b><br>Arrange layouts picked from pictures (as Windows' snap layouts) or a size of your own, the Game monitor with a minimise button on each window, a mini window, RAM warnings. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/windows-and-stats/">Windows and stats</a></td>
</tr>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/settings/"><img src="docs/screenshots/settings.png" width="420" alt="Roblox your way"></a><br><b>Roblox your way</b><br>Graphics, FPS and volume per account; open with Roblox, Bloxstrap, Fishstrap, Voidstrap or your own launcher. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/settings/">Settings</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/discord/"><img src="docs/screenshots/stats.png" width="420" alt="Stats and alerts"></a><br><b>Stats and alerts</b><br>Play time compared with the days before, a timeline of each account, why accounts dropped and the time it cost; alerts and a live status in your Discord channel or on your phone (ntfy); <code>/pc</code> in Discord to check and control your PC from anywhere, and <code>/pc screenshot</code> to see a picture of your accounts' Roblox windows. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/discord/">Discord</a></td>
</tr>
<tr>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/playing/#the-games-tab"><img src="docs/screenshots/games.png" width="420" alt="Games"></a><br><b>Games</b><br>Popular games with their wiki inside Bloxkeep: search, item tables, recent buffs and nerfs, live stock and spawns, Blox Fruits prices. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/playing/#the-games-tab">The Games tab</a></td>
<td width="50%" valign="top"><a href="https://bloxkeep.github.io/roblox-account-manager/guide/backup-and-security/"><img src="docs/screenshots/lock.png" width="420" alt="Safe by default"></a><br><b>Safe by default</b><br>Encrypted vault, lock, backups to a OneDrive or Google Drive folder, hide names for streams. <a href="https://bloxkeep.github.io/roblox-account-manager/guide/backup-and-security/">Backup and security</a></td>
</tr>
<tr>
<td width="50%" valign="top"><img src="docs/screenshots/themes.png" width="420" alt="Yours"><br><b>Yours</b><br>98 themes, English and Vietnamese, quick search with <code>Ctrl+K</code>.</td>
<td width="50%"></td>
</tr>
</table>

Every feature in one list, the illustrated guide and a version to try in your browser: [Bloxkeep's website](https://bloxkeep.github.io/roblox-account-manager/#all).

## Download and install

Windows 10 or 11, 64-bit. Nothing else to install first.

| File | For |
|---|---|
| [`Bloxkeep-Setup.exe`](https://github.com/Bloxkeep/roblox-account-manager/releases/latest/download/Bloxkeep-Setup.exe) | A normal install: small, downloads the newest version, any folder, no admin rights. |
| `Bloxkeep-Setup-Offline.exe` | The same with everything inside, for a PC without internet. |
| `Bloxkeep-Portable.zip` | No install: unzip and run `Bloxkeep.exe`. |

All three are on [Releases](https://github.com/Bloxkeep/roblox-account-manager/releases/latest).

> [!CAUTION]
> Only download Bloxkeep from this repository's Releases. A copy from anywhere else may steal your accounts.

The files aren't code-signed yet, so SmartScreen warns the first time: **More info › Run anyway**. Check a file against `SHA256SUMS.txt` in the release if you like. Details: [Install](https://bloxkeep.github.io/roblox-account-manager/guide/getting-started/#install).

### Updates

Bloxkeep updates itself in the background and installs when you aren't using it, coming back unlocked where you left it. **Restart to update** does it now. **Settings › Bloxkeep** turns it off or adds beta versions. [Updates](https://bloxkeep.github.io/roblox-account-manager/guide/getting-started/#updates)

<img src="docs/screenshots/2-restart-to-update.png" width="640" alt="Restart to update in the sidebar">

### Uninstall

Windows **Settings › Apps › Installed apps › Bloxkeep › Uninstall**. Tick **Also delete my accounts and settings** to remove your data too; otherwise it's kept for a reinstall. [Uninstall](https://bloxkeep.github.io/roblox-account-manager/guide/getting-started/#uninstall)

## Getting started

1. **Set a master password.** It unlocks Bloxkeep and **can't be recovered**. New PC? Choose **Restore from a backup**.
2. **Add accounts.** **Add account**, then **Sign in with your phone** (recommended), username and password, or a browser window.
3. **Play.** Type a game in the bar at the bottom, click accounts to choose them, press **Play**.

<table>
<tr>
<td width="33%" valign="top"><img src="docs/screenshots/lock.png" width="280" alt="The lock screen"><br><b>1.</b> Master password</td>
<td width="33%" valign="top"><img src="docs/screenshots/add-account.png" width="280" alt="Add account"><br><b>2.</b> Add accounts</td>
<td width="33%" valign="top"><img src="docs/screenshots/game-picker.png" width="280" alt="Choosing a game"><br><b>3.</b> Pick a game, press Play</td>
</tr>
</table>

<img src="docs/screenshots/demo.gif" width="800" alt="Choosing two accounts, switching the game, looking at the server list, then changing the theme">

Step by step, with pictures: [Getting started](https://bloxkeep.github.io/roblox-account-manager/guide/getting-started/).

## Community

<a href="https://bloxkeep-bot.bloxkeep-app.workers.dev/discord"><img src="https://img.shields.io/badge/Discord-Join_the_Bloxkeep_server-5865F2?style=for-the-badge&logo=discord&logoColor=white" height="36" alt="Join Bloxkeep's Discord server"></a>

Ask for help, share an idea, and hear about Roblox updates and new Bloxkeep versions as they happen. A quick check when you join keeps scam bots out.

| Channel | For |
|---|---|
| `#announcements` | New Bloxkeep versions and news |
| `#roblox-updates` | Roblox builds coming and live, and outages |
| `#help` | One question per post: what you did, what happened, your version |
| `#bug-reports` | Something broke? The steps and what happened |
| `#ideas` | One idea per post. React to the ones you want |
| `#general-vn` | Trò chuyện bằng tiếng Việt |

Type `/multi` to the bot to see whether Multi-Roblox works on the current Roblox and the one coming next. Nobody from Bloxkeep will ever ask for your cookie, password or 2-step codes.

## Keyboard shortcuts

`Ctrl+K` quick search · `Ctrl+F` search · `Ctrl+L` game box · `Ctrl+A` select all · `Enter` play · `F2` edit · `Delete` delete · `Esc` clear the selection · `Ctrl+Alt+1`–`9` run a macro, `Ctrl+Alt+0` stop them all. Right-click an account for its menu. All of them: [Keyboard shortcuts](https://bloxkeep.github.io/roblox-account-manager/guide/troubleshooting/#keyboard-shortcuts).

## Data and security

- **On your PC only**, in `%LOCALAPPDATA%\Bloxkeep` (the portable copy: a `data` folder beside it).
- **Encrypted** with AES-256-GCM, the key made from your master password (PBKDF2-HMAC-SHA256, 600,000 rounds). The master password is never stored.
- **Your password and cookies only go to Roblox.** Bloxkeep has no analytics. Besides Roblox it talks to GitHub (updates), Roblox's captcha providers (only in a puzzle window), and its own bot for the online count, which you can turn off (a code made from this PC that can't be traced back, the version, the language, the games your accounts are in, how many run and why one dropped out; never an account); plus whatever you choose to send (feedback, your Discord webhook, your backup folder). Every host and what goes there: [privacy policy](PRIVACY.md).
- **Locking** drops every decrypted cookie and password from memory. Copied secrets are wiped from the clipboard after 30 seconds.

> [!WARNING]
> Forget the master password and your saved accounts are gone. A backup only opens with the password it was saved with.

Never send anyone a cookie, `vault.rmv` or a backup: each is a full sign-in to your accounts. More: [Backup and security](https://bloxkeep.github.io/roblox-account-manager/guide/backup-and-security/).

## Backup and restore

Turn on **Settings › Backup & data › Back up to a folder** and pick a OneDrive or Google Drive folder: the encrypted vault is copied there after each change. On a new PC: install, **Restore from a backup**, enter the password it was saved with. [Backups](https://bloxkeep.github.io/roblox-account-manager/guide/backup-and-security/#automatic-backups)

<img src="docs/screenshots/settings-backup.png" width="640" alt="Settings, Backup and data">

## FAQ

**Is this a virus?** No. New unsigned files get warnings. Compare the SHA-256 with `SHA256SUMS.txt`, read the source or [build it yourself](#build-from-source). If your antivirus removed it, report it as a false positive.

**Found a bug or want a feature?** **Settings › Bloxkeep › Send feedback** (it shows exactly what it sends), post in `#bug-reports` or `#ideas` on [Discord](#community), or [open an issue](https://github.com/Bloxkeep/roblox-account-manager/issues).

More answers (bans, macros, Vietnam, a forgotten master password): [the website](https://bloxkeep.github.io/roblox-account-manager/#faq) and [Troubleshooting](https://bloxkeep.github.io/roblox-account-manager/guide/troubleshooting/).

## Not tried with real accounts yet

Macros in real games over many hours · auto-rejoin and **Restart Roblox** over many hours · scheduled presets · Discord · **Light AFK** · very small windows · filling in 2FA codes · a really banned account · **Join a player…** · the last step of phone sign-in · Windows 10. If something breaks, please send feedback from the app.

## Build from source

Needs Windows and the .NET 10 SDK. A C# core (.NET 10, WinForms) with an HTML/React interface in WebView2.

```bat
scripts\build.bat 1.0.1
```

It runs the checks in `Tests\Bloxkeep.Tests` first, then puts the installers, the portable zip, the update packages and `SHA256SUMS.txt` in `dist\releases\`. Each version needs its notes, in English and Vietnamese, in `RobloxManager\whatsnew.json`. To try the interface without building, serve the repository root (`python -m http.server 5178`) and open `/ui/dev/preview.html`.

| Folder | What's in it |
|---|---|
| `RobloxManager/` | The app: C# core, `wwwroot/` interface, `whatsnew.json` |
| `ui/` | The React parts of the interface |
| `Tests/` | The app's checks and the interface checks |
| `services/` | The Discord bot, the feedback service, the Roblox and domain watches |
| `site/`, `docs/` | The website and the user guide |

## Code signing policy

Releases are built from this repository by the [Release workflow](.github/workflows/release.yml) on GitHub's Windows machines. They aren't code-signed yet: see the [code signing policy](docs/CODE_SIGNING_POLICY.md).

## License

[MIT](LICENSE) © 2026 minhhoang0122

Made your own version with other features? Please give it its own name and logo, so people can tell it from Bloxkeep. See [TRADEMARKS.md](TRADEMARKS.md).
