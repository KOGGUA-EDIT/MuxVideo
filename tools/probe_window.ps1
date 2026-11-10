param(
    [int]$x = 500,
    [int]$y = 400
)
$sig = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinProbe {
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(int x, int y);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int cx, int cy, int cw, int ch, uint flags);
}
'@
if (-not ('WinProbe' -as [type])) { Add-Type -TypeDefinition $sig }

$p = Get-Process muxvideo -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) { Write-Output 'NO PROCESS'; exit 1 }
$hwnd = $p.MainWindowHandle
[WinProbe]::SetWindowPos($hwnd, [IntPtr](-1), 0, 0, 0, 0, 0x0013) | Out-Null
[WinProbe]::SetForegroundWindow($hwnd) | Out-Null
Start-Sleep -Milliseconds 250
[WinProbe]::SetWindowPos($hwnd, [IntPtr](-2), 0, 0, 0, 0, 0x0013) | Out-Null
Start-Sleep -Milliseconds 250

$w = [WinProbe]::WindowFromPoint($x, $y)
$cls = New-Object System.Text.StringBuilder 128
[void][WinProbe]::GetClassName($w, $cls, 128)
$txt = New-Object System.Text.StringBuilder 256
[void][WinProbe]::GetWindowText($w, $txt, 256)
$root = [WinProbe]::GetAncestor($w, 2)
$rcls = New-Object System.Text.StringBuilder 128
[void][WinProbe]::GetClassName($root, $rcls, 128)
Write-Output "pid=$($p.Id) point=$x,$y class='$($cls.ToString())' title='$($txt.ToString())' rootClass='$($rcls.ToString())'"
if ($cls.ToString() -eq 'MuxVideoSurface') { Write-Output 'PROBE: video plane under cursor (OK)' } else { Write-Output 'PROBE: cursor is NOT over the video plane' }
