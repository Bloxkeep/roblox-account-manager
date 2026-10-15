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
<p align="center"><a href="#download-and-install">Download</a> · <a href="#getting-started">Getting started</a> · <a href="docs/guide.md">User guide</a> · <a href="#data-and-security">Security</a> · <a href="#faq">FAQ</a> · <a href="CODE_SIGNING_POLICY.md">Code signing policy</a> · <a href="PRIVACY.md">Privacy</a></p>

<p align="center"><img src="docs/screenshots/hero.png" width="860" alt="Bloxkeep main window: account cards with two accounts chosen, the game bar at the bottom, and Multi-Roblox turned on"></p>

Bloxkeep is for people who play on several Roblox accounts on one Windows PC: keep the accounts in one place, pick a game, pick accounts and press **Play**. Your data stays on your PC, encrypted with your master password; Bloxkeep has no server of its own.

*Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*

## Features

- Add accounts with a username and password (with 2-step verification, and captchas solved in a small window), with a code or QR scanned in the Roblox phone app, through roblox.com in a browser window, with a cookie, or import them from another Bloxkeep file.
- See each account's status: online, playing on this PC or another device, why it was disconnected, RAM, play time, PID, whether it needs to sign in again or is banned. Search, filter by status or group, sort, and switch between cards and a list.
- Pick a game by name, link or Place ID. Pick a server (most players, fewest, lowest ping, room for N accounts, random), join friends, or join the exact server a player is in by their username.
- Run several Roblox windows at once with Multi-Roblox, open them all windowed and tile them. It's off until you turn it on.
- Presets: save accounts, a game and a server choice to launch in one click, with a schedule to start and stop, and options of their own (anti-AFK, auto-rejoin, a light AFK mode, restarting Roblox every few hours).
- Auto-rejoin when Roblox disconnects, kicks or loses the connection, retrying with longer waits instead of giving up.
- Game monitor: live pictures of every Roblox window in one tab. Statistics of play time per account and per game, with an activity log.
- Authenticator (2FA) codes in the app, hiding account names for screenshots and streams, a daily sign-in check, and spotting banned accounts.
- Send play-time summaries, a status message and alerts to your Discord channel.
- Back up to a OneDrive or Google Drive folder. Also: 74 themes, quick search with `Ctrl+K`, English and Vietnamese, runs from the tray, Windows notifications.

<img src="docs/screenshots/themes.png" width="800" alt="Six Bloxkeep themes: Dark, City, Aurora, Sakura, Paper and Terminal">

## Download and install

You need Windows 10 or 11, 64-bit. You don't need to install .NET. The installer installs Microsoft Edge WebView2 if it's missing.

Get it from [Releases](../../releases/latest):

