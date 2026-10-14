<p align="center">
  <img src="RobloxManager/wwwroot/logo.svg" width="88" alt="Bloxkeep logo">
</p>
<h1 align="center">Bloxkeep</h1>
<p align="center">Manage and launch several Roblox accounts on Windows. Accounts are encrypted and stay on your PC.</p>
<p align="center"><a href="README.md">Tiếng Việt</a> | English</p>
<p align="center">
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20(64--bit)-0078D4" alt="Windows 10 or 11, 64-bit">
  <img src="https://img.shields.io/badge/.NET-10-512BD4" alt="Built with .NET 10">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>
<p align="center"><a href="#download-and-install">Download</a> · <a href="#getting-started">Getting started</a> · <a href="#data-and-security">Security</a> · <a href="#faq">FAQ</a></p>

<p align="center"><img src="docs/screenshots/hero.png" width="860" alt="Bloxkeep main window: account cards, the game bar at the bottom, and Multi-Roblox turned on"></p>

Bloxkeep keeps your Roblox accounts in one list. You sign in once per account, then open one or several of them in the game and server you pick. It is meant for people who play on more than one account on the same Windows PC, and it runs entirely on that PC: there is no Bloxkeep server and no Bloxkeep account.

*Bloxkeep is an independent project. It is not affiliated with, endorsed by, or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*

## Features

- Add accounts five ways: username and password (with 2-step verification; if Roblox asks for a captcha, you solve it in a small window), a code or QR scanned with the Roblox phone app, the roblox.com sign-in page in a browser window, a pasted cookie, or an import from Roblox Account Manager.
- See where each account stands: online, playing on this PC or on another device, why it was disconnected, how much RAM its Roblox window uses and how long it has played, or that it was signed out. Search, filter, sort, and switch between cards and a list.
- Pick a game by name, link or Place ID, or from **Recent**, **Pinned**, **Roblox favorites** and **Popular**. Choose a server (most or fewest players, lowest ping, room for your accounts, random) or join a friend, so several accounts end up in the same server.
- Run several Roblox windows at the same time with Multi-Roblox. It stays off until you turn it on.
- Save presets (accounts, game and server choice) and run them again with one click.
- Rejoin a game automatically, per account, after a crash, a kick or a lost connection.
- Set Roblox graphics quality, FPS limit and volume per account.
- Keep RAM under control: warnings before you run out, freeing RAM from background windows, arranging Roblox windows in a grid, and account names on window titles.
- Back up to a OneDrive or Google Drive folder. Also: 32 themes, English and Vietnamese, runs from the tray, Windows notifications.

<img src="docs/screenshots/themes.png" width="800" alt="Six Bloxkeep themes: Aurora, Sunset, Sakura, Paper, Terminal and Dark">

## Download and install

Windows 10 or 11, 64-bit. You don't need to install .NET. Microsoft Edge WebView2 is installed automatically if it's missing.

Download from the [Releases page](../../releases/latest):

| File | When to use |
|---|---|
| `Bloxkeep-Setup.exe` | Normal install. Installs for your Windows user only (no admin rights needed), adds Start Menu and Desktop shortcuts, and installs WebView2 if the PC doesn't have it. |
| `Bloxkeep-Portable.zip` | No install. Unzip it and run `Bloxkeep.exe`. Your data is kept in a `data` folder next to it. The portable copy updates itself too. |

> [!CAUTION]
> Download Bloxkeep only from this repository's Releases page. Copies from other sites may be modified to steal your accounts.

The files aren't code-signed, so Windows SmartScreen warns the first time you run them. Click **More info › Run anyway**. To check that your file is the one published here, compare its SHA-256 hash (in PowerShell: `Get-FileHash .\Bloxkeep-Setup.exe`) with `SHA256SUMS.txt` from the same release.

<img src="docs/screenshots/1-install.png" width="480" alt="Bloxkeep installer while installing">

### Updates

