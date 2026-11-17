# Builds Shortwave and installs it for the current user: %LOCALAPPDATA%\Programs\<name>, a Start menu shortcut
# (carrying the app's AppUserModelID, so the media overlay and notifications show its name and icon) and an
# entry in Settings > Apps. No admin rights needed. Release ("Shortwave") and Nightly ("Shortwave Nightly")
# install side by side. Your library and settings are kept across installs and uninstalls.
# Usage: windows\scripts\install.ps1 [-Flavor Nightly|Release] [-NoBuild]
param(
    [ValidateSet('Nightly', 'Release')][string]$Flavor = 'Nightly',
    [switch]$NoBuild
)
. "$PSScriptRoot\common.ps1"
$name = Get-ShortwaveName $Flavor
$aumid = if ($Flavor -eq 'Release') { 'Joshspeakman.Shortwave' } else { 'Joshspeakman.Shortwave.Nightly' }

if (-not (& dotnet --list-runtimes 2>$null | Select-String 'Microsoft.NETCore.App 10\.')) {
    Write-Warning 'The .NET 10 runtime is not installed; Shortwave will not start without it (winget install Microsoft.DotNet.Runtime.10).'
}
if (-not $NoBuild) { & "$PSScriptRoot\build.ps1" -Flavor $Flavor | Out-Host }
$src = Get-BuildDir $Flavor
if (-not (Test-Path (Join-Path $src 'Shortwave.exe'))) { throw "No build in $src" }

$dest = Join-Path $env:LOCALAPPDATA "Programs\$name"
# Ask a running copy of this flavor to quit (files in use can't be replaced), then make sure it is gone.
if (Test-Path (Join-Path $dest 'Shortwave.exe')) { Start-Process -FilePath (Join-Path $dest 'Shortwave.exe') -ArgumentList '--quit' -Wait }
Get-Process Shortwave -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($dest, [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object { if (-not $_.WaitForExit(5000)) { Stop-Process -Id $_.Id -Force } }

New-Item -ItemType Directory -Force $dest | Out-Null
& robocopy $src $dest /MIR /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Copying to $dest failed (robocopy $LASTEXITCODE)" }
Copy-Item (Join-Path $PSScriptRoot 'uninstall.ps1') (Join-Path $dest 'uninstall.ps1') -Force
$exe = Join-Path $dest 'Shortwave.exe'

$lnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$name.lnk"
Start-Process -FilePath $exe -ArgumentList @('--create-shortcut', "`"$lnk`"") -Wait   # a GUI exe: & would not wait
if (-not (Test-Path $lnk)) { throw "Shortcut not created: $lnk" }

$version = (Get-Item $exe).VersionInfo.ProductVersion
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$aumid"
New-Item -Path $key -Force | Out-Null
$uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$dest\uninstall.ps1`" -Flavor $Flavor"
$values = @{
    DisplayName = $name; DisplayIcon = "$exe,0"; DisplayVersion = $version; Publisher = 'Josh Speakman'
    InstallLocation = $dest; UninstallString = $uninstall; QuietUninstallString = $uninstall
}
foreach ($k in $values.Keys) { Set-ItemProperty -Path $key -Name $k -Value $values[$k] }
Set-ItemProperty -Path $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $key -Name NoRepair -Value 1 -Type DWord
$size = [int]((Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
Set-ItemProperty -Path $key -Name EstimatedSize -Value $size -Type DWord

Write-Output "Installed $name $version to $dest"
Write-Output "Start menu: $lnk"
