# Writes artifacts\SetupInfo.cs for the installer and uninstaller windows: their version, and the space an install
# takes (the program files, the package Velopack keeps for updates, and room for Update.exe and the unpacking). Run by
# build.bat before packing (for the uninstaller, the package size guessed) and after (measured, for the installer).
param([Parameter(Mandatory)][string]$Version)
$ErrorActionPreference = 'Stop'
$root = Split-Path (Split-Path $PSScriptRoot)
$app = (Get-ChildItem (Join-Path $root 'artifacts\app') -Recurse -File | Measure-Object Length -Sum).Sum
$package = Join-Path $root "dist\releases\BloxkeepApp-$Version-full.nupkg"
$nupkg = if (Test-Path $package) { (Get-Item $package).Length } else { [long]($app * 0.45) }
$bytes = $app + $nupkg + 64MB
$numeric = ($Version -split '[-+]')[0]
@"
[assembly: System.Reflection.AssemblyTitle("Bloxkeep Setup")]
[assembly: System.Reflection.AssemblyProduct("Bloxkeep")]
[assembly: System.Reflection.AssemblyCompany("minhhoang0122")]
[assembly: System.Reflection.AssemblyVersion("$numeric")]
[assembly: System.Reflection.AssemblyFileVersion("$numeric")]
[assembly: System.Reflection.AssemblyInformationalVersion("$Version")]
[assembly: System.Reflection.AssemblyMetadata("InstallBytes", "$bytes")]
"@ | Set-Content (Join-Path $root 'artifacts\SetupInfo.cs') -Encoding ascii
