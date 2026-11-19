# Installs Driftwave for you (no admin rights needed):
# - the program in %LOCALAPPDATA%\Programs\Driftwave;
# - a Start menu shortcut, which carries the app's id so its notifications and the media overlay show its name and
#   icon;
# - an entry in Settings > Apps, to uninstall it.
# An earlier Driftwave there (the C# one too) is replaced. Your stations, settings and saved songs stay where they
# are; the first launch brings the earlier Driftwave's settings and themes across.
# Ships in the release zip (it installs the files next to it).
# Usage: right-click > Run with PowerShell, or: powershell -ExecutionPolicy Bypass -File Install.ps1 [-Launch]
# (-Dest, -StartMenu and -NoRegistry are for trying the installer out in a scratch folder.)
param(
    [string]$Source = $PSScriptRoot,
    [switch]$Launch,
    [string]$Dest = (Join-Path $env:LOCALAPPDATA 'Programs\Driftwave'),
    [string]$StartMenu = [Environment]::GetFolderPath('Programs'),
    [switch]$NoRegistry
)
$ErrorActionPreference = 'Stop'
$name = 'Driftwave'
$aumid = 'Joshspeakman.Driftwave'
$exeName = 'Driftwave.exe'
if (-not (Test-Path (Join-Path $Source $exeName))) { throw "No $exeName in $Source" }

# Stops the copies running from $folder: asks politely first (--quit), then makes sure.
function Stop-Installed([string]$folder) {
    $exe = Join-Path $folder $exeName
    if (Test-Path $exe) { Start-Process -FilePath $exe -ArgumentList '--quit' -Wait }
    Get-Process ([IO.Path]::GetFileNameWithoutExtension($exeName)) -ErrorAction SilentlyContinue |
        Where-Object { $_.Path -and $_.Path.StartsWith($folder.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase) } |
        ForEach-Object { if (-not $_.WaitForExit(5000)) { Stop-Process -Id $_.Id -Force } }
}

$dest = $Dest
Stop-Installed $dest
New-Item -ItemType Directory -Force $dest | Out-Null
& robocopy $Source $dest /MIR /XF Install.ps1 README.txt /NFL /NDL /NJH /NJS /NP | Out-Null
if ($LASTEXITCODE -ge 8) { throw "Copying to $dest failed (robocopy $LASTEXITCODE)" }
$exe = Join-Path $dest $exeName

# The shortcut is made by Driftwave itself: it sets the app id on it.
$lnk = Join-Path $StartMenu "$name.lnk"
$made = Start-Process -FilePath $exe -ArgumentList @('--create-shortcut', "`"$lnk`"") -Wait -PassThru
if ($made.ExitCode -ne 0 -or -not (Test-Path $lnk)) { throw "The Start menu shortcut wasn't made: $lnk" }

$version = (Get-Item $exe).VersionInfo.ProductVersion
if ($NoRegistry) {
    Write-Output "Installed $name $version to $dest (no Settings > Apps entry)"
    return
}
$key = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$aumid"
New-Item -Path $key -Force | Out-Null
$uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$dest\uninstall.ps1`""
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
if ($Launch) { Start-Process -FilePath $exe }
