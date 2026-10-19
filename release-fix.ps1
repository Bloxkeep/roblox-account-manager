# Publishes a quiet fix that build.bat made in dist\releases: a small fix after a public version, numbered below the
# next one (1.0.7-alpha.1 after 1.0.6; see WhatsNew.IsQuietFix). Instead of a release of its own, its update packages go
# inside the newest public release, whose releases.win.json then lists them too. Installed copies read the feeds of the
# newest releases together, so they find it on their next check and install it in the background, without "Restart to
# update" or notes, and still show the public version. The Releases page gets no new release.
# Used by release.bat for such a version. Needs gh, with GITHUB_TOKEN (or gh signed in) able to edit the releases.
#   release-fix.ps1 <version> <github repo url>   publish
#   release-fix.ps1 next <github repo url>        print the number the next quiet fix takes
param([string]$Version, [string]$Repo)
$ErrorActionPreference = 'Stop'
$repoName = ([Uri]$Repo).AbsolutePath.Trim('/')
$dist = Join-Path $PSScriptRoot 'dist\releases'

# The newest public release ("Latest" on the Releases page) and the quiet fixes already inside it.
$tag = gh release view --repo $repoName --json tagName --jq .tagName
if ($LASTEXITCODE -ne 0 -or -not $tag) { throw "No public release in $repoName to put the fix in." }
$base = [version]$tag.TrimStart('v')
$work = Join-Path ([IO.Path]::GetTempPath()) ("bloxkeep-fix-" + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $work | Out-Null
gh release download $tag --repo $repoName --pattern releases.win.json --dir $work
if ($LASTEXITCODE -ne 0) { throw "$tag has no releases.win.json." }
$feed = Get-Content -Raw -Encoding UTF8 (Join-Path $work 'releases.win.json') | ConvertFrom-Json
$next = "$($base.Major).$($base.Minor).$($base.Build + 1)"
$fixes = @($feed.Assets | Where-Object { $_.Version -like "$next-alpha.*" } | ForEach-Object { [int]($_.Version -replace '^.*-alpha\.', '') })

if ($Version -eq 'next') {
    $n = if ($fixes.Count) { ($fixes | Measure-Object -Maximum).Maximum + 1 } else { 1 }
    "$next-alpha.$n"
    exit 0
}
if ($Version -notlike "$next-alpha.*") { throw "A quiet fix after $base is numbered $next-alpha.N, not $Version." }
if ($feed.Assets | Where-Object { $_.Version -eq $Version }) { throw "$Version is already in $tag." }

$built = Get-Content -Raw -Encoding UTF8 (Join-Path $dist 'releases.win.json') | ConvertFrom-Json
$new = @($built.Assets | Where-Object { $_.Version -eq $Version })
if (-not ($new | Where-Object { $_.Type -eq 'Full' })) { throw "dist\releases has no full package of ${Version}: run build.bat $Version first." }

# The packages first, then the list that points at them: a copy checking in between never finds a missing file.
foreach ($asset in $new) {
    gh release upload $tag (Join-Path $dist $asset.FileName) --repo $repoName
    if ($LASTEXITCODE -ne 0) { throw "Uploading $($asset.FileName) failed." }
}
$merged = [ordered]@{ Assets = @($feed.Assets) + $new }
$json = Join-Path $work 'releases.win.json'
[IO.File]::WriteAllText($json, ($merged | ConvertTo-Json -Depth 6), (New-Object Text.UTF8Encoding($false)))
gh release upload $tag $json --repo $repoName --clobber
if ($LASTEXITCODE -ne 0) { throw "Uploading the new releases.win.json failed: the packages are there, the list isn't yet." }
Remove-Item -Recurse -Force $work
"$Version is in $tag; installed copies pick it up on their next check."
