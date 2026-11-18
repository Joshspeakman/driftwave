# Keyboard shortcuts with real key presses on muted test copies. It takes over the screen for about a
# minute: the test window comes to the front, and before every key press the script checks that a test
# window still has focus (it stops if not, so keys never go to another app). Media keys and Ctrl+Alt+M (mute)
# are never pressed. Ctrl+Up also unmutes (as on the Mac), so the volume keys run in a copy that plays nothing.
# Needs Python 3 and ffmpeg (local test stream). Usage: windows\scripts\shortcut-test.ps1
. "$PSScriptRoot\common.ps1"
Assert-Tool ffmpeg
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -Namespace SW -Name Win -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
'@
$app = Get-TestApp
$python = Find-Python
$out = New-ScratchDir 'shortwave-shortcuts'
$testSupport = Join-Path $env:APPDATA 'ShortwaveTestRun'
$A = [Windows.Automation.AutomationElement]
$script:fail = 0
function Test-Check([string]$Name, [bool]$Ok) {
    if ($Ok) { Write-Output "PASS $Name" } else { Write-Output "FAIL $Name (logs in $out)"; $script:fail = 1 }
}
function Get-ForegroundPid { $fpid = 0; [SW.Win]::GetWindowThreadProcessId([SW.Win]::GetForegroundWindow(), [ref]$fpid) | Out-Null; $fpid }
function Show-App($p) {
    for ($i = 0; $i -lt 5 -and (Get-ForegroundPid) -ne $p.Id; $i++) {
        [SW.Win]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [SW.Win]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)   # Alt lets a background app take the foreground
        [SW.Win]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
        Start-Sleep -Milliseconds 400
    }
}
function Send-Keys($p, [string]$Keys, [double]$Wait = 1) {
    if ((Get-ForegroundPid) -ne $p.Id) { Show-App $p }
    if ((Get-ForegroundPid) -ne $p.Id) { throw "The test window isn't in front; stopped before pressing $Keys." }
    [System.Windows.Forms.SendKeys]::SendWait($Keys)
    Start-Sleep -Milliseconds ([int]($Wait * 1000))
}
function Get-Window($p) { $A::RootElement.FindFirst('Children', (New-Object Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id))) }
function Find-Named($root, [string]$Name) { $root.FindFirst('Descendants', (New-Object Windows.Automation.PropertyCondition($A::NameProperty, $Name))) }
function Test-Selected($p, [string]$Item) {
    $e = Find-Named (Get-Window $p) $Item
    try { $e -and $e.GetCurrentPattern([Windows.Automation.SelectionItemPattern]::Pattern).Current.IsSelected } catch { $false }
}
function Get-TopWindows($p) {
    @($A::RootElement.FindAll('Children', (New-Object Windows.Automation.PropertyCondition($A::ProcessIdProperty, $p.Id))))
}
# The system file dialog (class #32770); it can take a few seconds to appear.
function Wait-Picker($p) {
    for ($i = 0; $i -lt 40; $i++) {
        # UI Automation lists the dialog under the window that owns it.
        $d = Get-TopWindows $p | ForEach-Object { if ($_.Current.ClassName -eq '#32770') { $_ } else { $_.FindAll('Children', (New-Object Windows.Automation.PropertyCondition($A::ClassNameProperty, '#32770'))) } } | Select-Object -First 1
        if ($d) { Start-Sleep -Milliseconds 500; return $d }
        Start-Sleep -Milliseconds 250
    }
    $null
}
function Get-Volume { (Get-Content (Join-Path $testSupport 'settings.json') -Raw | ConvertFrom-Json).volume }
function Count-Log([string]$Log, [string]$Pattern) { @(Get-Content -Encoding UTF8 $Log | Where-Object { $_ -match $Pattern }).Count }

Remove-Item -LiteralPath (Join-Path $testSupport 'library.json') -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath (Join-Path $testSupport 'Covers') -Recurse -Force -ErrorAction SilentlyContinue
$seed = Join-Path $RepoRoot 'fixtures\exchange\seed.json'

