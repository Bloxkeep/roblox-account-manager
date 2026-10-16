[Tiếng Việt](presets.vi.md)

# Presets

A preset saves a set of accounts, a game and how to open them, so you can start it with one click or on a schedule.

<img src="../screenshots/presets.png" width="760" alt="The Presets tab: three presets, each with its game, a one-line summary, the accounts' avatars, a Schedule switch and Run or Close, Edit, Duplicate and Delete buttons">

## Quick steps

1. Open the **Presets** tab and press **New preset**.
2. Type a **Name** and pick the **Game**. Leave the game empty to open only the Roblox home screen.
3. Choose the **Server** and tick the **Accounts**, in the order you want them to open.
4. Press **Save**, then **Run** on its row.

A preset keeps the accounts (in order), the game, how to pick the server, the wait between accounts, options for the windows, where its Discord messages go, and a schedule. You can keep up to 50.

## Creating a preset

- **Presets** tab › **New preset**.
- The **Presets** button in the game bar › **Save as preset…**. It starts from what the bar holds: the selected accounts, the game, the Job ID as a **Fixed server**, and the game's name as the preset's name.
- Ctrl+K › "Save as preset…".

| Field | What it means |
|---|---|
| **Name** | Required. |
| **Game** | Same box as the game bar. Empty just opens Roblox. |
| **Server** | **Any (Roblox picks)**, **Most players**, **Fewest players**, **Lowest ping**, **Random**, or **Fixed server (Job ID)**. |
| **Job ID** | Only for **Fixed server**. |
| **Wait between accounts (s)** | 0–120. Empty uses the one in Settings. |
| **Accounts** | Tick them. Ticked accounts open in this order: drag the handle or use the arrows. |

<details>
<summary>More details</summary>

- The server is picked when the preset runs, among servers with room for its accounts. If none has room, Roblox picks.
- If accounts were deleted, the form says so; they're left out when you save.
- If saving is refused, the reason shows above the buttons and the box to fix gets the focus.

</details>

## Options for the Roblox windows

These apply to the windows this preset opens, until they close (also when they rejoin). Pressing **Play** the normal way still follows Settings. An account in two presets follows the one that opened its window.

<img src="../screenshots/preset-editor.png" width="760" alt="The Edit preset window: For the Roblox windows (Anti-AFK, Auto-rejoin, Performance, Restart Roblox, Arrange windows), Discord with a webhook of its own, and Schedule">

| Option | Choices |
|---|---|
| **Anti-AFK** | **Follow Settings (on/off)**, **On**, **Off** |
| **Auto-rejoin** | **Follow each account**, **On**, **Off** |
| **Performance** | **Normal**, or **Light AFK**: lowest graphics, 30 FPS, no sound, and the window is minimised once it's in the game. |
| **Restart Roblox** | **Never**, or **Every 2 / 4 / 6 / 12 hours**: each window is closed and opened again into the same game, one at a time. Needs a game. |
| **Arrange windows** | **Follow Settings (on/off)**, **On**, **Off**: tiles the windows once the preset has opened them. |

### Discord for a preset

**Send this preset's messages to**: **The channel in Settings**, or **A webhook of its own** (paste its link; **Send test** checks it). That covers its schedule messages, its summary after a scheduled stop, and the alerts about the windows it opened. Everything else goes to the channel in Settings.

If **Send to Discord** is off in **Settings › Discord**, nothing goes out. If Discord deletes the preset's webhook, its messages go to the channel in Settings again. More in [Discord](discord.md).

## Schedule and stop after

Turn on **Schedule** ("Runs this preset by itself at a set time") in the editor.

| Setting | Choices |
|---|---|
| **Start time** | The time it starts. |
| **Repeat** | **On these weekdays** (click the days) or **Once, on a date**. |
| **Stop** | **Never (keep playing)**, **After some hours** (½ to 168, in half hours), or **At a time**. |

- Bloxkeep must be running (it can be in the tray) and unlocked.
- At the stop, only the accounts this run opened are closed.
- A stop still happens while Bloxkeep is locked. One that falls due while the PC sleeps happens when it wakes.
- A one-off schedule switches itself off once it has run (or once its time has passed).

## Missed runs

> [!WARNING]
> A start time that passes while Bloxkeep is closed or locked, or while the PC sleeps, isn't made up later.

- If Bloxkeep was locked or the PC was asleep, you're told once ("…didn't run at 20:00: Bloxkeep was locked." or "…the PC was asleep or busy.").
- If opening would need a confirmation (Multi-Roblox off or not ready, low RAM, an account in another game), the run is skipped and you're told. A warning you answered **Don't ask again** doesn't stop it.

For help finding the cause, see [Troubleshooting](troubleshooting.md).

## The Presets tab

Each row shows the game's icon, the preset's name, its game ("Roblox home" when it has none), and one line of what it does: the server, the wait, the schedule, and options set differently from Settings. Below that: "Next: Today 20:00", "Last run 2 h ago", or "3 windows open". On the right are the accounts' avatars, the **Schedule** switch (only once a schedule has a start time), then:

| Button | What it does |
|---|---|
| **Run** | Opens the preset. Greyed out when none of its accounts is left, or while another launch is running. While its windows are open it becomes **Close**, which closes only the windows this preset opened. |
| **Edit** | Opens the editor. |
| **Duplicate** | Makes a copy. Its schedule is off if the original's was on. |
| **Delete** | Asks first; **Don't ask again** is possible. |

- The handle and arrows change the order (hidden while you search). The game bar's **Presets** menu uses the same order.
- The header counts presets and scheduled ones, and says "the most you can keep" at 50.
- **Search presets or games** (Ctrl+F) filters the list. A preset whose accounts were all deleted shows **No accounts left**.

> [!TIP]
> The game bar's **Presets** button lists every preset (click one to run it), then **Save as preset…** and **Edit presets…**. In Ctrl+K, type a preset's name to run it.

← [User guide](../guide.md) · Next: [Staying in game](staying-in-game.md) →
