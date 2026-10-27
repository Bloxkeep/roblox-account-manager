[Tiếng Việt](accounts.vi.md)

# Accounts

How to add accounts, read their cards, find and select them, and use the account menu.

<img src="../screenshots/accounts-list.png" width="760" alt="The Accounts tab as a list: filter chips with counts, search, Group and Sort, and one line per account with its status">

## Quick steps

1. Click **Add account**.
2. On your phone, open the Roblox app signed in to the account and go to **Settings → Quick Log In**.
3. Enter the code Bloxkeep shows, or scan the QR, then confirm on the phone.
4. A new code appears for the next account. Press **Done** when you have them all.

## Ways to add an account

**Add account** opens **Sign in to Roblox**. The arrow beside it, **Other ways to add**, lists the rest.

<img src="../screenshots/add-account.png" width="760" alt="The Sign in to Roblox dialog: Sign in with your phone (recommended), username and password, and the other ways: browser, cookie, Bloxkeep file">

| Method | When to use it | Notes |
|---|---|---|
| **Sign in with your phone** (recommended) | Almost always. | Code or QR, no password, no puzzle. The code renews itself a few times, then press **New code**. |
| **Use username & password** | No phone at hand. | The password goes only to Roblox; Bloxkeep never stores it. Roblox may ask for a puzzle or a 2-step code. |
| **Sign in with a browser** | The other ways fail, or Roblox wants an extra check. | The official roblox.com sign-in page. Needs www.roblox.com and Microsoft Edge WebView2. Not for Roblox VNG. |
| **Paste a cookie** | You already have the account's `.ROBLOSECURITY` cookie. | Only **Cookie .ROBLOSECURITY** is required. |
| **Open roblox.com**, then **Save to Bloxkeep** | You mostly want the website, several accounts side by side. | A blank tab in the Bloxkeep browser window ([Open website](playing.md#open-website)). Sign in, then **Save to Bloxkeep**. |
| **Import from: Bloxkeep file** | Moving accounts from another PC or a backup. | Needs that file's master password ([Import](#import-from-another-bloxkeep-file)). |

Adding an account that is already in the list never makes a copy: its sign-in is updated. Bloxkeep can't read other account managers' files; if the other tool shows an account's cookie, use **Paste a cookie**.

> [!WARNING]
> The `.ROBLOSECURITY` cookie is the full key to an account: anyone who has it can use the account. Never share it.

<details>
<summary>More details</summary>

- **Phone**: when the phone has entered the code, confirm there. "The code expired" and "Declined on the device." both need **New code**. The **Roblox** / **Roblox VNG** tabs choose the realm.
- **2-step verification**: type the 6-digit code from the tab your account has (**Authenticator app**, **Email**, **Text message**). If the authenticator key is saved in Bloxkeep, the code is filled in: press **Verify**.
- Roblox sometimes refuses a password sign-in (locked account, passkey or security key). The message then tells you to use your phone or the browser.

</details>

## The account card

The badge in the card's corner is **On this PC** (Roblox is open for it here; point at it to see the PID), **Launching**, **Banned** or **Needs sign-in**. The dot on the avatar shows the status, and the text under the name says it in words:

| Dot | Text under the name | Meaning |
|---|---|---|
| Green | **Playing**, **Joining**, **Waiting for a server**, **Server full, waiting**, **Teleporting**, **Playing · another device** | In a game or on the way in, on this PC or another device. **Server full** shows the place in line when Roblox gives one. |
| Blue | **Online**, **Roblox open** · Not in a game, **Didn't get into the game** | On the Roblox website or app, or Roblox is on its home screen on this PC (after a join that failed, or a wait given up, it says so). |
| Orange | **Roblox Studio** | Building a game. |
| Yellow | The reason, such as **Kicked by the game** or **Server shut down** | The window is open but out of the server. When Roblox didn't log why, it says **Disconnected** · **Reason unknown**. |
| Grey | **Offline**, **Signed out**, **Status unknown** | Not active; Roblox is open here without this account; or not loaded yet. |

A red ring around the avatar means **Needs sign-in** or **Banned**. While Bloxkeep is working you also see **Opening Roblox…**, **Rejoining · this PC** (**Cancel** stops it) and **Restarting · this PC** (a preset's planned restart).

<img src="../screenshots/card-rejoin.png" width="760" alt="Account cards: one rejoining in 5 minutes with Cancel, one disconnected for being idle, one needing sign-in, one banned">

The line above the buttons shows the account's **Robux** (**R$ 1,234**), read with its own sign-in every 30 minutes and kept only on this PC. An account open on this PC also shows **RAM in use**, **Play time** and **PID** (click to copy). The buttons are **Play** (**Close** while open, **Cancel** while a rejoin waits, **Sign in** when it needs sign-in), **Edit (F2)** and **⋯ More**. Status refreshes every 30 seconds; **Refresh online status** does it now.

## Select, search and sort

- **Click** a card to select it, click again to unselect. **Shift+click** selects a range. **Double-click** plays that account alone.
- **Ctrl+A** or **Select all** selects every account shown. **Esc** or **Clear** clears the selection.
- **Search** (Ctrl+F) matches names, @usernames, nicknames, groups, notes, the game being played and user IDs. Accents don't matter.
- **Filter chips** show a count: **All**, **Needs sign-in** and **Banned** (only when some accounts are), **Playing**, **Online**, **Offline**, **On this PC**.
- **Group** appears once an account has one (set it under **Edit › Group**). **Sort**: **Name**, **Status**, **Last played**, **Launch order**.
- **Cards** / **List** switch the layout. Layout and sort are remembered.

If the search or a filter hides some selected accounts, the header says how many. **Play** and **Delete** still act on them.

## The account menu

Open it with **⋯ More** on a card, or right-click the account. If the account isn't selected, the menu acts on that account alone and leaves your selection as it is; if it is in a selection, the menu acts on every selected account.

<img src="../screenshots/account-menu.png" width="760" alt="The account menu open on a card: Play, Open Roblox, Close Roblox, Force close, Edit, Check sign-in, Join this server, Auto-rejoin, Roblox profile, Open website, the Copy items and Delete">

| Item | What it does | Unavailable when |
|---|---|---|
| **Play** / **Play (N accounts)** (Enter) | Opens the game in the game box, or Roblox's home screen if it is empty. | — |
| **Open Roblox only** | Opens Roblox on its home screen, one window per account. | — |
| **Close Roblox** | Closes the Roblox window normally. | None of the selected accounts is open here. |
| **Force close** | Ends Roblox at once, for a frozen window. Unsaved game progress is lost. | Same as **Close Roblox**. |
| **Edit…** (F2) | Nickname, group, password, authenticator key, note. | Several selected. |
| **Check sign-in** | Checks the saved sign-in and whether Roblox reports a ban. | — |
| **Join this server** | Puts this account's game and server (Job ID) in the game bar; select others and press **Play** to land together. | Several selected, or the server isn't known. |
| **Switch to another server** | Closes the window and opens it again in another server of the same game with room. Accounts of the same game switched together land in the same server. | None of the selected accounts is playing in a public server on this PC. |
| **Solve the bot check** | Only on a card that says **Roblox wants a bot check**: shows Roblox's check in Bloxkeep's own window, then opens the window again into its game once Roblox lets it in. | — |
| **Auto-rejoin** | Turns auto-rejoin on or off for these accounts. | — |
| **Use Roblox settings** | Includes these accounts in **Roblox settings** (graphics, FPS, volume). | Shown only when **Settings › Roblox version & graphics › Apply to** is **Chosen accounts**. |
| **Roblox profile** | Opens the profile on roblox.com in your browser. | Several selected, or no user ID. |
| **Open website** | Opens roblox.com signed in as this account, in the Bloxkeep browser window. | Several selected, or Roblox VNG. |
| **Copy username** | Copies the username. | Several selected. |
| **Copy password** | Copies the saved password. | Several selected. No password saved: it says why. |
| **Copy 2FA code** | Copies the current 6-digit code. | Several selected. No key saved: it says why. |
| **Delete** / **Delete N accounts** (Del) | Removes the accounts after a confirmation. It can't be undone. | — |

The **⋯ More** menu at the top of the page has **Check sign-in of all accounts**, **Select all**, **Arrange Roblox windows**, **Launch order…** (needs 2 or more accounts; see [Playing](playing.md#launch-order)), **Close all Roblox**, **Force close all Roblox** and **User guide**.

## Copy password and Copy 2FA code

Bloxkeep never keeps the password you sign in with. Until you save one yourself, **Copy password** says "No password is saved for this account". **Copy 2FA code** needs an authenticator key in the same way. To save them:

1. Select the account and press **F2** (or **Edit…** in the menu).
2. Type the password in **Roblox password**. For 2FA, paste the key (letters A–Z and digits 2–7) or the `otpauth://` link into **Authenticator key**, then check that "Code now: …" matches your authenticator app.
3. Press **Save**. **Edit** now shows the running 2FA code with **Copy** and **Remove**.

- **Copy password** clears the clipboard after 30 seconds. The password sits in the encrypted vault, only for you: Bloxkeep doesn't use it to sign in.
- **Copy 2FA code** copies the current code ("It changes in *N* s."). A password sign-in fills it in for you too.

> [!WARNING]
> With the key saved in Bloxkeep, anyone with your master password can make codes. Save it only if that is fine with you.

## Needs sign-in and Banned

| Badge | Why | What to do |
|---|---|---|
| **Needs sign-in** | Roblox ended the saved sign-in: a password change, "log out of all sessions" or a security check. | Press **Sign in** on the card and confirm with your phone, or add the account again any way (it is updated, not copied). |
| **Banned** | Roblox's public information says so; it is read during sign-in checks. | **Play** and presets skip it, and auto-rejoin doesn't reopen it. When the ban is over, **⋯ › Check sign-in** clears the badge. |

The **Needs sign-in** filter lists them all. **Check sign-ins every day** finds them before you press **Play** ([Staying in game](staying-in-game.md)). To check now, use **⋯ › Check sign-in** on an account, or **⋯ More › Check sign-in of all accounts**.

## Import from another Bloxkeep file

For accounts in `vault.rmv` from another PC, or a `Bloxkeep-backup-…` file. Open it from **Other ways to add › Import from another Bloxkeep vault file…**, or **Settings › Backup & data › Import accounts from a vault file › Import…**.

1. **Choose file…** and pick the `.rmv` file.
2. Type **Master password of this file** and press **Open**.
3. Tick the accounts to bring over. Each shows **New** or **Already here**. **All** / **None** tick everything or nothing.
4. Optionally tick **Also import presets (N)**, then press **Import**.

- The file itself isn't changed. Imported accounts are locked with this Bloxkeep's master password.
- For an account already here, only a sign-in that no longer works is replaced. Your names, groups and notes stay.
- Imported presets have their schedules switched off. To replace everything instead (a new PC), use **Restore** ([Backup and security](backup-and-security.md)).

## Roblox VNG

Add these accounts on the **Roblox VNG** tab of **Sign in to Roblox** or **Sign in with your phone** (confirm in the Roblox VNG app). There is no browser sign-in for them. They carry a **VNG** tag.

- **Play** opens them like any account (new, still being tried out): Bloxkeep gets their sign-in from Roblox VNG and opens Roblox with your usual launcher. Which servers they can join is up to Roblox. If Roblox refuses, the result says why.
- **Open website** is greyed out for them.

← [User guide](../guide.md) · Next: [Playing](playing.md) →
