# Shared helpers for the Windows scripts (dot-source this file). Works in Windows PowerShell 5.1 and PowerShell 7.
$ErrorActionPreference = 'Stop'
$WindowsRoot = Split-Path -Parent $PSScriptRoot
$RepoRoot = Split-Path -Parent $WindowsRoot

function Get-ShortwaveName([string]$Flavor) {
    if ($Flavor -eq 'Release') { 'Shortwave' } else { 'Shortwave Nightly' }
}

# Where scripts\build.ps1 puts a build.
function Get-BuildDir([string]$Flavor) { Join-Path $WindowsRoot "out\$Flavor" }

# The app the test scripts run: $env:SHORTWAVE_APP, else the Nightly build.
function Get-TestApp {
    if ($env:SHORTWAVE_APP) { return $env:SHORTWAVE_APP }
    $exe = Join-Path (Get-BuildDir 'Nightly') 'Shortwave.exe'
    if (-not (Test-Path $exe)) { throw "No build at $exe. Run windows\scripts\build.ps1 first, or set SHORTWAVE_APP." }
    return $exe
}

# A fresh folder for test output: $env:SHORTWAVE_SCRATCH\<name>, else under %TEMP%.
function New-ScratchDir([string]$Name) {
    $base = if ($env:SHORTWAVE_SCRATCH) { $env:SHORTWAVE_SCRATCH } else { [IO.Path]::GetTempPath() }
    $dir = Join-Path $base ("$Name-" + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Force $dir | Out-Null
    return $dir
}

# A real Python 3 (not the Microsoft Store stub): @{ Exe = path; Args = extra arguments }.
function Find-Python {
    foreach ($c in @('py', 'python3', 'python')) {
        $cmd = Get-Command $c -ErrorAction SilentlyContinue | Where-Object { $_.Source -notlike '*WindowsApps*' } | Select-Object -First 1
        if ($cmd) { return @{ Exe = $cmd.Source; Args = $(if ($c -eq 'py') { @('-3') } else { @() }) } }
    }
    $local = Get-ChildItem "$env:LOCALAPPDATA\Programs\Python\Python3*\python.exe" -ErrorAction SilentlyContinue | Sort-Object FullName -Descending | Select-Object -First 1
    if ($local) { return @{ Exe = $local.FullName; Args = @() } }
    throw 'Python 3 not found (winget install Python.Python.3.13).'
}

function Assert-Tool([string]$Name) {
    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) { throw "$Name not found on PATH (winget install Gyan.FFmpeg)." }
}

# Starts the app with SHORTWAVE_LOG=1, stderr to $Log. Returns the process.
function Start-Shortwave([string]$App, [string[]]$Arguments, [string]$Log) {
    $env:SHORTWAVE_LOG = '1'
    try {
        $quoted = $Arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }
        return Start-Process -FilePath $App -ArgumentList $quoted -RedirectStandardError $Log -RedirectStandardOutput "$Log.out" -PassThru -WindowStyle Minimized
    }
    finally { Remove-Item Env:SHORTWAVE_LOG -ErrorAction SilentlyContinue }
}

function Stop-Shortwave($Process) {
    if ($Process -and -not $Process.HasExited) {
        Stop-Process -Id $Process.Id -Force -ErrorAction SilentlyContinue
        $Process.WaitForExit(5000) | Out-Null
    }
}

# Seconds of audio in a file, from ffprobe.
function Get-Duration([string]$File) {
    $d = & ffprobe -v error -show_entries format=duration -of default=nw=1:nk=1 $File 2>$null
    if ($d) { [double]::Parse(($d | Select-Object -First 1), [Globalization.CultureInfo]::InvariantCulture) } else { $null }
}
