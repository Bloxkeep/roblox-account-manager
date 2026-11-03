# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

The main window, tray panel and dialogs are a React page in WebView2 inside a Windows desktop app (C#, .NET). A few
windows are still drawn natively with WinForms: the macro panel shown beside a game, the game monitor, the mini
window, the on-game overlay for areas and points.

## Users

Confirmed by the owner (2026-11-03): people who keep many Roblox accounts open at once (5 to 20 or more) to farm or
AFK, and need them to stay in game for hours without watching. They sign accounts in once, then mostly glance at
Bloxkeep while it runs beside a grid of Roblox windows.

Two languages: English (default) and Vietnamese (a large share of users; YouTube referrals).

## Product Purpose

A free Windows account manager for Roblox: keep accounts signed in, open many Roblox windows at once, put each into
the right game and server, and keep them there (auto-rejoin, Anti-AFK, getting unstuck, leaving crowded servers),
with presets and schedules, macros (Beta), statistics and Discord alerts. Success: a farm left overnight is still
in game in the morning, and setting it up took minutes.

## Positioning

Inferred from the repository, not confirmed by the owner: everything in one free app, built around keeping
accounts in game rather than just switching between them, and doing it without injecting into Roblox or reading its
memory, so it never puts the user's accounts at risk.

## Operating Context

- Runs beside many Roblox windows, often on a laptop (the owner: 150 % scaling, 240 Hz); the window is often small or
  narrow, and the tray panel is used while the main window is hidden.
- Roblox reads input only in the window that has focus; macros bring each window forward in turn.
- Roblox's own server list, logs and web APIs are the only sources of game state.

## Capabilities and Constraints

- Windows only. Never injects into Roblox, hooks it, reads its memory, bypasses its anti-cheat or solves its bot
  checks for mass account creation.
- Master password protects a local vault; cookies and passwords are never shown or sent anywhere.
- No native browser widgets (native select popups, date, time and colour pickers): the app has its own TimeField,
  ColorField and select styling. Popups close on a second click of what opened them.
- Many themes, including animated picture themes; every surface must read on all of them.
- Terminology: "Preset", "account" / "tài khoản" (never "acc").

## Brand Commitments

- Name: Bloxkeep. Logo: the yellow block face (RobloxManager/wwwroot/logo).
- No AI-looking gradients, glows or jargon; solid colours. Short, natural wording; Vietnamese reads like real
  Vietnamese apps (labels of 1 to 4 words).
- Never markets itself through other projects or claims a feature that isn't there.

## Evidence on Hand

- Real screenshots and animated demos in docs/guide and What's new (account names always blurred).
- Usage counts from the Discord bot (other PCs online); no testimonials or reviews collected.

## Product Principles

1. Keeping accounts in game comes first; anything that risks an account (injection, memory reading) is out.
2. Practical features over polish, but anything ugly, cut off, misaligned or meaningless is a bug.
3. Glanceable: a user checking a running farm sees what needs them in a second.
4. Say it short, in the user's language, with no jargon.
