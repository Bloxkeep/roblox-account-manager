# Generates release checksums using .NET, including in Windows PowerShell without Get-FileHash installed.
param([string]$Directory = (Join-Path (Split-Path $PSScriptRoot) 'dist\releases'))
$ErrorActionPreference = 'Stop'
$releaseDir = [IO.Path]::GetFullPath($Directory)
$manifest = Join-Path $releaseDir 'SHA256SUMS.txt'
$lines = @(Get-ChildItem -LiteralPath $releaseDir -File | Where-Object Name -ne 'SHA256SUMS.txt' | Sort-Object Name | ForEach-Object {
    $stream = [IO.File]::OpenRead($_.FullName)
    $sha = [Security.Cryptography.SHA256]::Create()
    try {
        $digest = [BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '').ToLowerInvariant()
        '{0}  {1}' -f $digest, $_.Name
    } finally {
        $sha.Dispose()
        $stream.Dispose()
    }
})
[IO.File]::WriteAllLines($manifest, [string[]]$lines, [Text.Encoding]::ASCII)
Write-Host ('Wrote SHA256SUMS.txt for {0} files.' -f $lines.Count)