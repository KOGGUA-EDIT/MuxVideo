# MuxVideo — 项目约定

基于 **mpv (in-process libmpv)** 的视频对比与查看器。Rust + eframe/egui 0.33，Material You (M3) 风格，性能优先。

## 构建环境（重要）

沙箱内 `D:\ruanjian\rust-env\cargo` 不可写（会报 `Error code 8: attempt to write a readonly database`），
所有 agent 一律使用工作区内的 cargo home：

```powershell
$env:RUSTUP_HOME='D:\ruanjian\rust-env\rustup'
$env:CARGO_HOME='D:\app edit\Muxvideo\.cargo-home'
$env:PATH='D:\ruanjian\rust-env\cargo\bin;D:\ruanjian\rust-env\w64devkit\bin;'+$env:PATH
$env:CARGO_NET_OFFLINE='true'
cd 'D:\app edit\Muxvideo'
cargo build --offline
```

## 运行 / 截图

- 沙箱里 `Start-Process` 起的 GUI 会在工具调用结束后被清理 → 必须用
  `mcp__wincu__windows_computer_use_launch_app` 启动 exe，再用
  `mcp__wincu__windows_computer_use_screenshot`（`scope=desktop` 更稳）。
- 沙箱进程无法访问命名管道（mpv 的 `--input-ipc-server` 路线因此放弃，改用进程内 libmpv API）。
- libmpv：`tools\mpv\libmpv-2.dll`（mpv 0.39），release 分发时与 exe 同目录。

## 关键设计

- 视频面 = Win32 子窗口：`root eframe hwnd → stage container(WS_CLIPCHILDREN) → plane A / B / D`，
  每个 plane 一个 libmpv 实例，用 `wid` 选项内嵌。
- 对比模式：Single / Pair（左右|上下）/ Wipe（B 面 SetWindowRgn 裁剪）/ Difference
  （单实例 `lavfi-complex [vid1][vid2]blend=all_mode=difference[vo]`）。
- A 面为同步 master，B 面漂移 > 80 ms 时 IPC-free 的 `seek_absolute` 纠偏（250 ms 节流）。
- 官方 `mpv.exe` 的 `--wid` 不可用（`[vo/gpu/win32] unable to create window!`），不要回退到 shell out。

## 目录

- `src/app/` —— App 外壳：`mod.rs`（状态与 update）、`bars.rs`（标题栏/rail/对比条/舞台/transport）、
  `panels.rs`（播放列表/设置）、`controls.rs`（M3 控件）、`icons.rs`（矢量图标）、`stage.rs`（原生窗口层）、
  `state.rs`（设置/模式/格式化）、`open_dialog.rs`（Win32 文件选择）。
- `src/mpv/` —— libmpv 加载器、播放器、事件队列。
- `src/video/` —— `ChildWindow` 原生 surface。
- `src/ui/` —— Material You 调色板、egui 样式安装、通用控件。
- `tools/` —— 探针脚本与第三方运行时（不随源码分发）。

## 约定

- 版本号在 `Cargo.toml`，改动可见时 +0.0.1。
- 公共签名（`Palette`、`MpvPlayer`、`ChildWindow` 等）改动需先同步其它模块负责人。
- 每次交付前 `cargo build --offline` 与 `cargo clippy --offline --all-targets -- -D warnings` 至少不新增警告。
