# Roblox watch: is Multi-Roblox still working?

A Discord message within a few minutes of anything Roblox does that can stop Multi-Roblox working, like the bots that
track Roblox exploits. Nothing runs on your PC.

| What | Where | How fast |
|---|---|---|
| Notices a new Windows build: offered on Roblox's beta channel, live, or uploaded ahead of release | Cloudflare Worker, `RobloxWatch/worker` | every minute |
| Downloads the new build and compares it with the one before; reads Roblox's single-window settings; posts to Discord | GitHub Actions, `.github/workflows/roblox-watch.yml` running `RobloxWatch/watch.py` | started by the Worker; also every 10 minutes by itself |
| Remembers what it saw | `multi-status.json` on the `roblox-status` branch | |

## What it can and can't tell

- **The lock names.** Bloxkeep holds `ROBLOX_singletonEvent` (`RobloxManager/MultiInstance.cs`). If a build adds or
  drops a lock name, or a name in Roblox's single-window code (`SingleInstanceGuard`, `WndProcessCheck`,
  `WindowsAppReload…`), Multi-Roblox may be blocked (`changed`, amber). If the name Bloxkeep holds is gone and exactly
  one new `ROBLOX_…singleton…` name came in its place, Roblox renamed its lock (`renamed`, amber): the build's `hold`
  says to hold the new name, and Bloxkeep does, without an update of its own. With nothing to put in its place, it's
  likely blocked (`gone`, red).
- **Roblox's settings.** Flags such as `DFFlagEnableWindowsAppReloadCheck` switch the single-window check from Roblox's
  servers, with no new build. Any change to one is posted (amber).
- **Before release.** Roblox's beta channel (`…/client-version/WindowsPlayer/channel/zbeta`) offers the next build
  before it goes live, with its download: it's checked then, and when it goes live the message only says so with the
  result already found. A build that shows up in Roblox's deploy history before that (download still hidden) is
  posted as uploaded, and checked as soon as Roblox offers it.
- **It can't prove Multi-Roblox works.** The October 2026 change (0.742 → 0.743) kept every name the same: only two
  windows playing together show it. Bloxkeep already notices when Roblox closes a window as another opens, and says so.

## Setting it up

All the keys go into the repository's secrets (Settings › Secrets and variables › Actions › New repository secret),
never into a file or a chat.

1. **Discord.** In your server: Edit Channel › Integrations › Webhooks › New Webhook › Copy Webhook URL. Secret
   `DISCORD_WEBHOOK`. From then on, the 10-minute check posts to that channel.
2. **GitHub token for the Worker.** github.com › Settings › Developer settings › Fine-grained tokens › Generate new
   token: only the `bloxkeep` repository, permission **Actions: Read and write**, an expiry you'll remember. Secret
   `WATCH_TOKEN`.
3. **Cloudflare.** A free account at dash.cloudflare.com. Account ID: Workers & Pages › Overview, right side; secret
   `CLOUDFLARE_ACCOUNT_ID`. API token: My Profile › API Tokens › Create Token › template **Edit Cloudflare Workers**,
   your account, Create; secret `CLOUDFLARE_API_TOKEN`.
4. Actions › **Cloudflare workers** › Run workflow. It puts the Worker on Cloudflare (with the feedback one,
   [docs/feedback.md](feedback.md)); from then on an update is noticed within a minute. Run it again after changing
   `RobloxWatch/worker`.

When the GitHub token expires, the Worker can't start the check any more and the 10-minute schedule carries on alone:
make a new one and replace `WATCH_TOKEN`, then run step 4 again.

## By hand

- Actions › **Roblox watch** › Run workflow (tick *Check the live build again* to post the live build's result again).
- `python RobloxWatch/watch.py --status multi-status.json --dry` checks and prints instead of posting.
- `python RobloxWatch/watch.py --fingerprint RobloxPlayerBeta.exe` prints the names compared between builds.

## multi-status.json

```json
{
  "schema": 1,
  "live":   { "version": "0.743.0.7431129", "hash": "version-7a0c6d10fcb34a2c" },
  "beta":   { "version": "0.743.0.7431134", "hash": "version-11f155f3450f4c8e" },
  "future": { "version": "0.743.0.7431134", "uploadedAt": "10/15/2026 11:43:03 AM" },
  "flags":  { "DFFlagEnableWindowsAppReloadCheck": "True", "…": "…" },
  "builds": [ { "version": "…", "hash": "version-…", "channel": "beta | live", "seenAt": "…", "liveAt": "…",
                "verdict": "first | same | changed | renamed | gone", "hold": ["ROBLOX_singletonEvent"],
                "locks": ["…"], "related": ["…"], "added": [], "removed": [] } ],
  "changedAt": "…"
}
```

A build's `hash` is the name of its folder under Roblox's `Versions` folder, so the app finds the verdict for the build
it has installed.

## In the app

Bloxkeep reads this file from the `roblox-status` branch when it starts and every 30 minutes (`RobloxManager/RobloxWatch.cs`).
It takes only the verdicts and the `hold` names shaped like `ROBLOX_…`, at most four for a build; the rest is ignored.

- **Settings › Version & graphics › Roblox updates** shows the verdict for the build Multi-Roblox meets next (the
  installed one, or the live one a launcher updates to first), the next build checked ahead of release, and a verdict
  next to each build in the history.
- When that verdict is `changed`, `renamed` or `gone`, the Multi-Roblox card turns yellow and opening several accounts
  asks first, until two windows have played together on that build here (remembered in `prefs.json`, `multiWorked`).
- The `hold` names of the installed and the live build are held next to `ROBLOX_singletonEvent`, taken again once no
  Roblox is running.
