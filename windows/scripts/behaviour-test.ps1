# Two player behaviours against the local test server (port of scripts/behaviour-test.sh; muted test
# runs; needs Python 3 and ffmpeg):
#  1. reconnect: the server closes the title (ICY) connection every 20 s while the audio keeps playing.
#     The app must reconnect it, keep following titles, and not list a song twice.
#     Windows difference: against this server (2 s head start, then real time) Media Foundation drains its buffer
#     and reports Buffering now and then, with or without drops; a real stream doesn't. So instead of counting
#     "state loading", this test watches the app's TCP connections: the audio connection must never be reopened.
#  2. playlist probe: a local station whose link is a .pls with no file extension (served as
#     audio/x-scpls) must be followed to the stream.
# Usage: windows\scripts\behaviour-test.ps1
. "$PSScriptRoot\common.ps1"
Assert-Tool ffmpeg
$app = Get-TestApp
$python = Find-Python
$out = New-ScratchDir 'shortwave-behaviour'
New-Item -ItemType Directory -Force "$out\media" | Out-Null
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=440:duration=30' -ac 2 -ar 44100 -b:a 128k "$out\media\tone.mp3"
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=660:duration=30' -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts "$out\media\tone.aac"
$port = 8766
$server = Join-Path $RepoRoot 'scripts\icy-test-server.py'
$serverArgs = @($python.Args) + @("`"$server`"", '--port', $port, '--interval', '15', '--drop-icy-after', '20', '--media', "`"$out\media`"")
$srv = Start-Process -FilePath $python.Exe -ArgumentList $serverArgs -PassThru -WindowStyle Hidden
$script:fail = 0
function Test-Check([string]$Name, [bool]$Ok) {
    if ($Ok) { Write-Output "PASS $Name" } else { Write-Output "FAIL $Name (logs in $out)"; $script:fail = 1 }
}
function Invoke-Run([string[]]$Arguments, [string]$Log, [int]$Seconds) {
    $p = Start-Shortwave $app (@('-muted', '-quitAfter', "$Seconds") + $Arguments) $Log
    if (-not $p.WaitForExit(($Seconds + 30) * 1000)) { Stop-Shortwave $p }
}
try {
    Start-Sleep -Seconds 1
    $log = Join-Path $out 'reconnect.log'
    $p = Start-Shortwave $app @('-muted', '-quitAfter', '52', '-playURL', "http://127.0.0.1:$port/mp3") $log
    # When each of the app's connections to the server was first and last seen (seconds after launch).
    $t0 = Get-Date; $seen = @{}; $now = 0
    while (-not $p.HasExited) {
        $now = ((Get-Date) - $t0).TotalSeconds
        Get-NetTCPConnection -RemotePort $port -State Established -ErrorAction SilentlyContinue |
            Where-Object OwningProcess -eq $p.Id | ForEach-Object {
                if ($seen.ContainsKey($_.LocalPort)) { $seen[$_.LocalPort].Last = $now } else { $seen[$_.LocalPort] = @{ First = $now; Last = $now } } }
        Start-Sleep -Milliseconds 300
        if ($now -gt 80) { Stop-Shortwave $p }
    }
    $lines = @(Get-Content -Encoding UTF8 $log)
    $connects = @($lines | Where-Object { $_ -match 'icy connected titles=true' }).Count
    Test-Check "reconnect: title connection came back ($connects connections)" ($connects -ge 2)
    $stops = @($lines | Where-Object { $_ -match 'stop \(' -and $_ -notmatch 'stop \(station change\)' -and $_ -notmatch 'stop \(quit\)' })
    $failed = @($lines | Where-Object { $_ -match 'state failed' })
    $loading = @($lines | Where-Object { $_ -match 'state loading' }).Count
    Test-Check 'reconnect: playback never stopped or failed' ($stops.Count -eq 0 -and $failed.Count -eq 0)
    # The audio connection opens in the first few seconds and stays open to the end. Every connection opened
    # later is a title reconnect (the first seconds also hold the local station's content-type probe).
    $whole = @($seen.Values | Where-Object { $_.First -lt 5 -and $_.Last -gt 45 })
    $later = @($seen.Values | Where-Object { $_.First -ge 5 })
    Test-Check "reconnect: audio connection never reopened ($($whole.Count) open throughout, $($later.Count) opened later for $($connects - 1) title reconnects)" ($whole.Count -eq 1 -and $later.Count -eq $connects - 1)
    Write-Output "info reconnect: Media Foundation reported loading $loading times (see the note at the top)"
    Test-Check 'reconnect: titles kept coming (Song 3 shown)' (@($lines | Where-Object { $_ -match 'now playing "Test Artist - Song 3"' }).Count -ge 1)
    $dups = @($lines | Where-Object { $_ -match 'now playing' } | ForEach-Object { ($_ -replace '^.*now playing ', '') } | Group-Object | Where-Object Count -gt 1)
    Test-Check 'reconnect: no song shown twice' ($dups.Count -eq 0)

    $log = Join-Path $out 'playlist.log'
    Invoke-Run @('-playURL', "http://127.0.0.1:$port/playlist") $log 12
    $text = Get-Content -Raw -Encoding UTF8 $log
    Test-Check 'playlist probe: extensionless .pls link played' ($text -match 'state playing' -and $text -match 'icy connected titles=true')
}
finally {
    Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
}
if ($script:fail -eq 0) { Write-Output 'PASS behaviour test' }
exit $script:fail