Bloxkeep checks this repository's Releases about 20 seconds after it starts and then every 6 hours, and downloads a new version in the background. When it's ready, the sidebar shows **Restart to update**. Roblox keeps running while Bloxkeep restarts. If you don't restart, the update is installed when you quit. Updates only work while the GitHub repository is public.

After an update, Bloxkeep shows **What's new** for that version. To stop automatic updates, turn off **Settings › Updates › Update automatically**. **Check now** works either way.

Running a newer `Bloxkeep-Setup.exe` over an installed copy works too: it offers to update (or to repair the same version) and keeps your data.

### Uninstall

Quit Bloxkeep first if it's running (right-click the tray icon › **Quit Bloxkeep**): the uninstaller ends it without asking, and an ended app's tray icon can linger until the mouse passes over it. Then go to Windows **Settings › Apps › Installed apps**, find **Bloxkeep** (publisher `minhhoang0122`) and choose **Uninstall**. The installer and uninstaller dialogs are in English on every Windows language.

| Removed | Kept |
|---|---|
| The program in `%LOCALAPPDATA%\BloxkeepApp`, the Start menu and Desktop shortcuts (also ones you pinned to Start or the taskbar), the interface cache, and the entry in Installed apps. Bloxkeep adds nothing else to Windows: no startup entry, no service, no `roblox-player:` link handler. | Your accounts and settings in `%LOCALAPPDATA%\Bloxkeep` (`vault.rmv`, `prefs.json`, `error.log`), so a reinstall picks them up where you left off, and any backup files in your backup folder. To remove these as well, delete that folder and the backups yourself. |

<details>
<summary>Screenshots: updating and uninstalling</summary>

<img src="docs/screenshots/2-restart-to-update.png" width="800" alt="Restart to update button in the sidebar">
<img src="docs/screenshots/6-whats-new.png" width="800" alt="What's new panel after an update">
<img src="docs/screenshots/3-settings-updates.png" width="800" alt="Updates section in Settings">
<img src="docs/screenshots/4-uninstall-progress.png" width="480" alt="Uninstall in progress">
<img src="docs/screenshots/5-uninstall-done.png" width="480" alt="Uninstall finished, account data kept">
</details>

## Getting started

1. **Set a master password.** On first start, Bloxkeep asks for a master password of at least 6 characters. It unlocks the app and protects your saved accounts, and it can't be recovered if you forget it (see [Data and security](#data-and-security)). Setting up on a new PC? Choose **Restore from a backup** on this screen instead (see [Backup and restore](#backup-and-restore)).

   The app opens in English. The screenshots in this README show the Vietnamese interface; the EN/VI switch in the top corner of this screen, or **Settings › Language**, changes it.

   <img src="docs/screenshots/lock.png" width="640" alt="Unlock screen with the EN/VI language switch in the top corner">

