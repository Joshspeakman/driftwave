# Builds Driftwave and publishes it as a GitHub release on the public repo (github.com/Joshspeakman/driftwave), where
# the app's Settings > Updates finds it. Nightly: tag nightly-<version>, a pre-release. Release: tag v<version>.
# The zip holds the app (no .pdb files), Install.ps1, uninstall.ps1 and a README; a .sha256 file sits beside it.
# The package is scanned for personal paths first and the script stops if it finds any.
# Usage: windows\scripts\publish-release.ps1 [-Flavor Nightly|Release] [-DryRun] [-NoBuild] [-OutDir <folder>] [-NotesFile <md>]
#   -NotesFile: the "What's new" part of the notes (default: the Windows commits since the last release).
#   -DryRun builds and packages into a scratch folder without publishing.
param(
    [ValidateSet('Nightly', 'Release')][string]$Flavor = 'Nightly',
    [switch]$DryRun,
    [switch]$NoBuild,
    [string]$Repo = 'Joshspeakman/driftwave',
    [string]$OutDir = '',
    [string]$NotesFile = ''
)
. "$PSScriptRoot\common.ps1"
if (-not $NoBuild) { & "$PSScriptRoot\build.ps1" -Flavor $Flavor -NoSymbols | Out-Host }
$src = Get-BuildDir $Flavor
$exe = Join-Path $src 'Driftwave.exe'
if (-not (Test-Path $exe)) { throw "No build in $src" }
$product = (Get-Item $exe).VersionInfo.ProductVersion
$version = $product.Split('+')[0]
$commit = if ($product.Contains('+')) { $product.Split('+')[1] } else { (git -C $RepoRoot rev-parse --short HEAD) }
$nightly = $Flavor -eq 'Nightly'
$tag = if ($nightly) { "nightly-$version" } else { "v$version" }
$title = if ($nightly) { "Driftwave Nightly $version" } else { "Driftwave $version" }
$zipName = if ($nightly) { "Driftwave-Nightly-$version-win-x64.zip" } else { "Driftwave-$version-win-x64.zip" }

$stage = if ($OutDir) { New-Item -ItemType Directory -Force $OutDir | Select-Object -ExpandProperty FullName } else { New-ScratchDir 'driftwave-release' }
$pkg = Join-Path $stage 'pkg'
if (Test-Path $pkg) { Remove-Item -Recurse -Force $pkg }
& robocopy $src $pkg /E /XF *.pdb /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Packaging failed (robocopy $LASTEXITCODE)" }
Copy-Item (Join-Path $PSScriptRoot 'install-files.ps1') (Join-Path $pkg 'Install.ps1')
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') (Join-Path $pkg 'uninstall.ps1')
Set-Content -Path (Join-Path $pkg 'flavor.txt') -Value $Flavor -Encoding ascii
@"
$title

Internet radio for Windows 10/11 (x64). Needs the .NET 10 Desktop Runtime:
winget install Microsoft.DotNet.DesktopRuntime.10

Install: right-click Install.ps1 and choose "Run with PowerShell" (no admin rights needed).
It installs to %LOCALAPPDATA%\Programs\$(if ($nightly) { 'Driftwave Nightly' } else { 'Driftwave' }) with a Start menu shortcut.
Updates: Settings > Updates in the app.
Uninstall: Settings > Apps, or uninstall.ps1 in the install folder.

Driftwave is inspired by GNOME Shortwave by Felix Haecker and contributors (https://gitlab.gnome.org/World/Shortwave).
It contains none of Shortwave's code, artwork or other assets. Station data: radio-browser.info.
Bundled fonts are under the SIL Open Font License (Assets\Fonts\*-OFL.txt).
https://github.com/$Repo
"@ | Set-Content -Path (Join-Path $pkg 'README.txt') -Encoding utf8

# Nothing personal may ship: no user name, no local source paths.
$needles = @("\Users\$env:USERNAME", "/Users/$env:USERNAME", 'I:\Projects', 'I:/Projects', 'shortwave-macos') | Where-Object { $_ }
$hits = foreach ($f in Get-ChildItem $pkg -Recurse -File) {
    $bytes = [IO.File]::ReadAllBytes($f.FullName)
    $text = [Text.Encoding]::ASCII.GetString($bytes) + [Text.Encoding]::Unicode.GetString($bytes)
    foreach ($n in $needles) { if ($text.IndexOf($n, [StringComparison]::OrdinalIgnoreCase) -ge 0) { "$($f.Name): $n" } }
}
if ($hits) { throw "Personal paths in the package, not publishing:`n$($hits -join "`n")" }

$zip = Join-Path $stage $zipName
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path (Join-Path $pkg '*') -DestinationPath $zip -CompressionLevel Optimal
$sha = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
Set-Content -Path "$zip.sha256" -Value "$sha  $zipName" -Encoding ascii

# Release notes: the Windows changes since the previous release of this channel (its notes end with "Commit: <sha>").
$previous = $null
try {
    $list = @(gh release list --repo $Repo --limit 30 --json tagName 2>$null | ConvertFrom-Json)
    $prevTag = $list | Where-Object { $_.tagName -like $(if ($nightly) { 'nightly-*' } else { 'v*' }) } | Select-Object -First 1 -ExpandProperty tagName
    if ($prevTag) { $previous = ((gh release view $prevTag --repo $Repo --json body | ConvertFrom-Json).body | Select-String 'Commit: ([0-9a-f]+)').Matches | Select-Object -First 1 | ForEach-Object { $_.Groups[1].Value } }
}
catch { Write-Output "No earlier release found on $Repo (first release?)." }
[Console]::OutputEncoding = [Text.Encoding]::UTF8   # git log writes UTF-8
$range = if ($previous) { "$previous..$commit" } else { "-15 $commit" }
$changes = if ($NotesFile) { @((Get-Content -Raw $NotesFile).Trim()) } else { @(git -C $RepoRoot log --no-merges --pretty='- %s' $range.Split(' ') -- windows themes docs/THEMES.md 2>$null) }
if (-not $changes) { $changes = @('- Maintenance build') }
$notes = @"
## What's new
$($changes -join "`n")

## Install
Download **$zipName**, unzip it, right-click **Install.ps1** and choose *Run with PowerShell*. Needs the .NET 10 Desktop Runtime (``winget install Microsoft.DotNet.DesktopRuntime.10``). Already installed? Use Settings > Updates in the app.

The ``.sha256`` file lets the app check the download arrived intact.

Commit: $commit
"@
$notesOut = Join-Path $stage 'notes.md'
[IO.File]::WriteAllText($notesOut, $notes)   # no BOM (GitHub shows it)

Write-Output "Package: $zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"
Write-Output "SHA-256: $sha"
Write-Output "Tag: $tag"
if ($DryRun) { Write-Output "Dry run: not published. Notes: $notesOut"; return }

$flags = @('--repo', $Repo, '--title', $title, '--notes-file', $notesOut)
if ($nightly) { $flags += '--prerelease' } else { $flags += '--latest' }
gh release create $tag $zip "$zip.sha256" @flags
if ($LASTEXITCODE -ne 0) { throw "gh release create failed ($LASTEXITCODE)" }
Write-Output "Published ${title}: https://github.com/$Repo/releases/tag/$tag"
