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

- 视频面 = Win32 子窗口：`root eframe hwnd → stage container(WS_CLIPCHILDREN) → plane A / B`，
  每个 plane 一个 libmpv 实例，用 `wid` 选项内嵌。
- 对比模式：Single / Pair（左右|上下）/ Wipe（B 面 SetWindowRgn 裁剪，缝里有一个原生 grip 窗口当把手）/ Blink（A/B 同位置
  每 0.45s 轮流显示；`sync_layout` 的第 6 参 `blink_b` 决定此刻显示哪一路，停帧也要 30ms 重绘）。
- **不要试图用 `lavfi-complex` 做两文件的逐像素差值**：`[vid1]/[vid2]` 是同一文件的第 1/2 条
  视频轨，传两个文件会报 `Pad vid2 is not connected to anything.`（已实测），单实例做不到。
- 鼠标事件靠 `src/app/input.rs` + `src/video/childwin.rs` 的子类化钩子（`take_input_events()`）捞回，
  必须按 `InputEvent.hwnd` 过滤掉 container / eframe 顶层的冒泡副本，否则音量会翻倍。
- A 面为同步 master，B 面漂移 > 80 ms 时 IPC-free 的 `seek_absolute` 纠偏（250 ms 节流）。
- 官方 `mpv.exe` 的 `--wid` 不可用（`[vo/gpu/win32] unable to create window!`），不要回退到 shell out。

## 目录

- `src/app/` —— App 外壳：`mod.rs`（状态与 update）、`bars.rs`（标题栏/rail/对比条/舞台/transport）、`input.rs`（鼠标手势层）、
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

## 交付

- `dist\` = 可运行最小集：`muxvideo.exe` + `libmpv-2.dll` + `README.txt`（不进 git，见 `.gitignore`）。
- 重新发布：`pwsh -NoProfile -ExecutionPolicy Bypass -File tools\deploy.ps1`
  （构建 → 签名 → 安装到 `D:\ruanjian\MuxVideo\` → 把完整性级别改回 Medium）。

### 工作区 Low 完整性坑（必读）

工作区目录带 `Mandatory Label\Low Mandatory Level:(OI)(CI)(NW)`，里面新建/构建出来的
exe 会继承 **Low 完整性**。Windows 把 Low 完整性的可执行文件当成“来自 Internet 的文件”，
双击会弹 `Windows 已保护你的电脑`（SmartScreen）或 `打开文件 - 安全警告`。

- 症状：拷到 `dist\` 的 exe 双击必弹窗，同样的 exe 拷到工作区外（如 `D:\ruanjian\MuxVideo\`）
  就直接启动。`icacls <file>` 最后一行的 `Mandatory Label` 可确认。
- 修复：`icacls <file> /setintegritylevel Medium`（要管理员，因为要 SeRelabelPrivilege），
  或者干脆拷出工作区。
- **不要**把工作区目录本身改成 Medium：DSH 沙箱靠 Low 完整性做 no-write-up 限制，改高会让
  沙箱写工作区失败。
- 本地签名证书（`CN=MuxVideo Local Dev`）只影响对话框里显示的发布者名，不能消除弹窗；
  排查时不要被签名状态误导。
- 相关脚本：`tools\set_integrity_medium.ps1`（单次改级别）、`tools\deploy.ps1`（完整发布）。
  历史上为排查改过的 SmartScreen 注册表已全部还原（AppHost 三处 1/1/0，无 Policy 覆盖），
  还原脚本 `tools\restore_smartscreen_all.ps1`。
