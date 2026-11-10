param(
    [int]$X = 600,
    [int]$Y = 400,
    [int]$GapMs = 400
)

$sig = @'
using System;
using System.Runtime.InteropServices;
public class SeqMouse {
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, uint data, IntPtr extra);
  public const uint LEFTDOWN = 0x0002;
  public const uint LEFTUP = 0x0004;
}
'@
if (-not ('SeqMouse' -as [type])) { Add-Type -TypeDefinition $sig }

[void][SeqMouse]::SetCursorPos($X, $Y)
Start-Sleep -Milliseconds 120
for ($i = 0; $i -lt 2; $i++) {
    [SeqMouse]::mouse_event([SeqMouse]::LEFTDOWN, 0, 0, 0, [IntPtr]::Zero)
    Start-Sleep -Milliseconds 40
    [SeqMouse]::mouse_event([SeqMouse]::LEFTUP, 0, 0, 0, [IntPtr]::Zero)
    if ($i -eq 0) { Start-Sleep -Milliseconds $GapMs }
}
"clicked x2 at $X,$Y gap=$GapMs ms"
