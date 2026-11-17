# Draws the app icon, the same artwork as tools/make-icon.swift (dusk gradient, horizon, antenna and waves),
# and writes a multi-size .ico (PNG entries) plus a 256 px PNG.
# Usage: powershell -File windows\tools\make-icon.ps1 [-Out windows\src\Shortwave\Assets]
param([string]$Out = (Join-Path $PSScriptRoot '..\src\Shortwave\Assets'))
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

function New-IconBitmap([int]$px, [bool]$tile) {
    # Designed on a 1024 grid with the origin at the bottom left (CoreGraphics); flipped for GDI+.
    $bmp = New-Object System.Drawing.Bitmap $px, $px, ([System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'; $g.PixelOffsetMode = 'HighQuality'; $g.CompositingQuality = 'HighQuality'
    $s = $px / 1024.0
    $g.ScaleTransform($s, $s)
    $g.TranslateTransform(0, 1024); $g.ScaleTransform(1, -1)
    function C([double]$r, [double]$gg, [double]$b, [double]$a = 1) { [System.Drawing.Color]::FromArgb([int]($a * 255), [int]($r * 255), [int]($gg * 255), [int]($b * 255)) }

    # Windows icons have less margin than the macOS grid; the small sizes use the whole square.
    $inset = if ($tile) { 100.0 } else { 40.0 }
    $radius = if ($tile) { 185.0 } else { 200.0 }
    $body = New-Object System.Drawing.RectangleF $inset, $inset, (1024 - 2 * $inset), (1024 - 2 * $inset)
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = $radius * 2
    $path.AddArc($body.X, $body.Y, $d, $d, 180, 90)
    $path.AddArc($body.Right - $d, $body.Y, $d, $d, 270, 90)
    $path.AddArc($body.Right - $d, $body.Bottom - $d, $d, $d, 0, 90)
    $path.AddArc($body.X, $body.Bottom - $d, $d, $d, 90, 90)
    $path.CloseFigure()

    $g.SetClip($path)
    # Background gradient: dusk orange (top) to deep night blue (bottom).
    $top = New-Object System.Drawing.PointF 512, $body.Bottom
    $bottom = New-Object System.Drawing.PointF 512, $body.Top
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush $top, $bottom, (C 0.99 0.55 0.30), (C 0.18 0.15 0.42)
    $blend = New-Object System.Drawing.Drawing2D.ColorBlend 3
    $blend.Colors = @((C 0.99 0.55 0.30), (C 0.80 0.25 0.42), (C 0.18 0.15 0.42))
    $blend.Positions = @(0.0, 0.45, 1.0)
    $grad.InterpolationColors = $blend
    $g.FillRectangle($grad, $body)

    # Globe-ish horizon at the bottom.
    $hx = $body.X + $body.Width / 2 - 560; $hy = $body.Y - 820
    $g.FillEllipse((New-Object System.Drawing.SolidBrush (C 0.08 0.07 0.22 0.85)), $hx, $hy, 1120, 1120)
    $pen = New-Object System.Drawing.Pen (C 1 1 1 0.10), 6
    foreach ($i in 1..3) { $dx = $i * 140; $g.DrawEllipse($pen, $hx + $dx, $hy, 1120 - 2 * $dx, 1120) }

    # Antenna mast and waves.
    $tipX = $body.X + $body.Width / 2; $tipY = $body.Y + 420
    $white = New-Object System.Drawing.Pen (C 1 1 1), 30
    $white.StartCap = 'Round'; $white.EndCap = 'Round'
    $g.DrawLine($white, $tipX, $tipY - 30, $tipX - 95, $body.Y + 230)
    $g.DrawLine($white, $tipX, $tipY - 30, $tipX + 95, $body.Y + 230)
    $g.FillEllipse((New-Object System.Drawing.SolidBrush (C 1 1 1)), $tipX - 42, $tipY - 42, 84, 84)
    $radii = @(130.0, 215.0, 300.0)
    for ($i = 0; $i -lt 3; $i++) {
        $r = $radii[$i]
        $wp = New-Object System.Drawing.Pen (C 1 1 1 (1 - $i * 0.22)), 34
        $wp.StartCap = 'Round'; $wp.EndCap = 'Round'
        # CoreGraphics arc from 0.22pi to 0.78pi counter-clockwise (y up) = 39.6 to 140.4 degrees.
        $g.DrawArc($wp, $tipX - $r, $tipY - $r, 2 * $r, 2 * $r, 39.6, 100.8)
    }
    $g.Dispose()
    return $bmp
}

New-Item -ItemType Directory -Force $Out | Out-Null
$sizes = 16, 20, 24, 32, 40, 48, 64, 128, 256
$pngs = foreach ($sz in $sizes) {
    $b = New-IconBitmap $sz ($sz -ge 128)
    $ms = New-Object System.IO.MemoryStream
    $b.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($sz -eq 256) { $b.Save((Join-Path $Out 'Shortwave.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $b.Dispose()
    , $ms.ToArray()
}
# ICO container: header, directory, then PNG images.
$ico = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ico
$w.Write([UInt16]0); $w.Write([UInt16]1); $w.Write([UInt16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
for ($i = 0; $i -lt $sizes.Count; $i++) {
    $sz = $sizes[$i]; $len = $pngs[$i].Length
    $w.Write([byte]($sz % 256)); $w.Write([byte]($sz % 256)); $w.Write([byte]0); $w.Write([byte]0)
    $w.Write([UInt16]1); $w.Write([UInt16]32); $w.Write([UInt32]$len); $w.Write([UInt32]$offset)
    $offset += $len
}
foreach ($p in $pngs) { $w.Write($p) }
$w.Flush()
[System.IO.File]::WriteAllBytes((Join-Path $Out 'Shortwave.ico'), $ico.ToArray())
Write-Output (Join-Path $Out 'Shortwave.ico')
