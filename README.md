<p align="center">
  <img src="RobloxManager/wwwroot/logo.svg" width="88" alt="Bloxkeep logo">
</p>
<h1 align="center">Bloxkeep</h1>
<p align="center">Manage and launch several Roblox accounts on Windows. Accounts are encrypted and stay on your PC.</p>
<p align="center">English | <a href="README.vi.md">Tiếng Việt</a></p>
<p align="center">
  <img src="https://img.shields.io/badge/Windows-10%20%7C%2011%20(64--bit)-0078D4" alt="Windows 10 or 11, 64-bit">
  <img src="https://img.shields.io/badge/.NET-10-512BD4" alt="Built with .NET 10">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
</p>
<p align="center"><a href="#download-and-install">Download</a> · <a href="#getting-started">Getting started</a> · <a href="#data-and-security">Security</a> · <a href="#faq">FAQ</a></p>

<p align="center"><img src="docs/screenshots/hero.png" width="860" alt="Bloxkeep main window: account cards with two accounts chosen, the game bar at the bottom, and Multi-Roblox turned on"></p>

Bloxkeep is for people who play on several Roblox accounts on one Windows PC: keep the accounts in one place, pick a game, pick accounts and press **Play**. Your data stays on your PC, encrypted with your master password; Bloxkeep has no server of its own.

*Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*

## Features

- Add accounts with a username and password (with 2-step verification, and captchas solved in a small window), with a code or QR scanned in the Roblox phone app, through roblox.com in a browser window, with a cookie, or import them from Roblox Account Manager or from another Bloxkeep file.
- See each account's status: online, playing on this PC or another device, why it was disconnected, RAM, play time, PID, whether it needs to sign in again or is banned. Search, filter by status or group, sort, and switch between cards and a list.
- Pick a game by name, link or Place ID. Pick a server (most players, fewest, lowest ping, room for N accounts, random), join friends, or join the exact server a player is in by their username.
- Run several Roblox windows at once with Multi-Roblox, open them all windowed and tile them. It's off until you turn it on.
- Presets: save accounts, a game and a server choice to launch in one click, with a schedule to start and stop, and options of their own (anti-AFK, auto-rejoin, a light AFK mode, restarting Roblox every few hours).
- Auto-rejoin when Roblox disconnects, kicks or loses the connection, retrying with longer waits instead of giving up.
- Game monitor: live pictures of every Roblox window in one tab. Statistics of play time per account and per game, with an activity log.
- Authenticator (2FA) codes in the app, hiding account names for screenshots and streams, a daily sign-in check, and spotting banned accounts.
- Send play-time summaries, a status message and alerts to your Discord channel.
- Back up to a OneDrive or Google Drive folder. Also: 32 themes, quick search with `Ctrl+K`, English and Vietnamese, runs from the tray, Windows notifications.

<img src="docs/screenshots/themes.png" width="800" alt="Six Bloxkeep themes: Dark, City, Aurora, Sakura, Paper and Terminal">

## Download and install

You need Windows 10 or 11, 64-bit. You don't need to install .NET. The installer installs Microsoft Edge WebView2 if it's missing.

Get it from [Releases](../../releases/latest):

| File | Use it when |
|---|---|
| `Bloxkeep-Setup.exe` | You want a normal install. It installs for your Windows user, needs no admin rights, and adds Start menu and Desktop shortcuts. |
| `Bloxkeep-Portable.zip` | You'd rather not install. Unzip and run `Bloxkeep.exe`; its data goes into a `data` folder next to it. The portable copy updates itself too. |

> [!CAUTION]
> Only download Bloxkeep from this repository's Releases page. A copy from anywhere else may have been changed to steal accounts.

The files aren't code-signed yet, so Windows SmartScreen warns the first time. Click **More info**, then **Run anyway**. You can compare the file's SHA-256 with `SHA256SUMS.txt` in the same release.

<img src="docs/screenshots/1-install.png" width="480" alt="The Bloxkeep installer at work">

### Updates

Bloxkeep checks GitHub for a new version and downloads it in the background. When it's ready, the sidebar shows **Restart to update**; Roblox keeps running meanwhile. If you don't click it, the update installs when you quit. After updating, the app shows **What's new in Bloxkeep**.

Updating itself only works while the GitHub repository is public.

To turn automatic updates off, untick **Settings › Bloxkeep updates › Update automatically**. **Check now** still works.

