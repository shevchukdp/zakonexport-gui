# Generates res/app.ico for zakonexport: blue rounded square + yellow paragraph
# sign. Frames: 32/48 legacy DIB (via GetHicon -> Icon.Save) + 256 PNG entry.
# Re-run only when the design changes. Requires: System.Drawing (Windows).
Add-Type -AssemblyName System.Drawing
$ErrorActionPreference = "Stop"

$outDir = Join-Path $PSScriptRoot "."
$icoPath = Join-Path $outDir "app.ico"
$previewPath = Join-Path $outDir "preview_icon.png"

function New-BaseBitmap([int]$size) {
    $bmp = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit

    # rounded square path
    $r = [int]($size * 0.18)
    $w = $size - 1
    $path = New-Object System.Drawing.Drawing2D.GraphicsPath
    $path.AddArc(0, 0, 2 * $r, 2 * $r, 180, 90)
    $path.AddArc($w - 2 * $r, 0, 2 * $r, 2 * $r, 270, 90)
    $path.AddArc($w - 2 * $r, $w - 2 * $r, 2 * $r, 2 * $r, 0, 90)
    $path.AddArc(0, $w - 2 * $r, 2 * $r, 2 * $r, 90, 90)
    $path.CloseFigure()

    $rect = New-Object System.Drawing.Rectangle(0, 0, $size, $size)
    $c1 = [System.Drawing.Color]::FromArgb(30, 74, 143)   # top blue
    $c2 = [System.Drawing.Color]::FromArgb(12, 36, 82)    # bottom blue
    $br = New-Object System.Drawing.Drawing2D.LinearGradientBrush(
        $rect, $c1, $c2, [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
    $g.FillPath($br, $path)

    # yellow paragraph sign
    $yellow = [System.Drawing.Brushes]::Goldenrod
    $font = New-Object System.Drawing.Font(
        "Segoe UI", [float]($size * 0.66), [System.Drawing.FontStyle]::Bold,
        [System.Drawing.GraphicsUnit]::Pixel)
    $fmt = New-Object System.Drawing.StringFormat
    $fmt.Alignment = [System.Drawing.StringAlignment]::Center
    $fmt.LineAlignment = [System.Drawing.StringAlignment]::Center
    $charRect = New-Object System.Drawing.RectangleF(
        [float]0, [float](-$size * 0.06), [float]$size, [float]($size * 0.92))
    $g.DrawString([string][char]0x00A7, $font, $yellow, $charRect, $fmt)

    # small underline bar
    $ybrush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::FromArgb(255, 213, 74))
    $g.FillRectangle($ybrush, [float]($size * 0.30), [float]($size * 0.80),
        [float]($size * 0.40), [float]($size * 0.06))

    $g.Dispose()
    return $bmp
}

function Scale-Bitmap([System.Drawing.Bitmap]$src, [int]$size) {
    $dst = New-Object System.Drawing.Bitmap($size, $size)
    $g = [System.Drawing.Graphics]::FromImage($dst)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.DrawImage($src, (New-Object System.Drawing.Rectangle(0, 0, $size, $size)))
    $g.Dispose()
    return $dst
}

# 32bpp DIB frame built manually (GetHicon would degrade to 4bpp!)
# Layout: BITMAPINFOHEADER(biHeight=2*height) + XOR BGRA rows bottom-up + AND mask top-down
function Icon-Frame([System.Drawing.Bitmap]$bmp) {
    $s = $bmp.Width
    $maskRow = [int][Math]::Floor(($s + 31) / 32) * 4
    $maskSize = $maskRow * $s
    $ms = New-Object System.IO.MemoryStream
    $bw = New-Object System.IO.BinaryWriter($ms)

    # BITMAPINFOHEADER
    $bw.Write([uint32]40)        # biSize
    $bw.Write([int32]$s)         # biWidth
    $bw.Write([int32](2 * $s))   # biHeight = XOR height + AND height
    $bw.Write([uint16]1)         # biPlanes
    $bw.Write([uint16]32)        # biBitCount
    $bw.Write([uint32]0)         # biCompression = BI_RGB
    $bw.Write([uint32]($s * $s * 4)) # biSizeImage (XOR only)
    $bw.Write([int32]0)          # biXPelsPerMeter
    $bw.Write([int32]0)          # biYPelsPerMeter
    $bw.Write([uint32]0)         # biClrUsed
    $bw.Write([uint32]0)         # biClrImportant

    # XOR: BGRA, bottom-up
    for ($y = $s - 1; $y -ge 0; $y--) {
        for ($x = 0; $x -lt $s; $x++) {
            $c = $bmp.GetPixel($x, $y)
            $bw.Write([byte]$c.B); $bw.Write([byte]$c.G); $bw.Write([byte]$c.R); $bw.Write([byte]$c.A)
        }
    }

    # AND mask: top-down rows, LSB-first bits; 1 = fully transparent
    for ($y = 0; $y -lt $s; $y++) {
        $row = New-Object byte[] $maskRow
        for ($x = 0; $x -lt $s; $x++) {
            if ($bmp.GetPixel($x, $y).A -eq 0) {
                $row[[int][Math]::Floor($x / 8)] = $row[[int][Math]::Floor($x / 8)] -bor ([byte](128 -shr ($x -band 7)))
            }
        }
        $bw.Write($row)
    }

    $bw.Flush()
    $out = $ms.ToArray()
    $bw.Dispose(); $ms.Dispose()
    return $out
}

$base = New-BaseBitmap 256
$base.Save($previewPath, [System.Drawing.Imaging.ImageFormat]::Png)

$b32 = Scale-Bitmap $base 32
$b48 = Scale-Bitmap $base 48

$d32 = Icon-Frame $b32
$d48 = Icon-Frame $b48

# PNG frame via a real file round-trip (deterministic byte[])
$pngFile = Join-Path $outDir "app_256.png"
$base.Save($pngFile, [System.Drawing.Imaging.ImageFormat]::Png)
$pngBytes = [System.IO.File]::ReadAllBytes($pngFile)

$frames = @(@{ w = 32; data = $d32 }, @{ w = 48; data = $d48 }, @{ w = 256; data = $pngBytes })
$n = $frames.Count

# consistency check BEFORE writing
$expected = 6 + 16 * $n
foreach ($f in $frames) { $expected += $f.data.Length }
if ($expected -gt 8000000) { throw "icon payload too large: $expected" }

$ms = New-Object System.IO.MemoryStream
$bw = New-Object System.IO.BinaryWriter($ms)
$bw.Write([uint16]0)     # reserved
$bw.Write([uint16]1)     # type: icon
$bw.Write([uint16]$n)    # count
$offset = 6 + 16 * $n
foreach ($f in $frames) {
    $w = $f.w -band 0xFF   # 256 -> 0
    $bw.Write([byte]$w)
    $bw.Write([byte]$w)
    $bw.Write([byte]0)     # colors (0 = truecolor)
    $bw.Write([byte]0)     # reserved
    $bw.Write([uint16]1)   # planes
    $bw.Write([uint16]32)  # bpp
    $bw.Write([uint32]$f.data.Length)
    $bw.Write([uint32]$offset)
    $offset += [int]$f.data.Length
}
foreach ($f in $frames) {
    $len = [int]$f.data.Length
    $chunk = New-Object byte[] $len
    [System.Buffer]::BlockCopy([byte[]]$f.data, 0, $chunk, 0, $len)
    $bw.Write($chunk)
}
$bw.Flush()
if ($ms.Length -ne $expected) {
    throw ("ico size mismatch: wrote {0}, expected {1}" -f $ms.Length, $expected)
}
[System.IO.File]::WriteAllBytes($icoPath, $ms.ToArray())
$bw.Dispose(); $ms.Dispose()

"OK app.ico=$((Get-Item $icoPath).Length) bytes"
