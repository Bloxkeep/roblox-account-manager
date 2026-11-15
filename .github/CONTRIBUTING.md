# Contributing

Thanks for wanting to help with Bloxkeep. Bug reports and ideas help as much as code.

**Never post a cookie, password, 2-step code or your Bloxkeep data folder** in an issue, a pull request, a screenshot or
a log. For a security problem, see [SECURITY.md](SECURITY.md) instead of opening an issue.

## Report a bug

- **From the app**: **Settings › Bloxkeep updates › Help & feedback › Send feedback**. It can add Bloxkeep's error log
  with account names, cookies and passwords taken out, and shows everything before it is sent.
- **On GitHub**: open an issue with the **Bug report** form. Say which Bloxkeep version you use (**Settings › Bloxkeep
  updates**), your Windows version, which Roblox you have (from the website, or the Microsoft Store), what you did, what
  happened, and what you expected.

## Suggest an idea

Open an issue with the **Idea** form, use **Send feedback › An idea** in the app, or bring it up on
[Discord](https://bloxkeep-bot.bloxkeep-app.workers.dev/discord).

## Work on the code

You need Windows 10 or 11, **Rust**, **Node.js 22** or newer (for the small web pages), and the **.NET 10 SDK** only to build the installers.

| What | How |
|---|---|
| Build the small web pages | `cd ui`, then `npm install` and `npm run build` |
| Build the app | `cd ui-gpui`, then `cargo build --release --features core` |
| Run the app's checks | `ui-gpui\target\release\bloxkeep-ui.exe --core-suites` (each check runs in its own process with throwaway data, never yours) |
| Build the installers | `scripts\build.bat 1.0.1` (runs every check first) |

More detail is in the README's [Build from source](../README.md#build-from-source).

### Pull requests

- One change per pull request, described by what a person using Bloxkeep will see.
- Add a check in `ui-gpui/src/core/tests` that fails without your change, and keep every check passing.
- Match the code around your change: its naming, its comments (they say why, in plain words) and its structure.
- Text people read comes in English and Vietnamese together, in `RobloxManager/wwwroot/i18n.js` (the build makes
  `ui-gpui/assets/i18n.json` from it) or `ui-gpui/assets/words.json` for the app's own words.
- Bloxkeep doesn't change Roblox's files, inject code into Roblox or read its memory. Changes that do won't be accepted.

Releases are made by the maintainer. By contributing, you agree that your work is released under the
[MIT License](../LICENSE), like the rest of the project.
