"""Approach tests:
  A) external official mpv (standalone) + Win32 SetParent into a Tk frame
  B) in-process libmpv (mpv.net-DW v0.39 libmpv-2.dll) with wid option
Results -> tools/approaches_report.txt
"""
import ctypes
import os
import subprocess
import time
import tkinter as tk
from ctypes import wintypes

OFFICIAL = r"D:\app edit\Muxvideo\tools\mpv\mpv.exe"
LIBMPV = r"D:\ruanjian\mpv\mpv.net-DW v2.1.0\libmpv-2.dll"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"
ROOT = r"D:\app edit\Muxvideo\tools"
REPORT = os.path.join(ROOT, "approaches_report.txt")

report = open(REPORT, "w", encoding="utf-8", buffering=1)


def say(*a):
    report.write(" ".join(str(x) for x in a) + "\n")


user32 = ctypes.WinDLL("user32", use_last_error=True)
user32.GetWindowLongPtrW = user32.GetWindowLongW  # ok for style on 64-bit for GWL_STYLE? use proper below
try:
    user32.SetWindowLongPtrW
except AttributeError:
    user32.SetWindowLongPtrW = user32.SetWindowLongW

WS_CHILD = 0x40000000
WS_VISIBLE = 0x10000000
GWL_STYLE = -16
SWP_FRAMECHANGED = 0x0020
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010

EnumWindowsProc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)


def windows_of_pid(pid):
    out = []

    def cb(h, l):
        p = wintypes.DWORD()
        user32.GetWindowThreadProcessId(h, ctypes.byref(p))
        if p.value == pid:
            cls = ctypes.create_unicode_buffer(256)
            user32.GetClassNameW(h, cls, 256)
            out.append((h, cls.value, user32.IsWindowVisible(h)))
        return True

    user32.EnumWindows(EnumWindowsProc(cb), 0)
    return out


root = tk.Tk()
root.title("MUXAPPROACH")
root.geometry("1000x700")
root.configure(bg="#101014")
frame = tk.Frame(root, bg="black", highlightthickness=1, highlightbackground="#404060")
frame.place(x=20, y=20, width=640, height=360)
status = tk.Label(root, text="A: setparent...", fg="white", bg="#101014", justify="left")
status.place(x=20, y=400)
root.update()
frame_hwnd = frame.winfo_id()
say("frame hwnd", hex(frame_hwnd))


# ---------------- A: SetParent ---------------
def testA():
    logfile = os.path.join(ROOT, "A_setparent.log")
    args = [OFFICIAL, MEDIA, "--no-config", "--no-border", "--input-default-bindings=no",
            "--loop-file=inf", "--osd-level=0", "--audio=no", "--vo=gpu",
            "--gpu-context=d3d11", "--msg-level=all=info"]
    errf = open(logfile, "wb")
    p = subprocess.Popen(args, cwd=os.path.dirname(OFFICIAL), stderr=errf, stdout=errf)
    say("A spawned", p.pid)

    def attach():
        wins = windows_of_pid(p.pid)
        say("A windows:", [(hex(h), c, v) for h, c, v in wins])
        cands = [h for h, c, v in wins if c not in ("GDI+ Hook Window Class",) and v]
        if not cands:
            say("A FAIL no window")
            return
        mw = cands[0]
        say("A setparent ->", user32.SetParent(mw, frame_hwnd), "err", ctypes.get_last_error())
        style = user32.GetWindowLongW(mw, GWL_STYLE)
        user32.SetWindowLongW(mw, GWL_STYLE, (style | WS_CHILD | WS_VISIBLE) & ~0x00C00000)
        user32.SetWindowPos(mw, 0, 0, 0, 640, 360, SWP_FRAMECHANGED | SWP_NOZORDER | SWP_NOACTIVATE)
        say("A child list now:", [(hex(h), c) for h, c, v in windows_of_pid(p.pid)])
        say("A children of frame:", end_children(frame_hwnd))

    root.after(2500, attach)
    root.after(20000, lambda: (p.terminate(), errf.close(), testB()))


def end_children(parent):
    found = []
    proc = ctypes.WINFUNCTYPE(wintypes.BOOL, wintypes.HWND, wintypes.LPARAM)(
        lambda h, l: (found.append((hex(h), get_class(h))), True)[1])
    user32.EnumChildWindows(parent, proc, 0)
    return found


def get_class(h):
    b = ctypes.create_unicode_buffer(256)
    user32.GetClassNameW(h, b, 256)
    return b.value


# ---------------- B: libmpv in process ---------------
lib = ctypes.CDLL(LIBMPV)
lib.mpv_create.restype = ctypes.c_void_p
lib.mpv_initialize.argtypes = [ctypes.c_void_p]
lib.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
lib.mpv_command.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]
lib.mpv_wait_event.restype = ctypes.c_void_p
lib.mpv_wait_event.argtypes = [ctypes.c_void_p, ctypes.c_double]
lib.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
lib.mpv_request_log_messages.argtypes = [ctypes.c_void_p, ctypes.c_char_p]


class MpvEvent(ctypes.Structure):
    _fields_ = [("event_id", ctypes.c_int), ("error", ctypes.c_int),
                ("reply_userdata", ctypes.c_uint64), ("data", ctypes.c_void_p)]


class MpvLog(ctypes.Structure):
    _fields_ = [("prefix", ctypes.c_char_p), ("level", ctypes.c_char_p),
                ("text", ctypes.c_char_p), ("log_level", ctypes.c_int)]


def testB():
    status.config(text="B: libmpv + wid")
    root.update()
    h = lib.mpv_create()
    say("B handle", h)
    for k, v in [(b"wid", str(frame_hwnd).encode()), (b"vo", b"gpu"),
                 (b"gpu-context", b"d3d11"), (b"osc", b"no"), (b"input-default-bindings", b"no"),
                 (b"loop-file", b"inf"), (b"osd-level", b"0"), (b"audio", b"no"),
                 (b"config", b"no"), (b"terminal", b"yes")]:
        say("B set", k, lib.mpv_set_option_string(h, k, v))
    say("B initialize", lib.mpv_initialize(h))
    lib.mpv_request_log_messages(h, b"info")
    arr = (ctypes.c_char_p * 3)(b"loadfile", MEDIA.encode(), None)
    say("B loadfile", lib.mpv_command(h, arr))

    deadline = time.time() + 8
    logs = []
    while time.time() < deadline:
        ev = MpvEvent.from_address(lib.mpv_wait_event(h, 0.1))
        if ev.event_id == 2:
            m = MpvLog.from_address(ev.data)
            line = f"[{m.prefix.decode()}] {m.text.decode(errors='replace').strip()}"
            logs.append(line)
        root.update()
    say("B frame children:", end_children(frame_hwnd))
    for ln in logs[-15:]:
        say("B log:", ln)
    say("B ===== done, keeping window open 60s =====")


root.after(500, testA)
try:
    root.mainloop()
finally:
    report.close()
