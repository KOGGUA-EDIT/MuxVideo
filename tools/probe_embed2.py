import subprocess, time, os, sys
import tkinter as tk

MPV = sys.argv[1] if len(sys.argv) > 1 else r"C:\Program Files\Topaz Labs LLC\Topaz Video\mpv.exe"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"
LOG = r"D:\app edit\Muxvideo\tools\mpv_embed.log"

root = tk.Tk()
root.title("EMBEDPROBE")
root.geometry("900x560")
root.configure(bg="#101014")
stage = tk.Frame(root, bg="black", highlightthickness=2, highlightbackground="#3030a0")
stage.place(x=20, y=20, width=640, height=360)
tk.Label(root, text="hello tk (below stage)", fg="white", bg="#101014").place(x=20, y=400)
root.update()

hwnd = stage.winfo_id()
print("hwnd", hwnd, hex(hwnd), flush=True)
errf = open(LOG, "wb")
proc = subprocess.Popen([MPV, MEDIA, f"--wid={hwnd}", "--no-config", "--no-border",
                         "--input-default-bindings=no", "--loop-file=inf",
                         "--vo=gpu", "--gpu-context=d3d11", "--msg-level=all=info",
                         "--osd-level=0"],
                        cwd=os.path.dirname(MPV), stderr=errf, stdout=errf)
print("spawned", proc.pid, flush=True)


def list_children():
    import ctypes
    from ctypes import wintypes
    user32 = ctypes.WinDLL("user32", use_last_error=True)

    class RECT(ctypes.Structure):
        _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                    ("right", ctypes.c_long), ("bottom", ctypes.c_long)]

    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def cb(h, l):
        rect = RECT()
        user32.GetWindowRect(h, ctypes.byref(rect))
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(h, cls, 256)
        found.append((hex(h), cls.value, (rect.left, rect.top, rect.right, rect.bottom)))
        return True

    user32.EnumChildWindows(hwnd, cb, 0)
    for f in found:
        print("child", f, flush=True)


root.after(2500, list_children)
root.after(30000, lambda: (proc.terminate(), errf.close(), root.destroy()))
root.mainloop()
print("--- log ---", flush=True)
print(open(LOG, "r", errors="replace").read()[-2000:])
