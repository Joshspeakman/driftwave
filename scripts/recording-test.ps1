# End-to-end recording test against a local ICY server: plays MP3 and AAC streams (muted) in
# "Save All Tracks" mode and checks that whole tracks are cut, saved, tagged and playable.
# Port of scripts/recording-test.sh (ffprobe in place of afinfo). Needs Python 3 and ffmpeg.
# Usage: windows\scripts\recording-test.ps1 [-StatusLine http|icy]
#   -StatusLine icy serves "ICY 200 OK" like an old Shoutcast server (Windows-only check).
param([ValidateSet('http', 'icy')][string]$StatusLine = 'http')
. "$PSScriptRoot\common.ps1"
Assert-Tool ffmpeg; Assert-Tool ffprobe
$app = Get-TestApp
$python = Find-Python
$out = New-ScratchDir 'shortwave-rectest'
New-Item -ItemType Directory -Force "$out\media" | Out-Null
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=440:duration=30' -ac 2 -ar 44100 -b:a 128k "$out\media\tone.mp3"
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=660:duration=30' -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts "$out\media\tone.aac"
$port = 8765; $interval = 15
$server = if ($StatusLine -eq 'icy') { Join-Path $PSScriptRoot 'icy-v1-test-server.py' } else { Join-Path $RepoRoot 'scripts\icy-test-server.py' }
$serverArgs = @($python.Args) + @("`"$server`"", '--port', $port, '--interval', $interval, '--media', "`"$out\media`"")
$srv = Start-Process -FilePath $python.Exe -ArgumentList $serverArgs -PassThru -WindowStyle Hidden
$fail = 0
$checked = 0
try {
    Start-Sleep -Seconds 1
    foreach ($fmt in 'mp3', 'aac') {
        $tracks = Join-Path $out "tracks-$fmt"
        New-Item -ItemType Directory -Force $tracks | Out-Null
        $log = Join-Path $out "$fmt.log"
        $p = Start-Shortwave $app @('-playURL', "http://127.0.0.1:$port/$fmt", '-muted', '-recordingMode', 'everything',
            '-recordingMinimumDuration', '5', '-trackDirectory', $tracks) $log
        Start-Sleep -Seconds ($interval * 3 + 8)
        Stop-Shortwave $p
        $files = @(Get-ChildItem $tracks -File)
        Write-Output "--- ${fmt}: $($files.Count) saved"
        $files | ForEach-Object { Write-Output ("  {0,8} {1}" -f $_.Length, $_.Name) }
        if ($files.Count -lt 1) { Write-Output "FAIL ${fmt}: nothing saved (log $log)"; $fail = 1; continue }
        $checked++
        foreach ($f in $files) {
            $dur = Get-Duration $f.FullName
            $probe = (& ffprobe -v error -show_entries 'stream=codec_name:format_tags=title,artist,album' -of default=nw=1 $f.FullName 2>&1) -join ' '
            Write-Output ("  {0}: {1:N1} s  {2}" -f $f.Name, $dur, $probe)
            if ($null -eq $dur -or $dur -le ($interval - 4) -or $dur -ge ($interval + 4)) { Write-Output "FAIL ${fmt}: duration $dur not ~${interval}s"; $fail = 1 }
            $want = if ($fmt -eq 'mp3') { 'codec_name=mp3' } else { 'codec_name=aac' }
            if ($probe -notmatch $want) { Write-Output "FAIL ${fmt}: $($f.Name) is not $want"; $fail = 1 }
            if ($probe -notmatch 'TAG:title=Song \d+' -or $probe -notmatch 'TAG:artist=Test Artist' -or $probe -notmatch 'TAG:album=Test Stream') {
                Write-Output "FAIL ${fmt}: $($f.Name) tags missing"; $fail = 1
            }
            if ($fmt -eq 'aac' -and $f.Extension -ne '.m4a') { Write-Output "FAIL aac: saved as $($f.Extension), not .m4a"; $fail = 1 }
            # Tracks are cut at the byte where the title changed, so (as on the Mac) the first and last MP3 frames
            # can be partial; players resync. The whole file must still decode.
            $dec = Start-Process -FilePath ffmpeg -ArgumentList @('-v', 'error', '-i', "`"$($f.FullName)`"", '-f', 'null', '-') -NoNewWindow -Wait -PassThru `
                -RedirectStandardError (Join-Path $out "decode-$($f.BaseName).txt")
            if ($dec.ExitCode -ne 0) { Write-Output "FAIL ${fmt}: $($f.Name) does not decode"; $fail = 1 }
        }
        if (-not (Select-String -Path $log -Pattern 'idleIncomplete' -Quiet)) { Write-Output "FAIL ${fmt}: first track not marked incomplete"; $fail = 1 }
    }
}
finally {
    Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
}
# A run where nothing could be checked (a broken helper, no server) is a failure, not a pass.
if ($checked -lt 2) { Write-Output "FAIL only $checked of 2 formats checked"; $fail = 1 }
if ($fail -eq 0) { Write-Output 'PASS recording test' } else { Write-Output "logs in $out" }
exit $fail
