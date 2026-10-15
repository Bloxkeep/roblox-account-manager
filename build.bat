@echo off
rem Builds Bloxkeep into dist\releases (needs the .NET 10 SDK; the app is self-contained, users don't need .NET):
rem   Bloxkeep-Setup.exe            installer for users: they choose the folder (any drive); installs per user without
rem                                 admin rights, and installs WebView2 if missing. A small web installer, the same
rem                                 file in every release (Installer\Web): it downloads the newest offline one
rem   Bloxkeep-Setup-Offline.exe    the same installer with everything inside, for installing without internet
rem   Bloxkeep-Portable.zip         no install; keeps its data in a "data" folder next to it; updates itself too
rem   *.nupkg + releases.win.json   what installed copies download when they update
rem Usage: build.bat [version] [github repo url]   e.g. build.bat 1.0.1
rem The repo url defaults to Bloxkeep's GitHub repo; pass another one to build a fork. Updates only work while the repo is public. Each version needs its notes in RobloxManager\whatsnew.json
rem (shown in the app after updating, and on the GitHub release). Publish with release.bat.
setlocal
cd /d "%~dp0"
set VERSION=1.0.0
if not "%~1"=="" set VERSION=%~1
set REPO=https://github.com/minhhoang0122/bloxkeep
if not "%~2"=="" set REPO=%~2

if exist artifacts\app rmdir /s /q artifacts\app
rem Files left from an earlier build would be listed in SHA256SUMS.txt and stop the same version from packing again.
if exist dist\releases rmdir /s /q dist\releases
mkdir dist\releases

echo [1/5] Tools...
dotnet tool restore >nul || goto :fail

rem The checks in Tests\Bloxkeep.Tests (each in its own process, with throwaway data): a failing one stops the build.
echo [2/5] Checks...
dotnet run --project Tests\Bloxkeep.Tests -c Release || goto :fail

echo [3/5] App files...
dotnet publish RobloxManager\RobloxManager.csproj -c Release -r win-x64 --self-contained true -p:Version=%VERSION% -p:UpdateRepo=%REPO% -p:DebugType=none -p:AllowedReferenceRelatedFileExtensions=none -o artifacts\app || goto :fail
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
powershell -NoProfile -ExecutionPolicy Bypass -File release-notes.ps1 %VERSION% artifacts\release-notes.md || goto :fail
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

powershell -NoProfile -Command "Get-ChildItem dist\releases -File | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name } | Set-Content -Encoding ascii dist\releases\SHA256SUMS.txt" || goto :fail

echo.
echo Done:
dir /b dist\releases
exit /b 0

:fail
echo.
echo Build failed.
exit /b 1