| File | Use it when |
|---|---|
| `Bloxkeep-Setup.exe` | You want a normal install. A small file that downloads the newest Bloxkeep from this page as it installs (about 60 MB). You choose the folder, on any drive (pick `D:\` and it goes to `D:\Bloxkeep`). It installs for your Windows user, needs no admin rights, and adds Start menu and Desktop shortcuts. |
| `Bloxkeep-Setup-Offline.exe` | The same installer with everything inside (about 60 MB), for a PC without internet or to keep for later. |
| `Bloxkeep-Portable.zip` | You'd rather not install. Unzip and run `Bloxkeep.exe`; its data goes into a `data` folder next to it. The portable copy updates itself too. |

> [!CAUTION]
> Only download Bloxkeep from this repository's Releases page. A copy from anywhere else may have been changed to steal accounts.

The files aren't code-signed yet: Bloxkeep is applying to SignPath Foundation for free code signing ([code signing policy](CODE_SIGNING_POLICY.md)). Until then Windows SmartScreen warns the first time. Click **More info**, then **Run anyway**. You can compare the file's SHA-256 with `SHA256SUMS.txt` in the same release. `Bloxkeep-Setup.exe` is the same file in every release (only what it downloads changes), so SmartScreen gets to know it over time; the web installer checks its download against `SHA256SUMS.txt` itself.

Setup shows where Bloxkeep will go (`%LOCALAPPDATA%\BloxkeepApp` unless you choose another folder; the program takes about 230 MB), copies it with a progress bar and can open it when it's done. Your accounts and settings stay apart from the program, in `%LOCALAPPDATA%\Bloxkeep` (a few MB). Setup and the uninstaller speak English and Vietnamese.

<img src="docs/screenshots/setup-folder.png" width="420" alt="Bloxkeep Setup: choosing the install folder, here D:\Bloxkeep"> <img src="docs/screenshots/setup-progress.png" width="420" alt="Bloxkeep Setup copying the files"> <img src="docs/screenshots/setup-done.png" width="420" alt="Bloxkeep Setup done, with Open Bloxkeep now ticked">

### Updates

Bloxkeep checks GitHub for a new version when it starts and every hour after, and downloads it in the background. It then installs it by itself the next time you aren't using it, and comes back in a few seconds as it was: still unlocked, on the same tab, the window still in the tray or minimised. Roblox windows keep running all along and are followed again.

"Not using it" means the window is in the tray or minimised, or nobody has touched the PC for 10 minutes, and Bloxkeep is in the middle of nothing: no Roblox opening, auto-rejoin or planned restart under way, no sign-in, backup, import or half-filled form. A Roblox window that is just running doesn't hold the update back.

To update right away, click **Restart to update** in the sidebar as soon as it shows. Quitting Bloxkeep installs it too. After updating, the app shows **What's new in Bloxkeep**.

To come back unlocked, the running copy hands the vault key to the new one in a file that only your Windows account can read (Windows DPAPI, mixed with a random value given to the new copy alone). The new copy deletes it as it reads it, and refuses one older than 3 minutes. If you would rather type the master password again after each update, lock Bloxkeep: a locked copy hands over nothing.

A small watcher (the uninstaller's program, run from a temporary copy) waits for the new version to come up. If it doesn't within a minute or so (the install failed, the new version crashed), the watcher deletes that file and starts Bloxkeep itself: back in the tray, locked, with a notification saying so. An update that didn't install isn't tried again by itself for a day; **Restart to update** still works.

Updating itself only works while the GitHub repository is public.

To turn automatic updates off, untick **Settings › Bloxkeep updates › Update automatically**: nothing is downloaded or installed by itself then. **Check now** still works.

Running a newer `Bloxkeep-Setup.exe` over an installed copy works too: it installs into the same folder, closes a running Bloxkeep first, and keeps your data. To move Bloxkeep to another drive, uninstall it, then install it again in the new folder.

### Uninstall

Open Windows **Settings › Apps › Installed apps**, find **Bloxkeep** (publisher `minhhoang0122`) and choose **Uninstall**. Bloxkeep's uninstaller asks whether to **Also delete my accounts and settings**: leave it unticked to keep them for a later reinstall, or tick it to remove them for good. It closes Bloxkeep first if it's running.

<img src="docs/screenshots/uninstall.png" width="420" alt="Uninstall Bloxkeep, with Also delete my accounts and settings ticked">

| Always removed | Removed only when you tick **Also delete my accounts and settings** | Never touched |
|---|---|---|
| The program folder (`%LOCALAPPDATA%\BloxkeepApp` unless you chose another one), the Start menu and Desktop shortcuts (including ones you pinned to Start or the taskbar), the interface cache, and the Installed apps entry. Bloxkeep adds nothing else to Windows: no startup entry, no service, and it doesn't take over the `roblox-player:` protocol. | Your accounts and settings in `%LOCALAPPDATA%\Bloxkeep`: the vault (`vault.rmv` and its spare copies), `prefs.json` and `error.log`. | The backup files in your backup folder, and any other file you put in `%LOCALAPPDATA%\Bloxkeep`. |

<details>
<summary>Pictures: updating</summary>

<img src="docs/screenshots/2-restart-to-update.png" width="800" alt="Restart to update in the sidebar"> <img src="docs/screenshots/6-whats-new.png" width="800" alt="What's new after an update"> <img src="docs/screenshots/3-settings-updates.png" width="800" alt="The Bloxkeep updates page in Settings">
</details>

## Getting started

1. **Set a master password** the first time (at least 6 characters). It unlocks Bloxkeep and **can't be recovered** if you forget it; see [Data and security](#data-and-security). Moving to a new PC? Choose **Restore from a backup** instead; see [Backup and restore](#backup-and-restore).

   The app opens in English; the EN/VI switch in the top corner of this screen, or **Settings › Appearance › Language**, changes it.

   <img src="docs/screenshots/lock.png" width="640" alt="The unlock screen with the EN/VI switch in the top corner">

2. **Add accounts**: click **Add account**. The **Sign in to Roblox** dialog offers:
   - **Sign in with your phone** (recommended): Bloxkeep shows a code and a QR. In the **Roblox app** on your phone, open **Settings › Quick Log In**, enter the code or scan the QR, and confirm. No password or puzzle needed. After one account is added you get a new code for the next.
   - **Username** and **Password**: the password only goes to Roblox's sign-in server. Signing in with a password doesn't save it. 2-step verification works (**Authenticator app** or **Email**); if the account already has an authenticator key in Bloxkeep, the code is filled in and you only confirm. If Roblox asks for a captcha, click **Solve the puzzle** and do it in the small window that opens.
   - **Other ways: Sign in with a browser**: the official roblox.com sign-in page opens in a temporary window. Once you're in, Bloxkeep takes the session and deletes that temporary window. Bloxkeep doesn't read the password you type there.
   - **Other ways: Paste a cookie**, or **Import from: Bloxkeep file**: see [Bringing accounts over](#bringing-accounts-over).

   <img src="docs/screenshots/add-account.png" width="700" alt="Sign in to Roblox dialog with Roblox and Roblox VNG tabs, phone sign-in, username and password, and the other ways">

3. **Pick a game, pick accounts, press Play.** Type a game name or paste a link / Place ID in the bar at the bottom, click account cards to choose them (`Shift`-click for a range), then press **Play**. With the game box empty the button becomes **Open Roblox**: each chosen account opens Roblox on its home screen.

   <img src="docs/screenshots/demo.gif" width="800" alt="Choosing two accounts, switching the game, looking at the server list, then changing the theme">

4. Shortcuts: `Ctrl+K` quick search for accounts, presets, games and settings; `Ctrl+F` search accounts; `Ctrl+A` select all; `Enter` play; `F2` edit; `Delete` delete; `Esc` clear the selection. Right-click an account for more.

## Using Bloxkeep

### Account list

- Filter by **All**, **Playing**, **Online**, **Offline**, **On this PC**, **Needs sign-in**, **Banned**, and by group with the **Group** dropdown; sort by **Name**, **Status**, **Last played** or **Launch order**; search names, groups and notes. Switch between **Cards** and **List**.
- Status refreshes every 30 seconds. When an account drops, its card says why, read from Roblox's log: **Kicked for being idle**, **Kicked by the game**, **Signed in on another device**, **Connection lost**, **Couldn't join the server**.
- An account whose session ended is marked **Needs sign-in**; click **Sign in** to renew it. **Check sign-in** is in each account's ⋯ menu, and **Check sign-in of all accounts** is in the ⋯ menu above the list. **Check sign-ins every day** (on by default, in **Settings › Performance**, which shows when it last ran) does this once a day by itself, so you know before you press **Play**: one account at a time, 2 seconds apart, about 45 seconds after unlocking, while Bloxkeep is unlocked.
- While checking, Bloxkeep also looks at whether the account is **Banned** (Roblox's public information). A banned account gets a red tag, and **Play** and presets skip it.
- **Edit…** (`F2`) sets a **Nickname**, **Group** and **Note**. There you can also store the account's **Roblox password** if you want to (optional); it stays in the encrypted vault.
- **Authenticator key** (in **Edit…**): paste the account's 2FA key or `otpauth://` link and Bloxkeep shows the running 6-digit code. **Copy 2FA code** is in the ⋯ menu. Anyone with your master password can make codes too, so only store the key if that's fine with you.
- **Copy password** and **Copy 2FA code** stay out of Windows clipboard history and are wiped after 30 seconds. They work once you have saved the password or the authenticator key under **Edit** (no way of signing in keeps the password).
- **Hide account names** (in **Settings › Appearance**): every place shows **Account 1**, **Account 2**… instead, Roblox window titles included, for screenshots and streams.
- **Open website** (in each account's ⋯ menu): roblox.com signed in as that account, in a window of Bloxkeep's own. Open several to use different accounts side by side. The sign-in stays in that window only (never in your Chrome or Edge) and is gone when it closes. Signing out there would also sign the account out of Bloxkeep, so Bloxkeep stops it: just close the window.
- **Open roblox.com** (in the menu next to **Add account**): a blank window to sign in on the site itself, one account per window, as many side by side as you like. Its sign-in is gone when the window closes, unless you press **Save to Bloxkeep**; the account is then in your list and opens signed in next time.

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

Manage them in the **Presets** tab: **New preset**, edit, duplicate, reorder, **Run**. While a preset has windows open, its row says how many and **Run** becomes **Close**, which closes those windows (and no others). The presets button in the bottom bar has **Save as preset…** and quick runs. Up to 50 presets.

A preset's **For the Roblox windows** options apply to the windows that preset opens (also when they rejoin). Pressing **Play** for the same accounts the normal way still follows your general settings.

How the levels fit together, for each Roblox window: a preset's option set to **On** or **Off** decides for the windows that preset opened, until they close; left on **Follow…**, the account's own switch decides (auto-rejoin) or else **Settings** (Anti-AFK, arranging windows, the wait between accounts). An account in two presets follows the one that opened its window; running the other preset leaves an open window alone.

- **Anti-AFK**: **Follow Settings**, **On** or **Off**.
- **Auto-rejoin**: **Follow each account**, **On** or **Off**.
- **Performance**: **Light AFK** sets the lowest graphics, 30 FPS, no sound, and minimises the window once it's in the game. Windows opened without it keep your Roblox settings.
- **Restart Roblox**: every 2, 4, 6 or 12 hours, each window is closed and opened again into the same game, one at a time. The window you're using waits.
- **Arrange windows**: **Follow Settings**, **On** or **Off**: tiles the Roblox windows once the preset has opened them.

**Discord**: a preset can send its messages to **A webhook of its own** instead of the channel in Settings: its schedule messages, its summary after a scheduled stop (that preset's accounts only), and the alerts about the windows it opened. **Send test** checks the link before saving. Everything else, and the on/off switch, quiet hours and pings, stay in **Settings › Discord**. If Discord deletes that webhook, the preset goes back to the channel in Settings.

**Schedule**: a preset runs by itself at its **Start time**, once or on chosen weekdays, and can **Stop** after a few hours or at a set time (closing only the windows that run opened). Bloxkeep has to be running and unlocked at that moment.

<img src="docs/screenshots/presets.png" width="800" alt="The Presets tab with three presets: one scheduled, with three windows open and a Close button">

<img src="docs/screenshots/preset-editor.png" width="800" alt="Edit preset: the For the Roblox windows options (Anti-AFK, Auto-rejoin, Performance, Restart Roblox, Arrange windows), its own Discord webhook, and the Schedule">

### Multi-Roblox

Multi-Roblox is **off by default**. Roblox doesn't officially support running several clients; use it at your own risk. Turn it on with the Multi-Roblox switch in the sidebar or in **Settings › Launching**.

- Bloxkeep has to be open **before** Roblox and stay open while you play.
- The light shows **Off**, **On — several windows allowed**, or **Not working yet — click to fix** (yellow). To fix it: close every Roblox, including one in the system tray; if Roblox starts with Windows, click **Turn off autostart** (the same as switching it off in Task Manager › Startup apps, and you can switch it back there). The light then turns green.
- **Details** next to Multi-Roblox has two more options:
  - **Always open Roblox windowed** (on by default): all Roblox windows share one fullscreen setting, so with several open some end up fullscreen and some don't. Bloxkeep opens every window windowed and puts your setting back afterwards.
  - **Tile the windows after opening** (off by default): once the accounts are open, the Roblox windows are laid out in a grid on the main screen.
- One home screen per account: choose the accounts, leave the game box empty, press **Open Roblox**.

How it works: Roblox uses an object named `ROBLOX_singletonEvent` to allow only one window; since its October 2026 builds, a new window uses it to close the older ones. Bloxkeep takes that name first, while no Roblox is running, so Roblox can't set this up and the windows don't close each other. Bloxkeep injects nothing into Roblox and reads nothing from the Roblox process.

If a newer Roblox build starts closing windows when another opens again, **Settings › Roblox version & graphics** shows a yellow warning. It goes away once two or more accounts are in a game at the same time, when Roblox updates, or when you click **Hide**.

Bloxkeep's Roblox watch also checks each new Roblox build as soon as Roblox offers it, often before it reaches players: it downloads the build and compares the lock Multi-Roblox relies on with the build before ([how it works](docs/roblox-watch.md)). **Settings › Roblox version & graphics** shows what it found for your Roblox under **Multi-Roblox check**, and the Multi-Roblox card turns yellow when the lock changed. Opening several accounts then asks first, until two windows have played together on that build. When Roblox only renamed its lock, Bloxkeep holds the new name as well, without an update. The check can't prove Multi-Roblox works; two windows playing together can, and Bloxkeep remembers that for each build.

### Anti-AFK

Roblox kicks an account after 20 minutes without input. With **Settings › Staying in game › Anti-AFK** on (a preset can override it), Bloxkeep brings each Roblox window that is in a game to the front for a moment, moves the mouse a few pixels over it and back, and returns you to the window you were in. No key is pressed and nothing is clicked. It steps in from 12 minutes after a window's last input, at the first minute nobody uses the PC; a window two minutes from its kick goes in a 2-second pause even while you use the PC, but never over a full-screen app. It can't work while the PC is locked. If an account is kicked anyway, Bloxkeep says why.

### Auto-rejoin

Turn it on per account in the ⋯ menu › **Auto-rejoin**, or for every account at once in **Settings › Staying in game › Auto-rejoin** (a preset can override it; see [Presets](#presets)). If Roblox ended an account's sign-in, auto-rejoin turns itself off for it, and back on once the account is signed in again. When that account's Roblox disconnects, gets kicked or loses its connection, Bloxkeep rejoins the same game (or the same private server) after 10 seconds, though not necessarily the same server. If it keeps dropping, from the third time Bloxkeep waits 2, 5, 10, then 15 minutes between tries (the card shows the wait with a **Cancel** button); 10 minutes in the game resets the count. Without a network, it retries every 30 seconds, up to 20 times.

Bloxkeep doesn't rejoin when you leave the game yourself, when the account signed in on another device, when it's banned, or when Roblox itself closed the window because another one opened. Closing the Roblox window by hand counts as leaving, so use **Close** in the app.

### Roblox settings per account

In **Settings › Roblox version & graphics › Roblox settings**, **Apply to** chooses **Nobody**, **All accounts** or **Chosen accounts** (pick them with **Use Roblox settings** in the ⋯ menu). Set **Graphics quality** from 1 to 10, **Frame rate limit** 30/60/144/240, **Volume** from **Muted** to 100 %, or **Leave as is**.

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
| **Roblox (Microsoft Store)** | Being tested: each account opens in a Store Roblox window of its own, skipping Microsoft's launcher. The Microsoft Store keeps it updated |

### Windows, RAM and the game monitor

- The sidebar card **Roblox on this PC** shows the number of Roblox windows, the RAM they use, the free RAM, and **Mini window** and **Close all**. Its version line tells you whether your Roblox is the newest.
- **Game monitor** (a sidebar tab): live pictures of every Roblox window with the account, game, RAM and PID. Click a picture to bring that window to the front; right-click for more. A minimised window has no live picture. Windows draws the pictures itself (like the previews when you hover the taskbar), so they cost next to nothing.
- **Mini window**: a compact window to watch the accounts that are playing without the main window.
- **When a Roblox window opens** (in **Settings › Launching**): **As Roblox sizes it**, **Maximise it** (still a window, so the windows don't fight over Roblox's one fullscreen setting), or **Tile them side by side** (needs Multi-Roblox). A preset can tile its own windows whatever this says.
- **Arrange Roblox windows** (in the bottom bar) and **Arranging windows** (in **Settings › Launching**): **Fill the screen**, or **Small tiles** / **Medium tiles** / **Large tiles** laid out from the top-left, leaving the rest of the screen free. The smaller a Roblox window, the less work for the graphics card.
- **Low RAM warning** (on by default): warns before you open more accounts than the free RAM can take, and when the PC uses more than 92 % of its RAM.
- **Save RAM on background windows** (off by default): when RAM runs low, background windows give RAM back to the one you're playing. Roblox may refuse; then Bloxkeep leaves it.
- **Show account names on windows** (on by default): puts the account's name in the Roblox window title, to tell windows apart on the taskbar.
- **Wait between accounts**: 0 to 60 seconds, 8 by default. Opening too many at once can fail.

### Statistics and activity

The **Statistics** tab counts play time, sessions and disconnects for **Today**, **7 days** or **30 days**, per account and per game. Only games opened with Bloxkeep count. On a wide window it also shows how many accounts played, the longest stay, and play time by preset.

Switch to **Activity** for the log: accounts opened, games joined and left, drops (with the reason), rejoins, scheduled presets starting and stopping, accounts that need to sign in again. Filter by account and kind, and **Export CSV** when you need it. Bloxkeep keeps the last 1,000 events, in the encrypted vault.

<img src="docs/screenshots/stats.png" width="800" alt="The Statistics tab: total play time, a chart per day, play time per account and most played games">

### Tray, notifications, Discord and appearance

- **Keep running in the tray when closed** (on by default): the X button hides Bloxkeep to the tray, and auto-rejoin and Multi-Roblox keep working. To quit, right-click the tray icon and choose **Quit Bloxkeep**. Next time, the Bloxkeep window comes back where you left it, at the same size, maximised or not.
- **Windows notifications** (on by default): only while Bloxkeep is in the background. Turn each kind on or off in **Settings › Notifications**: account dropped or auto-rejoin, scheduled presets, sign-in needs renewing, low RAM, launch problems.
- **Discord** (off by default): paste a channel's webhook link in **Settings › Discord** (switching **Send to Discord** on without one asks for it). **Send automatically** a play-time summary every day, every week or both: a picture of play time by hour (or day) coloured by game with a row per account and its avatar, the totals against the period before, drops with Roblox's codes, the longest stay, the most windows at once, the top games, and a block per preset with its accounts; keep a **Status message** up to date; and send **Alerts** for the events above, with a **Ping** for a person or a role. There are **Quiet hours** and **Customise messages** (sender name and picture, language, message style, **Hide account names in messages**).
- **Settings › Appearance**: **Theme** has 74 themes in five groups (Basic, Scenery, Colourful, Styles, Seasonal) plus **Sync with Windows** (**Dark** by default). Scenery and Seasonal themes have a drawn scene behind glass panels; Styles change the shapes too (Windows 98, macOS with working window buttons, GNOME, Ubuntu, KDE, Game Boy, Notebook…). Also **Language** English / Tiếng Việt, and **Hide account names**.
- **Hardware acceleration** (in **Settings › Staying in game**): draws the interface with the graphics card instead of the CPU; it turns itself off if the graphics driver keeps failing.

<img src="docs/screenshots/discord.png" width="800" alt="The Discord page in Settings: send to Discord, webhook link, play-time summary">

### Bringing accounts over

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
| `%LOCALAPPDATA%\BloxkeepApp\`, or the folder you chose in Setup | The installed program (removed on uninstall) |

**How it's protected.** The vault is encrypted with AES-256-GCM. The key comes from your master password through PBKDF2-HMAC-SHA256 with 600,000 iterations and a random salt. The file header is authenticated too, so a wrong password or a tampered file is detected. The master password is never stored. Every save is written in full before it replaces the old file, and the previous save is kept in `vault.rmv.bak`. When you lock the app, Bloxkeep drops all the decrypted data it was holding (cookies, passwords, authenticator keys).

> [!WARNING]
> If you forget the master password, your saved accounts can't be recovered. A backup also only opens with the password it was saved with.

**What leaves your PC.** Bloxkeep has no telemetry or analytics. Data only goes to (also in the [privacy policy](PRIVACY.md)):

| Where | When, and what |
|---|---|
| Roblox's API hosts (`auth`, `users`, `presence`, `friends`, `games`… `.roblox.com`) | Signing in (the password only goes to `auth.roblox.com`), session and ban checks, status every 30 seconds, game, server, friend and player lookups, join tickets, the newest Roblox version |
| `www.roblox.com` | The browser sign-in window, profiles, and the join link handed to Roblox |
| `*.rbxcdn.com` | Avatars and game pictures |
| `*.robloxapp.vnggames.com` | The same as the Roblox API hosts, for Roblox VNG accounts only |
| Roblox's captcha providers (Arkose Labs, HUMAN Security) | Only in the puzzle window, when Roblox asks for a captcha during password sign-in |
| GitHub | Checking for and downloading updates, with no account data (can be turned off), and reading the [Roblox watch](docs/roblox-watch.md)'s findings |
| Bloxkeep's feedback service (a Cloudflare Worker that forwards to the developer's Discord) | Only when you press **Send** in **Send feedback**, and only what **See exactly what will be sent** shows ([details](docs/feedback.md)) |
| Microsoft (`go.microsoft.com`) | Only when WebView2 has to be installed |
| The backup folder you choose | A copy of the encrypted vault; your OneDrive / Google Drive app may sync it |
| Your Discord webhook (`discord.com`) | Only when **Settings › Discord** is on (off by default): account names (or **Account 1**, **Account 2**… with **Hide account names in messages**), game names, play time, sessions, PID, RAM, and links to Roblox's own pictures of the games and of the accounts' avatars (no avatar when names are hidden). Never cookies, passwords or the webhook link itself; the link is kept in the encrypted vault |

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
- Roblox from the Microsoft Store is still being tested (**Settings › Launching › Open Roblox with**). Bloxkeep's Roblox settings for the windows (windowed, graphics) aren't applied to it yet.
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

If something breaks, tell me from the app: **Settings › Bloxkeep updates › Send feedback** (or `Ctrl+K`, "feedback"). It can add Bloxkeep's error log, with account names, ids, passwords and cookies taken out, and **See exactly what will be sent** shows it before it goes ([how it works](docs/feedback.md)). Or [open an issue](../../issues) with the error.

## FAQ

Every feature is explained step by step in the [user guide](docs/guide.md).

### **Q:** Is this a virus? Why do Windows or antivirus apps warn?

**A:** No. The files aren't code-signed yet (Bloxkeep is applying for free signing through SignPath Foundation), so SmartScreen and some antivirus apps warn about any new file. You can compare the SHA-256 with `SHA256SUMS.txt`, read the source in this repository, or [build it yourself](#build-from-source).

### **Q:** My antivirus deleted Bloxkeep. Should I not use it?

**A:** Check the download first: its SHA-256 must match `SHA256SUMS.txt` on the release page, and it must come from this repository's [Releases](https://github.com/minhhoang0122/bloxkeep/releases). If it matches, restore it from your antivirus's quarantine and report the file as a false positive (for Windows Defender: [microsoft.com/wdsi/filesubmission](https://www.microsoft.com/wdsi/filesubmission)). Never use a "Bloxkeep" from anywhere else.

### **Q:** How do I turn on Multi-Roblox?

**A:** Flip the **Multi-Roblox** switch at the bottom of the sidebar (or **Settings › Launching › Multi-Roblox**) and confirm. Open Bloxkeep before Roblox, and keep it open while you play. The light tells you how it's going:

| Light | Meaning |
|---|---|
| Green | On: you can open several accounts at once |
| Yellow | Not working yet (a Roblox was already open), or a Roblox update may stop it. Click the card to see what to do |
| Grey | Off: each new Roblox window closes the one before |

If you press **Play** for two accounts while it's off, Bloxkeep offers **Turn on Multi-Roblox and play**.

### **Q:** The Multi-Roblox light is yellow.

**A:** A Roblox was open before Bloxkeep. Close every Roblox, including one in the system tray. If Roblox starts with Windows, click **Turn off autostart** in the help dialog. The light turns green. Next time, open Bloxkeep before Roblox. If the card says a Roblox update may stop it, keep playing; Bloxkeep tells you if Roblox starts closing windows, and an update of Bloxkeep is the fix.

### **Q:** I can't open several accounts: each new window closes the last one.

**A:** Multi-Roblox is off or not working yet: see the two answers above. If the light is green and windows still close, Roblox has changed how it blocks several windows. Look for a Bloxkeep update (**Settings › Bloxkeep updates › Check now**) and [send feedback](#q-you-should-add-a-feature--i-found-a-bug).

### **Q:** What do the coloured dots on an account mean?

**A:** The dot on the picture shows where the account is: **green** in a game, **blue** online on the website or app, **amber** in Roblox Studio, **yellow** disconnected while its window is still open, **grey** offline. A **red ring** means it needs to sign in again, or is banned. The **Needs sign-in** and **Banned** filters above the list show just those.

### **Q:** Adding an account doesn't work.

**A:** Try another way under **Add account**: **Sign in with your phone** (a code or QR, no password or puzzle; best when the account is signed in on your phone), **Sign in with a browser**, **Open roblox.com** (sign in on the site, then **Save to Bloxkeep**), or **Paste a cookie**. If the password sign-in keeps asking for a puzzle, the phone is the easy way round.

### **Q:** Copy password says no password is saved.

**A:** No way of signing in keeps the password: the phone, browser and cookie methods never see it, and the password sign-in sends it to Roblox and forgets it. To have it there, type it under **Edit (F2) › Roblox password**. The same goes for **Copy 2FA code**: paste the account's authenticator key under **Edit**.

### **Q:** Can I join private (VIP) servers?

**A:** Yes. Paste the private server link into the game box and press **Play**; Bloxkeep opens that server for every account you selected. A **Job ID** puts them in one particular server, and **Join a player** (next to the box) finds the server a player is in.

### **Q:** Some Roblox windows open fullscreen and some don't.

**A:** All Roblox windows share one fullscreen setting, so with several open they easily end up different. Keep **Always open Roblox windowed** on in Multi-Roblox's **Details** (it's on by default): every window opens windowed, and your setting is put back afterwards. To have each one fill the screen, choose **Maximise it** under **When a Roblox window opens**; to lay them out side by side, **Tile them side by side** or **Arranging windows**.

### **Q:** Do my Bloxstrap / Fishstrap mods still work?

**A:** Yes, if you choose **Bloxstrap** or **Fishstrap** in **Open Roblox with**. If the launcher blocks several windows, choose **RobloxPlayerBeta directly (skip launchers)**; Roblox then opens without the launcher.

### **Q:** Can I use Roblox from the Microsoft Store?

**A:** Yes, while it's being tested: choose **Roblox (Microsoft Store)** under **Settings › Launching › Open Roblox with**. Microsoft's own launcher lets only one Store Roblox run, so Bloxkeep starts the Store's Roblox itself, with each account's own sign-in, in a window of its own. Updates still come from the Microsoft Store. If it doesn't work for you, Roblox from roblox.com works the same way, with Bloxstrap or Fishstrap if you like.

### **Q:** I left the game but it still says "playing".

**A:** Roblox's status lags, sometimes by a few minutes. For windows on this PC Bloxkeep ignores the stale report; status from other devices can still lag.

### **Q:** Opening a game fails with 403 "An error was encountered during authentication".

**A:** Usually a per-app VPN: Roblox goes through it and Bloxkeep doesn't. A join ticket is tied to the IP address that asked for it, so let Bloxkeep take the same route as Roblox.

### **Q:** Can I use it in Vietnam without access to roblox.com?

**A:** Partly. Password sign-in and phone sign-in go through Roblox's API hosts, not www.roblox.com, and the captcha page is served by the app itself. The browser method needs www.roblox.com. Bloxkeep has no VPN or region bypass. Roblox VNG accounts can be added, but games can't be opened for them yet.

### **Q:** How do I back up my accounts?

**A:** Turn on **Settings › Backup & data › Back up automatically** and pick a folder, ideally one that OneDrive or Google Drive syncs. Each backup is encrypted with your master password. See [Backup and restore](#backup-and-restore).

### **Q:** How do I move to a new PC?

**A:** Turn on backups to a OneDrive or Google Drive folder on the old PC. On the new one, install Bloxkeep, click **Restore from a backup** and enter the password the backup was saved with.

### **Q:** I forgot my master password.

**A:** It can't be recovered; it isn't stored anywhere. You can delete `vault.rmv` and `vault.rmv.bak` in `%LOCALAPPDATA%\Bloxkeep` (losing every saved account) and start over, or restore a backup whose password you remember.

### **Q:** Does Bloxkeep send my password or cookies anywhere?

**A:** Only to Roblox. A password you sign in with only goes to Roblox's sign-in server, and each account's cookie is only sent to Roblox's own hosts (Roblox VNG's, for VNG accounts). Update checks to GitHub carry no account data. The full list is under [Data and security](#data-and-security).

### **Q:** Can I use it on Mac or Linux?

**A:** No. Bloxkeep runs on Windows 10 and 11 (64-bit) only.

### **Q:** Will my account get banned?

**A:** No one can promise that. Roblox doesn't officially support running several clients, so using Multi-Roblox is at your own risk. Bloxkeep signs in and opens games the same way roblox.com does, and doesn't touch the game's memory. Some games forbid alt accounts; check the rules of the game you play.

### **Q:** You should add (a feature) / I found a bug.

**A:** Tell us from the app: **Settings › Bloxkeep updates › Send feedback** (or Ctrl+K). A bug report can include Bloxkeep's error log, with account names, passwords and cookies taken out, and you see exactly what is sent. You can also open an [issue](https://github.com/minhhoang0122/bloxkeep/issues).

### **Q:** How do I remove Bloxkeep completely?

**A:** Uninstall it in Windows **Settings › Apps** and tick **Also delete my accounts and settings**. Backups in your backup folder stay; delete them yourself. For the portable copy, delete the app's folder (with its `data`).

## Build from source

You need Windows and the **.NET 10 SDK**. The app is a C# core (.NET 10, WinForms) with an HTML/CSS/JS interface in WebView2. The [Velopack](https://velopack.io) packager (`vpk`) is pinned in `dotnet-tools.json` and the scripts fetch it.

```bat
build.bat 1.0.1
```

It runs the checks in `Tests\Bloxkeep.Tests` first and stops if one fails (run them alone with `dotnet run --project Tests\Bloxkeep.Tests`; add `--online` for the ones that ask Roblox's servers). Each runs in its own process with throwaway data, never yours. The output goes to `dist\releases\`: `Bloxkeep-Setup.exe` (the web installer, copied as it is from `Installer\Web`), `Bloxkeep-Setup-Offline.exe`, `Bloxkeep-Portable.zip`, the update packages and `SHA256SUMS.txt`. The web installer is rebuilt only on purpose, with `Installer\Web\make.bat`: a new build is a new file to SmartScreen. A second argument is the GitHub repository the build looks for updates in (this one by default); pass another when you build a fork.

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
Installer/                 the installer's splash image; Setup/ the installer and uninstaller windows
dev/                       interface preview with made-up data (not packaged)
docs/screenshots/          screenshots (vi/: the Vietnamese ones)
```

## Code signing policy

Releases are built from this repository by the [Release workflow](.github/workflows/release.yml) on GitHub's own Windows machines. Bloxkeep is applying to SignPath Foundation for free code signing; what gets signed, how, and who approves it is in the [code signing policy](CODE_SIGNING_POLICY.md). What the app sends and where is in the [privacy policy](PRIVACY.md).

## License

[MIT](LICENSE) © 2026 minhhoang0122

*Bloxkeep is an independent project. It is not affiliated with, endorsed or sponsored by Roblox Corporation. Roblox is a trademark of Roblox Corporation.*
