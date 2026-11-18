# Captures the real window from the screen (Mica, title bar and all), not a -screenshot render. A muted test
# copy with the design library comes to the front in light and in dark, and its window area is copied from
# the screen into docs\design\windows\screen-<light|dark>.png. Needs the display on and unlocked.
# Usage: windows\scripts\screen-capture.ps1
. "$PSScriptRoot\common.ps1"
Add-Type -AssemblyName System.Drawing
Add-Type -Namespace SW -Name Cap -MemberDefinition @'
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
[DllImport("dwmapi.dll")] public static extern int DwmGetWindowAttribute(IntPtr h, int attr, out RECT r, int size);
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
'@
[SW.Cap]::SetProcessDPIAware() | Out-Null
$app = Get-TestApp
$out = New-ScratchDir 'shortwave-screen'
$testSupport = Join-Path $env:APPDATA 'ShortwaveTestRun'
$library = Join-Path $testSupport 'library.json'
$dest = Join-Path $RepoRoot 'docs\design\windows'

function Invoke-Quick([string[]]$Arguments) {
    $p = Start-Process -FilePath $app -ArgumentList $Arguments -PassThru -WindowStyle Minimized
    $p.WaitForExit(30000) | Out-Null
}
Remove-Item -LiteralPath $library -Force -ErrorAction SilentlyContinue
Invoke-Quick @('-muted', '-quitAfter', '2', '-importStations', "`"$(Join-Path $RepoRoot 'fixtures\exchange\seed.json')`"")
Invoke-Quick @('-muted', '-quitAfter', '2', '-importStations', "`"$(Join-Path $RepoRoot 'fixtures\design\library.json')`"")
try {
    foreach ($look in 'light', 'dark') {
        $p = Start-Process -FilePath $app -ArgumentList @('-muted', '-appearance', $look, '-page', 'library', '-quitAfter', '20') -PassThru
        Start-Sleep -Seconds 4
        $h = $p.MainWindowHandle
        for ($i = 0; $i -lt 5 -and [SW.Cap]::GetForegroundWindow() -ne $h; $i++) {
            [SW.Cap]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [SW.Cap]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
            [SW.Cap]::SetForegroundWindow($h) | Out-Null
            Start-Sleep -Milliseconds 400
        }
        Start-Sleep -Seconds 4   # covers load; Mica settles once the window is active
        $r = New-Object SW.Cap+RECT
        [SW.Cap]::DwmGetWindowAttribute($h, 9, [ref]$r, 16) | Out-Null   # DWMWA_EXTENDED_FRAME_BOUNDS: the window without its shadow
        $w = $r.Right - $r.Left; $hgt = $r.Bottom - $r.Top
        $bmp = New-Object System.Drawing.Bitmap $w, $hgt
        $g = [System.Drawing.Graphics]::FromImage($bmp)
        $g.CopyFromScreen($r.Left, $r.Top, 0, 0, $bmp.Size)
        $png = Join-Path $dest "screen-$look.png"
        $bmp.Save($png, [System.Drawing.Imaging.ImageFormat]::Png)
        $g.Dispose(); $bmp.Dispose()
        $front = [SW.Cap]::GetForegroundWindow() -eq $h
        Write-Output ("{0}: {1}x{2} at {3},{4}, in front: {5}" -f $png, $w, $hgt, $r.Left, $r.Top, $front)
        Stop-Shortwave $p
    }
}
finally { Remove-Item -LiteralPath $library -Force -ErrorAction SilentlyContinue }
