$sig = @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public class WinRect {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int cx, int cy, int cw, int ch, uint flags);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(int x, int y);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  public struct RECT { public int Left, Top, Right, Bottom; }
}
'@
if (-not ('WinRect' -as [type])) { Add-Type -TypeDefinition $sig }

$p = Get-Process muxvideo -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $p) { Write-Output 'NO PROCESS'; exit 1 }
$h = $p.MainWindowHandle
$r = New-Object WinRect+RECT
[void][WinRect]::GetWindowRect($h, [ref]$r)
$w = $r.Right - $r.Left
$hh = $r.Bottom - $r.Top
$px = $r.Left + [int]($w * 0.42)
$py = $r.Top + [int]($hh * 0.45)
[WinRect]::SetWindowPos($h, [IntPtr](-1), 0, 0, 0, 0, 0x0013) | Out-Null
[WinRect]::SetForegroundWindow($h) | Out-Null
Start-Sleep -Milliseconds 200
[WinRect]::SetWindowPos($h, [IntPtr](-2), 0, 0, 0, 0, 0x0013) | Out-Null
Start-Sleep -Milliseconds 300

$fg = [WinRect]::GetForegroundWindow()
$under = [WinRect]::WindowFromPoint($px, $py)
$cls = New-Object System.Text.StringBuilder 128
[void][WinRect]::GetClassName($under, $cls, 128)
Write-Output "app=$h rect=($($r.Left),$($r.Top),$w,$hh) probe=($px,$py)"
Write-Output "foreground=$fg isApp=$($fg -eq $h) underClass=$($cls.ToString())"
