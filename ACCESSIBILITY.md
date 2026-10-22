# Accessibility

Bloxkeep should be usable by anyone who plays Roblox on Windows. This page says what works today and what hasn't
been checked yet, so you know what to expect.

## What works

- **Keyboard**: most of the app works without a mouse. `Ctrl+K` finds accounts, presets, games, Settings pages and
  actions; `Ctrl+F` searches the tab you're on; `Ctrl+L` or `/` goes to the game box; on the accounts, `Ctrl+A`,
  `Enter`, `F2`, `Delete` and `Esc` select all, play, edit, delete and clear. The launch order can be changed with
  arrow buttons as well as by dragging. The full list is in the README's
  [Keyboard shortcuts](README.md#keyboard-shortcuts).
- **Labels**: buttons that show only an icon have a name a screen reader can read.
- **Less motion**: when Windows' **Animation effects** are off (**Settings › Accessibility › Visual effects**),
  Bloxkeep's animations stop too.
- **Colors**: 74 themes, light and dark, and a custom theme where you pick the colors yourself (**Settings ›
  Appearance**). An account's state is written out in words on its card and in the list (Playing, Joining, Kicked…),
  not only shown as a color.
- **Language**: English and Vietnamese, switched in **Settings**.

## Not checked yet

- **Screen readers** (Narrator, NVDA, JAWS): the labels are there, but nobody has used Bloxkeep from start to end with
  one.
- **Windows' contrast themes** and **text size** setting have not been tested.
- **Roblox's own windows** are Roblox's: Bloxkeep opens and arranges them but can't change what is inside.

## Tell us about a barrier

If something in Bloxkeep is hard or impossible for you to use, that's a bug. Report it any of these ways:

- an [issue](https://github.com/Bloxkeep/roblox-account-manager/issues/new?template=bug_report.yml), saying it's about
  accessibility;
- **Settings › Bloxkeep updates › Help & feedback › Send feedback** in the app;
- the [Discord server](https://bloxkeep-bot.bloxkeep-app.workers.dev/discord).

Say what you were trying to do and what you use (keyboard only, a screen reader and which one, zoom, a contrast theme).
