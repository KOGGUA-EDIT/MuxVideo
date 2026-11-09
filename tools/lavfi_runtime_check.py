"""Headless check: can `lavfi-complex` be set after `loadfile` at runtime?

No window is created (`vo=null`), so this can run while another agent owns the
desktop.  It also reads `time-pos` directly from libmpv, independent of the
event/property cache.
"""
import ctypes
import os
import sys
import time

DLL = os.path.join(os.path.dirname(os.path.abspath(__file__)),
                   "mpv", "libmpv-2.dll")
MEDIA = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                     "media", "clipA.mp4")

lib = ctypes.CDLL(DLL)
lib.mpv_create.restype = ctypes.c_void_p
lib.mpv_initialize.argtypes = [ctypes.c_void_p]
lib.mpv_initialize.restype = ctypes.c_int
lib.mpv_set_option_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
lib.mpv_set_option_string.restype = ctypes.c_int
lib.mpv_command.argtypes = [ctypes.c_void_p, ctypes.POINTER(ctypes.c_char_p)]
lib.mpv_command.restype = ctypes.c_int
lib.mpv_set_property_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_char_p]
lib.mpv_set_property_string.restype = ctypes.c_int
lib.mpv_get_property_string.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
lib.mpv_get_property_string.restype = ctypes.c_void_p
lib.mpv_free.argtypes = [ctypes.c_void_p]
lib.mpv_wait_event.argtypes = [ctypes.c_void_p, ctypes.c_double]
lib.mpv_wait_event.restype = ctypes.c_void_p
lib.mpv_terminate_destroy.argtypes = [ctypes.c_void_p]
lib.mpv_error_string.argtypes = [ctypes.c_int]
lib.mpv_error_string.restype = ctypes.c_char_p

failures = []


def say(*args):
    line = " ".join(str(a) for a in args)
    print(line, flush=True)


def check(name, rc):
    if rc < 0:
        err = lib.mpv_error_string(rc).decode("utf-8", "replace")
        say(f"FAIL {name}: rc={rc} ({err})")
        failures.append(name)
    return rc


def get_prop(handle, name):
    raw = lib.mpv_get_property_string(handle, name.encode())
    if not raw:
        return None
    try:
        return ctypes.cast(raw, ctypes.c_char_p).value.decode("utf-8", "replace")
    finally:
        lib.mpv_free(raw)


def pump(handle, seconds):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        lib.mpv_wait_event(handle, 0.05)


handle = lib.mpv_create()
if not handle:
    sys.exit("mpv_create failed")
say("dll:", DLL)
for name, value in [
    ("vo", "null"),
    ("audio", "no"),
    ("osc", "no"),
    ("osd-level", "0"),
    ("config", "no"),
    ("terminal", "no"),
    ("idle", "yes"),
    ("keep-open", "yes"),
    ("loop-file", "no"),
]:
    rc = lib.mpv_set_option_string(handle, name.encode(), value.encode())
    check(f"option {name}={value}", rc)
check("initialize", lib.mpv_initialize(handle))

argv = (ctypes.c_char_p * 4)(b"loadfile", MEDIA.encode(), b"replace", None)
check("loadfile clipA", lib.mpv_command(handle, argv))

time.sleep(1.2)
pump(handle, 0.2)
pos1 = get_prop(handle, "time-pos")
say("time-pos after 1.2s:", pos1)
time.sleep(1.2)
pump(handle, 0.2)
pos2 = get_prop(handle, "time-pos")
say("time-pos after 2.4s:", pos2)
if not pos1 or not pos2 or float(pos2) <= float(pos1):
    failures.append("time-pos did not advance")

benchmark = "[vid1]null[vo]"
rc = lib.mpv_set_property_string(handle, b"lavfi-complex", benchmark.encode())
check("set lavfi-complex at runtime", rc)
say("set_property rc:", rc, "->", lib.mpv_error_string(rc).decode("utf-8", "replace"))
seen = get_prop(handle, "lavfi-complex")
say("get_property lavfi-complex:", repr(seen))
if seen != benchmark:
    failures.append("lavfi-complex did not round-trip")

time.sleep(0.8)
pump(handle, 0.2)
pos3 = get_prop(handle, "time-pos")
say("time-pos after set lavfi-complex:", pos3)
if not pos3:
    failures.append("time-pos unavailable after lavfi-complex")

# Also verify the option is not listed as read-only at runtime.
rc = lib.mpv_set_property_string(handle, b"lavfi-complex", b"")
say("clear lavfi-complex rc:", rc,
    "->", lib.mpv_error_string(rc).decode("utf-8", "replace"))
seen = get_prop(handle, "lavfi-complex")
say("get_property after clear:", repr(seen))

lib.mpv_command(handle, (ctypes.c_char_p * 2)(b"quit", None))
lib.mpv_terminate_destroy(handle)

say("RESULT:", "PASS" if not failures else "FAIL: " + ", ".join(failures))
sys.exit(0 if not failures else 1)
