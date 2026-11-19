# Builds Driftwave and packages it as a release: dist\Driftwave-<version>-win-x64.zip and its .sha256, the two assets
# the in-app updater looks for on a v<version> release. Publishing them is a separate, deliberate step.
# The zip holds Driftwave.exe, the built-in themes and their art, Install.ps1, uninstall.ps1 and README.txt.
# Usage: scripts\package-windows.ps1 [-NoBuild]
param([switch]$NoBuild)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
if (-not $NoBuild) {
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $repo 'rust\cargo.ps1') build --release -p driftwave-app
    if ($LASTEXITCODE -ne 0) { throw 'The build failed.' }
}
$exe = Join-Path $repo 'rust\target\release\driftwave.exe'
$version = (Get-Item $exe).VersionInfo.ProductVersion
if (-not $version) { throw "$exe has no version (was it built with the Windows SDK's resource compiler?)" }
$name = "Driftwave-$version-win-x64"
$dist = Join-Path $repo 'dist'
$stage = Join-Path $dist $name
# Only this script's own staging folder is ever replaced.
if (Test-Path (Join-Path $stage 'Driftwave.exe')) { Remove-Item -LiteralPath $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item $exe (Join-Path $stage 'Driftwave.exe')
Copy-Item (Join-Path $repo 'themes') (Join-Path $stage 'themes') -Recurse
Copy-Item (Join-Path $repo 'assets\theme-art') (Join-Path $stage 'theme-art') -Recurse
Copy-Item (Join-Path $repo 'assets\icons\driftwave.ico') $stage
foreach ($f in 'Install.ps1', 'uninstall.ps1', 'README.txt') { Copy-Item (Join-Path $PSScriptRoot "windows\$f") $stage }

$zip = Join-Path $dist "$name.zip"
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
$hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLowerInvariant()
[IO.File]::WriteAllText("$zip.sha256", "$hash  $name.zip`n")
Write-Output "$zip ($([math]::Round((Get-Item $zip).Length / 1MB, 1)) MB)"
Write-Output "$zip.sha256: $hash"
