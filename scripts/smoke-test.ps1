# Plays real streams (muted) in the built app and checks that playback starts and titles arrive.
# Port of scripts/smoke-test.sh. Usage: windows\scripts\smoke-test.ps1 [-Seconds 20]
param([int]$Seconds = 20)
. "$PSScriptRoot\common.ps1"
$app = Get-TestApp
$out = New-ScratchDir 'shortwave-smoke'
$streams = @(
    @{ Name = 'mp3-icy'; Url = 'http://ice1.somafm.com/groovesalad-128-mp3'; Expect = 'titles=true format=mp3' },
    @{ Name = 'aac-icy'; Url = 'https://ice1.somafm.com/groovesalad-128-aac'; Expect = 'titles=true format=aac' },
    @{ Name = 'hls'; Url = 'https://live.m6radio.quortex.io/webM89Hc99XApzgfhXNX8ASN5/grouprtl/national/short/audio-64000/index.m3u8'; Expect = '' },
    @{ Name = 'ogg-vorbis'; Url = 'https://dancewave.online/dance.ogg'; Expect = '' },
    @{ Name = 'pls-playlist'; Url = 'https://somafm.com/groovesalad.pls'; Expect = 'titles=true' }
)
$fail = 0
foreach ($s in $streams) {
    $log = Join-Path $out "$($s.Name).log"
    $p = Start-Shortwave $app @('-playURL', $s.Url, '-muted') $log
    Start-Sleep -Seconds $Seconds
    Stop-Shortwave $p
    $text = if (Test-Path $log) { Get-Content -Raw -Encoding UTF8 $log } else { "" }
    $ok = $text -match 'state playing'
    if ($s.Expect -and $text -notmatch [regex]::Escape("icy connected $($s.Expect)")) { $ok = $false }
    if ($s.Name -ne 'hls' -and $s.Name -ne 'ogg-vorbis' -and $text -notmatch 'now playing') { $ok = $false }
    if ($ok) {
        $title = ([regex]::Match($text, 'now playing (.*)')).Groups[1].Value.Trim()
        Write-Output "PASS $($s.Name): $title"
    }
    else { Write-Output "FAIL $($s.Name) (log: $log)"; $fail = 1 }
}
exit $fail