Running a newer `Bloxkeep-Setup.exe` over an installed copy works too: the installer offers to update (or repair, for the same version) and keeps your data.

### Uninstall

If Bloxkeep is running, quit it first (right-click the tray icon › **Quit Bloxkeep**): the uninstaller closes it without asking, and the icon of an app closed that way can stay in the tray until you move the mouse over it. Then open Windows **Settings › Apps › Installed apps**, find **Bloxkeep** (publisher `minhhoang0122`) and choose **Uninstall**. The installer and uninstaller dialogs are in English only.

| Removed | Kept |
|---|---|
| The program in `%LOCALAPPDATA%\BloxkeepApp`, the Start menu and Desktop shortcuts (including ones you pinned to Start or the taskbar), the interface cache, and the Installed apps entry. Bloxkeep adds nothing else to Windows: no startup entry, no service, and it doesn't take over the `roblox-player:` protocol. | Your accounts and settings in `%LOCALAPPDATA%\Bloxkeep` (`vault.rmv`, `prefs.json`, `error.log`), so a reinstall picks up where you left off; and the backup files in your backup folder. Delete that folder and the backups yourself to remove those too. |

<details>
<summary>Pictures: updating and uninstalling</summary>

<img src="docs/screenshots/2-restart-to-update.png" width="800" alt="Restart to update in the sidebar"> <img src="docs/screenshots/6-whats-new.png" width="800" alt="What's new after an update"> <img src="docs/screenshots/3-settings-updates.png" width="800" alt="The Bloxkeep updates page in Settings">
<img src="docs/screenshots/4-uninstall-progress.png" width="480" alt="Uninstalling Bloxkeep"> <img src="docs/screenshots/5-uninstall-done.png" width="480" alt="Uninstall finished, account data kept">
</details>

## Getting started

1. **Set a master password** the first time (at least 6 characters). It unlocks Bloxkeep and **can't be recovered** if you forget it; see [Data and security](#data-and-security). Moving to a new PC? Choose **Restore from a backup** instead; see [Backup and restore](#backup-and-restore).

   The app opens in English; the EN/VI switch in the top corner of this screen, or **Settings › Appearance › Language**, changes it.

   <img src="docs/screenshots/lock.png" width="640" alt="The unlock screen with the EN/VI switch in the top corner">

