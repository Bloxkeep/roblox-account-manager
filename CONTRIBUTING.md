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

You need Windows 10 or 11, the **.NET 10 SDK**, and **Node.js 22** or newer (for the interface and its checks).

| What | How |
|---|---|
| Build the interface | `cd ui`, then `npm install` and `npm run build` |
| Run the app's checks | `dotnet run --project Tests\Bloxkeep.Tests` (each check runs in its own process with throwaway data, never yours) |
| Run the interface checks | `node Tests/ui/run.mjs` (uses Microsoft Edge) |
| Try the interface without building | serve the repository root with any static web server and open `dev/preview.html` (made-up data) |
| Build the installers | `build.bat 1.0.1` (runs every check first) |

More detail is in the README's [Build from source](README.md#build-from-source).

### Pull requests

- One change per pull request, described by what a person using Bloxkeep will see.
- Add a check in `Tests/Bloxkeep.Tests` (or `Tests/ui`) that fails without your change, and keep every check passing.
- Match the code around your change: its naming, its comments (they say why, in plain words) and its structure.
- Text people read comes in English and Vietnamese together: `RobloxManager/L.cs` for the app, and
  `RobloxManager/wwwroot/i18n.js` for the interface.
- Bloxkeep doesn't change Roblox's files, inject code into Roblox or read its memory. Changes that do won't be accepted.

Releases are made by the maintainer. By contributing, you agree that your work is released under the
[MIT License](LICENSE), like the rest of the project.
