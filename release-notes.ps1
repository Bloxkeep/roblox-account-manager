# Writes the GitHub release notes for one version from RobloxManager\whatsnew.json: English, then Vietnamese.
# Used by build.bat. Usage: release-notes.ps1 <version> <output .md>
param([string]$Version, [string]$Out)

$release = (Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot 'RobloxManager\whatsnew.json') | ConvertFrom-Json) |
    Where-Object { $_.version -eq $Version }
if (-not $release) {
    Write-Host "Add the release notes for $Version to RobloxManager\whatsnew.json first."
    exit 1
}
# Built from char codes: Windows PowerShell reads a script without a BOM as ANSI, which would garble the accents.
$vietnamese = 'Ti' + [char]0x1EBF + 'ng Vi' + [char]0x1EC7 + 't'
# "Title: what it does" gets its title in bold, as the app's What's new shows it (app.js, splitNote). Headline features
# (highlights: [title, text, button]) come first, the same way.
function Note([string]$n) { '- ' + ($n -replace '^([^.:]{1,60}): ', '**$1**: ') }
function Part([string]$lang) {
    @($release.highlights | Where-Object { $_ } | ForEach-Object { $w = $_.$lang; "- **$($w[0])**: $($w[1])" }) + @($release.$lang | ForEach-Object { Note $_ })
}
$lines = @(Part 'en') + '' + "**$vietnamese**" + '' + @(Part 'vi')
[IO.File]::WriteAllText($Out, ($lines -join "`n"))
