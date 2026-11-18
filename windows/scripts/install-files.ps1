# Installs a built Driftwave for the current user: %LOCALAPPDATA%\Programs\<name>, a Start menu shortcut (carrying
# the app's AppUserModelID, so the media overlay and notifications show its name and icon) and an entry in
# Settings > Apps. No admin rights needed. Driftwave and Driftwave Nightly install side by side.
# The app used to be called Shortwave: an old "Shortwave" install of the same flavor is removed (one install only).
# Your library and settings are kept; Driftwave copies them from the old Shortwave folder on its first launch.
# Ships in the release zip as Install.ps1 (it installs the files next to it); scripts\install.ps1 builds and calls it.
# Usage: Install.ps1 [-Flavor Nightly|Release] [-Source <folder with Driftwave.exe>]
param(
    [ValidateSet('Nightly', 'Release')][string]$Flavor = $(if (Test-Path (Join-Path $PSScriptRoot 'flavor.txt')) { (Get-Content (Join-Path $PSScriptRoot 'flavor.txt') -Raw).Trim() } else { 'Nightly' }),
    [string]$Source = $PSScriptRoot
)
$ErrorActionPreference = 'Stop'
$name = if ($Flavor -eq 'Release') { 'Driftwave' } else { 'Driftwave Nightly' }
$aumid = if ($Flavor -eq 'Release') { 'Joshspeakman.Driftwave' } else { 'Joshspeakman.Driftwave.Nightly' }
if (-not (Test-Path (Join-Path $Source 'Driftwave.exe'))) { throw "No Driftwave.exe in $Source" }
if (-not (& dotnet --list-runtimes 2>$null | Select-String 'Microsoft.NETCore.App 10\.')) {
    Write-Warning 'The .NET 10 Desktop Runtime is not installed; Driftwave will not start without it (winget install Microsoft.DotNet.DesktopRuntime.10).'
}

# Stops the copies of an app that run from $folder: asks politely first (--quit), then makes sure.
function Stop-Installed([string]$folder, [string]$exeName) {
    $exe = Join-Path $folder $exeName
    if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--quit' -Wait }
    Get-Process ([IO.Path]::GetFileNameWithoutExtension($exeName)) -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($folder.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) } |
        ForEach-Object { if (-not $_.WaitForExit(5000)) { Stop-Process -Id $_.Id -Force } }
}

# The old name: remove that install (program, shortcut, Settings > Apps entry, notification registration).
$oldName = if ($Flavor -eq 'Release') { 'Shortwave' } else { 'Shortwave Nightly' }
$oldAumid = if ($Flavor -eq 'Release') { 'Joshspeakman.Shortwave' } else { 'Joshspeakman.Shortwave.Nightly' }
$oldDest = Join-Path $env:LOCALAPPDATA "Programs\$oldName"
$oldExe = Join-Path $oldDest 'Shortwave.exe'
if (Test-Path $oldExe) {
    Stop-Installed $oldDest 'Shortwave.exe'
    Start-Process -FilePath $oldExe -ArgumentList '--unregister' -Wait
    $oldLnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$oldName.lnk"
    if (Test-Path $oldLnk) { Remove-Item $oldLnk -Force }
    Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$oldAumid" -Recurse -Force -ErrorAction SilentlyContinue
    if ((Split-Path -Leaf $oldDest) -eq $oldName) { Remove-Item -LiteralPath $oldDest -Recurse -Force }
    Write-Output "Removed the old $oldName install (your library and settings are kept)."
}

$dest = Join-Path $env:LOCALAPPDATA "Programs\$name"
Stop-Installed $dest 'Driftwave.exe'
New-Item -ItemType Directory -Force $dest | Out-Null
& robocopy $Source $dest /MIR /XF Install.ps1 flavor.txt README.txt /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Copying to $dest failed (robocopy $LASTEXITCODE)" }
$uninstallSource = @((Join-Path $Source 'uninstall.ps1'), (Join-Path $PSScriptRoot 'uninstall.ps1')) | Where-Object { Test-Path $_ } | Select-Object -First 1
Copy-Item $uninstallSource (Join-Path $dest 'uninstall.ps1') -Force
$exe = Join-Path $dest 'Driftwave.exe'

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
    URLInfoAbout = 'https://github.com/Joshspeakman/driftwave'
}
foreach ($k in $values.Keys) { Set-ItemProperty -Path $key -Name $k -Value $values[$k] }
Set-ItemProperty -Path $key -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -Path $key -Name NoRepair -Value 1 -Type DWord
$size = [int]((Get-ChildItem $dest -Recurse -File | Measure-Object Length -Sum).Sum / 1KB)
Set-ItemProperty -Path $key -Name EstimatedSize -Value $size -Type DWord

Write-Output "Installed $name $version to $dest"
Write-Output "Start menu: $lnk"
