# Bloxkeep user guide

English | [Tiếng Việt](guide.vi.md)

This guide covers every part of Bloxkeep: where each feature is, what it does, and when it is greyed out or doesn't apply. Menu paths are written like **Settings › Launching › Open Roblox with**. "⋯" is the three-dot **More** button.

## Contents

1. [Getting started](#getting-started)
2. [Adding accounts](#adding-accounts)
3. [The account list](#the-account-list)
4. [The account menu](#the-account-menu)
5. [Playing](#playing)
6. [Multi-Roblox](#multi-roblox)
7. [Presets and schedules](#presets-and-schedules)
8. [Keeping accounts in game](#keeping-accounts-in-game)
9. [Game monitor, windows and statistics](#game-monitor-windows-and-statistics)
10. [Sending to Discord](#sending-to-discord)
11. [Settings, page by page](#settings-page-by-page)
    - [Launching](#launching) · [Roblox version & graphics](#roblox-version--graphics) · [Staying in game](#staying-in-game) · [Notifications](#notifications) · [Discord](#discord) · [Appearance](#appearance) · [Security](#security) · [Backup & data](#backup--data) · [Bloxkeep updates](#bloxkeep-updates)
12. [Keyboard shortcuts](#keyboard-shortcuts)
13. [Troubleshooting and FAQ](#troubleshooting-and-faq)
14. [Privacy](#privacy)

---

## Getting started

### Install

Bloxkeep runs on Windows 10 or 11, 64-bit. Download it only from the [Releases page](https://github.com/minhhoang0122/bloxkeep/releases/latest): a copy from anywhere else may have been changed to steal accounts. There is a normal installer, an offline installer and a portable zip; the [README](../README.md#download-and-install) says which to pick. The files aren't code-signed yet, so Windows SmartScreen warns the first time: click **More info**, then **Run anyway**.

### The master password

The first time Bloxkeep opens, it asks you to **Set a master password**. It encrypts everything you save in Bloxkeep.

- At least 6 characters, and not only spaces. The **Weak / OK / Strong** bar is only a guide.
- It **can't be recovered**. Bloxkeep never stores it, so if you forget it, your saved accounts are gone. The only way back is a backup whose password you remember.
- Turn on backups early: **Settings › Backup & data › Back up to a folder** (see [Backup & data](#backup--data)). A backup opens with the password it was saved with.
- Moving to a new PC? On the first screen choose **Restore from a backup** instead of setting a new password.
- To change it later: **Settings › Security**.

The EN / VI switch in the top corner of this screen changes the language (also **Settings › Appearance › Language**).

### Unlocking and locking

Each time Bloxkeep starts it shows **Welcome back**: type the master password and press **Unlock**. A warning shows while Caps Lock is on.

**Lock app** is at the bottom of the sidebar. Locking:

- keeps every Roblox window open;
- pauses, until you unlock: auto-rejoin, scheduled presets and Discord messages;
- stops an account launch that is under way, closes the Bloxkeep browser window (see [Open website](#open-website)), and clears a password or 2FA code you copied from the clipboard;
- drops all decrypted data from memory.

When something would pause (auto-rejoin on for open windows, a scheduled preset, Discord), Bloxkeep asks **Lock Bloxkeep?** and lists it, for example "Auto-rejoin for 3 accounts" or "2 scheduled presets". Tick **Don't ask again** to skip this question.

While locked:

- A scheduled start doesn't run. After you unlock, Bloxkeep tells you, once: "*Preset* didn't run at *time*: Bloxkeep was locked." It isn't run late.
- A scheduled stop still closes its windows on time.
- The lock screen reminds you that Roblox windows are still open.

---

## Adding accounts

Click **Add account** (top right of **Accounts**) to open **Sign in to Roblox**. The arrow next to it, **Other ways to add**, has: **Sign in with your phone**, **Sign in with a browser**, **Paste a cookie**, **Import from another Bloxkeep vault file…** and **Open roblox.com**. With no accounts yet, the empty list shows the same choices.

| Way | Use it when | What it needs |
|---|---|---|
| **Sign in with your phone** (Recommended) | Almost always. No password, no puzzle. | The account already signed in to the Roblox app on a phone. |
| **Use username & password** | You have no phone at hand. | Roblox may ask for a puzzle or a 2-step code. |
| **Sign in with a browser** | The other ways fail or Roblox wants an extra check. | www.roblox.com reachable. Not for Roblox VNG. |
| **Paste a cookie** | You already have the account's cookie. | The `.ROBLOSECURITY` cookie. |
| **Open roblox.com**, then **Save to Bloxkeep** | You mostly want the website, several accounts side by side. | Nothing else. |
| **Import from: Bloxkeep file** | Moving accounts from another PC or a backup. | That file's master password. |

Adding an account that is already in the list never makes a copy: its sign-in is updated ("*name* was already here — its session was updated.").

### Sign in with your phone

1. Open the Roblox app on your phone and sign in to the account.
2. Go to **Settings → Quick Log In**.
3. Enter the code or scan the QR, then confirm.

The dialog shows the code, a QR and "Waiting for you to confirm · expires in *m:ss*".

- When the phone has entered the code: "*name* entered the code. Confirm on that device." Confirm on the phone.
- After each account, a new code appears for the next one ("Added: … Use the new code for the next account."). Press **Done** when finished.
- The code renews itself a few times. After that it says "The code expired": press **New code**. "Declined on the device." also needs **New code**.
- The **Roblox** / **Roblox VNG** tabs choose the realm. For Roblox VNG, confirm in the Roblox VNG app.
- **Sign in with a browser** at the bottom switches to the browser way (not on the Roblox VNG tab).

The **Sign in** button on a signed-out account opens this same dialog, with "Signing in again to *name*." at the top.

### Username and password

In **Sign in to Roblox**, fill **Username** and **Password** and press **Sign in**.

- The password goes only to Roblox. **Bloxkeep never stores it.** That is why **Copy password** has nothing to copy after a sign-in (see [Copy password](#copy-password-and-saving-a-password)).
- **2-step verification**: type the 6-digit code. The tabs (**Authenticator app**, **Email**, **Text message**) are the ways the account has. For email and text, Roblox sends the code itself; **Send a code** / **Resend code** sends another (Resend waits 30 seconds). If the account's authenticator key is saved in Bloxkeep, the code is filled in for you ("Filled in from the saved authenticator key."): just press **Verify**. After too many wrong codes, **Verify** counts down before you can try again.
- **Quick check** (a puzzle): press **Solve the puzzle** and solve it in the small window. Roblox may ask for a second one.
- **Almost done**: Roblox let you in but the account couldn't be loaded. Press **Try again**; the password isn't sent again.
- **Back** returns to the first screen.
- You can add several accounts in a row: after one is added, the form is empty for the next.

Roblox sometimes refuses this way (the account is locked for protection, it uses a passkey or security key, or Roblox blocks the sign-in). The message then tells you to sign in with your phone or the browser.

### Sign in with a browser

A **Sign in to Roblox** window opens the official roblox.com sign-in page. Sign in; Bloxkeep saves the account and opens a fresh page for the next one. Press **Done** when finished. Bloxkeep never reads the password you type there. It needs www.roblox.com and Microsoft Edge WebView2 (the installer adds it). It isn't offered for Roblox VNG.

### Paste a cookie

A cookie is a small piece of data roblox.com keeps in your browser to remember that you're signed in. The `.ROBLOSECURITY` cookie is the full key to an account: anyone who has it can use the account. Never share it.

**Add account › Other ways to add › Paste a cookie** opens **Add account with cookie**:

- **Cookie .ROBLOSECURITY** (required);
- **Nickname**, **Group**, **Roblox password** (optional, for your reference), **Note**.

Press **Add account**. An authenticator key can be added afterwards, under **Edit**.

### Open roblox.com and Save to Bloxkeep

**Add account › Other ways to add › Open roblox.com** (or **Only need the website? Open roblox.com** on the empty list) opens a blank tab in the Bloxkeep browser window, on roblox.com's sign-in page.

- Each tab is a separate account with its own private sign-in, kept in memory only. Nothing goes into your own Chrome or Edge.
- Sign in on the site, then press **Save to Bloxkeep**: the account joins your list (or its saved sign-in is renewed) and opens signed in next time.
- Without saving, closing the tab signs it out there.
- The **+** at the end of the tab strip opens another blank tab.

### Import from another Bloxkeep file

Use this to bring accounts from `vault.rmv` on another PC, or from a `Bloxkeep-backup-…` file. It is in three places: **Add account › Other ways to add › Import from another Bloxkeep vault file…**, **Import from: Bloxkeep file** in **Sign in to Roblox**, and **Settings › Backup & data › Import accounts from a vault file › Import…**.

1. **Choose file…** and pick the `.rmv` file.
2. Type **Master password of this file** and press **Open**.
3. Tick the accounts to bring over. Each shows **New** or **Already here**, and whether it is signed in, needs sign-in, wasn't checked, or has a password saved. **All** / **None** tick everything or nothing.
4. Optionally tick **Also import presets (N)**.
5. Press **Import**.

Good to know:

- The file itself isn't changed. Imported accounts are locked with this Bloxkeep's master password.
- For an account already here, only a sign-in that no longer works is replaced. Your names, groups and notes stay.
- Imported presets come with their schedules switched off, and accounts that aren't here are left out of them. You can keep up to 50 presets.
- After three wrong passwords, **Open** counts down before the next try. You can't import the vault you are using.
- To replace everything instead (a new PC), use **Restore** (see [Backup & data](#backup--data)).

### Accounts from other account managers

Bloxkeep has no importer for other account managers' files. Add those accounts with one of the ways above. If the other tool shows an account's cookie, **Paste a cookie** works.

### Roblox VNG accounts

Add them on the **Roblox VNG** tab of **Sign in to Roblox** (username and password) or of **Sign in with your phone** (confirm in the Roblox VNG app). There is no browser sign-in for them. They carry a **VNG** tag ("Roblox VNG account (Vietnam)").

Limits:

- Bloxkeep can't open games for them yet. Their **Play** button is greyed out ("Opening Roblox VNG isn't supported yet.").
- When you play several accounts at once, VNG accounts are left out and the result says so.
- **Open website** is greyed out for them.

---

## The account list

### The top of the page

Under **Accounts** you see "*N* accounts · *P* playing · *O* online" and a **Select all** link. Once you select accounts it reads "Selected *N* — **Clear**". If the search or a filter hides some selected accounts, it says how many ("(2 hidden by the search or filter)"): **Play** and **Delete** still act on them.

To the right: the search box, **Refresh online status** (circle arrow), **⋯ More**, and **Add account** with its arrow.

**⋯ More** menu:

| Item | Greyed out when |
|---|---|
| **Check sign-in of all accounts** | No accounts, or a check is already running. |
| **Select all** (Ctrl+A) | No accounts. |
| **Arrange Roblox windows** | Never. Says "No Roblox windows to arrange." if there are none. |
| **Launch order…** | Fewer than 2 accounts. |
| **Close all Roblox** | Never. Asks first. |
| **Force close all Roblox** | Never. Asks first. |
| **User guide** | Never. Opens this page. |

### What a card shows

The badge in the card's corner:

| Badge | Meaning |
|---|---|
| **On this PC** | Roblox is open for this account on this PC. Point at it to see the PID. |
| **Launching** | Bloxkeep is opening Roblox for it. |
| **Banned** | Roblox reports the account as banned. **Play** and presets skip it. |
| **Needs sign-in** | Roblox ended the saved sign-in. Press **Sign in**. |

The status box in the middle:

| Status | Meaning |
|---|---|
| **Opening Roblox…** | Waiting for the window, then the game's name. |
| **Rejoining · this PC** | Auto-rejoin is waiting its turn: "In 2 min", "In a moment". **Cancel** stops it. |
| **Restarting · this PC** | A preset's planned restart is about to open it again. |
| **Playing** / **Joining** | In a game (or joining one) on this PC, with the game's name. |
| **Disconnected** | The window is still open but out of the server. The reason: **Kicked for being idle**, **Kicked by the game**, **Signed in on another device**, **Connection lost**, **Couldn't join the server**, or **Code** *N*. "Roblox is still open" when Roblox gave no reason. |
| **Roblox open** · Not in a game | Roblox is on its home screen. |
| **Signed out** | Roblox is open without this account (signed out inside the Roblox window). |
| **Playing · another device** | In a game on another device. "Game not visible (privacy settings)" when its privacy hides the game. |
| **Online** | On the Roblox website or app, not in a game. |
| **Roblox Studio** | Building a game. |
| **Offline** | "Last online …", or "Not active". |
| **Needs sign-in** | "Sign in again to keep using it." |
| **Status unknown** | Not loaded yet ("Updating…"). |

While an account is open on this PC, the card also shows its **RAM in use**, **Play time** and **PID** (click the PID to copy it). Below that: a **VNG** tag, the account's group, a loop icon when **Auto-rejoin is on**, and when it was last opened from Bloxkeep ("Opened 5 min ago" or "Never opened"). The note, if any, is the last line.

The buttons at the bottom:

- **Play**, or **Close** while it is open or opening, **Cancel** while a rejoin waits, **Sign in** when it needs sign-in. **Play** is greyed out for a Roblox VNG account and while it is launching.
- **Edit (F2)**.
- **⋯ More** (see [The account menu](#the-account-menu)).

Status refreshes every 30 seconds; **Refresh online status** does it now. Roblox's own status can lag a few minutes for other devices.

### Selecting accounts

- **Click** a card to select it; click again to unselect it. Each click adds to the selection.
- **Shift+click** selects everything between the last card you clicked and this one.
- **Ctrl+A** or **Select all** selects every account shown (only those the search and filters show).
- **Esc** or **Clear** clears the selection.
- **Double-click** a card to play that account alone.
- With the keyboard: the arrow keys, Home and End move between cards; **Space** selects the focused card.

### Search, filters, groups, sort and view

- **Search** (Ctrl+F): names, usernames (typing `@name` works), display names, nicknames, groups, notes, the game being played, and user IDs. Accents don't matter.
- **Filter chips**, each with a count: **All**, **Needs sign-in** and **Banned** (only when some accounts are), **Playing**, **Online**, **Offline**, **On this PC** (only while some are open here).
- **Group** menu ("Group: All") appears once an account has a group. Set an account's group under **Edit › Group**.
- **Sort** (with 2 accounts or more): **Name**, **Status**, **Last played**, **Launch order**.
- **Cards** / **List**: the two buttons switch the layout. The list has the same buttons and menu as the cards.
- Your layout and sort are remembered.
- When nothing matches: **No matching accounts**, with **Clear the search** or **Show all accounts**.

### Hiding account names

**Settings › Appearance › Hide account names** shows "Account 1", "Account 2"… (each account's place in the list) everywhere instead of names, Roblox window titles included, for screenshots and streams. @usernames disappear, **Edit** hides the user ID, and the **Nickname** box can't be edited meanwhile ("Hidden while account names are hidden"). The search still finds accounts by their real names. Discord messages have their own switch (see [Customise messages](#customise-messages)).

---

## The account menu

Open it with an account's **⋯ More** button, or right-click the account. If that account isn't selected, it becomes the only selection. If it is part of a selection, the menu acts on every selected account.

| Item | What it does | Greyed out or missing when |
|---|---|---|
| **Play** / **Play (N accounts)** (Enter) | Opens the game in the game box (the Roblox home screen if the box is empty), in [launch order](#launch-order). | — |
| **Open Roblox (no game)** | Opens Roblox on its home screen, one window per account. | — |
| **Close Roblox** | Closes the Roblox window normally. | None of the selected accounts is open on this PC. |
| **Force close** | Ends Roblox at once, for a frozen window. Anything not saved in the game is lost. | Same as Close Roblox. |
| **Edit…** (F2) | Nickname, group, password, authenticator key, note. | Several accounts selected. |
| **Check sign-in** | Checks the saved sign-in and whether Roblox reports a ban. The result says how many need to sign in again, with a **Show** button that jumps to **Needs sign-in**. | — |
| **Join this server** | Puts this account's game and server (Job ID) in the game bar. Then select other accounts and press **Play**: they land in the same server. | Several selected, or the server isn't known (not in a game, or its privacy settings hide it). |
| **Auto-rejoin** | Turns auto-rejoin on or off for these accounts. The tick shows the current state. | — (when a preset decides for an open window, the item says so) |
| **Use Roblox settings** | Includes these accounts in **Roblox settings** (graphics, FPS, volume). | Only shown when **Settings › Roblox version & graphics › Apply to** is **Chosen accounts**. |
| **Roblox profile** | Opens the account's profile on roblox.com in your browser. | Several selected, or no user ID. |
| **Open website** | Opens roblox.com signed in as this account, in a tab of the Bloxkeep browser window. | Several selected, or a Roblox VNG account. |
| **Copy username** | Copies the username. | Several selected. |
| **Copy password** | Copies the saved password. | Several selected. With no password saved it says why instead (below). |
| **Copy 2FA code** | Copies the current 6-digit code. | Several selected. With no key saved it says why instead (below). |
| **Delete** / **Delete N accounts** (Del) | Removes the accounts from Bloxkeep after a confirmation. This can't be undone. | — |

### Copy password and saving a password

Bloxkeep never keeps a password from signing in, whichever way you signed in. So, until you save one yourself, **Copy password** shows: "No password is saved for this account: signing in never keeps it. To have it here, type it under Edit (F2)." The menu item's tooltip says the same.

To save a password:

1. Select the account and press **F2** (or the pencil button, or **Edit…** in the menu).
2. Type it in **Roblox password** (optional, for your reference).
3. Press **Save**.

The box then shows "•••••••• (saved)". The eye button shows it; typing a new one replaces it. It is kept in the encrypted vault and is only there for you: Bloxkeep doesn't use it to sign in.

A copied password is cleared from the clipboard after 30 seconds and stays out of Windows clipboard history. Never share it.

### Copy 2FA code and the authenticator key

A 2FA code is the 6-digit code an authenticator app makes for 2-step verification; it changes every 30 seconds. Bloxkeep can make the same codes if you save the account's authenticator key:

1. **Edit (F2) › Authenticator key** (optional).
2. Paste the key (letters A–Z and digits 2–7) or the `otpauth://` link.
3. Bloxkeep shows "Code now: … Check that your authenticator app shows the same." If they match, press **Save**.

From then on, **Edit** shows the running code with the seconds left, **Copy** and **Remove** (the key goes when you save; **Undo** brings it back). **Copy 2FA code** in the menu copies it ("2FA code copied. It changes in *N* s."), and a password sign-in fills it in for you.

Without a key, **Copy 2FA code** shows: "No authenticator key is saved for this account. Add it under Edit (F2) to copy its 2-step codes here."

Keeping the key in Bloxkeep means anyone with your master password can make codes. Only save it if that is fine with you. Bloxkeep only makes time-based codes, the kind Roblox uses.

### Edit (F2)

**Edit account** shows the avatar, name, @username and user ID, then: **Nickname**, **Group** (with your existing groups as suggestions), **Roblox password**, **Authenticator key** and **Note**. Press **Save**.

There is no cookie box here. To renew a sign-in, press **Sign in** on the account, or add the same account again any way: it is updated, not copied.

If you typed something and press Esc or ✕, Bloxkeep asks **Discard changes?** (you can tick **Don't ask again**).

### Open website

**Open website** opens roblox.com signed in as the account, in a tab of the Bloxkeep browser window.

- Each account has its own tab with its own private sign-in, kept in memory only. Nothing goes into your own browser.
- Opening an account that already has a tab switches to that tab.
- The **+** at the end of the tab strip opens a blank tab to sign in on roblox.com, then **Save to Bloxkeep**.
- Each tab has its own Back, Forward and Reload.
- Close a tab with its ✕, a middle-click, or Ctrl+W. Ctrl+T opens a blank tab, Ctrl+Tab switches tabs.
- Signing out in a saved account's tab would also sign it out of Bloxkeep, so Bloxkeep stops it and explains: choose **Close tab** or **Keep browsing**.
- Closing the window closes all its tabs. Locking Bloxkeep closes it too.

### Delete

**Delete** asks first and lists the accounts: "This can't be undone." This question is always asked. Statistics keep a deleted account's play time, shown as "Deleted account".

---

## Playing

### The game bar

The bar at the bottom of **Accounts** holds, from left to right: the game picture (**Choose a game**), the game box, the **Job ID (optional)** box, the **Join a player…** button, then **Arrange**, **Open Roblox without joining a game**, **Close**, **Presets** and **Play**.

**The game box** ("Search a game, or paste a link / Place ID…", Ctrl+L or `/`) takes:

- a game's name: pick it from the list that appears;
- a Place ID;
- a `roblox.com/games/…` link, including private server links;
- a private server share link (`roblox.com/share?…type=Server`);
- a `roblox://` link.

A name you typed but didn't pick from the list doesn't count: "Pick the game from the search list, or paste a game link or Place ID."

Under the box you see the game's name, its creator, how many are playing and a **Servers** button. A private server shows **Private server**; a share link shows "Private server link — Bloxkeep opens this link when you join."; a game with a Job ID shows **Specific server**. Up to three recent games show as shortcuts; their ✕ removes one.

**Choosing a game from the list**: click the game picture, or click the empty game box. The tabs are **Recent**, **Pinned**, **Roblox favorites** (games your accounts favorited on Roblox) and **Popular** (when available). Click ☆ to pin a game.

**Job ID (optional)**: the ID of one exact server (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`), so several accounts land together. Changing to another game clears a Job ID left from the previous one. Enter in this box presses **Play**.

### Servers

**Servers** (not shown for private servers) opens the game's server list.

- **Friends in this game**: friends of your accounts who are in this game, each "friend of *account*", with **Use server**. Their privacy settings may hide them.
- **Public servers**: sort by **Most players**, **Fewest players**, **Lowest ping** or **Best FPS**. **Room for N** (when 2 or more accounts are selected) shows only servers with room for all of them. **Random server** picks one. **Load more** loads more.
- **Use** puts that server's Job ID in the game bar: "Server chosen — select accounts and press Play." A full server is greyed out ("This server is full").
- The ping shown is the average of the players in that server, not your own.
- When Roblox is busy, the list tries again by itself, then offers **Try again**.

### Join a player

The person-with-arrow button next to the Job ID box (also in Ctrl+K). It is greyed out until you select accounts.

Type a player's **Roblox username or user ID** (a profile link works too) and press **Find and join**. Recent names are listed; ✕ forgets one. The selected accounts open into the server that player is in. When that can't happen, the dialog says why:

- no Roblox user has that name (it needs the username, not the display name);
- the player is in a game but their privacy settings don't show your accounts which server;
- the player is offline, or hides when they're online;
- the player is on the Roblox website or app, or in Roblox Studio, not in a game.

Bloxkeep only asks Roblox where they are, with your selected accounts. It never follows or friends anyone to get past their privacy settings.

### Play, Open Roblox, Close and Stop

- **Play** reads **Select accounts** (greyed out) while nothing is selected. With a game in the box it reads **Play** with the number of accounts. With the game box empty it reads **Open Roblox**: each selected account opens Roblox on its home screen.
- **Open Roblox without joining a game** (monitor icon) does that whatever is in the game box. Greyed out until you select accounts.
- **Close Roblox for the selected accounts** (power icon) is greyed out unless a selected account is open on this PC.
- While accounts open, **Play** becomes **Stop**: "Opened 2 accounts (stopped early)."

How a launch goes:

- Accounts open one after another, with **Wait between accounts** in between (**Settings › Launching**, 8 seconds by default; a preset can have its own).
- An account already in this game is left as it is ("1 account was already open, so it was left as it is.").
- An account in another game is asked about first, then closed and opened again in the new game (Roblox would kick a second window of the same account).
- Banned and Roblox VNG accounts are skipped, with a line in the result.
- If some accounts fail, **Some accounts didn't open** lists why.
- After opening, the windows can be arranged by themselves (see [Arranging windows](#arranging-windows)).

### Launch order

**⋯ More › Launch order…** lists every account: drag the handle or use the arrows, then press **Done**. **Play** opens the selected accounts in this order; accounts added later go last. **Sort › Launch order** shows the list in that order. A preset keeps its own order.

### The "Before you open" questions

Before opening, Bloxkeep may stop and ask in a dialog titled **Before you open**:

| Warning | When | Buttons | Don't ask again? |
|---|---|---|---|
| "*Account* will leave what they're playing now and open this game instead." (or "…will leave the game and go to the Roblox home screen.") | A selected account is already in another game or server. | Cancel, **Open anyway** | No, always asked. |
| "Multi-Roblox is off, so each new Roblox window closes the previous one." | You open more than one account, or one while another Roblox window is open, with Multi-Roblox off. | Cancel, **Open anyway**, **Turn on Multi-Roblox and play** | Yes |
| "Multi-Roblox isn't working yet: Roblox was already open before it was turned on." | Multi-Roblox is on but not ready. | Cancel, **Fix Multi-Roblox**, **Open anyway** | No |
| The Multi-Roblox check's warning (Roblox changed its one-window lock) | You open several windows on a Roblox build where two windows haven't played together yet. | Cancel, **Open anyway** | Yes |
| "Only *X* of RAM is free, and *N* more Roblox windows need about *Y*." | **Low RAM warning** is on and RAM would run short. | Cancel, **Open anyway** | Yes |

- **Don't ask again** only shows when every warning in the dialog can be skipped. Tick it and press **Open anyway**: each of those kinds is skipped from then on, on its own.
- A skipped warning doesn't stop a scheduled preset either.
- To get them back: **Settings › Notifications › Questions › Ask again**. The row **Questions you skipped** says how many there are.

Other questions with **Don't ask again**: deleting a preset, **Close all Roblox**, **Force close all Roblox**, **Turn on Multi-Roblox?**, **Discard changes?** and **Lock Bloxkeep?**. **Ask again** brings these back too. Deleting accounts, restoring a backup and removing the Discord link always ask.

---

## Multi-Roblox

Roblox normally allows one window: opening a second one closes the first. **Multi-Roblox** lets several accounts play at once. It is **off by default**. Roblox doesn't officially support running several clients, so turning it on asks **Turn on Multi-Roblox?** first: use it at your own risk.

How it works: Roblox uses a lock to allow only one window. Bloxkeep holds that lock's name first, while no Roblox is running, so the windows don't close each other. Bloxkeep injects nothing into Roblox. This means two rules:

- Bloxkeep has to be open **before** Roblox.
- Keep Bloxkeep open while you play (the tray is fine).

### The sidebar card

| Light | Text | Meaning | Clicking the text |
|---|---|---|---|
| Grey | **Off** | Multi-Roblox is off. | Opens the details. |
| Green | **On — several windows allowed** | Working. | Opens the details. |
| Yellow | **Not working yet — click to fix** | Roblox was open before Multi-Roblox was turned on. | Opens the details with the fix. |
| Yellow | **Roblox closed windows — click to see** | Roblox closed a window when another one opened. | Opens **Settings › Roblox version & graphics**. |
| Yellow | **A Roblox update may stop it — click to see** | The [Multi-Roblox check](#the-multi-roblox-check) found a change in the Roblox build. | Opens **Settings › Roblox version & graphics**. |

The switch on the card turns it on or off. The same switch is in **Settings › Launching › Multi-Roblox**, with a pill (**Off**, **Ready**, **Not ready**) and a **Details** link.

### Fixing "Not working yet"

The details dialog lists the steps:

1. **Close every Roblox**, including the one in the system tray: **Close all Roblox**.
2. Only if Roblox starts with Windows: **Stop Roblox from starting with Windows** with **Turn off autostart** (it then reads "Turned off ✓").
3. Multi-Roblox then turns on by itself (the light turns green).

Next time, open Bloxkeep before Roblox.

### Windowed and tiled windows

While Multi-Roblox is on, its details dialog has two switches:

- **Always open Roblox windowed** (on by default): all Roblox windows share one fullscreen setting, so with several open some end up fullscreen and some don't. Bloxkeep opens them windowed and puts your setting back once they're open.
- **Tile the windows after opening** (off by default): once the accounts are open, the windows are laid out side by side on the main screen. This is the same setting as **Settings › Launching › When a Roblox window opens › Tile them side by side**.

**Turn off Multi-Roblox** at the bottom turns it off.

### The Multi-Roblox check

**Settings › Roblox version & graphics › Roblox updates › Multi-Roblox check**. Bloxkeep checks each new Roblox build for changes that could stop Multi-Roblox, usually before the build reaches your PC. **How it works** opens the [details](roblox-watch.md).

| Result | Meaning |
|---|---|
| **No change found** | The lock is as before. |
| **Worked on this PC** | Two windows played together on this build here. That's proof. |
| **Changed — Bloxkeep adapted** | Roblox renamed the lock; Bloxkeep holds the new name too. Not proven yet. |
| **Changed — may be blocked** | The one-window code changed. |
| **Likely blocked** | The lock Multi-Roblox holds is gone. |
| **Not checked yet** / **Couldn't check right now** | No result yet. |

**Next Roblox build** shows the result for a build Roblox hasn't released yet. **Update history** lists every build seen on this PC.

What to do when it warns:

- **Keep playing as usual.** If Roblox starts closing one window when the next one opens, Bloxkeep says so, and an update of Bloxkeep is the fix. Press **Check for a Bloxkeep update**.
- If Roblox already closed windows ("Roblox closed *N* windows when another one opened"): update Bloxkeep, since a newer version may already fix it. Until then, play one account at a time. **Hide** removes the note. It also goes once two or more accounts are in a game together, or when Roblox updates.
- Until two windows have played together on that build, opening several accounts asks first (see [Before you open](#the-before-you-open-questions)).

---

## Presets and schedules

A preset saves accounts (in order), a game (or none, for the Roblox home screen), how to pick the server, the wait between accounts, options for the windows, where its Discord messages go, and a schedule. You can keep up to 50.

### Creating a preset

- **Presets** tab › **New preset**;
- or the **Presets** button in the game bar › **Save as preset…**: it starts from what the bar holds (the selected accounts, the game, the Job ID as a **Fixed server**, the game's name as the preset's name);
- or Ctrl+K › "Save as preset…".

| Field | What it means |
|---|---|
| **Name** | Required. |
| **Game** | Same box as the game bar. Empty just opens Roblox. |
| **Server** | **Any (Roblox picks)**, **Most players**, **Fewest players**, **Lowest ping**, **Random**, or **Fixed server (Job ID)**. The server is picked when the preset runs, among servers with room for its accounts; if none has room, Roblox picks. |
| **Job ID** | Only for **Fixed server**. |
| **Wait between accounts (s)** | 0–120. Empty uses the one in Settings. |
| **Accounts** | Tick them. Ticked accounts open in this order: drag the handle or use the arrows. If accounts were deleted, the form says so; they're left out when you save. |

If saving is refused, the reason shows above the buttons and the box to fix gets the focus.

### For the Roblox windows

These apply to the windows this preset opens, until they close (also when they rejoin). Pressing **Play** the normal way still follows Settings. An account in two presets follows the one that opened its window.

| Option | Choices |
|---|---|
| **Anti-AFK** | **Follow Settings (on/off)**, **On**, **Off** |
| **Auto-rejoin** | **Follow each account**, **On**, **Off** |
| **Performance** | **Normal**, or **Light AFK**: lowest graphics, 30 FPS, no sound, and the window is minimised once it's in the game. |
| **Restart Roblox** | **Never**, or **Every 2 / 4 / 6 / 12 hours**: each window is closed and opened again into the same game, one at a time. Needs a game. |
| **Arrange windows** | **Follow Settings (on/off)**, **On**, **Off**: tiles the windows once the preset has opened them. |

### Discord for a preset

**Send this preset's messages to**: **The channel in Settings**, or **A webhook of its own** (paste its link; **Send test** checks it). That covers its schedule messages, its summary after a scheduled stop, and the alerts about the windows it opened. Everything else goes to the channel in Settings. If **Send to Discord** is off in **Settings › Discord**, nothing goes out. If Discord deletes the preset's webhook, its messages go to the channel in Settings again.

### Schedule

Turn on **Schedule** ("Runs this preset by itself at a set time"):

- **Start time**;
- **Repeat**: **On these weekdays** (click the days) or **Once, on a date**;
- **Stop**: **Never (keep playing)**, **After some hours** (½ to 168, in half hours), or **At a time**.

Good to know:

- Bloxkeep must be running (it can be in the tray) and unlocked.
- A start time that passes while Bloxkeep is closed or locked, or while the PC sleeps, isn't made up later. When Bloxkeep was locked or the PC was asleep, you're told once ("…didn't run at 20:00: Bloxkeep was locked." or "…the PC was asleep or busy.").
- If opening would need a confirmation (Multi-Roblox off or not ready, low RAM, an account in another game), the run is skipped and you're told. A warning you answered **Don't ask again** doesn't stop it.
- At the stop, only the accounts this run opened are closed. A stop still happens while Bloxkeep is locked; one due while the PC slept happens when it wakes.
- A one-off schedule switches itself off once it has run (or once its time has passed).

### The Presets tab

Each row shows the game's icon, the preset's name, its game ("Roblox home" when it has none), and one line of what it does: the server, the wait, the schedule, and options set differently from Settings. Below: "Next: Today 20:00", "Last run 2 h ago", or "3 windows open". On the right: the accounts' avatars, the **Schedule** switch (only once a schedule has a start time), then:

- **Run**: greyed out when none of its accounts is left, or while another launch is running. While its windows are open it becomes **Close**, which closes only the windows this preset opened.
- **Edit**, **Duplicate** (a copy; its schedule is off if the original's was on), **Delete** (asks; **Don't ask again** possible).
- The handle and arrows change the order (hidden while you search). The game bar's **Presets** menu uses the same order.

The header counts presets and scheduled ones, and says "the most you can keep" at 50. **Search presets or games** (Ctrl+F) filters the list. A preset whose accounts were all deleted shows **No accounts left**.

The game bar's **Presets** button lists every preset (click one to run it; greyed out when it has no accounts), then **Save as preset…** and **Edit presets…** (opens the tab). In Ctrl+K, type a preset's name to run it.

---

## Keeping accounts in game

### Auto-rejoin

Turn it on:

- for one account: its **⋯ › Auto-rejoin**;
- for all: **Settings › Staying in game › Auto-rejoin › On for all accounts** (it reads "On for 3 of 10" when mixed);
- for a preset's windows: its **Auto-rejoin** option.

When an account's Roblox disconnects, gets kicked or loses the connection, Bloxkeep opens the same game (or private server) again after 10 seconds, though not always the same server. If it keeps dropping, from the third time it waits 2, 5, 10, then 15 minutes between tries; the card shows the wait and a **Cancel** button. Ten minutes in the game resets the count. Without internet it retries every 30 seconds, up to 20 times. After the PC wakes up, it waits for the connection.

It doesn't rejoin when:

- you left the game yourself, or closed the Roblox window yourself;
- the account signed in on another device;
- the account is banned;
- Roblox itself closed the window because another one opened;
- Bloxkeep is locked (auto-rejoin works again once you unlock).

If Roblox ends an account's sign-in, auto-rejoin turns itself off for it, and back on once you sign in again. With Multi-Roblox not ready, a rejoin is skipped (a new window would close the open one) and you're told.

### Anti-AFK

**Settings › Staying in game › Anti-AFK** (a preset can override it). Roblox kicks an account after 20 minutes without input. Bloxkeep brings each Roblox window that is in a game to the front for a moment, moves the mouse a few pixels over it and back, and returns you to your window. No key is pressed and nothing is clicked.

- It steps in from 12 minutes after a window's last input, at the first minute nobody uses the PC.
- A window two minutes from its kick gets it even while you use the PC, in a 2-second pause, but never over a full-screen app.
- It can't work while the PC is locked.
- If an account is kicked anyway, Bloxkeep says why (the PC was in use, locked, a full-screen app was in front, or Windows kept the window in the background).

### Daily sign-in check

**Settings › Staying in game › Check sign-ins every day** (on by default). Roblox can end a saved sign-in without telling anyone. Once a day, about 45 seconds after you unlock, Bloxkeep checks every account's sign-in, one at a time, 2 seconds apart, and whether Roblox reports a ban. One notification names the accounts that need to sign in again. The row shows "Last check: …".

To check now: **⋯ › Check sign-in** on an account, or **⋯ More › Check sign-in of all accounts**.

### Low RAM warning

**Settings › Staying in game › Low RAM warning** (on by default):

- before opening, asks when the free RAM can't take the new windows (see [Before you open](#the-before-you-open-questions));
- while playing, warns when the PC uses 92 % of its RAM or more (at most every 5 minutes).

The bar in the sidebar's **Roblox on this PC** card turns red from 90 %.

### Saving RAM on background windows

**Settings › Staying in game › Save RAM on background windows** (off by default). Roblox windows you aren't playing in get a low memory priority, so Windows takes RAM from them first; when the PC is really short of RAM, they are asked to give some back. The window in front gets its normal priority back. Roblox may refuse; Bloxkeep then skips that step.

### The tray

**Settings › Staying in game › Keep running in the tray when closed** (on by default). The ✕ button hides Bloxkeep to the system tray; auto-rejoin, Multi-Roblox and schedules keep working. Right-click the tray icon for **Mini window**, **Open Bloxkeep** and **Quit Bloxkeep**. With this off, ✕ quits Bloxkeep.

---

## Game monitor, windows and statistics

### Game monitor

**Game monitor** in the sidebar shows a live picture of every Roblox window, with its account, game, RAM and PID.

- Click a picture to bring that window to the front.
- Right-click for **Show**, **Close Roblox** and **Copy PID**.
- A minimised window shows "Minimised — no live picture".
- The pictures step aside while a dialog or menu is open.

### Mini window

**Mini window** in the sidebar (also in the tray menu and Ctrl+K) opens a small window that stays on top: the accounts playing now with their game, RAM and PID, each with **Show** and **Close**, plus **Close all** (asks first) and **Open Bloxkeep**. It keeps working while the main window is in the tray. Esc hides it; the sidebar button then reads **Hide mini window** while it's open. It remembers its place and height.

### Roblox on this PC

The sidebar card shows:

- how many Roblox windows are open (point at the number to see each account and PID) and the RAM they use;
- a bar of the PC's RAM: Roblox's share, then the rest in use; "PC RAM 64% used · 5.2 GB free";
- your Roblox version with a light: green "up to date", yellow when a newer one is out ("0.744 available", or "update available" for a new build of the same version);
- **Mini window**, and **Close all** while Roblox is running (it asks first).

If Roblox isn't installed, a warning card says "Roblox doesn't seem to be installed on this PC."

### Arranging windows

- **Arrange** in the game bar, **⋯ More › Arrange Roblox windows**, or Ctrl+K. With several screens, pick one first ("Screen 2 (1920×1080)", "main" marks the main screen).
- The layout comes from **Settings › Launching › Arranging windows**: **Fill the screen**, or **Small tiles**, **Medium tiles**, **Large tiles** laid out from the top-left, leaving the rest of the screen free. Smaller windows are less work for the graphics card.
- To arrange by themselves after opening: **Settings › Launching › When a Roblox window opens › Tile them side by side** (needs Multi-Roblox), or a preset's **Arrange windows**. Windows of a **Light AFK** preset aren't tiled (they are minimised).

### Statistics

**Statistics** in the sidebar has two tabs, **Statistics** and **Activity**. Only games opened with Bloxkeep are counted.

**Statistics**: pick **Today**, **7 days** or **30 days**.

- Tiles: **Play time**, **Sessions**, **Disconnects**, **Most played**; on a wide window also **Accounts played** and **Longest stay**.
- **Play time by hour** (Today) or **Play time by day**.
- **By account**: play time, sessions and disconnects for each account.
- **Top games**, and **By preset**.
- The figures refresh every 30 seconds while the tab is open.

**Activity**: what Bloxkeep saw happen, newest first: accounts opened, games joined and left, disconnects with the reason, rejoins, schedules, sign-ins, updates. The last 1,000 events are kept.

- Filter by **Account** and **Kind of event** (**Games**, **Disconnects and problems**, **Schedules**, **Sign-in**, **Updates**).
- **Show more** loads older events.
- **Export CSV** saves what the filters show to a file (also Ctrl+K › "Export the activity log (CSV)").

---

## Sending to Discord

Bloxkeep can post play-time summaries, a live status message and alerts to a Discord channel of yours, through a webhook (a link that lets a program post to one channel). It is off until you turn it on.

### Setting it up

1. In Discord: **Channel settings › Integrations › Webhooks › New Webhook › Copy Webhook URL**.
2. In Bloxkeep: **Settings › Discord › Webhook link › Paste link…**, paste it and press **Save**. (Turning on **Send to Discord** without a link asks for it.) The link looks like `https://discord.com/api/webhooks/…`.
3. Turn on **Send to Discord**, then press **Send test**: "Test message sent. Check the channel."

Good to know:

- The link is kept encrypted with your accounts. Anyone who has it can post in that channel, so keep it to yourself.
- **Show** displays the link for 30 seconds. **Replace…** pastes another. **Remove** asks first; messages already in the channel stay.
- The status line says "Last sent: …", "Nothing sent yet", how many messages are waiting, or the last error.
- If Discord says the webhook no longer exists (deleted, or its link reset), sending stops, a note says so and the **Discord** page gets a red dot. Paste a new link.
- Nothing is sent while Bloxkeep is locked. Messages that couldn't go while offline wait; very old ones are dropped, and the next message says so.

### Play time summary

| Row | Choices |
|---|---|
| **Send automatically** | **Never**, **Every day**, **Every week**, **Both**. **Every day at** a time; **Every week on** a day and time. |
| **When a scheduled preset finishes** | Sent half a minute after its accounts close, counting that preset's accounts only (to its own channel when it has one). |
| **Covers** | **Last 24 hours**, **Last 7 days**, **Last 30 days**. For the daily summary and **Send one now**. The weekly one always covers 7 days; the one after a schedule covers today so far. |
| **Includes** | **Total play time**, **Time per account**, **Top games**, **Sessions**, **Disconnects** (at least one). |
| **Accounts** | **All accounts**, or **Choose…** some. |
| **Ping** | See [Pings](#pings). |
| **Send one now** | **Send summary** sends it now, with the choices above. |

### Alerts

- **Send alerts** turns every kind on or off.
- **Ping** sets the same ping for every kind ("Varies by alert" when they differ).
- **Choose which alerts** opens one switch and one ping per kind: **Account dropped or auto-rejoin**, **Scheduled presets**, **Sign-in needs renewing** (bans too), **Low RAM**, **Launch problems**.
- They're separate from Windows notifications. Alerts within 10 seconds go out as one message.

### Status message

**Keep a status message up to date** with **Every 2 / 5 / 10 / 15 / 30 / 60 min**: one message, edited in place, with the accounts open, their game, time playing, PID and RAM. It never pings. When Bloxkeep is locked or closed, it says so.

### Pings

**Nobody**, **@here**, **@everyone**, **A person…** or **A role…**. A person or a role needs its ID (17 to 20 digits): in Discord turn on **Settings › Advanced › Developer Mode**, then right-click the person or role › **Copy ID**.

### Quiet hours

**Quiet hours** with **From** and **To**: messages still arrive, without pings. If From and To are the same time, quiet hours never apply.

### Customise messages

Open **Customise messages**:

- **Sender name** (up to 80 characters, without "discord" or "clyde");
- **Sender picture** (an `https://` link; empty uses Bloxkeep's logo);
- **Language**: **Same as Bloxkeep**, English, Tiếng Việt;
- **Message style**: **Cards** (easier to read) or **Plain text** (suits phone notifications);
- **Hide account names in messages**: "Account 1", "Account 2"… instead.

Messages carry account names (or "Account N"), game names, play time, sessions, PID, RAM and links to Roblox's own pictures. Never cookies, passwords or the webhook link.

---

## Settings, page by page

**Settings** in the sidebar lists its pages on the left. **Search settings** at the top (Ctrl+F while in Settings) finds rows on every page; Esc clears it. A red dot on a page means it needs a look: Roblox closed windows or a Multi-Roblox warning (**Roblox version & graphics**), or Discord stopped sending (**Discord**).

### Launching

**Multi-Roblox & windows**

| Row | Choices | What it does |
|---|---|---|
| **Multi-Roblox** | Switch, **Details** | Play on several accounts at once. See [Multi-Roblox](#multi-roblox). |
| **Show account names on windows** | Switch (on) | Puts the account's name in the Roblox window title, to tell windows apart on the taskbar. |
| **When a Roblox window opens** | **As Roblox sizes it**, **Maximise it**, **Tile them side by side** | Maximised windows are still windows, so they don't share Roblox's one fullscreen setting. Tiling needs Multi-Roblox. |
| **Arranging windows** | **Fill the screen**, **Small tiles**, **Medium tiles**, **Large tiles** | How **Arrange windows** (and tiling after opening) lays them out. |

**Launcher & timing**

| Row | Choices | What it does |
|---|---|---|
| **Open Roblox with** | See below; **Choose…** | Which program opens Roblox. Bloxstrap and Fishstrap keep your mods. |
| **Wait between accounts** | 0–60 s (8 by default) | Opening too many at once can fail. |

**Open Roblox with**:

| Option | Notes |
|---|---|
| **Default** (*name*) | Whatever opens Roblox links on this PC. |
| **Bloxstrap**, **Fishstrap** | Keep your mods. |
| Other Bloxstrap-style launchers | Listed when found on this PC. |
| **Roblox (official)** | The client from roblox.com. |
| **RobloxPlayerBeta directly (skip launchers)** | No launcher at all. Try it when a launcher blocks several windows. |
| **Roblox (Microsoft Store) — being tested** | Listed when the Microsoft Store's Roblox is installed. Bloxkeep starts the Store's Roblox inside its package with each account's own sign-in, skipping Microsoft's launcher screen (which allows only one window), so each account opens in a window of its own. The Microsoft Store still installs its updates. If it doesn't work, say so under **Bloxkeep updates › Send feedback**. |
| **Your launcher:** *name* | A program you picked with **Choose…** ("Choose another launcher…"). If it is moved or deleted, launching says so. |

An option that isn't installed shows "*name* — not installed" and is greyed out. When the Microsoft Store's Roblox is the only Roblox on the PC, a note says so here.

### Roblox version & graphics

**Roblox updates**

- A note when Roblox closed windows, with **Hide**.
- **Roblox version**: "Up to date", or that a newer version is out and how you get it: Roblox updates the next time it opens; with **RobloxPlayerBeta directly**, open Roblox normally once; for the Microsoft Store's Roblox, the Store installs it by itself, or now from **Microsoft Store › Library › Get updates**. "Not installed" or "Couldn't check right now" otherwise.
- **Installed build** (and which launcher it comes from), **Newest from Roblox** (only when different).
- **Multi-Roblox check**, with **How it works**, and **Next Roblox build**. See [The Multi-Roblox check](#the-multi-roblox-check). When it warns, a note says what to do, with **Check for a Bloxkeep update**.
- **Update history**: every Roblox build seen on this PC, newest first, with **Notes** (Roblox's release notes, written for game creators; they rarely mention the Roblox app itself).

**Roblox settings**

| Row | Choices |
|---|---|
| **Apply to** | **Nobody**, **All accounts**, **Chosen accounts** (then **Choose accounts…**, or **⋯ › Use Roblox settings** on each account) |
| **Graphics quality** | **Leave as is**, 1–10 |
| **Frame rate limit** | **Leave as is**, 30 / 60 / 144 / 240 FPS |
| **Volume** | **Leave as is**, **Muted**, 25 / 50 / 75 / 100 % |

The three values are greyed out while **Apply to** is **Nobody**. Bloxkeep writes them into Roblox's settings just before opening an account, and puts your own settings back once Roblox is running.

### Staying in game

**Keeping accounts going**

| Row | What it does |
|---|---|
| **Auto-rejoin** | **On for all accounts** / **Off for all accounts** ("On for *n* of *m*" when mixed; greyed out with no accounts). Each account's ⋯ menu sets it for that one; a preset that sets it decides for its windows. See [Auto-rejoin](#auto-rejoin). |
| **Anti-AFK** | Stay in game while you're away or busy in another window. Doesn't work when the PC is locked. See [Anti-AFK](#anti-afk). |
| **Check sign-ins every day** | On by default. Shows "Last check: …". See [Daily sign-in check](#daily-sign-in-check). |

**Memory**

| Row | What it does |
|---|---|
| **Low RAM warning** | On by default. See [Low RAM warning](#low-ram-warning). |
| **Save RAM on background windows** | Off by default. See [Saving RAM](#saving-ram-on-background-windows). |

**This app**

| Row | What it does |
|---|---|
| **Keep running in the tray when closed** | On by default. See [The tray](#the-tray). |
| **Hardware acceleration** | Draws Bloxkeep's window with the graphics card. Turns itself off if the graphics driver acts up. Switching reloads the window. |

### Notifications

- **Windows notifications** (on by default): only while Bloxkeep is in the background (hidden, minimised or behind another window). At most one per account and kind every 5 minutes. Messages inside Bloxkeep show either way.
- **Which events**: **Account dropped or auto-rejoin**, **Scheduled presets**, **Sign-in needs renewing** (bans too), **Low RAM**, **Launch problems**. Greyed out while **Windows notifications** is off.
- **Questions › Questions you skipped**: how many questions you answered **Don't ask again**. **Ask again** brings them all back (greyed out when there are none). See [Before you open](#the-before-you-open-questions).

### Discord

Every row is explained in [Sending to Discord](#sending-to-discord): **Send to Discord**, **Webhook link**, **Send test**, **Play time summary**, **Alerts**, **Keep a status message up to date**, **Quiet hours**, **Customise messages**.

### Appearance

| Row | What it does |
|---|---|
| **Theme** | **Sync with Windows** (light or dark like Windows), or one of 74 themes in **Basic**, **Scenery**, **Colourful**, **Styles** and **Seasonal**. **Dark** by default. Arrow keys try them; the pick is saved once you stop. |
| **Language** | EN / VI. |
| **Hide account names** | See [Hiding account names](#hiding-account-names). |

### Security

**Master password**: **Current password**, **New password (6+ characters)**, **Confirm new password**, **Change password**. The strength shows as you type. It refuses a wrong current password, one shorter than 6 characters, one of only spaces, or the same password again. Backups made before the change still open only with the old password.

### Backup & data

**Backup**

| Row | What it does |
|---|---|
| **Back up to a folder** | Copies the encrypted vault into a folder, so your accounts survive a dead PC. Pick a Google Drive or OneDrive folder. |
| **Folder** | **Choose…**, or a synced folder Bloxkeep found (**Use Google Drive**, **Use OneDrive**). **Open folder** opens it. It must be outside Bloxkeep's own data folder. |
| Last backup | "Last backup: …" or "No backups yet", with **Back up now** (greyed out until a folder is chosen). |

- Bloxkeep backs up after changes (at most every 10 minutes), when you lock or quit if something changed, and right after you change the master password or pick a new folder.
- It keeps the 10 newest, plus the newest of each day for 30 days. Your OneDrive or Google Drive app uploads them.
- If your master password is easy to guess and the folder is a cloud folder, a note suggests a longer one.

**Restore & import**

| Row | What it does |
|---|---|
| **Restore** › **Restore…** | Asks first (always). Your current data is kept as a copy, then Bloxkeep locks: unlock with the password the backup was saved with. |
| **Import accounts from a vault file** › **Import…** | Adds accounts from another vault or backup; yours stay. See [Import](#import-from-another-bloxkeep-file). |

**Data**

| Row | What it does |
|---|---|
| **Data location** › **Open folder** | Where Bloxkeep keeps your data (see [Where is my data?](#where-is-my-data)). |

### Bloxkeep updates

- **Bloxkeep** *version*, with its state: "Bloxkeep updates automatically", "Checking for updates…", "Updating Bloxkeep… *N*%", "Almost done! …", "Bloxkeep is up to date · checked at …", "Couldn't check for updates", "Automatic updates are off". Buttons: **What's new**, and **Check now** (greyed out while checking or downloading) or **Restart to update**.
- **Update automatically**: Bloxkeep downloads new versions and installs them by itself the next time you're not using it. Off, nothing is downloaded or installed by itself; **Check now** still works. Greyed out on a copy that can't update itself ("Only the installed version updates itself").
- **Help & feedback**: **User guide › Open** opens this guide. **Send feedback › Write** (see [Sending feedback](#sending-feedback-with-the-error-log)).

When an update is ready, **Restart to update** shows in the sidebar. Roblox keeps running while Bloxkeep restarts. After an update, **What's new** shows what changed.

---

## Keyboard shortcuts

**Anywhere in Bloxkeep**

| Keys | What they do |
|---|---|
| Ctrl+K | Quick search: accounts, presets, games, Settings pages and actions. Press again to close. Not on the lock screen or over another dialog. |
| Ctrl+F | The search box of the tab you're on (Accounts, Presets once there is one, Settings). From other tabs, the accounts search. |
| Ctrl+L or `/` | The game box. |
| Esc | Leaves a text box; otherwise clears the selection. Closes a menu, a list or a dialog. |

**Accounts tab** (not while typing in a box)

| Keys | What they do |
|---|---|
| Click / Shift+click | Select or unselect a card / select a range. |
| Double-click | Play that account alone. |
| Right-click | The account menu. |
| Arrow keys, Home, End | Move between cards. |
| Space | Select the focused card. |
| Ctrl+A | Select all accounts shown. |
| Enter | Play the selected accounts. |
| F2 | Edit the selected account (exactly one). |
| Delete | Delete the selected accounts (asks first). |

**Game box and Job ID box**

| Keys | What they do |
|---|---|
| ↓ / ↑ | Move through the game list. |
| Enter | Pick the highlighted game; with a link or Place ID in the box, play. In the Job ID box, play. |
| Esc | Close the game list. |

**Quick search (Ctrl+K)**

| Keys | What they do |
|---|---|
| ↓ / ↑, Page Down / Page Up | Move through the results. |
| Enter | Open the highlighted entry (an account is selected and shown; a preset runs; a game goes into the game box). |
| Ctrl+Enter / Shift+Enter | On an account: **Play** / **Edit**. |
| Esc | Close. |

**Menus and dialogs**

| Keys | What they do |
|---|---|
| ↓ / ↑ | Move through a menu. Tab or Esc closes it. |
| Enter | In a form, presses its main button. |
| Esc | Closes the dialog (asks first if you typed something). |
| Tab | Moves around inside the dialog. |

**Elsewhere**

| Keys | Where | What they do |
|---|---|---|
| ↓ / ↑, Home, End | Settings page list | Move between pages (← / → too in a narrow window). |
| Arrow keys, Home, End | Theme picker | Try themes. |
| Esc | Settings search | Clear it. |
| Esc | Mini window | Hide it. |
| Ctrl+T / Ctrl+W / Ctrl+Tab | Bloxkeep browser window | New blank tab / close the tab / switch tabs. |

---

## Troubleshooting and FAQ

### Roblox closes my other windows

- Is Multi-Roblox on? The sidebar card says **Off** → turn it on.
- Yellow **Not working yet — click to fix**: Roblox was open before Bloxkeep. Click it and follow the steps (close every Roblox, including the one in the tray; turn off autostart if offered). Open Bloxkeep before Roblox next time.
- Yellow **Roblox closed windows** or **A Roblox update may stop it**: a Roblox update may have changed how it blocks several windows. Open **Settings › Roblox version & graphics**, press **Check for a Bloxkeep update**, and play one account at a time until an update fixes it.
- Using a launcher? If it blocks several windows, choose **RobloxPlayerBeta directly (skip launchers)** in **Settings › Launching › Open Roblox with**.

### An account says "Needs sign-in"

Roblox ended its saved sign-in (a password change, "log out of all sessions", a security check). Press **Sign in** on the card and confirm with your phone, or add the account again any way: it is updated, not copied. The **Needs sign-in** filter shows them all. **Check sign-ins every day** finds them before you press Play.

### An account is banned

The **Banned** badge comes from Roblox's public information, read during sign-in checks. **Play** and presets skip the account, and auto-rejoin doesn't reopen it. Once the ban is over, **⋯ › Check sign-in** clears the badge.

### Roblox isn't installed, or only the Microsoft Store version is

- "Roblox doesn't seem to be installed on this PC." in the sidebar: install Roblox from roblox.com/download.
- Only Roblox from the Microsoft Store: choose **Roblox (Microsoft Store) — being tested** in **Settings › Launching › Open Roblox with**, or install Roblox from roblox.com.
- A launcher that is missing shows "— not installed" in that list. Pick another option.

### "Copy password" is greyed out, or says nothing is saved

- Greyed out: several accounts are selected. Copy works on one account at a time.
- "No password is saved for this account": Bloxkeep never keeps the password you sign in with. Type it under **Edit (F2) › Roblox password** and save. See [Copy password](#copy-password-and-saving-a-password).
- **Copy 2FA code** works the same way, with **Edit › Authenticator key**.

### Something else is greyed out

| Greyed out | Why |
|---|---|
| **Play** in the game bar ("Select accounts") | No account is selected. |
| **Play** on a card | A Roblox VNG account, or it is launching. |
| **Open Roblox**, **Join a player…** | No account is selected. |
| **Close** in the game bar, **Close Roblox**, **Force close** | None of the selected accounts is open on this PC. |
| **Edit…**, **Join this server**, **Roblox profile**, **Open website**, the **Copy** items | Several accounts are selected. **Join this server** also needs a known server; **Open website** isn't available for Roblox VNG. |
| **Run** on a preset | It has no accounts left, or another launch is running. |
| **Graphics quality**, **Frame rate limit**, **Volume** | **Apply to** is **Nobody**. |
| The kinds under **Which events** | **Windows notifications** is off. |
| **Back up now** | No backup folder chosen yet. |
| **Ask again** | You haven't skipped any question. |
| **Update automatically** | This copy can't update itself. |

### Where is my data?

**Settings › Backup & data › Data location › Open folder** shows it. For the installed app it is `%LOCALAPPDATA%\Bloxkeep`; the portable copy uses a `data` folder next to `Bloxkeep.exe`. Your accounts are in `vault.rmv`, encrypted; `vault.rmv.bak` is the previous save; `error.log` holds errors without account details. Never send anyone `vault.rmv` or a backup file. The full list is in the [README](../README.md#data-and-security).

### I forgot my master password

It can't be recovered. Restore a backup whose password you remember (**Restore from a backup** on the first screen, or **Settings › Backup & data › Restore…**), or delete `vault.rmv` and `vault.rmv.bak` and start over, losing the saved accounts.

### A scheduled preset didn't run

Bloxkeep must be running and unlocked at the start time. Runs missed while it was closed, locked or the PC slept aren't made up. A run is also skipped when opening would need a confirmation (Multi-Roblox off or not ready, low RAM, an account in another game); the notification says why.

### It says "playing" but I left

Roblox's status lags, sometimes by a few minutes. For windows on this PC, Bloxkeep follows Roblox's own log instead.

### Sending feedback with the error log

**Settings › Bloxkeep updates › Send feedback › Write** (or Ctrl+K › "feedback"):

1. **What is it?**: **Something doesn't work** or **An idea**.
2. **Message**, and **How to reach you** (optional) if you'd like an answer.
3. **Add Bloxkeep's error log** is ticked for a bug. Account names, cookies and passwords are taken out first. Bloxkeep must be unlocked for this.
4. **See exactly what will be sent** shows everything before it goes.
5. **Send**. You can send one message a minute.

It goes to Bloxkeep's developer through Discord, with nothing else from your PC ([details](feedback.md)).

---

## Privacy

Bloxkeep has no server, accounts or analytics of its own. Your accounts, settings and history stay on your PC, in a vault encrypted with your master password, which is never stored. Data only leaves your PC for what you use: Roblox (signing in, status, games, servers), GitHub (Bloxkeep updates, no account data), your backup folder (the encrypted vault), your Discord webhook (only when you turn it on, never cookies or passwords), and Bloxkeep's developer (only feedback you choose to send). The full list is in the [privacy policy](../PRIVACY.md).
