[Tiếng Việt](troubleshooting.vi.md)

# Troubleshooting and FAQ

Short answers to the problems people meet most, then every keyboard shortcut.

## Roblox and sign-in

### **Q:** Roblox closes my other windows

**A:** Check these in order:

- Is [Multi-Roblox](multi-roblox.md) on? If the sidebar card says **Off**, turn it on.
- Yellow **Not working yet — click to fix**: Roblox was open before Bloxkeep. Click it and follow the steps (close every Roblox, including the one in the tray; turn off autostart if offered). Open Bloxkeep before Roblox next time.
- Yellow **Roblox closed windows** or **A Roblox update may stop it**: a Roblox update may have changed how it blocks several windows. Open **Settings › Roblox version & graphics**, press **Check for a Bloxkeep update**, and play one account at a time until an update fixes it.
- Using a launcher? If it blocks several windows, choose **RobloxPlayerBeta directly (skip launchers)** in **Settings › Launching › Open Roblox with**.

### **Q:** A Roblox window crashes as it opens ("Memory dump captured", or Bloxstrap's "Roblox failed to launch")

**A:** Windows' virtual memory is full, even if RAM is free: each Roblox window reserves about 2.5 GB of it. Bloxkeep warns before **Play** when this is about to happen.

- Close apps you don't need, or
- Increase virtual memory. It can go on any drive, so drive C needs no space: **Win+R** › `sysdm.cpl` › **Advanced › Performance › Settings › Advanced › Virtual memory › Change**. Untick **Automatically manage**, pick a drive with room, set **Custom size** 16384 / 32768 MB, press **Set**, **OK**, and restart the PC.

### **Q:** An account says "Needs sign-in"

**A:** Roblox ended its saved sign-in (a password change, "log out of all sessions", a security check). Press **Sign in** on the card and confirm with your phone, or add the account again any way: it is updated, not copied. The **Needs sign-in** filter shows them all, and **Check sign-ins every day** finds them before you press Play.

### **Q:** An account is banned

**A:** The **Banned** badge comes from Roblox's public information, read during sign-in checks. **Play** and presets skip the account, and auto-rejoin doesn't reopen it. Once the ban is over, **⋯ › Check sign-in** clears the badge.

### **Q:** A game banned one of my accounts. Can my other accounts still play it?

**A:** Careful: a game bans the person, not only the account. Roblox's ban tools also ban the accounts they take for that person's other ones, "for using an account to avoid a ban". So when you press **Play** for other accounts into a game one of your accounts is banned from, Bloxkeep asks first and names both. A preset opens them, but says so in its result. If a game banned you by mistake, appeal to that game (most use their Discord server): only the game can lift its ban.

### **Q:** Roblox isn't installed, or only the Microsoft Store version is

**A:**

- "Roblox doesn't seem to be installed on this PC." in the sidebar: install Roblox from roblox.com/download.
- Only Roblox from the Microsoft Store: choose **Roblox (Microsoft Store) — being tested** in **Settings › Launching › Open Roblox with**, or install Roblox from roblox.com.
- A launcher that is missing shows "— not installed" in that list. Pick another option.

### **Q:** "Copy password" is greyed out, or says nothing is saved

**A:** Greyed out means several accounts are selected: copying works on one account at a time. "No password is saved for this account" means Bloxkeep never keeps the password you sign in with: type it under **Edit (F2) › Roblox password** and save. **Copy 2FA code** works the same way, with **Edit › Authenticator key**. See [Accounts](accounts.md).

### **Q:** Why is something else greyed out?

**A:** Look for it here:

| Greyed out | Why |
|---|---|
| **Play** in the game bar ("Select accounts first") | No account is selected. |
| **Play** on a card | It is launching. |
| **Open Roblox** | No account is selected. |
| **Browse servers** | No game is in the game box yet, or it holds a private server. |
| **Close** in the game bar, **Close Roblox**, **Force close** | None of the selected accounts is open on this PC. |
| **Edit…**, **Join this server**, **Roblox profile**, **Open website**, the **Copy** items | Several accounts are selected. **Join this server** also needs a known server; **Open website** isn't available for Roblox VNG. |
| **Run** on a preset | It has no accounts left, or another launch is running. |
| **Graphics quality**, **Frame rate limit**, **Volume** | **Apply to** is **Nobody**. |
| The kinds under **Which events** | **Windows notifications** is off. |
| **Back up now** | No backup folder chosen yet. |
| **Ask again** | You haven't skipped any question. |
| **Update automatically**, **Get beta versions** | This copy can't update itself. |

