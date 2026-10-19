[Tiếng Việt](playing.vi.md)

# Playing

How to pick a game and a server, open Roblox for your accounts, and use Bloxkeep's browser window for roblox.com.

<img src="../screenshots/game-picker.png" width="760" alt="The game list open above the game bar, with the tabs Recent, Pinned, Roblox favorites and Popular">

## Quick steps

1. On the **Accounts** tab, select the accounts you want to open.
2. Click the game box at the bottom and type a game's name, a Place ID or a link. Pick the game from the list.
3. Press **Play**. It shows how many accounts will open.
4. If Bloxkeep asks **Before you open**, read it and press **Open anyway**.
5. To stop opening early, press **Stop**. To close Roblox, press **Close**.

## The game bar

The bar at the bottom of **Accounts** holds, left to right: the game picture (**Choose a game**), the game box, the **Job ID (optional)** box, **Join a player…**, then **Arrange**, **Open Roblox without joining a game**, **Close**, **Presets** and **Play**.

The game box ("Search a game, or paste a link / Place ID…", Ctrl+L or `/`) takes:

| You type or paste | Notes |
|---|---|
| A game's name | Pick it from the list. A name you typed but didn't pick doesn't count. |
| A Place ID | |
| A `roblox.com/games/…` link | Private server links work too. |
| A private server share link | `roblox.com/share?…type=Server` |
| A `roblox://` link | |

Under the box you see the game's name, its creator, how many are playing and a **Servers** button. A private server shows **Private server**, a game with a Job ID shows **Specific server**, and a share link shows "Private server link — Bloxkeep opens this link when you join." Up to three recent games show as shortcuts; ✕ removes one.

**Choosing from the list**: click the game picture, or click the empty game box. The tabs are **Recent**, **Pinned**, **Roblox favorites** (games your accounts favorited on Roblox) and **Popular** (when available). Click ☆ to pin a game.

**Job ID (optional)** is the ID of one exact server (`xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx`), so several accounts land together. Changing to another game clears a Job ID left from the previous one. Enter in this box presses **Play**.

> [!TIP]
> To put several accounts in one server, pick it under **Servers** (**Use**), or use **Join this server** on an account already there. Then select the others and press **Play**.

## Servers

**Servers** (not shown for private servers) opens the game's server list.

<img src="../screenshots/servers.png" width="760" alt="The server list for a game: friends in this game with Use server, then public servers with sort buttons, Room for 2 and Random server">

- **Friends in this game**: friends of your accounts who are in this game, each "friend of *account*", with **Use server**. Their privacy settings may hide them.
- **Public servers**: sort by **Most players**, **Fewest players**, **Lowest ping** or **Best FPS**. **Room for N** (with 2 or more accounts selected) shows only servers with room for all of them. **Random server** picks one.
- **Use** puts that server's Job ID in the game bar ("Server chosen — select accounts and press Play."). A full server is greyed out.
- The ping is the average of the players in that server, not your own. When Roblox is busy, the list tries again by itself, then offers **Try again**.
- Empty servers aren't listed: a server whose last player has left is closing, and joining it ends on "This experience has ended".
- Several accounts sent to one server: the first one to open shows whether the server lets it in (a normal join gets a server in about a second). If it doesn't within 12 seconds (the server is full or has closed), that account and the rest go to another server with room for them all.

## Saved servers

Keep the servers you go back to in the game box's **Saved** tab.

- **Save server** (under the game box) saves what the box holds: a private server's link (`privateServerLinkCode`, or a share link), or the game with the **code it asks for** when it has private servers of its own that you join by typing a code in the game. Give it a name; the same link saved again updates the one saved.
- Pick a saved server in the **Saved** tab: a link goes in the game box, so **Play** sends the selected accounts straight there. A code goes on the clipboard and its game in the box: once in the game, paste the code where the game asks for it. Bloxkeep never types into Roblox.
- **Copy** next to a saved server copies its code again; **×** deletes it.

## Join a player

The button with a person and an arrow, next to the Job ID box (also in Ctrl+K). It is greyed out until you select accounts. Type a player's **Roblox username or user ID** (a profile link works too) and press **Find and join**: the selected accounts open into the server that player is in. When that can't happen, the dialog says why:

- no Roblox user has that name (it needs the username, not the display name);
- the player is in a game but their privacy settings don't show your accounts which server;
- the player is offline, or hides when they're online;
- the player is on the Roblox website or app, or in Roblox Studio, not in a game.

Bloxkeep only asks Roblox where the player is, using your selected accounts. It never follows or friends anyone to get past their privacy settings.

## Play, Open Roblox, Close and Stop

