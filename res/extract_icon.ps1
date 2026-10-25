Add-Type -AssemblyName System.Drawing
$exe = Join-Path $PSScriptRoot "..\target\release\zakonexport-gui.exe"
$i = [System.Drawing.Icon]::ExtractAssociatedIcon((Resolve-Path $exe).Path)
$bmp = $i.ToBitmap()
$out = Join-Path $PSScriptRoot "icon_extracted.png"
$bmp.Save($out, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Host ("saved " + $i.Width + "x" + $i.Height)
