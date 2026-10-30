@echo off
rem Publishes what build.bat made in dist\releases as a GitHub release. Installed copies of Bloxkeep find it on
rem their next update check, download it in the background and offer "Restart to update" (a quiet fix: none of that,
rem see release-fix.ps1).
rem Usage: scripts\release.bat <version> <github repo url>
rem Needs GITHUB_TOKEN: a token with write access to that repository's contents (releases).
setlocal
cd /d "%~dp0.."
if "%~2"=="" (
  echo Usage: scripts\release.bat ^<version^> ^<github repo url^>
  exit /b 1
)
if "%GITHUB_TOKEN%"=="" (
  echo Set GITHUB_TOKEN first.
  exit /b 1
)
rem A beta (1.0.5-beta.1) goes up as a pre-release: only copies with Settings > Get beta versions on take it, and it
rem never becomes the Latest release the web installer downloads.
set VERSION=%~1
rem A quiet fix (1.0.7-alpha.1) gets no release of its own: it goes inside the newest public one (release-fix.ps1).
echo %VERSION%| findstr /r /c:"-alpha\.[0-9][0-9]*$" >nul && goto :fix
rem Only a number above the Latest release goes up: a lower one (a typo, an older branch's build run from Actions) became
rem a release no copy installs, or the Latest over a newer one (review, 2026-10-30). Quiet fixes are checked in release-fix.ps1.
powershell -NoProfile -Command "$r = ([Uri]'%~2').AbsolutePath.Trim('/'); $t = gh release view --repo $r --json tagName --jq .tagName; if ($LASTEXITCODE -ne 0 -or -not $t) { Write-Host ('Could not read the Latest release of ' + $r + '.'); exit 1 }; if ([version]('%VERSION%' -replace '-.*$', '') -le [version]$t.TrimStart('v')) { Write-Host ('%VERSION% is not above the Latest release, ' + $t + '.'); exit 1 }" || exit /b 1
set PRE=
if not "%VERSION:-=%"=="%VERSION%" set PRE=--pre
rem The tag goes on the commit that was built, not on the default branch's newest one (GitHub's choice for a new tag): a
rem build run from Actions on another branch tagged the wrong source with its files (review, 2026-10-30).
set COMMIT=
for /f %%c in ('git rev-parse HEAD') do set COMMIT=%%c
if "%COMMIT%"=="" (
  echo Could not tell which commit was built.
  exit /b 1
)
dotnet tool restore >nul || exit /b 1
dotnet vpk upload github --repoUrl %~2 --token %GITHUB_TOKEN% -o dist\releases --publish %PRE% --releaseName "Bloxkeep %~1" --tag v%~1 --targetCommitish %COMMIT%
exit /b %ERRORLEVEL%

:fix
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\release-fix.ps1 %VERSION% %~2
