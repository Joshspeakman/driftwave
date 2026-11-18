# Renders every main screen of the built app, in light and dark, into docs\design\windows\ (port of
# scripts/screenshots.sh). Muted test runs with their own library; the real app's data is never touched.
# The renders are the window's XAML (-screenshot): they leave out Mica and the title bar buttons.
# Usage: windows\scripts\screenshots.ps1 [-Theme <id>] [-OutDir <dir>] [name ...]   (default: all, Default theme)
#   -Theme renders one theme (docs/THEMES.md) into docs\design\windows	hemes\<id>\ unless -OutDir says otherwise.
[CmdletBinding(PositionalBinding = $false)]
param([string]$Theme = '', [string]$OutDir = '', [Parameter(ValueFromRemainingArguments = $true)][string[]]$Names = @())
. "$PSScriptRoot\common.ps1"
$app = Get-TestApp
$outDir = if ($OutDir) { $OutDir } elseif ($Theme) { Join-Path $RepoRoot "docs\design\windows	hemes\$Theme" } else { Join-Path $RepoRoot 'docs\design\windows' }
$themeArgs = if ($Theme) { @('-theme', $Theme) } else { @() }
$logs = New-ScratchDir 'shortwave-shots'
New-Item -ItemType Directory -Force $outDir | Out-Null
$testSupport = Join-Path $env:APPDATA 'ShortwaveTestRun'
$stream = 'http://ice1.somafm.com/groovesalad-128-mp3'
$fixtures = Join-Path $RepoRoot 'fixtures'

function Reset-TestLibrary {
    Remove-Item -LiteralPath (Join-Path $testSupport 'library.json') -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $testSupport 'Covers') -Recurse -Force -ErrorAction SilentlyContinue
}

# Runs the app in a normal (not minimized) window: XAML isn't rendered while the window is minimized.
function Invoke-App([string[]]$Arguments, [string]$Log) {
    $env:SHORTWAVE_LOG = '1'
    try {
        $quoted = $Arguments | ForEach-Object { if ($_ -match '[\s"]') { '"' + ($_ -replace '"', '\"') + '"' } else { $_ } }
        $p = Start-Process -FilePath $app -ArgumentList $quoted -RedirectStandardError $Log -RedirectStandardOutput "$Log.out" -PassThru
    }
    finally { Remove-Item Env:SHORTWAVE_LOG -ErrorAction SilentlyContinue }
    if (-not $p.WaitForExit(90000)) { Stop-Shortwave $p }
}

function Wanted([string]$Name) { $Names.Count -eq 0 -or $Names -contains $Name }

# name, delay, arguments; -Mini saves the mini player's window instead of the main window.
function Save-Shot([string]$Name, [int]$Delay, [string[]]$Arguments, [switch]$Mini) {
    # A theme has its own light or dark base, so it needs one render per screen.
    $looks = if ($Theme) { @('dark') } else { @('light', 'dark') }
    foreach ($look in $looks) {
        $png = if ($Theme) { Join-Path $outDir "$Name.png" } else { Join-Path $outDir "$Name-$look.png" }
        Remove-Item $png -ErrorAction SilentlyContinue
        $target = if ($Mini) { Join-Path $logs "$Name-$look-main.png" } else { $png }
        $log = Join-Path $logs "$Name-$look.log"
        Invoke-App (@('-muted', '-appearance', $look, '-screenshotDelay', "$Delay", '-screenshot', $target) + $themeArgs + $Arguments) $log
        if ($Mini) {
            $miniPng = Join-Path $logs "$Name-$look-main-mini.png"
            if (Test-Path $miniPng) { Move-Item $miniPng $png -Force }
        }
        if (Test-Path $png) { Write-Output "ok   $Name-$look" } else { Write-Output "FAIL $Name-$look (log $log)" }
    }
}

Reset-TestLibrary
if (Wanted 'library-empty') { Save-Shot 'library-empty' 5 @('-page', 'library') }
Invoke-App @('-muted', '-quitAfter', '2', '-importStations', (Join-Path $fixtures 'exchange\seed.json')) (Join-Path $logs 'import-seed.log')
Invoke-App @('-muted', '-quitAfter', '2', '-importStations', (Join-Path $fixtures 'design\library.json')) (Join-Path $logs 'import-design.log')
if (Wanted 'library') { Save-Shot 'library' 8 @('-page', 'library') }
if (Wanted 'discover') { Save-Shot 'discover' 10 @('-page', 'discover') }
if (Wanted 'search') { Save-Shot 'search' 10 @('-page', 'search', '-searchText', 'jazz') }
if (Wanted 'settings') { Save-Shot 'settings' 5 @('-page', 'settings') }
if (Wanted 'station-details') { Save-Shot 'station-details' 10 @('-page', 'search', '-searchText', 'groove salad', '-openFirstResult') }
if (Wanted 'add-station') { Save-Shot 'add-station' 5 @('-page', 'library', '-addStation') }
if (Wanted 'player-playing') { Save-Shot 'player-playing' 16 @('-page', 'library', '-playURL', $stream) }
if (Wanted 'mini-player') { Save-Shot 'mini-player' 16 @('-miniPlayer', '-playURL', $stream) -Mini }
Reset-TestLibrary
Write-Output "logs in $logs"
