# Separate Roblox home-screen sessions: implementation and limits

Status: experimental, unreleased. This is not evidence that long sessions or future Roblox builds are safe.

The manual test on 2026-10-18 used Roblox 0.743.0.7431134. A client started through a unique directory junction opened a second window without a sign-in ticket. Holding both LocalStorage/RobloxCookies.dat and LocalStorage/appStorage.json exclusively made that window start at sign-in. The user signed in with another account and signed out of the new window; the original window remained signed in. That was a short test, not an overnight or cookie-renewal test.

## Changes after the initial experiment

- Junctions live under Bloxkeep's data directory, in roblox-client-paths, instead of TEMP. Removing a junction is always non-recursive; its target installation is never traversed by cleanup.
- Session lifetime follows clients launched through those junctions, including their tray copies, rather than all Roblox processes. Normal clients (including accounts launched with tickets) do not keep these locks or the update delay active after the separate clients have closed.
- Windows QueryFullProcessImageName resolves junctions to the real installation, so it cannot identify the alias used at launch. The executable argument from the process command line retains it. The query validates process creation time through the same handle; cached classifications use PID plus creation time. Command lines and authentication tickets are never logged.
- An unknown process whose launch path cannot be queried keeps the locks held conservatively. Failed queries are retried. A reused PID or missing creation time never inherits a stale classification.
- Only identified alias tray copies are ended during session cleanup. The following process snapshot must confirm their exit before unlocking.
- Existing shared files are opened read-only with FileShare.None, without reading their contents. A missing file is created exclusively with DeleteOnClose, so Windows removes only that empty placeholder when its handle closes, including on process termination. Partial acquisition releases the first handle if the second cannot be locked.
- Turning off Multi-Roblox, or losing its ready state while acquiring the file locks, produces a notification instead of silently abandoning the action. Fresh locks are released if no client was started.
- Account launches and automatic rejoins do not overlap a separate-window launch in progress.
- An explicit experimental-mode warning appears before acquiring a new session's locks. Automatic and manual app updates still wait while these separate sessions need the locks.

## Remaining limits

Locking these files affects every Roblox window for that Windows user, not just junction clients. Ordinary launches during that interval may also start signed out and cannot persist their sign-ins or some settings.

A renewed cookie that cannot be persisted might make the normal saved login invalid later. This is a plausible risk, not a reproduced result. Login renewal and long-running sessions have not been validated.

Quitting or crashing Bloxkeep releases its handles even if separate windows remain open. Those clients may subsequently persist their own session over the shared login. There is no crash-surviving isolation here. Close separate windows before exiting Bloxkeep. Moving the junctions and fixing lock lifetime do not solve this risk.

Antivirus behavior, later Hyperion behavior, and automatic-rejoin attribution when a separate window opens at the same time as a disconnect have not been validated. Keep these limits visible; do not treat the short manual test or passing unit checks as release approval.

## Validation

The home-session suite uses disposable files and stand-in processes. It covers actual Windows file sharing, existing file preservation, cleanup of missing-file placeholders, partial acquisition rollback, junction cleanup preserving target files, launch-path queries through junctions, PID reuse, query failures, readiness notifications, and controller lock release with ordinary clients still present. It does not log into Roblox or change the real Roblox storage.
