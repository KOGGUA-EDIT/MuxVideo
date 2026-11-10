param(
    [int]$X1 = 500,
    [int]$Y1 = 400,
    [int]$X2 = 620,
    [int]$Y2 = 400,
    [int]$Steps = 6
)

$sig = @'
using System;
using System.Runtime.InteropServices;
public class RealMouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
  public const uint LEFTDOWN = 0x0002;
  public const uint LEFTUP = 0x0004;
}
'@
if (-not ('RealMouse' -as [type])) { Add-Type -TypeDefinition $sig }

[void][RealMouse]::SetCursorPos($X1, $Y1)
Start-Sleep -Milliseconds 120
[RealMouse]::mouse_event([RealMouse]::LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds 80
for ($i = 1; $i -le $Steps; $i++) {
    $x = [int]($X1 + ($X2 - $X1) * $i / $Steps)
    $y = [int]($Y1 + ($Y2 - $Y1) * $i / $Steps)
    [void][RealMouse]::SetCursorPos($x, $y)
    Start-Sleep -Milliseconds 40
}
Start-Sleep -Milliseconds 60
[RealMouse]::mouse_event([RealMouse]::LEFTUP, 0, 0, 0, [IntPtr]::Zero)
"dragged $X1,$Y1 -> $X2,$Y2"