## Data and passwords

### **Q:** Where is my data?

**A:** **Settings › Backup & data › Data location › Open folder** shows it. For the installed app it is `%LOCALAPPDATA%\Bloxkeep`; the portable copy uses a `data` folder next to `Bloxkeep.exe`. See [Backup and security](backup-and-security.md).

### **Q:** I forgot my master password

**A:** It can't be recovered, and there's no need to reinstall. Right under **Unlock** are two ways on:

- **Restore from a backup**: open a backup whose password you remember. The vault you're locked out of is kept beside it.
- **Create a new vault**: a vault of its own with a new master password. Your Roblox accounts are fine: sign in to add them again. The old vault stays on the list, so pick it again if you remember its password, or take it off with **×** in the picker (its file is kept in the data folder).

## Schedules and status

### **Q:** A scheduled preset didn't run

**A:** Bloxkeep must be running and unlocked at the start time. Runs missed while it was closed, locked or the PC slept aren't made up. A run is also skipped when opening would need a confirmation (Multi-Roblox off or not ready, low RAM, an account in another game); the notification says why. See [Presets](presets.md).

### **Q:** It says "playing" but I left

**A:** Roblox's status lags, sometimes by a few minutes. For windows on this PC, Bloxkeep follows Roblox's own log instead.

## Feedback

### **Q:** How do I send feedback, with the error log?

**A:** Go to **Settings › Bloxkeep updates › Send feedback › Write** (or Ctrl+K › "feedback"):

1. **What is it?**: **Something doesn't work** or **An idea**.
2. **Message**, and **How to reach you** (optional) if you'd like an answer.
3. **Add Bloxkeep's error log** is ticked for a bug. Account names, cookies and passwords are taken out first. Bloxkeep must be unlocked for this.
4. **See exactly what will be sent** shows everything before it goes.
5. **Send**. You can send one message a minute.

It goes to Bloxkeep's developer through Discord, with nothing else from your PC ([details](../feedback.md)).

## Keyboard shortcuts

| Where | Keys | What they do |
|---|---|---|
| Anywhere | Ctrl+K | Quick search: accounts, presets, games, Settings pages and actions. Press again to close. Not on the lock screen or over another dialog. |
| Anywhere | Ctrl+F | The search box of the tab you're on (Accounts, Presets once there is one, Settings). From other tabs, the accounts search. |
| Anywhere | Ctrl+L or `/` | The game box. |
| Anywhere | Esc | Leaves a text box; otherwise clears the selection. Closes a menu, a list or a dialog. |
| Accounts | Click / Shift+click | Select or unselect a card / select a range. |
| Accounts | Double-click | Play that account alone. |
| Accounts | Right-click | The account menu. |
| Accounts | Arrow keys, Home, End | Move between cards. |
| Accounts | Space | Select the focused card. |
| Accounts | Ctrl+A | Select all accounts shown. |
| Accounts | Enter | Play the selected accounts. |
| Accounts | F2 | Edit the selected account (exactly one). |
| Accounts | Delete | Delete the selected accounts (asks first). |
| Game box, Job ID box | ↓ / ↑ | Move through the game list. |
| Game box, Job ID box | Enter | Pick the highlighted game; with a link or Place ID in the box, play. In the Job ID box, play. |
| Game box, Job ID box | Esc | Close the game list. |
| Quick search | ↓ / ↑, Page Down / Page Up | Move through the results. |
| Quick search | Enter | Open the highlighted entry (an account is selected and shown; a preset runs; a game goes into the game box). |
| Quick search | Ctrl+Enter / Shift+Enter | On an account: **Play** / **Edit**. |
| Menus and dialogs | ↓ / ↑ | Move through a menu. Tab or Esc closes it. |
| Menus and dialogs | Enter | In a form, presses its main button. |
| Menus and dialogs | Esc | Closes the dialog (asks first if you typed something). |
| Menus and dialogs | Tab | Moves around inside the dialog. |
| Settings page list | ↓ / ↑, Home, End | Move between pages (← / → too in a narrow window). |
| Theme picker | Arrow keys, Home, End | Try themes. |
| Settings search | Esc | Clear it. |
| Mini window | Esc | Hide it. |
| Bloxkeep browser window | Ctrl+T / Ctrl+W / Ctrl+Tab | New blank tab / close the tab / switch tabs. |

Accounts shortcuts work when you aren't typing in a box.

← [User guide](../guide.md)
