# Writes the GitHub release notes for one version from RobloxManager\whatsnew.json: English, then Vietnamese.
# Used by build.bat. Usage: release-notes.ps1 <version> <output .md>
param([string]$Version, [string]$Out)

$release = (Get-Content -Raw -Encoding UTF8 (Join-Path $PSScriptRoot 'RobloxManager\whatsnew.json') | ConvertFrom-Json) |
    Where-Object { $_.version -eq $Version }
if (-not $release) {
    # A quiet fix (1.0.7-alpha.1, see release-fix.ps1) has no notes of its own: the next public version's say what it fixed.
    if ($Version -match '-alpha\.\d+$') {
        [IO.File]::WriteAllText($Out, 'Small fixes.')
        exit 0
    }
    Write-Host "Add the release notes for $Version to RobloxManager\whatsnew.json first."
    exit 1
}
# Built from char codes: Windows PowerShell reads a script without a BOM as ANSI, which would garble the accents.
$vietnamese = 'Ti' + [char]0x1EBF + 'ng Vi' + [char]0x1EC7 + 't'
$heads = @{
    en = @{ new = 'New'; improved = 'Improved'; fixed = 'Fixed' }
    vi = @{ new = 'M' + [char]0x1EDB + 'i'; improved = 'C' + [char]0x1EA3 + 'i thi' + [char]0x1EC7 + 'n'; fixed = [char]0x0110 + [char]0x00E3 + ' s' + [char]0x1EED + 'a' }
}
# "Title: what it does" gets its title in bold, as the app's What's new shows it (app.js, splitNote). Headline features
# (highlights: [title, text, button]) come first, the same way, then the notes in no group, then New, Improved and
# Fixed under their own headings, as in the app.
function Note([string]$n) { '- ' + ($n -replace '^([^.:]{1,60}): ', '**$1**: ') }
function Part([string]$lang) {
    $out = @($release.highlights | Where-Object { $_ } | ForEach-Object { $w = $_.$lang; "- **$($w[0])**: $($w[1])" }) + @($release.$lang | Where-Object { $_ } | ForEach-Object { Note $_ })
    foreach ($kind in 'new', 'improved', 'fixed') {
        $items = @($release.$kind.$lang | Where-Object { $_ })
        if ($items.Count) { $out += @('', "#### $($heads[$lang][$kind])") + @($items | ForEach-Object { Note $_ }) }
    }
    $out
}
$lines = @(Part 'en') + '' + "**$vietnamese**" + '' + @(Part 'vi')
# Without highlights or notes in no group, a part starts with a heading: no blank lines before it, one between.
[IO.File]::WriteAllText($Out, (($lines -join "`n") -replace "`n{3,}", "`n`n").Trim())
