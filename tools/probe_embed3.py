"""Embedding probe v3: Tk window + mpv --wid + IPC + resize follow.

Run outside the sandbox (launch_app) so named pipes work.
Writes a report to tools/probe_report.txt.
"""
import ctypes
import json
import os
import subprocess
import sys
import time
from ctypes import wintypes

import tkinter as tk

MPV = sys.argv[1] if len(sys.argv) > 1 else r"D:\app edit\Muxvideo\tools\mpv\mpv.exe"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"
ROOT = r"D:\app edit\Muxvideo\tools"
LOG = os.path.join(ROOT, "mpv_embed.log")
REPORT = os.path.join(ROOT, "probe_report.txt")
PIPE = r"\\.\pipe\muxprobe3"

report = open(REPORT, "w", encoding="utf-8", buffering=1)


def say(*a):
    line = " ".join(str(x) for x in a)
    print(line, flush=True)
    report.write(line + "\n")


user32 = ctypes.WinDLL("user32", use_last_error=True)


class RECT(ctypes.Structure):
    _fields_ = [("left", ctypes.c_long), ("top", ctypes.c_long),
                ("right", ctypes.c_long), ("bottom", ctypes.c_long)]


def enum_children(hwnd, tag):
    found = []

    @ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)
    def cb(h, l):
        rect = RECT()
        user32.GetWindowRect(h, ctypes.byref(rect))
        cls = ctypes.create_unicode_buffer(256)
        user32.GetClassNameW(h, cls, 256)
        vis = user32.IsWindowVisible(h)
        found.append((hex(h), cls.value, vis,
                      (rect.left, rect.top, rect.right, rect.bottom)))
        return True

    user32.EnumChildWindows(hwnd, cb, 0)
    say(f"[{tag}] parent={hex(hwnd)} children={len(found)}")
    for f in found:
        say(f"[{tag}]   child {f}")
    return found


root = tk.Tk()
root.title("EMBEDPROBE")
root.geometry("960x600")
root.configure(bg="#101014")
stage = tk.Frame(root, bg="black", highlightthickness=2, highlightbackground="#3030a0")
stage.place(x=20, y=20, width=640, height=360)
tk.Label(root, text="hello tk (below stage)", fg="white", bg="#101014").place(x=20, y=400)
root.update()

hwnd = stage.winfo_id()
say("stage hwnd", hwnd, hex(hwnd))

errf = open(LOG, "wb")
proc = subprocess.Popen(
    [MPV, MEDIA, f"--wid={hwnd}", "--no-config", "--no-border",
     "--input-default-bindings=no", "--loop-file=inf", "--vo=gpu",
     "--gpu-context=d3d11", "--osd-level=0", "--msg-level=all=info",
     f"--input-ipc-server={PIPE}"],
    cwd=os.path.dirname(MPV), stderr=errf, stdout=errf)
say("spawned mpv pid", proc.pid)


def ipc(pipe_name, payload, timeout=5.0):
    deadline = time.time() + timeout
    while True:
        try:
            f = open(pipe_name, "r+b", buffering=0)
            break
        except OSError as e:
            if time.time() > deadline:
                return f"CONNECT-FAIL {e}"
            time.sleep(0.2)
    try:
        f.write((json.dumps(payload) + "\n").encode())
        buf = b""
        while not buf.endswith(b"\n"):
            ch = f.read(1)
            if not ch:
                break
            buf += ch
        return buf.decode(errors="replace").strip()
    finally:
        f.close()


def step_children_early():
    enum_children(hwnd, "t+3s")


def step_ipc():
    say("[ipc] time-pos ->", ipc(PIPE, {"command": ["get_property", "time-pos"]}))
    say("[ipc] pause ->", ipc(PIPE, {"command": ["set_property", "pause", True]}))
    say("[ipc] seek 5 ->", ipc(PIPE, {"command": ["seek", 5, "absolute"]}))
    say("[ipc] time-pos ->", ipc(PIPE, {"command": ["get_property", "time-pos"]}))
    say("[ipc] pause off ->", ipc(PIPE, {"command": ["set_property", "pause", False]}))


def step_resize():
    say("resizing stage to 860x484")
    stage.place_configure(width=860, height=484)
    root.update()


def step_children_after_resize():
    enum_children(hwnd, "after-resize")


def step_report_win():
    say("tk geometry", root.geometry())


def shutdown():
    try:
        ipc(PIPE, {"command": ["quit"]}, timeout=2)
    except Exception:
        pass
    try:
        proc.terminate()
    except Exception:
        pass
    try:
        errf.close()
    except Exception:
        pass
    say("--- mpv log tail ---")
    try:
        say(open(LOG, "r", errors="replace").read()[-1500:])
    except Exception as e:
        say("log read failed", e)
    root.destroy()


root.after(3000, step_children_early)
root.after(4500, step_ipc)
root.after(9000, step_resize)
root.after(11000, step_children_after_resize)
root.after(180000, shutdown)
try:
    root.mainloop()
finally:
    report.close()
