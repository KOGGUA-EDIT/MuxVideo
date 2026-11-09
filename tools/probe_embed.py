"""Probe: can mpv embed into a Tk frame on Windows, and can we talk JSON IPC?"""
import json, os, subprocess, sys, threading, time
import tkinter as tk

MPV = r"C:\Program Files\Topaz Labs LLC\Topaz Video\mpv.exe"
PIPE = r"\\.\pipe\muxprobe"
MEDIA = r"D:\app edit\Muxvideo\media\clipA.mp4"


def ipc_send(conn, timeout=2.0):
    pass


class Ipc:
    def __init__(self, path):
        self.path = path
        self.pipe = None
        self.lines = []
        self.cv = threading.Condition()
        self.ready = threading.Event()

    def connect(self, wait=8.0):
        end = time.time() + wait
        while time.time() < end:
            try:
                self.pipe = open(self.path, "r+b", buffering=0)
                self.ready.set()
                threading.Thread(target=self._reader, daemon=True).start()
                return True
            except OSError:
                time.sleep(0.1)
        return False

    def _reader(self):
        buf = b""
        while True:
            try:
                chunk = self.pipe.read(65536)
            except OSError:
                break
            if not chunk:
                break
            buf += chunk
            while b"\n" in buf:
                line, buf = buf.split(b"\n", 1)
                if not line.strip():
                    continue
                try:
                    msg = json.loads(line)
                except Exception:
                    continue
                with self.cv:
                    self.lines.append(msg)
                    self.cv.notify_all()

    def send(self, *args):
        data = json.dumps({"command": list(args)}, ensure_ascii=False).encode() + b"\n"
        self.pipe.write(data)

    def wait_reply(self, request_id, timeout=3.0):
        end = time.time() + timeout
        with self.cv:
            while time.time() < end:
                for m in self.lines:
                    if m.get("request_id") == request_id:
                        return m
                self.cv.wait(0.05)
        return None

    def get(self, prop):
        rid = 999
        self.send("get_property", prop)
        data = json.dumps({"command": ["get_property", prop], "request_id": rid}).encode() + b"\n"
        self.pipe.write(data)
        return self.wait_reply(rid)


root = tk.Tk()
root.title("embed probe")
root.geometry("1000x600")
root.configure(bg="#101014")

left = tk.Frame(root, bg="black")
left.place(x=10, y=10, width=480, height=270)
right = tk.Frame(root, bg="#301030")
right.place(x=510, y=10, width=480, height=270)
root.update()

hwnd = left.winfo_id()
print("tk hwnd:", hwnd, hex(hwnd))

args = [
    MPV, MEDIA,
    f"--wid={hwnd}",
    f"--input-ipc-server={PIPE}",
    "--no-config", "--no-border", "--osc=no", "--input-default-bindings=no",
    "--keep-open=yes", "--pause=yes", "--vo=gpu", "--gpu-context=d3d11",
    "--hwdec=auto-copy", "--really-quiet",
]
proc = subprocess.Popen(args, cwd=os.path.dirname(MPV))

ipc = Ipc(PIPE)
ok = ipc.connect()
print("ipc connected:", ok)

def after():
    if not ok:
        print("IPC FAILED")
        return
    ipc.send("set_property", "pause", False)
    time.sleep(0.5)
    print("time-pos:", ipc.get("time-pos"))
    print("width/height:", ipc.get("width"), ipc.get("height"))
    root.update()
    print("proc alive:", proc.poll() is None)

root.after(1500, after)
root.after(9000, lambda: (proc.terminate(), root.destroy()))
root.mainloop()
