# Injects res/app.ico into the built executables via UpdateResourceW.
# The GNU toolchain here has no windres/rc.exe, so resources are added
# post-link with the Win32 API instead. Idempotent: same ids are rewritten.
# Usage: powershell -File res\inject_icon.ps1 [exe...]
#        (default: target\release\zakonexport-cli.exe + zakonexport-gui.exe)
$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

$src = @'
using System;
using System.Runtime.InteropServices;
public static class ResInject {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr BeginUpdateResourceW(string pFileName, bool bDeleteExistingResources);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool UpdateResourceW(IntPtr hUpdate, IntPtr lpType, IntPtr lpName, ushort wLanguage, byte[] lpData, uint cb);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool EndUpdateResourceW(IntPtr hUpdate, bool fDiscard);
}
'@
Add-Type -TypeDefinition $src

# ---- parse res/app.ico ------------------------------------------------------
$icoPath = Join-Path $PSScriptRoot "app.ico"
$ico = [System.IO.File]::ReadAllBytes($icoPath)
function ReadU16([byte[]]$b, [int]$i) { $b[$i] -bor ($b[$i + 1] -shl 8) }
function ReadU32([byte[]]$b, [int]$i) { [BitConverter]::ToUInt32($b, $i) }

$count = ReadU16 $ico 4
$entries = @()
for ($e = 0; $e -lt $count; $e++) {
    $off = 6 + 16 * $e
    $entries += [pscustomobject]@{
        Width    = [int]$ico[$off]        # 256 stored as 0
        Height   = [int]$ico[$off + 1]
        Colors   = [int]$ico[$off + 2]
        Planes   = ReadU16 $ico ($off + 4)
        BitCount = ReadU16 $ico ($off + 6)
        Length   = [int](ReadU32 $ico ($off + 8))
        Off      = [int](ReadU32 $ico ($off + 12))
    }
}

# ---- run ---------------------------------------------------------------------
$exes = if ($args.Count -gt 0) { $args } else {
    @("$PSScriptRoot\..\target\release\zakonexport-cli.exe",
      "$PSScriptRoot\..\target\release\zakonexport-gui.exe")
}
$RT_ICON = [IntPtr]3
$RT_GROUP_ICON = [IntPtr]14
$LANG_EN = 0x0409

foreach ($exe in $exes) {
    if (-not (Test-Path $exe)) { throw "exe not found: $exe" }
    $h = [ResInject]::BeginUpdateResourceW($exe, $false)
    if ($h -eq [IntPtr]::Zero) { throw "BeginUpdateResource failed for $exe (locked?)" }
    try {
        for ($i = 0; $i -lt $entries.Count; $i++) {
            $en = $entries[$i]
            $data = New-Object byte[] $en.Length
            [System.Buffer]::BlockCopy($ico, [int]$en.Off, $data, 0, [int]$en.Length)
            $id = [IntPtr]($i + 1)
            if (-not [ResInject]::UpdateResourceW($h, $RT_ICON, $id, $LANG_EN, $data, [uint32]$en.Length)) {
                throw "UpdateResource RT_ICON #$($i+1) failed for $exe"
            }
        }
        # GRPICONDIRENTRY table: reserved,type,count + 14-byte descriptors
        $grp = New-Object System.IO.MemoryStream
        $bw = New-Object System.IO.BinaryWriter($grp)
        $bw.Write([uint16]0); $bw.Write([uint16]1); $bw.Write([uint16]$entries.Count)
        for ($i = 0; $i -lt $entries.Count; $i++) {
            $en = $entries[$i]
            $bw.Write([byte]($en.Width -band 0xFF)); $bw.Write([byte]($en.Height -band 0xFF))
            $bw.Write([byte]$en.Colors); $bw.Write([byte]0)
            $bw.Write([uint16]$en.Planes); $bw.Write([uint16]$en.BitCount)
            $bw.Write([uint32]$en.Length)
            $bw.Write([uint16]($i + 1))   # ordinal of the matching RT_ICON
        }
        $bw.Flush()
        if (-not [ResInject]::UpdateResourceW($h, $RT_GROUP_ICON, [IntPtr]1, $LANG_EN, $grp.ToArray(), [uint32]$grp.Length)) {
            throw "UpdateResource RT_GROUP_ICON failed for $exe"
        }
        $bw.Dispose(); $grp.Dispose()
    } finally {
        if (-not [ResInject]::EndUpdateResourceW($h, $false)) {
            throw "EndUpdateResource failed for $exe"
        }
    }
    "injected -> $exe"
}
