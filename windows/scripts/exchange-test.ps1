# Station import/export test (port of scripts/exchange-test.sh): the built app imports fixtures/exchange/seed.json
# into an empty test-run library and exports it again, then checks that a second import adds nothing and that the
# Mac app's export imports. Muted test runs; the real library is never touched.
# Usage: windows\scripts\exchange-test.ps1 [-UpdateFixture]   (writes fixtures/exchange/windows-export.json)
param([switch]$UpdateFixture)
. "$PSScriptRoot\common.ps1"
$app = Get-TestApp
$out = New-ScratchDir 'shortwave-exchange'
$testSupport = Join-Path $env:APPDATA 'ShortwaveTestRun'
$fixtures = Join-Path $RepoRoot 'fixtures\exchange'
$script:fail = 0

function Reset-TestLibrary {
    Remove-Item -LiteralPath (Join-Path $testSupport 'library.json') -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $testSupport 'Covers') -Recurse -Force -ErrorAction SilentlyContinue
}

function Invoke-Run([string]$Name, [string[]]$Arguments) {
    $log = Join-Path $out "$Name.log"
    $p = Start-Shortwave $app (@('-muted', '-quitAfter', '3') + $Arguments) $log
    if (-not $p.WaitForExit(30000)) { Stop-Shortwave $p }
}

function Assert-Log([string]$Name, [string]$Text) {
    $log = Join-Path $out "$Name.log"
    $content = if (Test-Path $log) { Get-Content -Raw -Encoding UTF8 $log } else { '' }
    if ($content -and $content.Contains($Text)) { Write-Output "PASS ${Name}: $Text" }
    else { Write-Output "FAIL ${Name}: wanted '$Text' (log $log)"; $script:fail = 1 }
}

Reset-TestLibrary
$export = Join-Path $out 'windows-export.json'
Invoke-Run 'seed' @('-importStations', (Join-Path $fixtures 'seed.json'), '-exportStations', $export)
Assert-Log 'seed' 'imported added=3 present=0 unreadable=0 covers=1'
Assert-Log 'seed' 'exported 3 stations'
if (Get-ChildItem (Join-Path $testSupport 'Covers') -Filter '*.png' -ErrorAction SilentlyContinue) { Write-Output 'PASS seed: cover written' }
else { Write-Output 'FAIL seed: cover not written'; $script:fail = 1 }

Invoke-Run 'again' @('-importStations', $export)
Assert-Log 'again' 'imported added=0 present=3 unreadable=0'

Reset-TestLibrary
Invoke-Run 'mac' @('-importStations', (Join-Path $fixtures 'mac-export.json'))
Assert-Log 'mac' 'imported added=3 present=0 unreadable=0 covers=1'

Reset-TestLibrary
Invoke-Run 'not-json' @('-importStations', (Join-Path $PSScriptRoot 'common.ps1'))
Assert-Log 'not-json' 'import failed: This file isn'
Reset-TestLibrary

if ($script:fail -eq 0 -and $UpdateFixture) {
    Copy-Item $export (Join-Path $fixtures 'windows-export.json') -Force
    Write-Output 'wrote fixtures/exchange/windows-export.json'
}
if ($script:fail -eq 0) { Write-Output 'PASS exchange test' } else { Write-Output "logs in $out" }
exit $script:fail
