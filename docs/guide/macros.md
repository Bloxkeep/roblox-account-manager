[Tiếng Việt](macros.vi.md)

# Macros (Beta)

A macro is a short list of steps, like "Hold the left button for 0.6 s" or "Wait until the colour shows in Fish area", that Bloxkeep plays in your Roblox windows. Each macro can have a small **panel** beside the game, with Run and the settings you let the player change.

> [!NOTE]
> Macros are in Beta: they may not work the same on every PC. Tell us what goes wrong under **Settings › Bloxkeep › Send feedback**.

## How a macro reaches Roblox

Bloxkeep brings each Roblox window to the front for a moment, sends the keys and clicks, then gives back the window you were using. One window at a time.

- It pauses while you use the mouse or keyboard, while a full-screen app is in front and while the PC is locked, then carries on by itself.
- Vietnamese typing (Unikey and similar) must be off. The macro's line says so and waits.
- **Ctrl+Alt+0** stops every macro, from any app. **Lock app** stops them too.
- Bloxkeep doesn't read Roblox's memory, change its files or hook into it. It only sees the window, like you do.

## Make one

1. Open the **Macros** tab and press **New macro**: **Record**, **From a template**, **Blank** or **Import file**.
2. Steps read as sentences. Click one to open it and change its values in place. **Add step** lists the common steps first, the rest are under **More steps**.
3. Press **Try it**: the macro runs once on one window and says what happened, step by step.
4. Press **Save**, then **Run** on its row. The row's line says what each account is doing.

A template starts with a short **To do before it runs** list, for example "Place Bar area with Edit on game".

## Points and areas

- A click has a numbered **point**. An **area** is a box on the game where a step looks ("in Fish area").
- **Edit on game** shows the points and areas on the real Roblox window. Drag them there, then press **Done**.
- No window open? **Take picture** shows the same points and areas on a picture of the window, to drag there.
- Pick an area in a step's sentence, or **+ New area**. Points and areas follow the window when it's resized.

## Player can change

Open a step and tick **Player can change** under its sentence (a key, a hold time, a colour). The value then shows on the macro's panel, under **Player sees**. Click a name there to rename it, or change its order. Counters a macro adds to (for example "fish") show on the panel too.

Tick **Point** on a click to let the player set where it clicks: the panel gets a **Set** button, then a click on the game. **Add** under **Player sees** puts a switch (on/off), a choice (a list) or a heading on the panel; an **If** step can check a switch or a choice. **Add a tab** splits a long panel into tabs.

A **Tick list** is a list of words (for example enchants) the player ticks on the panel. A **Wait for text** or **If** step can then read "a word ticked in" that list: any ticked word counts, a letter read wrong in a long word is forgiven, and a longer name in the list ("Blessed Song") never counts as a shorter one ("Blessed").

**Click a colour** can also click **each spot once, as it shows**, once the spot has been there for the time you set (rings or targets that pop up one after another).

The panel opens with **Panel** on the macro's row, from the tray icon's **Macros** page, or with Ctrl+K "Show panel". **Show the panel by itself** opens it while an account is in the game you choose.

> [!IMPORTANT]
> Steps that need constant input, such as **Keep a bar on target**, keep their window in front while they run. Those games work one account at a time; the others wait their turn.

## When it gets stuck

A pop-up or a missed bite can leave a macro waiting for something that never comes.

- Open the line under the name. Set **If something doesn't show in time or it gets stuck** to **Start again from the top**.
- **Counts as stuck** says how long it may find nothing (for example 5 min) before that happens.
- Under **More steps**, add **If stuck…**. It goes last; put the steps that clear the way after it, such as closing a pop-up. They run before the macro starts again.

After 5 tries in a row that find nothing, the macro stops on that window.

## Limits

Up to 100 macros of 500 steps each, 30 settings for the player, 6 tabs and 8 areas per macro. A run stops after 10 minutes unless you change **Stop after** (open the line under the name).

## FAQ

**Q: Can it run while I use the PC?**
A: No. It pauses while you're active and goes on when you stop.

**Q: Does it type my saved codes into the game?**
A: No. A macro types only what you put in its own steps.

**Q: Try it says "didn't see it yet".**
A: Get that thing on screen (for example, wait for a bite), then press **Try it** again. If it still isn't seen, use **Edit on game** to put the area over it.

**Q: A step clicks in the wrong place.**
A: Press **Edit on game** and drag that step's numbered point to the right spot.
