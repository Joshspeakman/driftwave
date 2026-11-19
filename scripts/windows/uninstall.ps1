# Removes Driftwave: the program, its Start menu shortcut and its entry in Settings > Apps (which runs this).
# Your stations and settings (%APPDATA%\DriftwaveRust) and the songs you saved stay.
# Installed beside Driftwave.exe; the in-app updater also uses this file to recognise an installed copy.
# (-StartMenu and -NoRegistry are for trying it out in a scratch folder.)
param([string]$StartMenu = [Environment]::GetFolderPath('Programs'), [switch]$NoRegistry)
$ErrorActionPreference = 'Stop'
$name = 'Driftwave'
$aumid = 'Joshspeakman.Driftwave'
$exeName = 'Driftwave.exe'
$dest = $PSScriptRoot

$exe = Join-Path $dest $exeName
if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--quit' -Wait }
Get-Process ([IO.Path]::GetFileNameWithoutExtension($exeName)) -ErrorAction SilentlyContinue |
    Where-Object { $_.Path -and $_.Path.StartsWith($dest.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) } |
    ForEach-Object { if (-not $_.WaitForExit(5000)) { Stop-Process -Id $_.Id -Force } }

$lnk = Join-Path $StartMenu "$name.lnk"
if (Test-Path $lnk) { Remove-Item -LiteralPath $lnk -Force }
if (-not $NoRegistry) { Remove-Item "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$aumid" -Recurse -Force -ErrorAction SilentlyContinue }

# The program's own folder, and only that (this script lives in it, so step out first).
Set-Location $env:TEMP
if ((Split-Path -Leaf $dest) -eq $name -and (Test-Path (Join-Path $dest $exeName))) {
    Remove-Item -LiteralPath $dest -Recurse -Force
}
Write-Output "Driftwave is uninstalled. Your stations, settings and saved songs are still there."
