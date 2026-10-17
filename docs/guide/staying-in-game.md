[Tiếng Việt](staying-in-game.vi.md)

# Staying in game

How Bloxkeep keeps accounts in their games: Auto-rejoin, Anti-AFK, the daily sign-in check, the RAM options and running from the tray. All of it is on **Settings › Staying in game**.

<img src="../screenshots/settings-staying.png" width="760" alt="Settings, Staying in game page: Auto-rejoin, Anti-AFK, Check sign-ins every day, Low RAM warning, Save RAM on background windows and Keep running in the tray when closed">

## Quick steps

1. Open **Settings › Staying in game**.
2. Next to **Auto-rejoin**, choose **On for all accounts**. For a single account, use its **⋯ › Auto-rejoin** instead.
3. Turn on **Anti-AFK**.
4. Leave **Keep running in the tray when closed** on, so the ✕ button hides Bloxkeep instead of quitting it.

## Auto-rejoin

When an account's Roblox disconnects, gets kicked or loses the connection, Bloxkeep opens the same game (or private server) again, though not always the same server.

| Turn it on for | Where |
|---|---|
| One account | Its **⋯ › Auto-rejoin**. |
| All accounts | **Settings › Staying in game › Auto-rejoin › On for all accounts** (it reads "On for 3 of 10" when mixed). |
| A preset's windows | The preset's **Auto-rejoin** option (see [Presets](presets.md)). |

<img src="../screenshots/card-rejoin.png" width="760" alt="Account cards: one is rejoining in 5 minutes with a Cancel button, another shows Disconnected after being kicked for being idle">

### How long it waits

| Situation | What Bloxkeep does |
|---|---|
| Roblox disconnects | Opens the game again after 10 seconds. |
| It keeps dropping | From the third time it waits 2, 5, 10, then 15 minutes between tries. The card shows the wait and a **Cancel** button. |
| 10 minutes in the game | The count starts over. |
| No internet | Retries every 30 seconds, up to 20 times. |
| The PC wakes up | Waits for the connection first. |

### When it doesn't rejoin

- You left the game yourself, or closed the Roblox window yourself.
- The account signed in on another device.
- The account is banned.
- Roblox itself closed the window because another one opened.
- Bloxkeep is locked. Auto-rejoin works again once you unlock.

<details>
<summary>More details</summary>

- If Roblox ends an account's sign-in, auto-rejoin turns itself off for it, and back on once you sign in again.
- With Multi-Roblox not ready, a rejoin is skipped (a new window would close the open one) and you're told. See [Multi-Roblox](multi-roblox.md).

</details>

## Anti-AFK

**Settings › Staying in game › Anti-AFK** (a preset can override it). Roblox kicks an account after 20 minutes without input. Bloxkeep brings each Roblox window that is in a game to the front for a moment, moves the mouse a few pixels over it and back, and returns you to your window. No key is pressed and nothing is clicked.

- It steps in from 12 minutes after a window's last input, at the first minute nobody uses the PC.
- A window two minutes from its kick gets it even while you use the PC, in a 2-second pause, but never over a full-screen app.

> [!WARNING]
> Anti-AFK can't work while the PC is locked.

If an account is kicked anyway, Bloxkeep says why: the PC was in use, it was locked, a full-screen app was in front, or Windows kept the window in the background.

## Daily sign-in check

**Settings › Staying in game › Check sign-ins every day** (on by default). Roblox can end a saved sign-in without telling anyone.

- Once a day, about 45 seconds after you unlock, Bloxkeep checks every account's sign-in, one at a time, 2 seconds apart. It also checks whether Roblox reports a ban.
- One notification names the accounts that need to sign in again. The row shows "Last check: …".
- To check now: **⋯ › Check sign-in** on an account, or **⋯ More › Check sign-in of all accounts**.

## Low RAM warning

**Settings › Staying in game › Low RAM warning** (on by default).

- Before opening, it asks when the free RAM can't take the new windows (the **Before you open** question, see [Playing](playing.md)).
- While playing, it warns when the PC uses 92 % of its RAM or more, at most every 5 minutes.
- The bar in the sidebar's **Roblox on this PC** card turns red from 90 % (see [Windows and statistics](windows-and-stats.md)).

## Save RAM for background windows

**Settings › Staying in game › Save RAM on background windows** (off by default). Roblox windows you aren't playing in get a low memory priority, so Windows takes RAM from them first. When the PC is really short of RAM, they are asked to give some back.

The window in front gets its normal priority back. Roblox may refuse; Bloxkeep then skips that step.

## Fast cores for the window you play

**Settings › Staying in game › Fast cores for the window you play** (off by default). Most recent CPUs have two kinds of cores: fast ones and efficiency ones (Intel 12th gen and later, many recent laptops). With this on, every Roblox window in the background runs on the efficiency cores, in Windows' power-saving mode; the window you're playing, and the rest of the PC, keep the fast cores. Click another Roblox window and it gets the fast cores back within a second.

- Many accounts open stay smooth where you play, and a laptop runs cooler.
- On a CPU with one kind of core, background windows only get the power-saving mode.
- These are Windows' own settings (Task Manager's "Set affinity" and efficiency mode): nothing in Roblox is changed. Turning it off, or closing Bloxkeep, gives the windows Windows' own choice back.

## The tray

**Settings › Staying in game › Keep running in the tray when closed** (on by default). The ✕ button hides Bloxkeep to the system tray. Auto-rejoin, Multi-Roblox and schedules keep working.

| Tray icon, right-click | What it does |
|---|---|
| **Mini window** | Opens the small always-on-top window. |
| **Open Bloxkeep** | Brings the main window back. |
| **Quit Bloxkeep** | Closes Bloxkeep. |

With the option off, ✕ quits Bloxkeep.

← [User guide](../guide.md) · Next: [Windows and statistics](windows-and-stats.md) →
