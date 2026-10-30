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
</p>
<p align="center"><a href="https://github.com/Bloxkeep/roblox-account-manager/releases/latest/download/Bloxkeep-Setup.exe"><img src="https://img.shields.io/badge/Download_for_Windows-Bloxkeep--Setup.exe-2ea44f?style=for-the-badge" height="36" alt="Download Bloxkeep for Windows (Bloxkeep-Setup.exe, the newest version)"></a></p>
<p align="center"><a href="#download-and-install">Download</a> · <a href="#getting-started">Getting started</a> · <a href="docs/guide.md">User guide</a> · <a href="https://bloxkeep.github.io/roblox-account-manager/">Website</a> · <a href="#data-and-security">Security</a> · <a href="#faq">FAQ</a> · <a href="PRIVACY.md">Privacy</a></p>

<p align="center"><img src="docs/screenshots/multi-roblox-real.jpg" width="860" alt="A real screenshot of one Windows PC: five Roblox windows open at once, titled Roblox - Account 1 to Roblox - Account 5, with Bloxkeep behind them counting 5 windows and Multi-Roblox on"></p>
<p align="center"><sub>A real screenshot from one PC: five Roblox accounts open at once with Bloxkeep. Names in the game are blurred.</sub></p>

Keep your Roblox accounts in one place, pick a game, pick accounts, press **Play**. Everything stays on your PC, encrypted with your master password. Bloxkeep has no server of its own.

*Bloxkeep is an independent project, not affiliated with or endorsed by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*

## Features

<img src="docs/screenshots/hero.png" width="860" alt="Bloxkeep main window: account cards with two accounts chosen, the game bar at the bottom, and Multi-Roblox turned on">

