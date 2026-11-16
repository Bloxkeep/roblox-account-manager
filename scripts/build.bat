@echo off
rem Builds Bloxkeep into dist\releases (needs Rust, Node and the .NET 10 SDK for vpk; users need none of them):
rem   Bloxkeep-Setup.exe            installer for users: they choose the folder (any drive); installs per user without
rem                                 admin rights, and installs WebView2 if missing. A small web installer, the same
rem                                 file in every release (Installer\Web): it downloads the newest offline one
rem   Bloxkeep-Setup-Offline.exe    the same installer with everything inside, for installing without internet
rem   Bloxkeep-Portable.zip         no install; keeps its data in a "data" folder next to it; updates itself too
rem   *.nupkg + releases.win.json   what installed copies download when they update
rem Usage: scripts\build.bat [version] [github repo url]   e.g. scripts\build.bat 1.0.1
rem The repo url defaults to Bloxkeep's GitHub repo; pass another one to build a fork. Updates only work while the repo is public. Each version needs its notes in RobloxManager\whatsnew.json
rem (shown in the app after updating, and on the GitHub release); a quiet fix (1.0.7-alpha.1) has none. Publish with release.bat.
setlocal
cd /d "%~dp0.."
set VERSION=1.0.0
if not "%~1"=="" set VERSION=%~1
set REPO=https://github.com/Bloxkeep/roblox-account-manager
if not "%~2"=="" set REPO=%~2

if exist artifacts\app rmdir /s /q artifacts\app
rem Files left from an earlier build would be listed in SHA256SUMS.txt and stop the same version from packing again.
if exist dist\releases rmdir /s /q dist\releases
mkdir dist\releases

echo [1/5] Tools...
dotnet tool restore >nul || goto :fail

