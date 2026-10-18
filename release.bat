@echo off
rem Publishes what build.bat made in dist\releases as a GitHub release. Installed copies of Bloxkeep find it on
rem their next update check, download it in the background and offer "Restart to update".
rem Usage: release.bat <version> <github repo url>
rem Needs GITHUB_TOKEN: a token with write access to that repository's contents (releases).
setlocal
cd /d "%~dp0"
if "%~2"=="" (
  echo Usage: release.bat ^<version^> ^<github repo url^>
  exit /b 1
)
if "%GITHUB_TOKEN%"=="" (
  echo Set GITHUB_TOKEN first.
  exit /b 1
)
rem A beta (1.0.5-beta.1) goes up as a pre-release: only copies with Settings > Get beta versions on take it, and it
rem never becomes the Latest release the web installer downloads.
set VERSION=%~1
set PRE=
if not "%VERSION:-=%"=="%VERSION%" set PRE=--pre
dotnet tool restore >nul || exit /b 1
dotnet vpk upload github --repoUrl %~2 --token %GITHUB_TOKEN% -o dist\releases --publish %PRE% --releaseName "Bloxkeep %~1" --tag v%~1
