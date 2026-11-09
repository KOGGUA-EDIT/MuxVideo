# MuxVideo

基于 mpv（libmpv 0.39 进程内嵌入）的视频对比与查看器，Rust + eframe/egui 0.33，
Material You（MD3）深/浅色主题，自绘 SVG 级矢量图标，无边框窗口。

![MuxVideo](docs/screenshot-pair.png)

## 功能

- **单画面查看**：A 面为主播放器，支持播放/暂停、±5s / ±10s 跳转、逐帧步进、倍速、音量、循环。
- **A/B 对比**：两路 libmpv 实例各占一块原生子窗口，A 为 master、B 每 250ms 纠偏（阈值 80ms），
  工具条实时显示 `同步 Δ±N ms`。
- **并排**：左右 / 上下各占一半，中间留 2px 缝；B 默认静音避免双声道回声。
- **划像**：两层全屏叠放，用 `SetWindowRgn` 把 B 裁到分界线右侧（或下方），分界位置可拖动。
- **差值**：单实例 `lavfi-complex=[vid1][vid2]blend=all_mode=difference[vo]`，
  两轨都加载完成后才设图（mpv 要求输入存在），失败会提示并保留原画面。
- **播放列表**：拖入多个视频自动填充，列表行内 A/B 按钮分配平面，双击载入 A，右键移出。
- **设置页**：深浅色、8 个 MD3 种子色 + 任意 `#RRGGBB`、hwdec 策略、默认音量、列表显隐、快捷键表。
- 打开方式：命令行参数、拖放到窗口、Win32 文件对话框；`muxvideo a.mp4 b.mp4` 直接进对比。

## 快捷键

| 键 | 动作 | 键 | 动作 |
| --- | --- | --- | --- |
| 空格 | 播放 / 暂停 | `←` `→` | ±5 秒 |
| `J` `L` | ±10 秒 | `,` `.` | 上一帧 / 下一帧 |
| `↑` `↓` | 音量 ±5 | `M` | 静音 |
| `1`–`4` | 单画面 / 并排 / 划像 / 差值 | `[` `]` | 分界 ±2% |
| `Tab` | 播放列表显隐 | `F` | 全屏 |
| `Home` | 回到开头 | `Esc` | 退出全屏 / 退出 |

## 运行

```
muxvideo.exe
muxvideo.exe "D:\clipA.mp4" "D:\clipB.mp4"
```

`libmpv-2.dll` 的查找顺序：环境变量 `MUXVIDEO_LIBMPV` → exe 同目录 → exe 同目录 `tools\mpv` →
项目目录 `tools\mpv` → `D:\ruanjian\mpv\mpv.net-DW v2.1.0`。

设置保存在 exe 同目录 `muxvideo.settings.json`（主题种子、模式、音量、最近列表等）。

## 构建

```powershell
$env:RUSTUP_HOME='D:\ruanjian\rust-env\rustup'
$env:CARGO_HOME='D:\app edit\Muxvideo\.cargo-home'   # 沙箱里 D:\ruanjian\rust-env\cargo 不可写
$env:PATH='D:\ruanjian\rust-env\cargo\bin;D:\ruanjian\rust-env\w64devkit\bin;'+$env:PATH
$env:CARGO_NET_OFFLINE='true'
cargo build --offline --release
```

产物：`target\release\muxvideo.exe`，同目录放一份 `libmpv-2.dll`（或设 `MUXVIDEO_LIBMPV`）。

## 发布目录

`dist\` 里是可运行的最小集合，拷走整个目录即可运行：

```
dist/muxvideo.exe       # release 构建，windows 子系统，无控制台窗口
dist/libmpv-2.dll       # libmpv 0.39（mpv.net-DW 发行版提供的 DLL）
dist/README.txt         # 运行说明
```

`dist` 缺 `libmpv-2.dll` 时按上面的查找顺序兜底找本机 mpv 安装。

## 结构

```
src/app/            外壳：布局、标题栏、导航栏、播放条、列表、设置、图标、窗口装饰
src/app/stage.rs    原生子窗口层：container + A/B/D 三个平面、布局计算、裁剪、同步
src/mpv/            libmpv 动态加载、MpvPlayer（事件线程 + 属性缓存）、事件类型
src/video/          Win32 子窗口封装（注册类、WndProc、SetWindowRgn 裁剪）
src/ui/             MD3 动态配色（HCT 近似）、调色板角色、egui 样式、控件库
tools/              探针与验证脚本（Python/Win32），不含运行时依赖
```

### 原生窗口层级

```
eframe toplevel (MuxVideo, winit/glow)
└── muxvideo-stage        WS_CLIPCHILDREN，正好覆盖 egui 的 stage 矩形
    ├── muxvideo-plane-a  ← libmpv 的 mpv 子窗口渲染到这里（wid）
    ├── muxvideo-plane-b
    └── muxvideo-plane-d  差值模式
```

egui 只画 chrome，视频区域由原生子窗口承托，避免每帧把视频纹理拷进 egui。

## 已知限制

- 官方 `mpv.exe` 的 `--wid` 在 0.42 构建上无法创建窗口（`[vo/gpu/win32] unable to create window!`），
  所以本项目只用 libmpv，不 shell out。
- 差值模式需要两段视频同分辨率；不同分辨率时由 blend 取较小者，画面会居中裁切。
- 播放列表不写回磁盘，仅本次会话有效（最近文件会持久化）。
- 鼠标点击落在视频画面上时事件被原生子窗口吃掉（egui 收不到），所以「点画面暂停」暂不支持，
  请用空格或底部播放按钮；键盘焦点不会被视频抢走。
- 浅色主题下视频区域的黑边是 mpv 自己的画面底色（播放器惯例），不跟随主题。