| | |
|---|---|
| **Add accounts** | With your phone (code or QR), username and password (2-step verification and captchas work), a browser window, a cookie or another Bloxkeep file. [Accounts](docs/guide/accounts.md) |
| **See every account** | Online, in game here or on another device, why it disconnected, RAM, play time, needs sign-in, banned. Search, groups, cards or list. [Accounts](docs/guide/accounts.md) |
| **Play** | Pick a game by name, link or Place ID, and a server: most or fewest players, lowest ping, room for N accounts, your private servers, a friend's or any player's server. [Playing](docs/guide/playing.md) |
| **Multi-Roblox** | Several Roblox windows at once. Off until you turn it on; every new Roblox build is checked. [Multi-Roblox](docs/guide/multi-roblox.md) |
| **Presets** | Accounts, game and server saved for one click, with a schedule to start and stop. [Presets](docs/guide/presets.md) |
| **Stay in game** | Anti-AFK against idle kicks, auto-rejoin after a disconnect. [Staying in game](docs/guide/staying-in-game.md) |
| **Windows** | Arrange layouts picked from pictures (as Windows' snap layouts) or a size of your own, the Game monitor, a mini window, RAM warnings. [Windows and stats](docs/guide/windows-and-stats.md) |
| **Roblox your way** | Graphics, FPS and volume per account; open with Roblox, Bloxstrap, Fishstrap or your own launcher. [Settings](docs/guide/settings.md) |
| **Stats and Discord** | Play time, sessions and disconnects; alerts and a live status in your Discord channel. [Discord](docs/guide/discord.md) |
| **Safe by default** | Encrypted vault, lock, backups to a OneDrive or Google Drive folder, hide names for streams. [Backup and security](docs/guide/backup-and-security.md) |
| **Yours** | 74 themes, English and Vietnamese, quick search with `Ctrl+K`. |

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

The files aren't code-signed yet, so SmartScreen warns the first time: **More info › Run anyway**. Check a file against `SHA256SUMS.txt` in the release if you like. Details: [Install](docs/guide/getting-started.md#install).

### Updates

Bloxkeep updates itself in the background and installs when you aren't using it, coming back unlocked where you left it. **Restart to update** does it now. **Settings › Bloxkeep** turns it off or adds beta versions. [Updates](docs/guide/getting-started.md#updates)

### Uninstall

Windows **Settings › Apps › Installed apps › Bloxkeep › Uninstall**. Tick **Also delete my accounts and settings** to remove your data too; otherwise it's kept for a reinstall. [Uninstall](docs/guide/getting-started.md#uninstall)

## Getting started

1. **Set a master password.** It unlocks Bloxkeep and **can't be recovered**. New PC? Choose **Restore from a backup**.
2. **Add accounts.** **Add account**, then **Sign in with your phone** (recommended), username and password, or a browser window.
3. **Play.** Type a game in the bar at the bottom, click accounts to choose them, press **Play**.

<img src="docs/screenshots/demo.gif" width="800" alt="Choosing two accounts, switching the game, looking at the server list, then changing the theme">

Step by step, with pictures: [Getting started](docs/guide/getting-started.md).

## Keyboard shortcuts

`Ctrl+K` quick search · `Ctrl+F` search · `Ctrl+L` game box · `Ctrl+A` select all · `Enter` play · `F2` edit · `Delete` delete · `Esc` clear the selection. Right-click an account for its menu. All of them: [Keyboard shortcuts](docs/guide/troubleshooting.md#keyboard-shortcuts).

## Data and security

- **On your PC only**, in `%LOCALAPPDATA%\Bloxkeep` (the portable copy: a `data` folder beside it).
- **Encrypted** with AES-256-GCM, the key made from your master password (PBKDF2-HMAC-SHA256, 600,000 rounds). The master password is never stored.
- **Your password and cookies only go to Roblox.** Bloxkeep has no analytics. Besides Roblox it talks to GitHub (updates), Roblox's captcha providers (only in a puzzle window), and its own bot for an anonymous "open now" count you can turn off; plus whatever you choose to send (feedback, your Discord webhook, your backup folder). Every host and what goes there: [privacy policy](PRIVACY.md).
- **Locking** drops every decrypted cookie and password from memory. Copied secrets are wiped from the clipboard after 30 seconds.

> [!WARNING]
> Forget the master password and your saved accounts are gone. A backup only opens with the password it was saved with.

Never send anyone a cookie, `vault.rmv` or a backup: each is a full sign-in to your accounts. More: [Backup and security](docs/guide/backup-and-security.md).

## Backup and restore

Turn on **Settings › Backup & data › Back up to a folder** and pick a OneDrive or Google Drive folder: the encrypted vault is copied there after each change. On a new PC: install, **Restore from a backup**, enter the password it was saved with. [Backups](docs/guide/backup-and-security.md#automatic-backups)

## FAQ

**Is this a virus?** No. New unsigned files get warnings. Compare the SHA-256 with `SHA256SUMS.txt`, read the source or [build it yourself](#build-from-source). If your antivirus removed it, report it as a false positive.

**Does Multi-Roblox really work?** Yes, the picture at the top is real. Roblox can change how it blocks extra windows in any update, so Bloxkeep checks every new build. [Multi-Roblox](docs/guide/multi-roblox.md)

**Will my account get banned?** No one can promise that. Roblox doesn't officially support several clients, and some games forbid alt accounts. Bloxkeep signs in and opens games the way roblox.com does and never touches the game's memory.

**Can I use it in Vietnam without roblox.com?** Partly: phone and password sign-in go through Roblox's API, not www.roblox.com; the browser sign-in needs www.roblox.com. Bloxkeep has no VPN. Roblox VNG accounts can be added, but games can't be opened for them yet.

**I forgot my master password.** It can't be recovered. Restore a backup whose password you remember, or start over.

**Mac or Linux?** No, Windows 10 and 11 only.

**Found a bug or want a feature?** **Settings › Bloxkeep › Send feedback** (it shows exactly what it sends), or [open an issue](https://github.com/Bloxkeep/roblox-account-manager/issues).

More answers: [Troubleshooting](docs/guide/troubleshooting.md).

## Not tried with real accounts yet

Auto-rejoin and **Restart Roblox** over many hours · scheduled presets · Discord · **Light AFK** · very small windows · filling in 2FA codes · a really banned account · **Join a player…** · the last step of phone sign-in · Windows 10. If something breaks, please send feedback from the app.

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
