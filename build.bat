@echo off
rem Builds Bloxkeep into dist\releases (needs the .NET 10 SDK; the app is self-contained, users don't need .NET):
rem   Bloxkeep-Setup.exe            one-click installer for users (installs per user, installs WebView2 if missing)
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
if not exist dist\releases mkdir dist\releases

echo [1/4] Tools...
dotnet tool restore >nul || goto :fail

echo [2/4] App files...
dotnet publish RobloxManager\RobloxManager.csproj -c Release -r win-x64 --self-contained true -p:Version=%VERSION% -p:UpdateRepo=%REPO% -p:DebugType=none -p:AllowedReferenceRelatedFileExtensions=none -o artifacts\app || goto :fail

if not "%REPO%"=="" (
  echo [3/4] Previous release, so updates only download what changed...
  dotnet vpk download github --repoUrl %REPO% -o dist\releases || echo     none yet - this will be the first release
) else (
  echo [3/4] No repo url: skipping the previous release
)

echo [4/4] Installer, portable zip and update packages...
powershell -NoProfile -ExecutionPolicy Bypass -File release-notes.ps1 %VERSION% artifacts\release-notes.md || goto :fail
dotnet vpk pack --packId BloxkeepApp --packVersion %VERSION% --packDir artifacts\app --mainExe Bloxkeep.exe ^
  --packTitle Bloxkeep --packAuthors Bloxkeep --icon RobloxManager\app.ico --runtime win-x64 ^
  --splashImage Installer\splash.png --splashProgressColor #f5c933 --framework webview2 ^
  --releaseNotes artifacts\release-notes.md -o dist\releases || goto :fail

rem Plain names for the two files people download; the update packages keep vpk's names.
move /y dist\releases\BloxkeepApp-win-Setup.exe dist\releases\Bloxkeep-Setup.exe >nul || goto :fail
move /y dist\releases\BloxkeepApp-win-Portable.zip dist\releases\Bloxkeep-Portable.zip >nul || goto :fail
powershell -NoProfile -Command "$f = 'dist\releases\assets.win.json'; [IO.File]::WriteAllText($f, [IO.File]::ReadAllText($f).Replace('BloxkeepApp-win-Setup.exe', 'Bloxkeep-Setup.exe').Replace('BloxkeepApp-win-Portable.zip', 'Bloxkeep-Portable.zip'))" || goto :fail

powershell -NoProfile -Command "Get-ChildItem dist\releases -File | Where-Object Name -ne 'SHA256SUMS.txt' | ForEach-Object { '{0}  {1}' -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name } | Set-Content -Encoding ascii dist\releases\SHA256SUMS.txt" || goto :fail

echo.
echo Done:
dir /b dist\releases
exit /b 0

:fail
echo.
echo Build failed.
exit /b 1
