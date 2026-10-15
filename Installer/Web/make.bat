@echo off
rem Builds the web installer, Installer\Web\Bloxkeep-Setup.exe: the installer window of Installer\Setup\Setup.cs with
rem Web.cs (/define:WEB), and no Bloxkeep inside. build.bat ships this committed file unchanged in every release, so its
rem hash stays the same and the reputation Windows SmartScreen gives it carries over from release to release.
rem Rebuilding makes a different file (the compiler stamps each build), which starts that reputation over: rebuild only
rem when the web installer itself needs a change, then commit the new exe. Changes to Setup.cs reach the offline
rem installer and the uninstaller at every build without this.
setlocal
cd /d "%~dp0..\.."
"%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe" /nologo /codepage:65001 /target:winexe /optimize+ /define:WEB ^
  /win32icon:RobloxManager\app.ico /win32manifest:Installer\Setup\app.manifest /resource:RobloxManager\app.ico,app.ico ^
  /r:System.Windows.Forms.dll /r:System.Drawing.dll /out:Installer\Web\Bloxkeep-Setup.exe ^
  Installer\Setup\Setup.cs Installer\Web\Web.cs Installer\Web\WebInfo.cs || exit /b 1
powershell -NoProfile -Command "'SHA-256: ' + (Get-FileHash Installer\Web\Bloxkeep-Setup.exe -Algorithm SHA256).Hash.ToLower()"
