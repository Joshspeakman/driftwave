# Change Station Cover… and Use Default Cover, through UI Automation on a muted test copy. The system file
# picker opens on screen for a few seconds. Its controls show up in UI Automation without patterns, so the
# image path is typed into it, and only after checking that the picker window has focus.
# Usage: windows\scripts\cover-test.ps1
. "$PSScriptRoot\common.ps1"
Assert-Tool ffmpeg
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes, System.Windows.Forms
Add-Type -Namespace SW -Name Fg -MemberDefinition @'
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
'@
$app = Get-TestApp
$out = New-ScratchDir 'shortwave-cover'
$testSupport = Join-Path $env:APPDATA 'ShortwaveTestRun'
$library = Join-Path $testSupport 'library.json'
$covers = Join-Path $testSupport 'Covers'
$A = [Windows.Automation.AutomationElement]
$script:fail = 0
function Test-Check([string]$Name, [bool]$Ok) {
    if ($Ok) { Write-Output "PASS $Name" } else { Write-Output "FAIL $Name (logs in $out)"; $script:fail = 1 }
}
function Cond($prop, $value) { New-Object Windows.Automation.PropertyCondition($prop, $value) }
function Wait-For([scriptblock]$Find, [int]$Seconds = 10) {
    for ($i = 0; $i -lt $Seconds * 4; $i++) { $r = & $Find; if ($r) { return $r }; Start-Sleep -Milliseconds 250 }
    $null
}
function Get-Station([string]$Id) { (Get-Content $library -Raw | ConvertFrom-Json).stations | Where-Object id -eq $Id }
$stationId = '960cfd3b-0601-11e8-ae97-52543be04c81'   # SomaFM Groove Salad in seed.json

$image = Join-Path $out 'cover.png'
& ffmpeg -loglevel error -f lavfi -i 'color=c=0x2266cc:s=300x300' -frames:v 1 $image
Remove-Item -LiteralPath $library -Force -ErrorAction SilentlyContinue
Remove-Item -LiteralPath $covers -Recurse -Force -ErrorAction SilentlyContinue
$log = Join-Path $out 'cover.log'
$p = Start-Shortwave $app @('-muted', '-page', 'library', '-importStations', (Join-Path $RepoRoot 'fixtures\exchange\seed.json')) $log
try {
    $win = Wait-For { $A::RootElement.FindFirst('Children', (Cond $A::ProcessIdProperty $p.Id)) }
    # Start-Shortwave starts the window minimized; the picker needs a normal one.
    try { $win.GetCurrentPattern([Windows.Automation.WindowPattern]::Pattern).SetWindowVisualState('Normal') } catch { }
    function Open-Menu([string]$Item) {
        $card = Wait-For { $win.FindFirst('Descendants', (New-Object Windows.Automation.AndCondition((Cond $A::ClassNameProperty 'StationCard'), (Cond $A::NameProperty 'SomaFM Groove Salad')))) }
        # Two buttons are called More: the Library header's, and then the details dialog's.
        $header = @($win.FindAll('Descendants', (Cond $A::NameProperty 'More')) | ForEach-Object { ($_.GetRuntimeId() -join '.') })
        $card.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
        $more = Wait-For { @($win.FindAll('Descendants', (Cond $A::NameProperty 'More'))) | Where-Object { $header -notcontains ($_.GetRuntimeId() -join '.') } | Select-Object -First 1 }
        $more.GetCurrentPattern([Windows.Automation.ExpandCollapsePattern]::Pattern).Expand()
        $menuItem = Wait-For { $A::RootElement.FindFirst('Descendants', (New-Object Windows.Automation.AndCondition((Cond $A::ProcessIdProperty $p.Id), (Cond $A::NameProperty $Item)))) }
        if (-not $menuItem) { throw "menu item '$Item' not found" }
        $menuItem.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke()
    }
    function Close-Details {
        # The dialog's Close button (a XAML Button), not the title bar's Close.
        $close = Wait-For { @($win.FindAll('Descendants', (New-Object Windows.Automation.AndCondition((Cond $A::NameProperty 'Close'), (Cond $A::ClassNameProperty 'Button'))))) | Select-Object -First 1 }
        if ($close) { $close.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke(); Start-Sleep -Seconds 1 }
    }

    Open-Menu 'Change Station Cover…'
    $picker = Wait-For { $win.FindFirst('Children', (Cond $A::ClassNameProperty '#32770')) }   # listed under the window that owns it
    Test-Check 'Change Station Cover… opens the file picker' ($null -ne $picker)
    if ($picker) {
        $h = [IntPtr]$picker.Current.NativeWindowHandle
        for ($i = 0; $i -lt 5 -and [SW.Fg]::GetForegroundWindow() -ne $h; $i++) {
            [SW.Fg]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [SW.Fg]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            [SW.Fg]::SetForegroundWindow($h) | Out-Null
            Start-Sleep -Milliseconds 400
        }
        if ([SW.Fg]::GetForegroundWindow() -ne $h) { throw 'The file picker is not in front; stopped before typing.' }
        $escaped = [regex]::Replace($image, '[+^%~(){}\[\]]', '{$0}')
        [System.Windows.Forms.SendKeys]::SendWait('%n')            # Alt+N: the File name box
        Start-Sleep -Milliseconds 300
        if ([SW.Fg]::GetForegroundWindow() -ne $h) { throw 'The file picker lost focus; stopped before typing.' }
        [System.Windows.Forms.SendKeys]::SendWait($escaped + '{ENTER}')
        $set = Wait-For { $s = Get-Station $stationId; if ($s.customCover) { $s } }
        $file = if ($set) { Join-Path $covers $set.customCover }
        Test-Check "the station uses the new cover ($($set.customCover))" ($null -ne $set -and (Test-Path $file))
        if ($file -and (Test-Path $file)) {
            $sig = [IO.File]::ReadAllBytes($file)[0..3]
            Test-Check 'the cover was saved as PNG' ($sig[0] -eq 0x89 -and $sig[1] -eq 0x50)
        }
        Close-Details

        Open-Menu 'Use Default Cover'
        $cleared = Wait-For { $s = Get-Station $stationId; if ($s -and -not $s.customCover) { $s } }
        Test-Check 'Use Default Cover clears it' ($null -ne $cleared)
        Test-Check 'and deletes the old image' ($file -and -not (Test-Path $file))
        Close-Details
    }
}
finally {
    Stop-Shortwave $p
    Remove-Item -LiteralPath $library -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $covers -Recurse -Force -ErrorAction SilentlyContinue
}
if ($script:fail -eq 0) { Write-Output 'PASS cover test' }
exit $script:fail