| Button | What it does |
|---|---|
| **Play** | Reads **Select accounts** (greyed out) while nothing is selected. With a game in the box it reads **Play** and the number of accounts. With the game box empty it reads **Open Roblox**: each account opens Roblox's home screen. |
| **Open Roblox without joining a game** (monitor icon) | The same home-screen launch, whatever is in the game box. Greyed out until you select accounts. |
| **Close Roblox for the selected accounts** (power icon) | Greyed out unless a selected account is open on this PC. |
| **Stop** | Replaces **Play** while accounts are opening: "Opened 2 accounts (stopped early)." |

How a launch goes:

- Accounts open one after another, with **Wait between accounts** in between (**Settings › Launching**, 8 seconds by default; a preset can have its own).
- An account already in this game is left as it is. An account in another game is asked about first, then closed and opened again in the new game.
- Banned accounts are skipped, with a line in the result. If some accounts fail, **Some accounts didn't open** lists why.
- Afterwards the windows can arrange themselves: see [Windows and statistics](windows-and-stats.md).

## Launch order

**⋯ More › Launch order…** lists every account: drag the handle or use the arrows, then press **Done**. **Play** opens the selected accounts in this order, and accounts added later go last. **Sort › Launch order** shows the list in that order. A preset keeps its own order.

## Before you open

Before opening, Bloxkeep may stop and ask in a dialog titled **Before you open**. Every question has **Cancel** and **Open anyway**.

| Question | When it appears | Don't ask again? |
|---|---|---|
| "*Account* will leave what they're playing now and open this game instead." | A selected account is already in another game or server. | No, always asked. |
| "Multi-Roblox is off, so each new Roblox window closes the previous one." | You open more than one account, or one while another Roblox window is open, with Multi-Roblox off. It also offers **Turn on Multi-Roblox and play**. | Yes |
| "Multi-Roblox isn't working yet: Roblox was already open before it was turned on." | Multi-Roblox is on but not ready. It also offers **Fix Multi-Roblox**. | No |
| The Multi-Roblox check's warning | You open several windows on a Roblox build where two windows haven't played together yet. | Yes |
| "Only *X* of RAM is free, and *N* more Roblox windows need about *Y*." | **Low RAM warning** is on and RAM would run short. | Yes |
| "RAM is free, but Windows' virtual memory is almost full, so *N* more Roblox windows may crash as they open." | **Low RAM warning** is on, RAM is enough, but Windows' virtual memory would run short. See [Troubleshooting](troubleshooting.md#q-a-roblox-window-crashes-as-it-opens-memory-dump-captured-or-bloxstraps-roblox-failed-to-launch). | Yes |

<img src="../screenshots/before-you-open.png" width="760" alt="The Before you open question: Multi-Roblox is off, with a Don't ask again box and the buttons Cancel, Open anyway and Turn on Multi-Roblox and play">

- **Don't ask again** shows only when every warning in the dialog can be skipped. Tick it and press **Open anyway**: each of those kinds is skipped from then on, on its own. A scheduled preset isn't stopped by a skipped warning either.
- To get them back, use **Settings › Notifications › Questions › Ask again**. The row **Questions you skipped** says how many there are. For Multi-Roblox itself, see [Multi-Roblox](multi-roblox.md).

<details>
<summary>More details</summary>

Other questions also have **Don't ask again**: deleting a preset, **Close all Roblox**, **Force close all Roblox**, **Turn on Multi-Roblox?**, **Discard changes?** and **Lock Bloxkeep?**. **Ask again** brings these back too. Deleting accounts, restoring a backup and removing the Discord link always ask.

</details>

## Open website

**Open website** in the account menu opens roblox.com signed in as that account, in the Bloxkeep browser window. The window has one tab per account. It isn't available for Roblox VNG accounts.

- Each tab has its own private sign-in, kept in memory only. Nothing goes into your own browser.
- Opening an account that already has a tab switches to it. Each tab has its own Back, Forward and Reload.
- **+** at the end of the tab strip opens a blank tab. Sign in on roblox.com, then press **Save to Bloxkeep** to add the account (or renew its sign-in). Without saving, closing the tab signs it out there.
- Closing the window closes all its tabs. Locking Bloxkeep closes it too.

| Keys | What they do |
|---|---|
| Ctrl+T | Open a blank tab. |
| Ctrl+W, ✕ or middle-click | Close the tab. |
| Ctrl+Tab | Switch tabs. |

<details>
<summary>More details</summary>

Signing out in a saved account's tab would also sign it out of Bloxkeep, so Bloxkeep stops it and explains. Choose **Close tab** or **Keep browsing**.

</details>

← [User guide](../guide.md) · Next: [Multi-Roblox](multi-roblox.md) →
