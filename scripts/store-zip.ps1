# Packs the built installer into the zip that Nexus Mods and GameBanana get (both want an archive, not a bare .exe).
# Run after `npx tauri build`. Writes target\store\Pockymod-<version>-setup.zip with the installer, the license and a
# short read me. The release workflow runs it too and attaches the zip to the GitHub release.
param([string]$Installer)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot
if (-not $Installer) {
  $found = Get-ChildItem (Join-Path $root "target\release\bundle\nsis\Pockymod_*-setup.exe") -ErrorAction SilentlyContinue |
    Sort-Object LastWriteTime -Descending | Select-Object -First 1
  if (-not $found) { throw "No installer in target\release\bundle\nsis. Run npx tauri build first." }
  $Installer = $found.FullName
}
$exe = Get-Item $Installer
# Pockymod_1.0.0-alpha.2_x64-setup.exe -> 1.0.0-alpha.2
if ($exe.Name -notmatch '^Pockymod_(.+)_x64-setup\.exe$') { throw "Unexpected installer name: $($exe.Name)" }
$version = $Matches[1]

$out = Join-Path $root "target\store"
$work = Join-Path $out "Pockymod-$version"
if (Test-Path $work) { Remove-Item -Recurse -Force $work }
New-Item -ItemType Directory -Force $work | Out-Null

# the installer keeps its own name: the app spots it by that name and won't install itself as a mod
Copy-Item $exe.FullName $work
Copy-Item (Join-Path $root "LICENSE") (Join-Path $work "LICENSE.txt")
$readme = @"
Pockymod $version

Run $($exe.Name) to install. It installs for your Windows user, so it doesn't need admin rights.

The installer isn't signed yet, so Windows may say it protected your PC. Click More info, then Run anyway.

After that, Pockymod keeps itself up to date from GitHub (you can turn this off in Settings, Background).
Source code, bug reports and older versions: https://github.com/imanimal/Pockymod
"@
# CRLF so Notepad on any Windows shows the line breaks
Set-Content -Path (Join-Path $work "Read me.txt") -Value ($readme -replace "`r?`n", "`r`n") -Encoding UTF8 -NoNewline

$zip = Join-Path $out "Pockymod-$version-setup.zip"
if (Test-Path $zip) { Remove-Item -Force $zip }
Compress-Archive -Path (Join-Path $work "*") -DestinationPath $zip
Remove-Item -Recurse -Force $work
Write-Output $zip