2. **Add an account.** With an empty list, Bloxkeep offers **Use your phone**, **Use a browser** and **Use account & password**. Later, **Add account** opens the same choices.
   - **Use your phone**: Bloxkeep shows a code and a QR code. In the Roblox app on your phone, go to **Settings › Quick Log In**, enter the code or scan the QR, and confirm. A new code appears for the next account. Quick Log In is only in the phone app, not in the Roblox PC client.
   - **Use a browser**: the official roblox.com sign-in page opens in a temporary browser window. After you sign in, Bloxkeep keeps the session and deletes the temporary browser profile. It never reads the password you type there.
   - **Use account & password**: the password goes only to Roblox's sign-in server and is not saved. 2-step verification works with an authenticator app or email. If Roblox asks for a captcha, you solve it in a small window.
   - **Use a cookie** and **Import from Roblox Account Manager**: see [Moving from Roblox Account Manager](#moving-from-roblox-account-manager).

   <img src="docs/screenshots/add-account.png" width="700" alt="Sign in to Roblox dialog with Roblox and Roblox VNG tabs and the other sign-in options">

3. **Pick a game, select accounts, press Play.** Type a game name, or paste a game link or Place ID, into the bar at the bottom. Click accounts to select them (`Shift`+click selects a range) and press **Play**. If you leave the game box empty, the button becomes **Open Roblox** and opens Roblox's home screen for each selected account.

   <img src="docs/screenshots/demo.gif" width="800" alt="Choosing a game, selecting two accounts and pressing Play">

4. Keyboard: `Ctrl+F` search, `Ctrl+A` select all, `Enter` play the selected accounts, `F2` edit, `Delete` delete, `Esc` clear the selection or close a dialog. Right-click an account for more actions.

## Using Bloxkeep

### Account list

- Filter by **All**, **Playing**, **Online**, **Offline**, **On this PC** or **Needs sign-in**. **Sort** by **Name**, **Status** or **Last played**. Search covers names, groups and notes. Switch between **Cards** and **List**.
- Status refreshes every 30 seconds, or right away with **Refresh online status**. When a Roblox window on this PC drops, the account shows why, taken from Roblox's own log files: **Kicked for being idle**, **Kicked by the game**, **Signed in on another device**, **Connection lost**, **Couldn't join the server**, or an error code.
- An account whose session has ended shows **Signed out** and a **Sign in** button. **Check sign-in** checks one account; **Check sign-in of all accounts** is in the ⋯ menu at the top of the list.
- **Edit…** (`F2`) sets a **Nickname**, **Group** and **Note**, and optionally a **Roblox password** for your own reference.
- **Copy password** and **Copy cookie** keep the copied text out of Windows clipboard history and the cloud clipboard, and clear it after 30 seconds or when you lock or quit.

<img src="docs/screenshots/accounts-list.png" width="800" alt="Account list in list view with each account's status">

### Choosing a game and server

- The game box takes a game name, a Place ID, a `roblox.com/games/…` link (private server links included), a `roblox.com/share` server link, or a `roblox://` link.
- The picker has **Recent**, **Pinned**, **Roblox favorites** (games your accounts favorited on Roblox) and **Popular**. Pin a game with ☆.
- **Job ID (optional)** sends every selected account to one exact server.
- **Servers** lists public servers with free slots, sorted by **Most players**, **Fewest players**, **Lowest ping** or **Best FPS**. The **Room for N** filter, **Random server** and **Load more** are there too. The ping shown is the average of the players in that server, not your own.
- **Friends in this game**, in the same dialog, lists friends of your accounts who are playing it, each with a **Join** button. Friends only appear if their privacy settings allow it.
- On an account that is already playing, **Join this server** in its ⋯ menu picks that server for the other accounts.

<img src="docs/screenshots/game-picker.png" width="800" alt="Game picker with Recent, Pinned, Roblox favorites and Popular tabs">

<img src="docs/screenshots/servers.png" width="700" alt="Server list for a game, with friends in the game and a filter for servers with room for 2 accounts">

### Presets

A preset stores a name, a list of accounts in order, a game, a server choice and, if you want, its own delay. The **Game** box works like the bottom bar: pick from the **Recent**, **Pinned**, **Roblox favorites** and **Popular** tabs, type a name to search, or paste a link / Place ID. Leave it empty to open the **Roblox home** screen. Server choices are **Any (Roblox picks)**, **Most players**, **Fewest players**, **Lowest ping**, **Random** and **Fixed server (Job ID)**.

The **Presets** button in the bottom bar has **Save as preset…**, **Edit presets…** and **Run**. You can keep up to 50 presets.

A preset's **Options** apply to the Roblox windows it opens (also after they rejoin). Pressing **Play** for the same accounts uses the usual settings.

- **Anti-AFK**: **Follow Settings**, **On** or **Off**.
- **Auto-rejoin**: **Follow each account**, **On** or **Off**.
- **Performance**: **Light AFK** sets the lowest graphics, 30 FPS and no sound, and minimises the window once it's in the game. Windows opened without it keep your own Roblox settings.
- **Restart Roblox**: every 2, 4, 6 or 12 hours, each window is closed and opened again into the same game, one at a time. A window you're using waits.

<img src="docs/screenshots/presets.png" width="700" alt="New preset dialog: name, game, server, delay and accounts">

### Multi-Roblox

Multi-Roblox is off by default. Roblox doesn't officially support running several clients at once, so use it at your own risk, especially with accounts you care about. Turn it on with the switch in the sidebar or in Settings.

- Start Bloxkeep before Roblox and keep it open while you play.
- The light in the sidebar shows **Off**, **On — several windows allowed** or **Not working yet — click to fix**. If it isn't working, close every Roblox, including the one in the system tray. If Roblox starts with Windows, click **Turn off autostart**; this does the same as turning it off in Task Manager › Startup apps, and you can turn it back on there. The light then turns green by itself.
- If Bloxstrap or Fishstrap blocks a second window, choose **RobloxPlayerBeta directly (skip launchers)** in **Settings › Open Roblox with**.
- To give each account its own Roblox home screen, select the accounts, leave the game box empty and press **Open Roblox**.

How it works: Roblox allows a single window by holding two system objects, `ROBLOX_singletonMutex` and `ROBLOX_singletonEvent`. Bloxkeep creates and holds them before Roblox starts, so a new Roblox window no longer closes the previous one. Nothing is injected into Roblox, and its memory isn't read.

### Auto-rejoin

Turn on **Auto-rejoin** for an account in its ⋯ menu; Settings shows how many accounts have it on. When that account's Roblox crashes, is kicked or loses its connection, Bloxkeep opens the same game (or private server) again after 10 seconds. It doesn't return to the exact same server.

- If it keeps dropping, the third try onwards waits 2, 5, 10, then 15 minutes between tries; the account's card shows the wait and a **Cancel** button. Staying in the game for 10 minutes starts the count over.
- It doesn't rejoin when you left the game yourself or when the account signed in on another device.
- Closing a Roblox window yourself counts as leaving. To close an account's Roblox, use **Close** in Bloxkeep.
- Without a network connection it waits and retries every 30 seconds.

### Roblox settings per account

The **Roblox settings** card in Settings sets **Graphics quality** (1–10), **Frame rate limit** (30, 60, 144 or 240) and **Volume** (**Muted**, 25, 50, 75 or 100 %). Each can also be **Leave as is**. **Use for** applies them to **Nobody**, **All accounts** or **Chosen accounts**; choose accounts with **Use Roblox settings** in each account's ⋯ menu.

Bloxkeep writes these values into Roblox's settings file right before it opens the account, then puts your own values back once Roblox has started. If you changed a value yourself in the meantime, your change is kept.

### Open Roblox with

**Settings › Open Roblox with** chooses what opens Roblox. Installed launchers are found automatically.

| Option | What it does |
|---|---|
| **Default (…)** | Uses whatever opens Roblox links on your PC: Roblox, Bloxstrap, Fishstrap or similar |
| **Bloxstrap**, **Fishstrap** | Opens Roblox through that launcher, so your mods and FFlags still apply |
| Other Bloxstrap-style launchers | Listed if they are installed |
| **Roblox (official)** | The client installed from roblox.com |
| **RobloxPlayerBeta directly (skip launchers)** | Starts the newest `RobloxPlayerBeta.exe` without a launcher. Try it when a launcher blocks several windows |
| **Choose another launcher…** | Any program you pick; it gets the Roblox link as its first argument |
| **Roblox (Microsoft Store)** | Not supported: it can't open a chosen account. Install Roblox from roblox.com instead |

### RAM and windows

- The **Roblox on this PC** card in the sidebar shows how many Roblox windows are open, the RAM they use and how much RAM the PC has free. **Close all** closes them; **Force close all Roblox** is in the ⋯ menu at the top of the list.
- The **Roblox version** line compares the installed Roblox with the latest version.
- **Low RAM warning** (on by default) warns before you open more accounts than your free RAM can hold, counting about 900 MB per window, and when the PC goes over 92 % RAM use.
- **Save RAM on background windows** (off by default): when the PC is low on RAM, background Roblox windows are asked to give memory back to the one you're playing. Roblox can refuse, and then that window is skipped.
- **Arrange Roblox windows** tiles every Roblox window in a grid on the screen you choose. **Show account names on windows** (on by default) puts the account name in each window's title, so you can tell them apart on the taskbar.
- **Delay between accounts** spaces out launches: 0 to 60 seconds, 8 by default. Opening too many at once can fail.

### Tray, notifications and appearance

- **Keep running when closed** (on by default): the X button hides Bloxkeep to the tray, and auto-rejoin and Multi-Roblox keep working. To exit, right-click the tray icon and choose **Quit Bloxkeep**.
- **Windows notifications** (on by default) appear only while Bloxkeep is in the background: an account dropped, an account keeps getting disconnected, or RAM is low. You get at most one per account and event every 5 minutes.
- **Discord** (off by default): paste a channel's webhook link in **Settings › Discord** to get a daily or weekly play-time summary, a status message that updates itself, and the same alerts, each with its own switch and optional ping.
- **Theme**: 32 themes in five groups (Basic, Scenery, Colourful, Styles, Seasonal), or **Sync with Windows**. The default is **Dark**, and the window's title bar follows the theme.
- **Language**: English or Tiếng Việt.
- **Hardware acceleration** switches the interface between GPU and CPU drawing. It turns itself off if the graphics driver keeps failing.

### Moving from Roblox Account Manager

Roblox Account Manager (by ic3w0lf22) encrypts its `AccountData.json`, so decrypt it first with RAM's official [RAMDecrypt](https://github.com/ic3w0lf22/RAMDecrypt) tool. Then choose **Import from Roblox Account Manager** in the **Add account** dialog (or **Move over from Roblox Account Manager…** in the ▾ menu next to it) and pick the decrypted file. An encrypted file is refused with a hint to decrypt it.

Nicknames, groups, notes and any passwords RAM had are copied over. Accounts that are already in Bloxkeep get their cookie updated, and every imported cookie is checked right away.

To add a single account by cookie, choose **Use a cookie** and paste its `.ROBLOSECURITY` value.

### Roblox VNG

You can add Roblox VNG accounts with **Use your phone** (on the **Roblox VNG** tab) or with a username and password on the **Roblox VNG** tab of **Sign in to Roblox**. They get a VNG tag in the list. Bloxkeep can't open games for VNG accounts yet.

<details>
<summary>More screenshots</summary>

<img src="docs/screenshots/settings.png" width="800" alt="Settings page">
</details>

## Data and security

**What is stored.** For each account: its Roblox session cookie, username and ID, and the nickname, group and note you set. Also your settings, recent and pinned games, and presets. Signing in with a password doesn't save it. You can store one yourself in **Edit…**, as an option; it stays in the encrypted vault. An import from Roblox Account Manager also copies passwords RAM had saved.

**Where.** Everything is stored only on this PC:

| Path | What's in it |
|---|---|
| `%LOCALAPPDATA%\Bloxkeep\` | The data folder of the installed app. The portable build uses `data\` next to `Bloxkeep.exe`. |
| `vault.rmv` | Accounts, settings, recent and pinned games, and presets, encrypted with your master password |
| `vault.rmv.bak` | The previous save, used automatically if `vault.rmv` is damaged |
| `vault.rmv.damaged-…`, `vault.rmv.spare-…`, `vault-before-restore-….rmv` | Copies kept after an automatic repair or a restore; the app never deletes them |
| `prefs.json` | Preferences that aren't secret: language, theme, tray, updates, notifications, backup folder |
| `roblox-settings.json` | Your own Roblox graphics, FPS and volume, to put back after a launch |
| `error.log` | Errors and short sign-in notes, never account data |
| `webview\`, `webview-safe\` | Interface cache, deleted on uninstall |
| `%LOCALAPPDATA%\BloxkeepApp\` | The installed program, removed on uninstall |

**Settings › Data › Data location** shows the folder, and **Open folder** opens it.

**How it's protected.** The vault is encrypted with AES-256-GCM. The key is derived from your master password with PBKDF2-HMAC-SHA256 (600,000 iterations, random 16-byte salt). The file header is authenticated together with the data, so a wrong password or any change to the file is detected. The master password itself is never saved, and the key is wiped from memory when you lock or quit. Saves are atomic: the new file is written first and then swapped in, and the previous save is kept as `vault.rmv.bak`.

> [!WARNING]
> If you forget the master password, your saved accounts can't be recovered. A backup only opens with the password it was saved with.

**What leaves the PC.** There is no telemetry or analytics. Bloxkeep only talks to:

| Destination | When and what |
|---|---|
| Roblox API hosts (`auth`, `users`, `presence`, `friends`, `games`, `develop`, `thumbnails`, `apis`, `twostepverification`, `clientsettingscdn` on `roblox.com`) | Sign-in (the password goes only to `auth.roblox.com/v2/login`), session checks, status every 30 seconds with each account's own cookie, game search and info, server lists, friends, favorites, popular games, the launch ticket, and the latest Roblox version |
| `www.roblox.com` | The browser sign-in window, profile links, and the launch link handed to Roblox |
| `*.rbxcdn.com` | Avatar and game images |
| `*.robloxapp.vnggames.com` | The same as the Roblox API hosts, for Roblox VNG accounts only |
| Roblox's captcha providers: Arkose Labs (`arkoselabs.roblox.com`) and HUMAN Security (`*.px-cloud.net`, `captcha.px-cdn.net`) | Only in the puzzle window, when Roblox asks for a captcha during password sign-in |
| GitHub | Update checks and downloads, with no account data. Can be turned off |
| Microsoft (`go.microsoft.com`) | Only if WebView2 has to be installed. The downloaded installer runs only if it is signed by Microsoft Corporation |
| Your backup folder | An encrypted copy of the vault. If it's a OneDrive or Google Drive folder, their app syncs it |
| Your Discord webhook (`discord.com`) | Only if you turn on **Settings › Discord** (off by default): account names, game names, play times, counts, PIDs and RAM. Never cookies, passwords or the webhook link itself, which is kept in the encrypted vault |

**Inside the app.** The interface runs in WebView2 under a Content Security Policy (`connect-src 'none'`), so it can't make network requests itself; every network call goes through the app's C# side. A password or cookie reaches the interface only when you reveal or copy it. Copies are kept out of clipboard history and cleared after 30 seconds. Bloxkeep doesn't read the session of a Roblox client that is already signed in on your PC. To track game sessions it reads only Roblox's log files in `%LOCALAPPDATA%\Roblox\logs`. Requests to Roblox go over IPv4, because the launch ticket is tied to the IP address that requested it.

**Keep your cookie private.** A `.ROBLOSECURITY` cookie is a full login to that account. Never paste it into a website or send it to anyone, and don't share `vault.rmv` or a backup file. If a cookie leaks, sign out of all other sessions in Roblox's settings (on roblox.com or in the Roblox app).

**Check it yourself.** Compare the SHA-256 hash of your download with `SHA256SUMS.txt`, read the source, or [build it yourself](#build-from-source).

## Backup and restore

Turn on **Settings › Backup › Back up to a folder** and pick a folder. Bloxkeep suggests the OneDrive and Google Drive folders it finds on the PC (a `Bloxkeep` folder inside them), or you can pick any folder with **Choose…**. Bloxkeep never connects to OneDrive or Google Drive itself; it writes the file into the folder and their sync app uploads it. The folder can't be inside Bloxkeep's own data folder.

- A backup is a copy of the already encrypted vault. Its name starts with `Bloxkeep-backup-` and includes the date and time.
- Bloxkeep makes one after a change (at most every 10 minutes), on lock and quit if something changed, and right away after you change the master password or pick a new folder. **Back up now** makes one on the spot.
- It keeps the newest 10 backups plus the newest one from each of the last 30 days, and removes the rest. It only prunes this vault's own backups, so two PCs sharing one folder don't delete each other's.
- If your master password is weak (under 8 characters, or only lowercase letters and digits) and the folder is a cloud folder, Bloxkeep warns you.

To restore, use **Restore…** in **Settings › Backup**, or **Restore from a backup** on the first-run screen. Your current data is first kept as `vault-before-restore-….rmv`. Bloxkeep then locks; unlock it with the password the backup was saved with.

Moving to a new PC: install Bloxkeep, choose **Restore from a backup** on the first screen, pick the latest backup file, and enter the master password it was saved with.

<img src="docs/screenshots/settings-backup.png" width="700" alt="Backup section in Settings, backing up to a OneDrive folder">

## Known limits and testing status

- Roblox's online status lags. After you leave a game, Roblox can still report the account as playing for a few minutes. Bloxkeep corrects this for windows that ran on this PC, but status from other devices can still lag.
- The Microsoft Store version of Roblox can't be used.
- Roblox VNG accounts can be added, but games can't be opened for them yet.
- Multi-Roblox needs Bloxkeep to start before Roblox and stay open. If any Roblox was open first (including the tray copy that starts with Windows), the light stays yellow until every Roblox window is closed. Roblox doesn't officially support several clients.
- With a per-app (split-tunnel) VPN, if Roblox goes through the VPN and Bloxkeep doesn't, opening a game can fail with 403 "An error was encountered during authentication". See the [FAQ](#faq).
- Server ping is the players' average, not yours. Friends' servers only show if their privacy settings allow it.
- **Save RAM on background windows** can't force Roblox to give memory back; if Roblox refuses, that window is skipped.
- The files are unsigned, so SmartScreen warns on first run.
- A forgotten master password means the saved accounts can't be recovered.
- Automatic updates only work while this GitHub repository is public.

Tested with 2 real accounts on Windows 11 with Roblox 0.742, in October 2026: password sign-in with the proof-of-work check and the press-and-hold captcha, joining a game, 2 accounts in the same server with Multi-Roblox, and 2 separate home screens.

Not tested with real accounts yet:

- Auto-rejoin over long sessions.
- The last step of **Use your phone** sign-in, 2-step verification, and the rotate-image (Arkose) captcha.
- Running on Windows 10.

If something fails, please [open an issue](../../issues) with the error text.

## FAQ

<details>
<summary>Can my account get banned for using Bloxkeep?</summary>

There's no guarantee either way. Roblox doesn't officially support running several clients; use Multi-Roblox at your own risk. Bloxkeep signs in and launches games the same way roblox.com does, and it doesn't read or change Roblox's memory. Some games also forbid alt accounts in their own rules.
</details>

<details>
<summary>Is this a virus? Why does Windows or my antivirus warn about it?</summary>

No. The exe isn't code-signed, and Windows SmartScreen warns about unsigned programs. You can compare the SHA-256 hash with `SHA256SUMS.txt` from the release, read the source, or [build it yourself](#build-from-source).
</details>

<details>
<summary>I forgot my master password.</summary>

The saved accounts can't be recovered. Close Bloxkeep, delete `vault.rmv` and `vault.rmv.bak` from `%LOCALAPPDATA%\Bloxkeep` (or from `data\` in the portable build), start Bloxkeep, set a new master password and add your accounts again. If you have a backup whose password you remember, restore that instead.
</details>

<details>
<summary>How do I move to a new PC?</summary>

Install Bloxkeep on the new PC, choose **Restore from a backup** on the first screen, pick the latest backup file and enter the password it was saved with. This needs backups turned on beforehand; see [Backup and restore](#backup-and-restore).
</details>

<details>
<summary>The Multi-Roblox light is yellow.</summary>

A Roblox window was open before Multi-Roblox could take over. Close every Roblox, including the one in the system tray. If Roblox starts with Windows, click **Turn off autostart**. The light turns green by itself. Next time, start Bloxkeep before Roblox.
</details>

<details>
<summary>Bloxstrap / Fishstrap: do my mods still work?</summary>

Yes, when **Settings › Open Roblox with** is set to Bloxstrap, Fishstrap, or a **Default** that points to one of them. If the launcher blocks a second Roblox window, try **RobloxPlayerBeta directly (skip launchers)**.
</details>

<details>
<summary>The status still says "playing" after I left the game.</summary>

Roblox's own online status lags by a few minutes. Bloxkeep corrects it for windows that ran on this PC; for other devices, wait a few minutes.
</details>

<details>
<summary>Opening a game fails with 403 "An error was encountered during authentication".</summary>

This usually means a per-app (split-tunnel) VPN sends Roblox through the VPN but not Bloxkeep. The launch ticket Bloxkeep gets is tied to the IP address that requested it, so put Bloxkeep on the same route as Roblox.
</details>

<details>
<summary>Can I use it in Vietnam without access to roblox.com?</summary>

Partly. Password sign-in and phone sign-in go through Roblox's API hosts, not www.roblox.com, and the captcha page is served by the app itself. The browser method needs www.roblox.com. Bloxkeep has no VPN or region bypass. Roblox VNG accounts can be added, but games can't be opened for them yet.
</details>

<details>
<summary>Does Bloxkeep send my password or cookie anywhere?</summary>

Only to Roblox. A password you sign in with goes only to `auth.roblox.com/v2/login`, and each account's cookie is only sent to Roblox's own hosts (Roblox VNG's, for VNG accounts). Update checks to GitHub carry no account data. The full list is under [Data and security](#data-and-security).
</details>

<details>
<summary>Why isn't the Microsoft Store version of Roblox supported?</summary>

It signs itself in, so Bloxkeep can't make it open a chosen account. Install Roblox from roblox.com; Bloxkeep finds it automatically.
</details>

<details>
<summary>How do I remove everything?</summary>

Uninstall Bloxkeep in Windows **Settings › Apps**, then delete `%LOCALAPPDATA%\Bloxkeep`. For the portable build, delete the folder you unzipped, including `data\`. Backup files in your backup folder stay until you delete them.
</details>

## Build from source

You need Windows and the .NET 10 SDK. The app is C# (.NET 10, WinForms) with an HTML/CSS/JS interface running in WebView2. The packaging tool, [Velopack](https://velopack.io) (`vpk`), is a local dotnet tool listed in `dotnet-tools.json` and restored by the build script.

```bat
build.bat 1.0.1
```

This builds into `dist\releases\`: `Bloxkeep-Setup.exe`, `Bloxkeep-Portable.zip`, the update packages (`*.nupkg`, `releases.win.json`) and `SHA256SUMS.txt`. An optional second argument is the GitHub repository URL the build checks for updates. It defaults to this repository; pass your own when you build a fork.

```bat
release.bat 1.0.1 https://github.com/minhhoang0122/bloxkeep
```

This uploads the build to GitHub Releases and needs a `GITHUB_TOKEN` environment variable with write access to releases. Each version needs its notes, in English and Vietnamese, in `RobloxManager\whatsnew.json`; they show in the app after an update and become the release notes. To test updates before publishing, set `BLOXKEEP_UPDATE_SOURCE` to a local folder of release files.

To preview the interface without building, serve the repository root with any static web server (for example `python -m http.server 5178`) and open `http://127.0.0.1:5178/dev/preview.html`. It uses fake data from `dev/mock.js`.

```
RobloxManager/             source code
  wwwroot/                 interface (embedded in the exe); EN/VI strings in i18n.js
  AppController.cs         handles interface commands, launches games, watches processes, auto-rejoin
  RobloxApi.cs             Roblox API calls (sign-in, status, games, servers)
  RobloxLauncher.cs        opens Roblox through the chosen launcher
  RobloxLogWatcher.cs      reads Roblox's logs to see joins and kicks
  MultiInstance.cs         Multi-Roblox
  Vault.cs                 encrypted vault
  AppUpdater.cs            automatic updates (Velopack)
  whatsnew.json            notes for each version
Installer/                 installer splash image
dev/                       interface preview with fake data (not shipped)
docs/screenshots/          screenshots
```

## License

[MIT](LICENSE) © 2026 minhhoang0122

*Bloxkeep is an independent project. It is not affiliated with, endorsed by, or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*
