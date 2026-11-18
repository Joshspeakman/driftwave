# Builds Driftwave and installs it for the current user (see install-files.ps1): %LOCALAPPDATA%\Programs\<name>,
# a Start menu shortcut and a Settings > Apps entry. No admin rights needed. Driftwave and Driftwave Nightly
# install side by side; an old "Shortwave" install of the same flavor is removed.
# Usage: windows\scripts\install.ps1 [-Flavor Nightly|Release] [-NoBuild]
param(
    [ValidateSet('Nightly', 'Release')][string]$Flavor = 'Nightly',
    [switch]$NoBuild
)
. "$PSScriptRoot\common.ps1"
if (-not $NoBuild) { & "$PSScriptRoot\build.ps1" -Flavor $Flavor | Out-Host }
& "$PSScriptRoot\install-files.ps1" -Flavor $Flavor -Source (Get-BuildDir $Flavor)