rem Since 1.1 the app is the Rust one (ui-gpui: the window drawn with GPUI, the core moved from C#), built with cargo.
rem Its small web pages (tray, a macro's panel, mini, overlay, message box, the browser's top) are still the React ones
rem in RobloxManager\wwwroot, built by ui\build.mjs, and sit in a wwwroot folder beside it. The version and the update
rem repo go into the exe at build time (BLOXKEEP_VERSION, BLOXKEEP_UPDATE_REPO), and so do the notes shown after
rem updating (RobloxManager\whatsnew.json).
echo [2/5] App files...
pushd ui
if not exist node_modules\.package-lock.json call npm ci
if errorlevel 1 (popd & goto :fail)
node build.mjs
if errorlevel 1 (popd & goto :fail)
popd
set BLOXKEEP_VERSION=%VERSION%
set BLOXKEEP_UPDATE_REPO=%REPO%
pushd ui-gpui
cargo build --release --locked --features core
if errorlevel 1 (popd & goto :fail)
popd
mkdir artifacts\app
copy /y ui-gpui\target\release\bloxkeep-ui.exe artifacts\app\Bloxkeep.exe >nul || goto :fail
rem robocopy's exit codes below 8 mean it copied.
robocopy ui-gpui\assets artifacts\app\assets /e /xf photo-keep.jpg tray.ico /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail
robocopy RobloxManager\wwwroot artifacts\app\wwwroot /e /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail
pushd remote-monitor
call npm ci --omit=dev --ignore-scripts --no-audit --no-fund
if errorlevel 1 (popd & goto :fail)
popd
set MONITOR=artifacts\app\wwwroot\remote-monitor
robocopy remote-monitor "%MONITOR%" /e /xd node_modules /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail
robocopy remote-monitor\node_modules\jpeg-js "%MONITOR%\node_modules\jpeg-js" /e /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail
robocopy remote-monitor\node_modules\hls.js "%MONITOR%\node_modules\hls.js" LICENSE /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail
robocopy remote-monitor\node_modules\hls.js\dist "%MONITOR%\node_modules\hls.js\dist" hls.light.min.mjs /njh /njs /ndl /nfl /np >nul
if errorlevel 8 goto :fail

rem The checks (ui-gpui\src\core\tests, each suite in its own process on throwaway data; the ones that need the
rem network are left out): a failing one stops the build.
echo [3/5] Checks and the uninstaller...
rem Through PowerShell's pipe: the app is a Windows program, so cmd would not show what it prints.
powershell -NoProfile -Command "& .\artifacts\app\Bloxkeep.exe --core-suites --timeout=15 | Write-Host; exit $LASTEXITCODE"
if errorlevel 1 goto :fail
rem Bloxkeep's own installer and uninstaller windows (Installer\Setup\Setup.cs), compiled with the C# compiler that
rem ships with Windows (.NET Framework 4.x), so they need nothing installed. Velopack does the work underneath them.
rem The uninstaller travels inside the app; the app points Installed apps > Uninstall at it.
set CSC="%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /codepage:65001 /target:winexe /optimize+ /win32icon:RobloxManager\app.ico /win32manifest:Installer\Setup\app.manifest /resource:RobloxManager\app.ico,app.ico /r:System.Windows.Forms.dll /r:System.Drawing.dll
powershell -NoProfile -ExecutionPolicy Bypass -File Installer\Setup\info.ps1 %VERSION% || goto :fail
%CSC% /out:artifacts\app\BloxkeepUninstall.exe Installer\Setup\Setup.cs artifacts\SetupInfo.cs || goto :fail

if not "%REPO%"=="" (
  echo [4/5] Previous release, so updates only download what changed...
  dotnet vpk download github --repoUrl %REPO% -o dist\releases || echo     none yet - this will be the first release
) else (
  echo [4/5] No repo url: skipping the previous release
)

echo [5/5] Installer, portable zip and update packages...
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release-notes.ps1 %VERSION% artifacts\release-notes.md || goto :fail
rem What these become on the user's PC (Velopack 1.2.161):
rem   --packId         the install folder, %LOCALAPPDATA%\BloxkeepApp, and the name of the uninstall registry key
rem   --packTitle      "Bloxkeep" in Windows' Installed apps list, the shortcut names and the installer's dialogs
rem   --packAuthors    the Publisher column in Installed apps (keep it the same as Company in the csproj)
rem   --icon           the icon of Setup.exe; the shortcuts and the Apps list take theirs from Bloxkeep.exe itself
rem   --shortcuts      Desktop and the Start menu's root (no folder), like VS Code and Discord; a user who deletes one
rem                    doesn't get it back on update
rem   --framework      Setup installs Microsoft Edge WebView2 first if the PC lacks it (asks, then downloads it from
rem                    Microsoft, so that needs internet); --splashImage is shown at 100 %% and scaled up on high-DPI screens
dotnet vpk pack --packId BloxkeepApp --packVersion %VERSION% --packDir artifacts\app --mainExe Bloxkeep.exe ^
  --packTitle Bloxkeep --packAuthors minhhoang0122 --icon RobloxManager\app.ico --runtime win-x64 ^
  --shortcuts Desktop,StartMenuRoot ^
  --splashImage Installer\splash.png --splashProgressColor #f5c933 --framework webview2 ^
  --releaseNotes artifacts\release-notes.md -o dist\releases || goto :fail

rem Bloxkeep-Setup-Offline.exe is our installer window with vpk's installer inside it (vpk's alone installs to
rem %LOCALAPPDATA% without asking): people choose the folder, on any drive, and it runs vpk's silently with --installto.
rem Bloxkeep-Setup.exe, the web installer, downloads it from the release and does the same.
move /y dist\releases\BloxkeepApp-win-Setup.exe artifacts\VelopackSetup.exe >nul || goto :fail
powershell -NoProfile -ExecutionPolicy Bypass -File Installer\Setup\info.ps1 %VERSION% || goto :fail
%CSC% /out:dist\releases\Bloxkeep-Setup-Offline.exe /resource:artifacts\VelopackSetup.exe,VelopackSetup.exe Installer\Setup\Setup.cs artifacts\SetupInfo.cs || goto :fail
rem Plain names for the files people download, and the web installer and SHA256SUMS.txt in the upload list (the
rem update packages keep vpk's names).
powershell -NoProfile -ExecutionPolicy Bypass -File Installer\assets.ps1 || goto :fail

powershell -NoProfile -ExecutionPolicy Bypass -File Installer\checksums.ps1 || goto :fail

echo.
echo Done:
dir /b dist\releases
exit /b 0

:fail
echo.
echo Build failed.
exit /b 1
