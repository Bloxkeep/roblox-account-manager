# Run by build.bat after vpk pack: gives the files people download plain names and adds the ones vpk doesn't make to
# assets.win.json, the list `vpk upload` uploads to the GitHub release.
#   Bloxkeep-Setup.exe           the web installer (Installer\Web), the same file in every release
#   Bloxkeep-Setup-Offline.exe   our installer window with vpk's installer inside (build.bat compiles it)
#   Bloxkeep-Portable.zip        vpk's portable zip
#   SHA256SUMS.txt               written after this; the web installer checks its download against it
$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot
$list = Join-Path $root 'dist\releases\assets.win.json'
Move-Item -Force (Join-Path $root 'dist\releases\BloxkeepApp-win-Portable.zip') (Join-Path $root 'dist\releases\Bloxkeep-Portable.zip')
Copy-Item -Force (Join-Path $root 'Installer\Web\Bloxkeep-Setup.exe') (Join-Path $root 'dist\releases\Bloxkeep-Setup.exe')
$assets = [IO.File]::ReadAllText($list) | ConvertFrom-Json
$assets = @($assets | Where-Object { $_.RelativeFileName -ne 'BloxkeepApp-win-Setup.exe' } | ForEach-Object {
    if ($_.RelativeFileName -eq 'BloxkeepApp-win-Portable.zip') { $_.RelativeFileName = 'Bloxkeep-Portable.zip' }
    $_
})
# vpk only knows its own kinds of file; these upload as "Installer" like its setup did.
$assets = $assets + @('Bloxkeep-Setup.exe', 'Bloxkeep-Setup-Offline.exe', 'SHA256SUMS.txt' | ForEach-Object { [pscustomobject]@{ RelativeFileName = $_; Type = 'Installer' } })
[IO.File]::WriteAllText($list, (ConvertTo-Json -InputObject $assets -Compress))
