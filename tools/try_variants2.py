"""Variant matrix 2: is the failure about child windows vs top-level parents?

Also compare against the older Topaz mpv build (which earlier showed VO success).
"""
import ctypes
import os
import subprocess
import time
import tkinter as tk
from ctypes import wintypes

SHIN = r"D:\app edit\Muxvideo\tools\mpv\mpv.exe"
TOPAZ = r"C:\Program Files\Topaz Labs LLC\Topaz Video\mpv.exe"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"
ROOT = r"D:\app edit\Muxvideo\tools"
REPORT = os.path.join(ROOT, "variants2_report.txt")

report = open(REPORT, "w", encoding="utf-8", buffering=1)


def say(*a):
    report.write(" ".join(str(x) for x in a) + "\n")


root = tk.Tk()
root.title("MUXVARIANTS2")
root.geometry("1000x700")
root.configure(bg="#101014")
frame = tk.Frame(root, bg="black", highlightthickness=1, highlightbackground="#404060")
frame.place(x=20, y=20, width=480, height=270)
status = tk.Label(root, text="running...", fg="white", bg="#101014")
status.place(x=20, y=320)
root.update()

frame_hwnd = frame.winfo_id()
top_hwnd = int(root.wm_frame(), 16) if root.wm_frame().startswith("0x") else root.winfo_id()
top_hwnd = root.winfo_id()
say("frame hwnd", hex(frame_hwnd), "toplevel hwnd", hex(top_hwnd))

BASE = ["--no-config", "--no-border", "--input-default-bindings=no",
        "--loop-file=inf", "--osd-level=0", "--audio=no"]

VARIANTS = [
    ("shin-debug-frame", SHIN, ["--vo=gpu", "--gpu-context=d3d11", "--msg-level=all=debug"], frame_hwnd),
    ("shin-frame", SHIN, ["--vo=gpu", "--gpu-context=d3d11"], frame_hwnd),
    ("shin-toplevel", SHIN, ["--vo=gpu", "--gpu-context=d3d11"], top_hwnd),
    ("topaz-frame", TOPAZ, ["--vo=gpu", "--gpu-context=d3d11"], frame_hwnd),
    ("topaz-toplevel", TOPAZ, ["--vo=gpu", "--gpu-context=d3d11"], top_hwnd),
]

results = []


def run_variant(idx):
    if idx >= len(VARIANTS):
        say("=== SUMMARY ===")
        for name, ok, detail in results:
            say(f"{'OK  ' if ok else 'FAIL'} {name}: {detail}")
        status.config(text="done")
        root.after(1500, root.destroy)
        return
    name, exe, opts, parent = VARIANTS[idx]
    logfile = os.path.join(ROOT, f"var2_{name}.log")
    args = [exe, MEDIA] + BASE + opts + [f"--wid={parent}"]
    status.config(text=f"[{idx+1}/{len(VARIANTS)}] {name}")
    root.update()
    say(f"--- {name}: {os.path.basename(exe)} {' '.join(opts)} wid={hex(parent)}")
    with open(logfile, "wb") as errf:
        p = subprocess.Popen(args, cwd=os.path.dirname(exe), stderr=errf, stdout=errf)
        time.sleep(3.2)
        p.terminate()
        try:
            p.wait(timeout=3)
        except Exception:
            p.kill()
    text = open(logfile, "r", errors="replace").read()
    ok = "VO: [" in text and "unable to create window" not in text
    interesting = [ln for ln in text.splitlines()
                   if any(k in ln for k in ("VO:", "unable", "Failed", "Error",
                                            "class", "Class", "window", "Window"))]
    detail = " | ".join(interesting[-6:])[:700]
    say("    " + detail)
    results.append((name, ok, detail))
    root.after(300, lambda: run_variant(idx + 1))


root.after(500, lambda: run_variant(0))
try:
    root.mainloop()
finally:
    report.close()
