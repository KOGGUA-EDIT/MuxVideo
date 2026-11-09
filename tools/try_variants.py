"""Variant matrix: find an mpv config that embeds into a Tk frame on this machine.

Each variant spawns mpv with --wid=<container hwnd>, waits, then kills it and
records the interesting log lines. Results -> tools/variants_report.txt
"""
import ctypes
import os
import subprocess
import sys
import time
import tkinter as tk
from ctypes import wintypes

MPV = r"D:\app edit\Muxvideo\tools\mpv\mpv.exe"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"
ROOT = r"D:\app edit\Muxvideo\tools"
REPORT = os.path.join(ROOT, "variants_report.txt")

report = open(REPORT, "w", encoding="utf-8", buffering=1)


def say(*a):
    report.write(" ".join(str(x) for x in a) + "\n")


user32 = ctypes.WinDLL("user32", use_last_error=True)
KERNEL = ctypes.WinDLL("kernel32", use_last_error=True)

WS_CHILD = 0x40000000
WS_VISIBLE = 0x10000000
WS_CLIPCHILDREN = 0x02000000
WS_CLIPSIBLINGS = 0x04000000

user32.CreateWindowExW.restype = wintypes.HWND
user32.CreateWindowExW.argtypes = [
    wintypes.DWORD, wintypes.LPCWSTR, wintypes.LPCWSTR, wintypes.DWORD,
    ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_int,
    wintypes.HWND, wintypes.HMENU, wintypes.HINSTANCE, wintypes.LPVOID]
user32.DefWindowProcW.restype = ctypes.c_ssize_t
user32.DefWindowProcW.argtypes = [wintypes.HWND, wintypes.UINT, wintypes.WPARAM, wintypes.LPARAM]
WNDPROC = ctypes.WINFUNCTYPE(ctypes.c_ssize_t, wintypes.HWND, wintypes.UINT,
                             wintypes.WPARAM, wintypes.LPARAM)


class WNDCLASSW(ctypes.Structure):
    _fields_ = [("style", wintypes.UINT), ("lpfnWndProc", WNDPROC),
                ("cbClsExtra", ctypes.c_int), ("cbWndExtra", ctypes.c_int),
                ("hInstance", wintypes.HINSTANCE), ("hIcon", wintypes.HICON),
                ("hCursor", wintypes.HANDLE), ("hbrBackground", wintypes.HBRUSH),
                ("lpszMenuName", wintypes.LPCWSTR), ("lpszClassName", wintypes.LPCWSTR)]


_procs = []


def raw_child(parent, name="muxraw"):
    wndproc = WNDPROC(lambda h, m, w, l: user32.DefWindowProcW(h, m, w, l))
    _procs.append(wndproc)
    wc = WNDCLASSW()
    wc.style = 0
    wc.lpfnWndProc = wndproc
    wc.hInstance = KERNEL.GetModuleHandleW(None)
    wc.hbrBackground = None
    wc.lpszClassName = name
    atom = user32.RegisterClassW(ctypes.byref(wc))
    say(f"RegisterClassW {name} -> atom={atom} err={ctypes.get_last_error()}")
    h = user32.CreateWindowExW(0, name, "", WS_CHILD | WS_VISIBLE | WS_CLIPCHILDREN | WS_CLIPSIBLINGS,
                               0, 0, 400, 300, parent, None, wc.hInstance, None)
    say(f"raw child hwnd={h} err={ctypes.get_last_error()}")
    return h


root = tk.Tk()
root.title("MUXVARIANTS")
root.geometry("1000x700")
root.configure(bg="#101014")
frame = tk.Frame(root, bg="black", highlightthickness=1, highlightbackground="#404060")
frame.place(x=20, y=20, width=480, height=270)
tk.Frame(root, bg="#202030").place(x=520, y=20, width=460, height=270)
status = tk.Label(root, text="running variants...", fg="white", bg="#101014", justify="left")
status.place(x=20, y=320)
root.update()

tk_hwnd = frame.winfo_id()
say("tk frame hwnd", tk_hwnd, hex(tk_hwnd))
raw_hwnd = raw_child(tk_hwnd)
root.update()

VARIANTS = [
    ("gpu-auto-wid", ["--vo=gpu"], tk_hwnd),
    ("gpu-d3d11-wid", ["--vo=gpu", "--gpu-context=d3d11"], tk_hwnd),
    ("gpu-win-wid", ["--vo=gpu", "--gpu-context=win"], tk_hwnd),
    ("gpu-opengl-wid", ["--vo=gpu", "--gpu-context=opengl"], tk_hwnd),
    ("gpu-next-d3d11-wid", ["--vo=gpu-next", "--gpu-context=d3d11"], tk_hwnd),
    ("gpu-d3d11-raw", ["--vo=gpu", "--gpu-context=d3d11"], raw_hwnd),
    ("direct3d-wid", ["--vo=direct3d"], tk_hwnd),
    ("no-wid-standalone", ["--vo=gpu", "--gpu-context=d3d11"], None),
]

BASE = [MPV, MEDIA, "--no-config", "--no-border", "--input-default-bindings=no",
        "--loop-file=inf", "--osd-level=0", "--audio=no", "--msg-level=all=info"]

results = []


def run_variant(idx):
    if idx >= len(VARIANTS):
        say("=== SUMMARY ===")
        for name, ok, detail in results:
            say(f"{'OK  ' if ok else 'FAIL'} {name}: {detail}")
        status.config(text="done")
        root.after(1500, root.destroy)
        return
    name, opts, parent = VARIANTS[idx]
    logfile = os.path.join(ROOT, f"var_{name}.log")
    args = BASE + opts
    if parent is not None:
        args.append(f"--wid={parent}")
    status.config(text=f"[{idx+1}/{len(VARIANTS)}] {name}")
    root.update()
    say(f"--- {name}: {' '.join(opts)} wid={parent}")
    with open(logfile, "wb") as errf:
        p = subprocess.Popen(args, cwd=os.path.dirname(MPV), stderr=errf, stdout=errf)
        time.sleep(3.2)
        p.terminate()
        try:
            p.wait(timeout=3)
        except Exception:
            p.kill()
    text = open(logfile, "r", errors="replace").read()
    ok = "VO: [" in text and "unable to create window" not in text
    interesting = [ln for ln in text.splitlines()
                   if any(k in ln for k in ("VO:", "unable", "Failed", "Error", "error", "context"))]
    detail = " | ".join(interesting[-4:])[:400]
    say("    " + detail)
    results.append((name, ok, detail))
    root.after(300, lambda: run_variant(idx + 1))


root.after(500, lambda: run_variant(0))
try:
    root.mainloop()
finally:
    report.close()
