$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$root = Split-Path -Parent $PSScriptRoot
$out = Join-Path $root 'app-icon.png'
$size = 1024
$bmp = New-Object System.Drawing.Bitmap($size, $size)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
$g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAlias
$g.Clear([System.Drawing.Color]::Transparent)
$path = New-Object System.Drawing.Drawing2D.GraphicsPath
$r = 230
$d = $r * 2
$path.AddArc(0, 0, $d, $d, 180, 90)
$path.AddArc($size - $d, 0, $d, $d, 270, 90)
$path.AddArc($size - $d, $size - $d, $d, $d, 0, 90)
$path.AddArc(0, $size - $d, $d, $d, 90, 90)
$path.CloseFigure()
$p1 = New-Object System.Drawing.Point(0, 0)
$p2 = New-Object System.Drawing.Point($size, $size)
$c1 = [System.Drawing.Color]::FromArgb(255, 32, 36, 52)
$c2 = [System.Drawing.Color]::FromArgb(255, 10, 11, 16)
$grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush($p1, $p2, $c1, $c2)
$g.FillPath($grad, $path)
$white = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 240, 243, 250))
$font = New-Object System.Drawing.Font('Segoe UI', 470, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$fmt = New-Object System.Drawing.StringFormat
$fmt.Alignment = [System.Drawing.StringAlignment]::Center
$fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
$rect = New-Object System.Drawing.RectangleF(0, -20, 1024, 1024)
$g.DrawString('K', $font, $white, $rect, $fmt)
$accent = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 91, 140, 255))
$g.FillRectangle($accent, 347, 806, 330, 42)
$g.Dispose()
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
$bmp.Dispose()
Write-Output ('icon written: ' + $out + ' size ' + (Get-Item $out).Length)
