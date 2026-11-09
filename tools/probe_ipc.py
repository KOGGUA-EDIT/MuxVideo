import json, subprocess, time, sys, threading, os
MPV = r"C:\Program Files\Topaz Labs LLC\Topaz Video\mpv.exe"
PIPE = r"\\.\pipe\muxprobe"
p = subprocess.Popen([MPV, "--no-config", "--idle=yes", "--input-ipc-server=" + PIPE,
                      "--really-quiet"], cwd=os.path.dirname(MPV))
time.sleep(1.5)
try:
    f = open(PIPE, "r+b", 0)
    f.write(json.dumps({"command": ["get_property", "mpv-version"], "request_id": 1}).encode() + b"\n")
    time.sleep(0.4)
    print("reply:", f.read(65536)[:200])
except OSError as e:
    print("open failed:", e)
p.terminate()
