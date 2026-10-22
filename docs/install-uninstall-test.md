# Install / update / uninstall: manual test checklist

For `dist\releases\Bloxkeep-Setup-Offline.exe` and `Bloxkeep-Portable.zip` built by `scripts\build.bat` (Velopack 1.2.161).
`Bloxkeep-Setup.exe` (the web installer) installs the newest *published* release, so try it once after publishing (section 8).
Do this on a PC (or a Windows Sandbox / VM) where you don't mind touching `%LOCALAPPDATA%\Bloxkeep`; on your own PC
back up that folder first. Quit every dev build of Bloxkeep before you start: dev builds share the single-instance
mutex and the data folder with the installed copy, so an installed copy started while a dev build runs only brings the
dev build's window to the front and exits.

Tick each line. "150 %" means Settings › System › Display › Scale set to 150 %, which is where blurry images and icons show.

## 1. Fresh install

- [ ] Double-click `Bloxkeep-Setup-Offline.exe`. SmartScreen: "More info › Run anyway" appears (unsigned build, expected).
- [ ] PC without WebView2 only (rare; Windows 11 and updated Windows 10 have it): a dialog "Bloxkeep requires Microsoft Edge WebView2 Runtime … install?" appears, then a download progress box. Offline, this fails with an error and no Bloxkeep is installed: expected, note it.
- [ ] Splash: dark window with the yellow smiley and "Bloxkeep", yellow progress bar along the bottom, no title bar text. At 150 % the window is 720×450 and the text is slightly soft (the image is 480×300 and Velopack scales it); judge whether that's acceptable.
- [ ] Install takes a few seconds, no other dialog, then Bloxkeep starts by itself showing the master-password setup screen (first run).
- [ ] `%LOCALAPPDATA%\BloxkeepApp` exists with `Bloxkeep.exe` (stub), `Update.exe`, `current\`, `packages\`. `%LOCALAPPDATA%\Bloxkeep` exists after you set a master password.
- [ ] Start menu: "Bloxkeep" at the root of All apps (no folder), with the smiley icon. At 150 % the icon is crisp (36 px comes from the 48 px frame; 16/20/24/32/40/48/64/128/256 are in `app.ico`).
- [ ] Desktop: `Bloxkeep.lnk` with the same icon; Properties › Target is `…\BloxkeepApp\current\Bloxkeep.exe`.
- [ ] Settings › Apps › Installed apps › Bloxkeep: name **Bloxkeep**, publisher **minhhoang0122**, version **1.0.0**, a size of roughly 110 MB, the smiley icon, install date today. Three-dot menu shows only Uninstall (no Modify/Repair).
- [ ] Task Manager › Details › Bloxkeep.exe › Properties › Details: Company minhhoang0122, Product Bloxkeep, version 1.0.0.
- [ ] Pin Bloxkeep to the taskbar from the Start menu, start it from that pin: one taskbar button, not two (the app sets its AppUserModelID `velopack.BloxkeepApp` to match the shortcut).
- [ ] Settings › Data › Data location shows `%LOCALAPPDATA%\Bloxkeep`; Open folder works.
- [ ] Regedit: `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall\BloxkeepApp` has DisplayName, DisplayIcon, DisplayVersion, Publisher, InstallLocation, EstimatedSize, UninstallString `"…\Update.exe" --uninstall`. `HKCU\…\CurrentVersion\Run` has no Bloxkeep entry.

## 2. Install over an existing install

- [ ] With Bloxkeep running (window open), run the same `Bloxkeep-Setup-Offline.exe` again. A dialog "Bloxkeep is already installed … Repair?" appears. Choose Repair: the running Bloxkeep is ended without any prompt (Velopack kills every process under `BloxkeepApp`), the splash shows, Bloxkeep starts again, still unlocked with your accounts.
- [ ] Same with close-to-tray on and the window hidden to the tray: after the kill, a dead tray icon may stay until the mouse passes over it. Note whether it does on this Windows build.
- [ ] Run a Setup built with a higher version (`scripts\build.bat 1.0.1`): the dialog offers Update instead; data is kept and What's new shows after the restart. A Setup with a lower version offers a downgrade warning.
- [ ] Run Setup while a dev build is running: Setup finishes but the dev build's window comes to the front and the installed copy exits (expected, see top). Quit the dev build and start Bloxkeep from the Start menu.

## 3. Update in place (installed copy)

Needs the next version on GitHub Releases, or a folder with the next version's `releases.win.json` + `.nupkg` set in `BLOXKEEP_UPDATE_SOURCE` before starting Bloxkeep.

- [ ] About 20 s after start, Settings › Updates shows downloading, then the sidebar shows **Restart to update**.
- [ ] Click it: the window closes, no tray icon stays behind, the new version starts and shows What's new. Roblox clients that were running are untouched.
- [ ] Instead of clicking: quit Bloxkeep. The update is applied silently (Update.exe runs for a few seconds); the next start is the new version.
- [ ] Desktop and Start menu shortcuts still show the icon after the update (the app asks Explorer to refresh them); the taskbar pin still works.
- [ ] Installed apps shows the new version number.

## 4. Uninstall

- [ ] With Bloxkeep hidden to the tray (close-to-tray on): Settings › Apps › Installed apps › Bloxkeep › Uninstall. A "Bloxkeep Uninstall" progress box, then "Uninstall Complete". Bloxkeep was ended without a prompt; a dead tray icon may remain until you hover it (known, documented in the README).
- [ ] Afterwards: `%LOCALAPPDATA%\BloxkeepApp` is gone (Update.exe may take a few seconds to delete itself). Start menu entry, Desktop shortcut, Start pin and taskbar pin are gone. The Uninstall registry key is gone.
- [ ] `%LOCALAPPDATA%\Bloxkeep` still exists with `vault.rmv`, `vault.rmv.bak`, `prefs.json`, `roblox-settings.json`, `error.log`; the `webview` and `webview-safe` folders are gone (removed by the uninstall hook).
- [ ] `%TEMP%` has no `velopack_BloxkeepApp` folder and no `Bloxkeep-login-*` / captcha profile folders.
- [ ] `HKCU\…\CurrentVersion\Run` and `…\Explorer\StartupApproved\Run` have no Bloxkeep entry (Bloxkeep only ever edits Roblox's own `--launch-to-tray` entry there, and only when you ask it to).
- [ ] Uninstall with the "Couldn't save your changes. Quit anyway?" box open (make `vault.rmv` read-only, change a nickname, press X with close-to-tray off): the box and the app are simply killed. Afterwards the vault is the last good save; nothing is corrupted (start the portable build on the same data folder via `BLOXKEEP_DATA` to check).
- [ ] Uninstall while a dev build is running: the dev build is asked to quit by the uninstall hook (shared quit event) and does so. Expected; just don't be surprised.

## 5. Reinstall after uninstall

- [ ] Run `Bloxkeep-Setup-Offline.exe` again: no Repair dialog (folder was removed), splash, Bloxkeep starts at the **unlock** screen, your accounts are back after the master password.
- [ ] Theme, language and close-to-tray settings are as before (prefs.json survived).

## 6. Portable zip

- [ ] Unzip to a writable folder (Downloads, not Program Files). Contents: `.portable`, `Bloxkeep.exe` (stub), `Update.exe`, `current\`.
- [ ] Run `Bloxkeep.exe`: SmartScreen once, then the setup screen. A `data\` folder appears next to `current\`; `%LOCALAPPDATA%\Bloxkeep` is not touched.
- [ ] No entry in Installed apps, no shortcuts (expected). Settings › Updates still works for the portable copy; after an update `current\` is replaced and `data\` stays.
- [ ] Running the portable copy while the installed copy runs only brings the installed one to the front (shared single-instance mutex).

## 7. Remove everything

- [ ] Uninstall, then delete `%LOCALAPPDATA%\Bloxkeep` and the backup files in your backup folder. For the portable build delete the unzipped folder. Nothing else of Bloxkeep's is left on the PC.

## 8. Web installer (after publishing)

- [ ] Download `Bloxkeep-Setup.exe` from the release page in Edge and run it from Edge's downloads. Its SHA-256 is the same as `Installer\Web\Bloxkeep-Setup.exe` in the repo.
- [ ] Under the title it shows "Version" and the release just published (a second after opening), and the space needed.
- [ ] Install: "Downloading Bloxkeep x.y.z… n of about 57 MB" with the bar filling, then the usual copying and the done page.
- [ ] Close the window while it downloads: it stops and closes; nothing is installed and no `Bloxkeep-Setup-*` folder is left in `%TEMP%`.
- [ ] With the network off: Install ends on "Setup didn't finish" saying it couldn't download from GitHub, without the setup-log link; Try again works once the network is back.
