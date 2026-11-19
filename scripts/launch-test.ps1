# Launches the app (muted, as a test run) with the parts only a real launch otherwise starts: the notification-area icon
# and the system media controls (-tray -mediaControls). Fails if they don't start, or if the app panics or quits early.
# (1.0.0's first installed build crashed at launch: Windows' media controls need the window's handle.)
# Usage: scripts\launch-test.ps1   (the app: $env:SHORTWAVE_APP, else rust\target\release\driftwave.exe)
. "$PSScriptRoot\common.ps1"
$app = if ($env:SHORTWAVE_APP) { $env:SHORTWAVE_APP } else { Join-Path $RepoRoot 'rust\target\release\driftwave.exe' }
if (-not (Test-Path $app)) { throw "No app at $app. Run: rust\cargo.ps1 build --release -p driftwave-app" }
$out = New-ScratchDir 'driftwave-launchtest'
$log = Join-Path $out 'launch.log'
$env:SHORTWAVE_LOG = '1'
$env:RUST_BACKTRACE = '1'
try {
    $p = Start-Process -FilePath $app -ArgumentList @('-muted', '-tray', '-mediaControls', '-playURL', 'https://ice6.somafm.com/groovesalad-128-mp3', '-quitAfter', '12') `
        -RedirectStandardError $log -RedirectStandardOutput "$log.out" -PassThru -WindowStyle Minimized
}
finally { Remove-Item Env:SHORTWAVE_LOG, Env:RUST_BACKTRACE -ErrorAction SilentlyContinue }
Start-Sleep -Seconds 8
$early = $p.HasExited
$p.WaitForExit(20000) | Out-Null
if (-not $p.HasExited) { Stop-Process -Id $p.Id -Force }
$text = Get-Content $log -Raw
$fail = 0
if ($early) { Write-Output "FAIL quit within 8 s (log $log)"; $fail = 1 }
if ($text -match 'panicked') { Write-Output "FAIL panicked:"; ($text -split "`n" | Select-String -Pattern 'panicked' -Context 0, 2) | ForEach-Object { $_.ToString() }; $fail = 1 }
if ($text -notmatch 'integration: tray=true media=true') { Write-Output "FAIL the tray and media controls didn't both start (log $log)"; $fail = 1 }
if ($fail -eq 0) { Write-Output 'PASS launch test' } else { exit 1 }
