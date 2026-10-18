[Tiếng Việt](multi-roblox.vi.md)

# Multi-Roblox

How to let several accounts play at once, what the Multi-Roblox card in the sidebar is telling you, and which program opens Roblox.

<img src="../screenshots/multi-roblox-real.jpg" width="760" alt="A real screenshot of one Windows PC: five Roblox windows open at once, titled Roblox - Account 1 to Roblox - Account 5, four in the same game and the fifth joining, with Bloxkeep behind them counting 5 windows and Multi-Roblox on">

A real screenshot from one PC: five accounts open at once with Bloxkeep, four in the game and the fifth joining. Names in the game are blurred.

## Quick steps

1. Open Bloxkeep before Roblox. If Roblox is already open, press **Close all Roblox**.
2. Turn on the switch on the **Multi-Roblox** card at the bottom of the sidebar, and confirm **Turn on Multi-Roblox?**.
3. Check that the light is green and the card says **On — several windows allowed**.
4. Select your accounts and press **Play**. Keep Bloxkeep open while you play (the tray is fine).

You can also press **Play** with Multi-Roblox off: the **Before you open** question offers **Turn on Multi-Roblox and play**.

<img src="../screenshots/before-you-open.png" width="760" alt="The Before you open question: Multi-Roblox is off, with the buttons Cancel, Open anyway and Turn on Multi-Roblox and play">

## What it is

Roblox normally allows one window: opening a second one closes the first. Multi-Roblox lets several accounts play at once. It is **off by default**.

Roblox uses a lock to allow only one window. Bloxkeep holds that lock's name first, while no Roblox is running, so the windows don't close each other. Bloxkeep injects nothing into Roblox.

> [!WARNING]
> Roblox doesn't officially support running several clients, so use Multi-Roblox at your own risk.

## The sidebar card

| Light | Text | Meaning | Clicking the text |
|---|---|---|---|
| Grey | **Off** | Multi-Roblox is off. | Opens the details. |
| Green | **On — several windows allowed** | Working. | Opens the details. |
| Yellow | **Not working yet — click to fix** | Roblox was open before Multi-Roblox was turned on. | Opens the details with the fix. |
| Yellow | **Roblox closed windows — click to see** | Roblox closed a window when another one opened. | Opens **Settings › Roblox version & graphics**. |
| Yellow | **A Roblox update may stop it — click to see** | The Multi-Roblox check found a change in the Roblox build. | Opens **Settings › Roblox version & graphics**. |

The same switch is in **Settings › Launching › Multi-Roblox**, with a pill (**Off**, **Ready**, **Not ready**) and a **Details** link.

## Fixing "Not working yet"

1. **Close every Roblox**, including the one in the system tray: press **Close all Roblox**.
2. Only if Roblox starts with Windows: press **Turn off autostart** under **Stop Roblox from starting with Windows** (it then reads "Turned off ✓").
3. Multi-Roblox turns on by itself and the light turns green.

Next time, open Bloxkeep before Roblox.

## Windowed and tile

While Multi-Roblox is on, its details dialog has two switches:

<img src="../screenshots/multi-help.png" width="760" alt="The Multi-Roblox dialog: Multi-Roblox is on, with the switches Always open Roblox windowed and Tile the windows after opening">

| Switch | Default | What it does |
|---|---|---|
| **Always open Roblox windowed** | On | All Roblox windows share one fullscreen setting, so with several open some end up fullscreen and some don't. Bloxkeep opens them windowed and puts your setting back once they're open. |
| **Tile the windows after opening** | Off | Once the accounts are open, the windows are laid out side by side on the main screen. Same as **Settings › Launching › When a Roblox window opens › Tile them side by side**. |

**Turn off Multi-Roblox** at the bottom of the dialog turns the feature off.

## The Multi-Roblox check

Find it under **Settings › Roblox version & graphics › Roblox updates › Multi-Roblox check**. Bloxkeep checks each new Roblox build for changes that could stop Multi-Roblox, usually before the build reaches your PC.

<img src="../screenshots/roblox-updates.png" width="760" alt="Settings, Roblox version and graphics: the Multi-Roblox check shows Changed, may be blocked, with a what-to-do note and the update history">

| Result | Meaning |
|---|---|
| **No change found** | The lock is as before. |
| **Worked on this PC** | Two windows played together on this build here. That's proof. |
| **Changed — Bloxkeep adapted** | Roblox renamed the lock; Bloxkeep holds the new name too. Not proven yet. |
| **Changed — may be blocked** | The one-window code changed. |
| **Likely blocked** | The lock Multi-Roblox holds is gone. |
| **Not checked yet** / **Couldn't check right now** | No result yet. |

### When it warns

- **Keep playing as usual.** If Roblox starts closing one window when the next one opens, Bloxkeep says so, and an update of Bloxkeep is the fix. Press **Check for a Bloxkeep update**.
- If Roblox already closed windows ("Roblox closed *N* windows when another one opened"), update Bloxkeep: a newer version may already fix it. Until then, play one account at a time.
- Until two windows have played together on that build, opening several accounts asks first (see [Playing](playing.md)).

<details>
<summary>More details</summary>

- **Next Roblox build** shows the result for a build Roblox hasn't released yet. **Update history** lists every build seen on this PC.
- **Hide** removes the "Roblox closed windows" note. It also goes once two or more accounts are in a game together, or when Roblox updates.
- **How it works** opens a longer explanation (in English): [roblox-watch.md](../roblox-watch.md).

</details>

## Open Roblox with

**Settings › Launching › Launcher & timing › Open Roblox with** chooses the program that opens Roblox. Bloxstrap and Fishstrap keep your mods.

| Option | Notes |
|---|---|
| **Default** (*name*) | Whatever opens Roblox links on this PC. |
| **Bloxstrap**, **Fishstrap** | Keep your mods. |
| Other Bloxstrap-style launchers | Listed when found on this PC. |
| **Roblox (official)** | The client from roblox.com. |
| **RobloxPlayerBeta directly (skip launchers)** | No launcher at all. Try it when a launcher blocks several windows. |
| **Roblox (Microsoft Store) — being tested** | Listed when the Microsoft Store's Roblox is installed. Each account opens in a window of its own. |
| **Your launcher:** *name* | A program you picked with **Choose…**. If it is moved or deleted, launching says so. |

An option that isn't installed shows "*name* — not installed" and is greyed out. When the Microsoft Store's Roblox is the only Roblox on the PC, a note here says so.

<details>
<summary>More details</summary>

**Roblox (Microsoft Store)**: Bloxkeep starts the Store's Roblox inside its package with each account's own sign-in. It skips Microsoft's launcher screen, which allows only one window. The Microsoft Store still installs its updates. If it doesn't work, say so under **Bloxkeep updates › Send feedback**.

</details>

← [User guide](../guide.md) · Next: [Presets](presets.md) →
