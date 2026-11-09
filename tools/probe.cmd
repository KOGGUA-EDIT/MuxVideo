@echo off
cd /d "D:\app edit\Muxvideo"
python "%~dp0probe_embed2.py" > "%~dp0probe_out.txt" 2>&1
