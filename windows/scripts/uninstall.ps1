# Removes an installed Driftwave (the program, its Start menu shortcut and its Settings > Apps entry).
# Your library, covers, settings and saved tracks are kept.
# Usage: uninstall.ps1 [-Flavor Nightly|Release]
param([ValidateSet('Nightly', 'Release')][string]$Flavor = 'Nightly')
$ErrorActionPreference = 'Stop'
$name = if ($Flavor -eq 'Release') { 'Driftwave' } else { 'Driftwave Nightly' }
$aumid = if ($Flavor -eq 'Release') { 'Joshspeakman.Driftwave' } else { 'Joshspeakman.Driftwave.Nightly' }
$dest = Join-Path $env:LOCALAPPDATA "Programs\$name"
$exe = Join-Path $dest 'Driftwave.exe'

# Ask a running copy of this flavor to quit (files in use can't be replaced), then make sure it is gone.
if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--quit' -Wait }
Get-Process Driftwave -ErrorAction SilentlyContinue | Where-Object { $_.Path -and $_.Path.StartsWith($dest.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object { if (-not $_.WaitForExit(5000)) { Stop-Process -Id $_.Id -Force } }
if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--unregister' -Wait }

$lnk = Join-Path ([Environment]::GetFolderPath('Programs')) "$name.lnk"
if (Test-Path $lnk) { Remove-Item $lnk -Force }
Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$aumid" -Recurse -Force -ErrorAction SilentlyContinue

# Only ever delete the install folder itself, and only if it holds Driftwave.
if ((Test-Path $exe) -and ((Split-Path -Leaf $dest) -eq $name)) {
    # This script may be running from inside the folder; remove it after this process exits.
    $cmd = "ping 127.0.0.1 -n 3 > nul & rmdir /s /q `"$dest`""
    Start-Process -FilePath cmd.exe -ArgumentList '/c', $cmd -WindowStyle Hidden
}
Write-Output "Uninstalled $name. Your library and settings in $env:APPDATA\Driftwave are kept."