# Run A: nothing plays. Pages, volume, player pane, mini player, Add Local Station, import and export.
$logA = Join-Path $out 'a.log'
$env:SHORTWAVE_LOG = '1'
$p = Start-Process -FilePath $app -ArgumentList @('-muted', '-page', 'library', '-importStations', "`"$seed`"") -PassThru -RedirectStandardError $logA -RedirectStandardOutput "$logA.out"
Remove-Item Env:SHORTWAVE_LOG
try {
    Start-Sleep -Seconds 5
    Show-App $p
    Send-Keys $p '^2'; Test-Check 'Ctrl+2 shows Discover' (Test-Selected $p 'Discover')
    Send-Keys $p '^f'; Test-Check 'Ctrl+F shows Search' (Test-Selected $p 'Search')
    Send-Keys $p '{ESC}' 0.3   # leave the search box so Ctrl+1 isn't typed into it
    Send-Keys $p '^1'; Test-Check 'Ctrl+1 shows Library' (Test-Selected $p 'Library')

    Send-Keys $p '^{DOWN}' 1.5; $v = Get-Volume
    Send-Keys $p '^{UP}' 1.5; $v2 = Get-Volume
    Test-Check "Ctrl+Down / Ctrl+Up change the volume by 0.05 (now $v, then $v2)" ([math]::Abs(($v2 - $v) - 0.05) -lt 0.001)

    Send-Keys $p '^%p'; Test-Check 'Ctrl+Alt+P hides the player' ($null -ne (Find-Named (Get-Window $p) 'Show Player'))
    Send-Keys $p '^%p'; Test-Check 'Ctrl+Alt+P shows it again' ($null -ne (Find-Named (Get-Window $p) 'Hide Player'))

    Send-Keys $p '^n' 1.5; Test-Check 'Ctrl+N opens Add Local Station' ((Count-Log $logA 'dialog LocalStationDialog') -eq 1)
    Send-Keys $p '{ESC}'

    Send-Keys $p '^+i' 0.5
    $dlg = Wait-Picker $p
    Test-Check "Ctrl+Shift+I opens the import picker ($($dlg.Current.Name))" ((Count-Log $logA 'picker import') -eq 1 -and $null -ne $dlg)
    if ($dlg) { Send-Keys $p '{ESC}' }
    Send-Keys $p '^+e' 0.5
    $dlg = Wait-Picker $p
    # The picker's file name box isn't readable through UI Automation; DefaultFileName fills it in.
    Test-Check "Ctrl+Shift+E opens the export picker ($($dlg.Current.Name))" ((Count-Log $logA 'picker export') -eq 1 -and $null -ne $dlg)
    if ($dlg) { Send-Keys $p '{ESC}' }

    Send-Keys $p '^+m' 2
    Test-Check 'Ctrl+Shift+M opens the mini player' (@(Get-TopWindows $p | Where-Object { $_.Current.Name -eq 'Mini Player' }).Count -eq 1)
}
finally { Stop-Shortwave $p }

# Run B: a muted local test stream. Stop/start and next station (Ctrl+Up is never pressed here).
$mediaDir = Join-Path $out 'media'
New-Item -ItemType Directory -Force $mediaDir | Out-Null
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=440:duration=30' -ac 2 -ar 44100 -b:a 128k "$mediaDir\tone.mp3"
& ffmpeg -loglevel error -f lavfi -i 'sine=frequency=660:duration=30' -ac 2 -ar 44100 -c:a aac -b:a 128k -f adts "$mediaDir\tone.aac"
$port = 8768
$srv = Start-Process -FilePath $python.Exe -ArgumentList (@($python.Args) + @("`"$(Join-Path $RepoRoot 'scripts\icy-test-server.py')`"", '--port', $port, '--media', "`"$mediaDir`"")) -PassThru -WindowStyle Hidden
$logB = Join-Path $out 'b.log'
$env:SHORTWAVE_LOG = '1'
$p = Start-Process -FilePath $app -ArgumentList @('-muted', '-page', 'library', '-playURL', "http://127.0.0.1:$port/mp3") -PassThru -RedirectStandardError $logB -RedirectStandardOutput "$logB.out"
Remove-Item Env:SHORTWAVE_LOG
try {
    Start-Sleep -Seconds 6
    Show-App $p
    Test-Check 'the test stream plays' ((Count-Log $logB 'state playing') -ge 1)
    Send-Keys $p '^{ENTER}' 2; Test-Check 'Ctrl+Enter stops' ((Count-Log $logB 'stop \(toggle\)') -eq 1)
    $before = Count-Log $logB 'state playing'
    Send-Keys $p '^{ENTER}' 4; Test-Check 'Ctrl+Enter plays again' ((Count-Log $logB 'state playing') -gt $before)
    $before = Count-Log $logB 'stop \(station change\)'
    Send-Keys $p '^{RIGHT}' 2; Test-Check 'Ctrl+Right moves to a library station' ((Count-Log $logB 'stop \(station change\)') -gt $before)
}
finally {
    Stop-Shortwave $p
    Stop-Process -Id $srv.Id -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath (Join-Path $testSupport 'library.json') -Force -ErrorAction SilentlyContinue
}
if ($script:fail -eq 0) { Write-Output 'PASS shortcut test' }
exit $script:fail
