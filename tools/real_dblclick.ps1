param([int]$X = 100, [int]$Y = 100)

$sig = @'
using System;
using System.Runtime.InteropServices;
public class RealClick {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
  public const uint LEFTDOWN = 0x0002;
  public const uint LEFTUP = 0x0004;
}
'@
if (-not ('RealClick' -as [type])) { Add-Type -TypeDefinition $sig }

[void][RealClick]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 120
for ($i = 0; $i -lt 2; $i++) {
    [RealClick]::mouse_event([RealClick]::LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 30
    [RealClick]::mouse_event([RealClick]::LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 90
}
"double-clicked $X,$Y"