2. **Add accounts**: click **Add account**. The **Sign in to Roblox** dialog offers:
   - **Sign in with your phone** (recommended): Bloxkeep shows a code and a QR. In the **Roblox app** on your phone, open **Settings › Quick Log In**, enter the code or scan the QR, and confirm. No password or puzzle needed. After one account is added you get a new code for the next.
   - **Username** and **Password**: the password only goes to Roblox's sign-in server. Signing in with a password doesn't save it. 2-step verification works (**Authenticator app** or **Email**); if the account already has an authenticator key in Bloxkeep, the code is filled in and you only confirm. If Roblox asks for a captcha, click **Solve the puzzle** and do it in the small window that opens.
   - **Other ways: Sign in with a browser**: the official roblox.com sign-in page opens in a temporary window. Once you're in, Bloxkeep takes the session and deletes that temporary window. Bloxkeep doesn't read the password you type there.
   - **Other ways: Paste a cookie**, or **Import from: Roblox Account Manager** / **Bloxkeep file**: see [Bringing accounts over](#bringing-accounts-over).

   <img src="docs/screenshots/add-account.png" width="700" alt="Sign in to Roblox dialog with Roblox and Roblox VNG tabs, phone sign-in, username and password, and the other ways">

3. **Pick a game, pick accounts, press Play.** Type a game name or paste a link / Place ID in the bar at the bottom, click account cards to choose them (`Shift`-click for a range), then press **Play**. With the game box empty the button becomes **Open Roblox**: each chosen account opens Roblox on its home screen.

   <img src="docs/screenshots/demo.gif" width="800" alt="Choosing two accounts, switching the game, looking at the server list, then changing the theme">

4. Shortcuts: `Ctrl+K` quick search for accounts, presets, games and settings; `Ctrl+F` search accounts; `Ctrl+A` select all; `Enter` play; `F2` edit; `Delete` delete; `Esc` clear the selection. Right-click an account for more.

## Using Bloxkeep

### Account list

- Filter by **All**, **Playing**, **Online**, **Offline**, **On this PC**, **Needs sign-in**, **Banned**, and by group with the **Group** dropdown; sort by **Name**, **Status**, **Last played** or **Launch order**; search names, groups and notes. Switch between **Cards** and **List**.
- Status refreshes every 30 seconds. When an account drops, its card says why, read from Roblox's log: **Kicked for being idle**, **Kicked by the game**, **Signed in on another device**, **Connection lost**, **Couldn't join the server**.
- An account whose session ended is marked **Needs sign-in**; click **Sign in** to renew it. **Check sign-in** is in each account's ⋯ menu, and **Check sign-in of all accounts** is in the ⋯ menu above the list. **Check sign-ins every day** (on by default, in **Settings › Performance**) does this once a day by itself, so you know before you press **Play**.
- While checking, Bloxkeep also looks at whether the account is **Banned** (Roblox's public information). A banned account gets a red tag, and **Play** and presets skip it.
- **Edit…** (`F2`) sets a **Nickname**, **Group** and **Note**. There you can also store the account's **Roblox password** if you want to (optional); it stays in the encrypted vault.
- **Authenticator key** (in **Edit…**): paste the account's 2FA key or `otpauth://` link and Bloxkeep shows the running 6-digit code. **Copy 2FA code** is in the ⋯ menu. Anyone with your master password can make codes too, so only store the key if that's fine with you.
- **Copy password**, **Copy cookie** and **Copy 2FA code** stay out of Windows clipboard history and are wiped after 30 seconds.
- **Hide account names** (in **Settings › Appearance**): every place shows **Account 1**, **Account 2**… instead, Roblox window titles included, for screenshots and streams.

<img src="docs/screenshots/accounts-list.png" width="800" alt="The account list in list view with each account's status">

### Choosing a game and server

- The game box takes a game name, a Place ID, a `roblox.com/games/…` link (private-server links too), a server share link, or a `roblox://` link. When you type a name, pick the game from the list that appears.
- The game picker has **Recent**, **Pinned**, **Roblox favorites** and **Popular** tabs. Click ☆ to pin a game.
- **Job ID (optional)**: joins one exact server, so several accounts land together.
- **Server** lists public servers with room: sort by **Most players**, **Fewest players**, **Lowest ping**, **Best FPS**; filter **Room for N**; or click **Random server**. **Friends in this game** lets you join your accounts' friends (when their privacy settings allow it). The ping shown is the average of the players in that server, not yours.
- **Join a player…** (the button next to the Job ID box): type a player's username or user ID and the chosen accounts join the server that player is in. Bloxkeep asks for their status with the accounts you chose; when their privacy settings hide it, the app says so, and it never friends or follows anyone to get around that.
- On an account that's playing, **Join this server** sends the other accounts into that same server.

<img src="docs/screenshots/game-picker.png" width="800" alt="The game picker with Recent, Pinned, Roblox favorites and Popular tabs">

<img src="docs/screenshots/servers.png" width="700" alt="A game's server list, with friends playing and a filter for room for 2 accounts">

### Presets

A preset saves a name, a list of accounts (in order), a game, how to pick the server, and the wait between accounts. Leave the game empty to open only the **Roblox home screen**. The server can be **Any (Roblox picks)**, **Most players**, **Fewest players**, **Lowest ping**, **Random** or a **Fixed server (Job ID)**.

Manage them in the **Presets** tab: **New preset**, edit, duplicate, reorder, **Run**. The presets button in the bottom bar has **Save as preset…** and quick runs. Up to 50 presets.

A preset's **For the Roblox windows** options apply to the windows that preset opens (also when they rejoin). Pressing **Play** for the same accounts the normal way still follows your general settings.

- **Anti-AFK**: **Follow Settings**, **On** or **Off**.
- **Auto-rejoin**: **Follow each account**, **On** or **Off**.
- **Performance**: **Light AFK** sets the lowest graphics, 30 FPS, no sound, and minimises the window once it's in the game. Windows opened without it keep your Roblox settings.
- **Restart Roblox**: every 2, 4, 6 or 12 hours, each window is closed and opened again into the same game, one at a time. The window you're using waits.

**Schedule**: a preset runs by itself at its **Start time**, once or on chosen weekdays, and can **Stop** after a few hours or at a set time (closing only the windows that run opened). Bloxkeep has to be running and unlocked at that moment.

<img src="docs/screenshots/presets.png" width="800" alt="The Presets tab with three presets, one with its schedule on">

<img src="docs/screenshots/preset-editor.png" width="800" alt="Edit preset: the For the Roblox windows options (Anti-AFK, Auto-rejoin, Performance, Restart Roblox) and the Schedule">

### Multi-Roblox

Multi-Roblox is **off by default**. Roblox doesn't officially support running several clients; use it at your own risk. Turn it on with the Multi-Roblox switch in the sidebar or in **Settings › Launching**.

- Bloxkeep has to be open **before** Roblox and stay open while you play.
- The light shows **Off**, **On — several windows allowed**, or **Not working yet — click to fix** (yellow). To fix it: close every Roblox, including one in the system tray; if Roblox starts with Windows, click **Turn off autostart** (the same as switching it off in Task Manager › Startup apps, and you can switch it back there). The light then turns green.
- **Details** next to Multi-Roblox has two more options:
  - **Always open Roblox windowed** (on by default): all Roblox windows share one fullscreen setting, so with several open some end up fullscreen and some don't. Bloxkeep opens every window windowed and puts your setting back afterwards.
  - **Tile the windows after opening** (off by default): once the accounts are open, the Roblox windows are laid out in a grid on the main screen.
- One home screen per account: choose the accounts, leave the game box empty, press **Open Roblox**.

How it works: Roblox uses an object named `ROBLOX_singletonEvent` to allow only one window; since its October 2026 builds, a new window uses it to close the older ones. Bloxkeep takes that name first, while no Roblox is running, so Roblox can't set this up and the windows don't close each other. Bloxkeep injects nothing into Roblox and reads nothing from the Roblox process.

If a newer Roblox build starts closing windows when another opens again, **Settings › Version & graphics** shows a yellow warning. It goes away once two or more accounts are in a game at the same time, when Roblox updates, or when you click **Hide**.

### Auto-rejoin

Turn it on per account in the ⋯ menu › **Auto-rejoin** (a preset can override it; see [Presets](#presets)). When that account's Roblox disconnects, gets kicked or loses its connection, Bloxkeep rejoins the same game (or the same private server) after 10 seconds, though not necessarily the same server. If it keeps dropping, from the third time Bloxkeep waits 2, 5, 10, then 15 minutes between tries (the card shows the wait with a **Cancel** button); 10 minutes in the game resets the count. Without a network, it retries every 30 seconds, up to 20 times.

Bloxkeep doesn't rejoin when you leave the game yourself, when the account signed in on another device, when it's banned, or when Roblox itself closed the window because another one opened. Closing the Roblox window by hand counts as leaving, so use **Close** in the app.

### Roblox settings per account

In **Settings › Version & graphics › Roblox settings**, **Apply to** chooses **Nobody**, **All accounts** or **Chosen accounts** (pick them with **Use Roblox settings** in the ⋯ menu). Set **Graphics quality** from 1 to 10, **Frame rate limit** 30/60/144/240, **Volume** from **Muted** to 100 %, or **Leave as is**.

Bloxkeep writes these values into Roblox's settings file just before opening the account, and puts your own settings back once Roblox is running.

The same page's **Roblox updates** group shows the installed Roblox build, the newest one Roblox serves, and the builds seen on this PC, each with a link to Roblox's release **Notes**.

### Open Roblox with

Choose in **Settings › Launching › Open Roblox with**. Bloxkeep finds what's installed.

| Choice | Notes |
|---|---|
| **Default** | Whatever opens Roblox links on this PC (Roblox, Bloxstrap, Fishstrap…) |
| **Bloxstrap**, **Fishstrap** | Keeps your mods and FFlags |
| Other Bloxstrap-style launchers | Listed when installed |
| **Roblox (official)** | The client from roblox.com |
| **RobloxPlayerBeta directly (skip launchers)** | No launcher at all; try it when a launcher blocks several windows |
| **Choose another launcher…** | An exe you pick |
| **Roblox (Microsoft Store)** | Not supported: it can't open the account you chose. Install Roblox from roblox.com |

### Windows, RAM and the game monitor

- The sidebar card **Roblox on this PC** shows the number of Roblox windows, the RAM they use, the free RAM, and **Mini window** and **Close all**. Its version line tells you whether your Roblox is the newest.
- **Game monitor** (a sidebar tab): live pictures of every Roblox window with the account, game, RAM and PID. Click a picture to bring that window to the front; right-click for more. A minimised window has no live picture. Windows draws the pictures itself (like the previews when you hover the taskbar), so they cost next to nothing.
- **Mini window**: a compact window to watch the accounts that are playing without the main window.
- **Arrange Roblox windows** (in the bottom bar) and **Arranging windows** (in **Settings › Launching**): **Fill the screen**, or **Small tiles** / **Medium tiles** / **Large tiles** laid out from the top-left, leaving the rest of the screen free. The smaller a Roblox window, the less work for the graphics card.
- **Low RAM warning** (on by default): warns before you open more accounts than the free RAM can take, and when the PC uses more than 92 % of its RAM.
- **Save RAM on background windows** (off by default): when RAM runs low, background windows give RAM back to the one you're playing. Roblox may refuse; then Bloxkeep leaves it.
- **Show account names on windows** (on by default): puts the account's name in the Roblox window title, to tell windows apart on the taskbar.
- **Wait between accounts**: 0 to 60 seconds, 8 by default. Opening too many at once can fail.

### Statistics and activity

The **Statistics** tab counts play time, sessions and disconnects for **Today**, **7 days** or **30 days**, per account and per game. Only games opened with Bloxkeep count.

Switch to **Activity** for the log: accounts opened, games joined and left, drops (with the reason), rejoins, scheduled presets starting and stopping, accounts that need to sign in again. Filter by account and kind, and **Export CSV** when you need it. Bloxkeep keeps the last 1,000 events, in the encrypted vault.

<img src="docs/screenshots/stats.png" width="800" alt="The Statistics tab: total play time, a chart per day, play time per account and most played games">

### Tray, notifications, Discord and appearance

- **Keep running in the tray when closed** (on by default): the X button hides Bloxkeep to the tray, and auto-rejoin and Multi-Roblox keep working. To quit, right-click the tray icon and choose **Quit Bloxkeep**. Next time, the Bloxkeep window comes back where you left it, at the same size, maximised or not.
- **Windows notifications** (on by default): only while Bloxkeep is in the background. Turn each kind on or off in **Settings › Notifications**: account dropped or auto-rejoin, scheduled presets, sign-in needs renewing, low RAM, launch problems.
- **Discord** (off by default): paste a channel's webhook link in **Settings › Discord**. **Send automatically** a play-time summary every day, every week or both; keep a **Status message** up to date; and send **Alerts** for the events above, with a **Ping** for a person or a role. There are **Quiet hours** and **Customise messages** (sender name and picture, language, message style, **Hide account names in messages**).
- **Settings › Appearance**: **Theme** has 32 themes plus **Sync with Windows** (**Dark** by default), **Language** English / Tiếng Việt, and **Hide account names**.
- **Hardware acceleration** (in **Settings › Performance**): draws the interface with the graphics card instead of the CPU; it turns itself off if the graphics driver keeps failing.

<img src="docs/screenshots/discord.png" width="800" alt="The Discord page in Settings: send to Discord, webhook link, play-time summary">

### Bringing accounts over

**Roblox Account Manager**: it encrypts `AccountData.json`, so first decrypt it with the official [RAMDecrypt](https://github.com/ic3w0lf22/RAMDecrypt) tool. Then in the **Add account** dialog click **Import from: Roblox Account Manager** and pick the decrypted file. Nicknames, groups, notes and passwords (if RAM had them) come along. Accounts you already have get the new cookie, and cookies are checked right after the import.

**Another Bloxkeep file** (a `vault.rmv` or a backup from another PC): click **Import from: Bloxkeep file**, or **Import…** in **Settings › Backup & data**. Enter that file's own master password, tick the accounts you want, and import. The file itself isn't changed; the imported accounts are locked with your current master password.

**Cookie**: choose **Paste a cookie** and paste the `.ROBLOSECURITY` cookie.

### Roblox VNG

You can add Roblox VNG accounts on the **Roblox VNG** tab (with your phone or with a password). They get a "VNG" tag. Bloxkeep **can't open games** for VNG accounts yet; their **Play** button is off, with the reason.

<details>
<summary>More pictures</summary>

<img src="docs/screenshots/settings.png" width="800" alt="Settings, the Launching page: the page list on the left, Multi-Roblox and window arranging on the right"> <img src="docs/screenshots/palette.png" width="800" alt="Quick search with Ctrl+K: typing farm finds the matching accounts and preset">
</details>

## Data and security

**What's stored.** Accounts (cookie, username, ID, nickname, group, note, and the password and authenticator key if you entered them), settings, presets, recent and pinned games, play history and the activity log, the Discord webhook link.

**Where.** Everything stays on your PC. **Settings › Backup & data › Data location** shows the folder, with **Open folder**.

| Path | Contents |
|---|---|
| `%LOCALAPPDATA%\Bloxkeep\` | The data folder (installed copy). The portable copy uses a `data\` folder next to the app. |
| `vault.rmv` | The encrypted vault: accounts, settings, presets, recent and pinned games, play history, activity log, webhook link |
| `vault.rmv.bak` | The previous save, used automatically if `vault.rmv` is damaged |
| `vault.rmv.damaged-…`, `vault.rmv.spare-…`, `vault-before-restore-….rmv` | Copies kept after a self-repair or a restore; the app never deletes them |
| `prefs.json` | Non-secret settings: language, theme, updates, notifications, backup folder, window position, Roblox build history |
| `roblox-settings.json` | Your graphics, FPS, volume and fullscreen settings, to put back after opening an account |
| `error.log` | Errors and short sign-in notes, with no account details |
| `webview\`, `webview-safe\` | The interface cache, removed on uninstall |
| `%LOCALAPPDATA%\BloxkeepApp\` | The installed program (removed on uninstall) |

**How it's protected.** The vault is encrypted with AES-256-GCM. The key comes from your master password through PBKDF2-HMAC-SHA256 with 600,000 iterations and a random salt. The file header is authenticated too, so a wrong password or a tampered file is detected. The master password is never stored. Every save is written in full before it replaces the old file, and the previous save is kept in `vault.rmv.bak`. When you lock the app, Bloxkeep drops all the decrypted data it was holding (cookies, passwords, authenticator keys).

> [!WARNING]
> If you forget the master password, your saved accounts can't be recovered. A backup also only opens with the password it was saved with.

**What leaves your PC.** Bloxkeep has no telemetry or analytics. Data only goes to:

| Where | When, and what |
|---|---|
| Roblox's API hosts (`auth`, `users`, `presence`, `friends`, `games`… `.roblox.com`) | Signing in (the password only goes to `auth.roblox.com`), session and ban checks, status every 30 seconds, game, server, friend and player lookups, join tickets, the newest Roblox version |
| `www.roblox.com` | The browser sign-in window, profiles, and the join link handed to Roblox |
| `*.rbxcdn.com` | Avatars and game pictures |
| `*.robloxapp.vnggames.com` | The same as the Roblox API hosts, for Roblox VNG accounts only |
| Roblox's captcha providers (Arkose Labs, HUMAN Security) | Only in the puzzle window, when Roblox asks for a captcha during password sign-in |
| GitHub | Checking for and downloading updates, with no account data; can be turned off |
| Microsoft (`go.microsoft.com`) | Only when WebView2 has to be installed |
| The backup folder you choose | A copy of the encrypted vault; your OneDrive / Google Drive app may sync it |
| Your Discord webhook (`discord.com`) | Only when **Settings › Discord** is on (off by default): account names (or **Account 1**, **Account 2**… with **Hide account names in messages**), game names, play time, sessions, PID, RAM. Never cookies, passwords or the webhook link itself; the link is kept in the encrypted vault |

**Inside the app.** The interface is locked down with a Content Security Policy, so it can't send requests by itself; every connection goes through the app's core. A password or cookie only reaches the interface when you show or copy it. Copies stay out of clipboard history and are wiped after 30 seconds. `error.log` has no account details.

**Keep cookies safe.** A cookie is a full sign-in key for its account. Never send anyone your cookies, `vault.rmv` or a backup file. If one leaks, sign out of all other sessions in Roblox's settings (on roblox.com or in the Roblox app).

**Check it yourself.** Compare the SHA-256 with `SHA256SUMS.txt`, read the source, or [build it yourself](#build-from-source).

## Backup and restore

Turn on **Settings › Backup & data › Back up to a folder**, then click **Choose…** or use a OneDrive / Google Drive folder Bloxkeep found on your PC (for example **Use Google Drive**). Bloxkeep only copies files into that folder; your OneDrive or Google Drive app uploads them.

- A backup is a copy of the encrypted vault, named `Bloxkeep-backup-` plus the date and time.
- It backs up after every change (at most every 10 minutes), when you lock or quit if something changed, and right after you change the master password or pick a new folder. **Back up now** backs up at once.
- It keeps the 10 newest, plus the newest of each day for the last 30 days. The backup just made is never cleaned up, even after the PC clock is set back. Bloxkeep only cleans up backups of its own vault, so two PCs sharing a folder never delete each other's.
- If the master password is easy to guess and the folder is a cloud folder, the app suggests a longer one.

**Restore**: click **Restore…** in **Settings › Backup & data**, or **Restore from a backup** on the first screen of a new PC. Your current data is kept as `vault-before-restore-….rmv`, then the app locks; unlock it with the password the backup was saved with.

**Moving to a new PC**: install Bloxkeep, click **Restore from a backup**, pick the newest backup and enter the password it was saved with. To merge the accounts of two PCs instead of replacing them, use **Import…** (see [Bringing accounts over](#bringing-accounts-over)).

<img src="docs/screenshots/settings-backup.png" width="800" alt="The Backup & data page in Settings, backing up to a OneDrive folder">

## Known limits and testing status

- Roblox's online status lags: after you leave a game, Roblox can still report "playing" for a few minutes.
- The Microsoft Store version of Roblox can't be used.
- Roblox VNG accounts can be added, but games can't be opened for them yet.
- Multi-Roblox: Bloxkeep has to be open before Roblox and stay open while you play. If Roblox (including one in the tray) opened first, the light stays yellow until every Roblox is closed. Roblox doesn't officially support several clients, and a later Roblox build may change how it blocks them.
- Per-app VPNs: if Roblox goes through a VPN and Bloxkeep doesn't, opening a game can fail with error 403. See the [FAQ](#faq).
- A server's ping is its players' average, not yours. Friends' servers, and the server of a player you look up, only show when their privacy settings allow it.
- Saving RAM: Roblox may not give RAM back.
- The files aren't code-signed, so SmartScreen warns the first time. Updating itself only works while the GitHub repository is public.
- A forgotten master password can't be recovered.

Tried with real accounts on Windows 11 (Roblox 0.742 and 0.743, October 2026): password sign-in through the proof-of-work step and HUMAN's press-and-hold captcha, joining a game, 5 accounts in one game with Multi-Roblox on Roblox's October 2026 build, several separate home screens.

**Not tried with real accounts yet:**

- Auto-rejoin and **Restart Roblox** over many hours.
- Scheduled presets, sending to Discord, **Light AFK**, **Small tiles**.
- The 2FA code filled in at sign-in, the **Banned** tag on an actually banned account, **Join a player…** with a real player.
- The last step of **Sign in with your phone**, 2-step verification, and the rotating-picture captcha (Arkose).
- Windows 10.

If something breaks, please [open an issue](../../issues) with the error.

## FAQ

<details>
<summary>Will my account get banned?</summary>

No one can promise that. Roblox doesn't officially support running several clients, so using Multi-Roblox is at your own risk. Bloxkeep signs in and opens games the same way roblox.com does, and doesn't touch the game's memory. Some games forbid alt accounts; check the rules of the game you play.
</details>

<details>
<summary>Is this a virus? Why do Windows or antivirus apps warn?</summary>

No. The files aren't code-signed yet, so SmartScreen and some antivirus apps warn about new files. You can compare the SHA-256 with `SHA256SUMS.txt`, read the source in this repository, or [build it yourself](#build-from-source).
</details>

<details>
<summary>I forgot my master password.</summary>

It can't be recovered; it isn't stored anywhere. You can delete `vault.rmv` and `vault.rmv.bak` in `%LOCALAPPDATA%\Bloxkeep` (losing every saved account) and start over, or restore a backup whose password you remember.
</details>

<details>
<summary>How do I move to a new PC?</summary>

Turn on backups to a OneDrive or Google Drive folder on the old PC. On the new one, install Bloxkeep, click **Restore from a backup** and enter the password the backup was saved with. See [Backup and restore](#backup-and-restore).
</details>

<details>
<summary>The Multi-Roblox light is yellow.</summary>

A Roblox was open before Bloxkeep. Close every Roblox, including one in the system tray. If Roblox starts with Windows, click **Turn off autostart** in the help dialog. The light turns green. Next time, open Bloxkeep before Roblox.
</details>

<details>
<summary>Some Roblox windows open fullscreen and some don't.</summary>

All Roblox windows share one fullscreen setting, so with several open they easily end up different. Keep **Always open Roblox windowed** on in Multi-Roblox's **Details** (it's on by default): every window opens windowed, and your setting is put back afterwards. To lay them out neatly, use **Arranging windows**.
</details>

<details>
<summary>Do my Bloxstrap / Fishstrap mods still work?</summary>

Yes, if you choose **Bloxstrap** or **Fishstrap** in **Open Roblox with**. If the launcher blocks several windows, choose **RobloxPlayerBeta directly (skip launchers)**; Roblox then opens without the launcher.
</details>

<details>
<summary>I left the game but it still says "playing".</summary>

Roblox's status lags, sometimes by a few minutes. For windows on this PC Bloxkeep ignores the stale report; status from other devices can still lag.
</details>

<details>
<summary>Opening a game fails with 403 "An error was encountered during authentication".</summary>

Usually a per-app VPN: Roblox goes through it and Bloxkeep doesn't. A join ticket is tied to the IP address that asked for it, so let Bloxkeep take the same route as Roblox.
</details>

<details>
<summary>Can I use it in Vietnam without access to roblox.com?</summary>

Partly. Password sign-in and phone sign-in go through Roblox's API hosts, not www.roblox.com, and the captcha page is served by the app itself. The browser method needs www.roblox.com. Bloxkeep has no VPN or region bypass. Roblox VNG accounts can be added, but games can't be opened for them yet.
</details>

<details>
<summary>Does Bloxkeep send my password or cookies anywhere?</summary>

Only to Roblox. A password you sign in with only goes to Roblox's sign-in server, and each account's cookie is only sent to Roblox's own hosts (Roblox VNG's, for VNG accounts). Update checks to GitHub carry no account data. The full list is under [Data and security](#data-and-security).
</details>

<details>
<summary>Why isn't the Microsoft Store version of Roblox supported?</summary>

The Store version can't open the account you choose, so Bloxkeep can't use it. Install Roblox from roblox.com (Bloxstrap or Fishstrap on top is fine).
</details>

<details>
<summary>How do I remove Bloxkeep completely?</summary>

Uninstall it in Windows **Settings › Apps**, then delete the `%LOCALAPPDATA%\Bloxkeep` folder. For the portable copy, delete the app's folder (with its `data`).
</details>

## Build from source

You need Windows and the **.NET 10 SDK**. The app is a C# core (.NET 10, WinForms) with an HTML/CSS/JS interface in WebView2. The [Velopack](https://velopack.io) packager (`vpk`) is pinned in `dotnet-tools.json` and the scripts fetch it.

```bat
build.bat 1.0.1
```

The output goes to `dist\releases\`: `Bloxkeep-Setup.exe`, `Bloxkeep-Portable.zip`, the update packages and `SHA256SUMS.txt`. A second argument is the GitHub repository the build looks for updates in (this one by default); pass another when you build a fork.

```bat
release.bat 1.0.1 https://github.com/minhhoang0122/bloxkeep
```

This uploads the build to GitHub Releases and needs a `GITHUB_TOKEN` environment variable with write access to releases. Each version needs its notes, in English and Vietnamese, in `RobloxManager\whatsnew.json`; they show in the app after an update and become the release notes. To test updates before publishing, set `BLOXKEEP_UPDATE_SOURCE` to a local folder of release files.

**Try the interface without building**: serve the repository root with any static web server (for example `python -m http.server 5178`) and open `http://127.0.0.1:5178/dev/preview.html`. It runs on the made-up data in `dev/mock.js`.

```
RobloxManager/             C# source
  wwwroot/                 the interface (embedded in the exe); EN/VI strings in i18n.js
  AppController*.cs        commands from the interface, launching, process tracking, auto-rejoin, presets,
                           schedules, Discord, statistics, backups (one file per part)
  RobloxApi.cs             Roblox API calls (sign-in, status, games, servers)
  RobloxLauncher.cs        opens Roblox through the chosen launcher
  RobloxLogWatcher.cs      reads Roblox's logs to see joins and kicks
  RobloxGameSettings.cs    writes Roblox settings before opening an account, then puts yours back
  MultiInstance.cs         Multi-Roblox
  MonitorTab.cs            the Game monitor tab
  Vault.cs                 the encrypted vault
  AppUpdater.cs            self-update (Velopack)
  whatsnew.json            notes for each version
Installer/                 the installer's splash image
dev/                       interface preview with made-up data (not packaged)
docs/screenshots/          screenshots (vi/: the Vietnamese ones)
```

## License

[MIT](LICENSE) © 2026 minhhoang0122

*Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*
