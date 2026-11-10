param([int]$X = 600, [int]$Y = 400, [int]$HoldMs = 60)

$sig = @'
using System;
using System.Runtime.InteropServices;
public class TapMouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
  public const uint LEFTDOWN = 0x0002;
  public const uint LEFTUP = 0x0004;
}
'@
if (-not ('TapMouse' -as [type])) { Add-Type -TypeDefinition $sig }

[void][TapMouse]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 120
[TapMouse]::mouse_event([TapMouse]::LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
Start-Sleep -Milliseconds $HoldMs
[TapMouse]::mouse_event([TapMouse]::LEFTUP, 0, 0, 0, [IntPtr]::Zero)
"tapped $X,$Y"
