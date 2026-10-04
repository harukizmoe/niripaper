# niripaper — 开发文档

面向读者：只读本项目的开发者。本文档自带完整上下文——项目是什么、已定决策、要实现的确切规格、本机可依赖的事实、验证方法与已知风险。
不含任何外部仓库或历史叙述：读完即可开工。

---

## 快速开始

```
位置：~/workspace/rust/niripaper      # 已 git init（main）；尚无提交、无 remote
进度：M0a ✔、M0b ✔、**M1 ✔**；**M2 进行中**——静态图已可用（`render/image.rs`）。
      还剩 M2 的：换图过渡、libmpv 视频纹理
```

```sh
cd ~/workspace/rust/niripaper
cargo build

# 0) 真画壁纸层（跟随 niri 布局；静置时阻塞在 poll()，零开销）
./target/debug/niripaper daemon --output DP-1 --wallpaper ~/Pictures/Wallpapers/anime_car_sunset.webp
#    看效果最省事的方式：切到一个空工作区（没有窗口遮挡），图层就是可见背景。
#    namespace 默认 `niripaper`：用户没加 layer-rule 时它属于"工作区背景"，
#    有窗口的工作区只会露出 10 px 边距；加 --namespace mpvpaper 会命中本机既有
#    规则、进 backdrop，于是连总览背景都是壁纸（会盖住 Noctalia 的壁纸，退出即恢复）。
#    不加 --wallpaper 时画的是程序化测试图案（--pattern blocks|bands）。
#    配置：~/.config/niripaper/config.toml（可缺省）；优先级 CLI > 配置文件 > §2 默认值
#    daemon --config <路径> 可指定文件；启动日志会打印"生效值"与配置文件来源
#    注意：namespace 默认 `niripaper`；**不加规则**时它是工作区背景（推荐默认，总览背景干净、
#    窗口照样有壁纸和模糊）；想让它进 backdrop 才需要用户加 place-within-backdrop；
#    要零配置验证，用 --namespace mpvpaper 命中本机既有规则

# 0b) 只看视差目标（不需要 GPU，改列/切工作区时实时打印）
./target/debug/niripaper watch --output DP-1 --screen 2560x1440

# 1) 诊断：每个 DRM 节点能不能建 EGL 上下文、能不能产出可渲染的 dmabuf
./target/debug/eglpin

# 2) M0b 探针：真在 DP-1 上映射一层，画一次，然后静置
./target/debug/m0b --output DP-1 --namespace niripaper --frames 3 --idle-seconds 5
```

**开发约束（本机）**

- 不要改动本机的 Niri / Noctalia 配置（`~/.config/niri/**`、`~/.config/noctalia/**`）。需要 layer-rule 时：让维护者自己加，或**临时用 `mpvpaper` 作为命名空间**去命中已存在的规则（M0b 用此法，零配置改动）。
- 这台桌面**正在使用中**：探针、播放器、临时文件都必须自己收尾（只杀自己起的进程、删自己建的 socket 与临时文件），不留后台进程。M0b 验证时曾临时切到空工作区取截图，必须切回原工作区。
- **`$NIRI_SOCKET` 在 agent shell 里是过期的**：真实 socket 每次登录都换名字。用
  `NIRI_SOCKET=$(ls -t /run/user/1000/niri.*.sock | head -1)` 取当前的，否则 `niri msg` 报 "No such file or directory"。
- `socat` 未安装（用 `nc`，OpenBSD 版）。
- **下结论前确认指标可信**：`--dump-stats` 是抽样输出、`nvidia-smi` 在桌面日常负载下噪声极大（基线 26–65%）。不要用它们宣称"更省"或"更流畅"（§8 有可用方法）。

---

## 1. 项目定义

**niripaper = Niri 的壁纸引擎**：独立守护进程，自己拥有壁纸图层与渲染循环，支持静态图 / 视频 / 动画格式，**布局反应式视差**（随焦点列与活动工作区移动），换壁纸有过渡，并且**窗口模糊采样到的画面与可见壁纸始终一致**。

**不做（v1）**：shader / 粒子 / 交互式场景壁纸、音频或鼠标反应特效、壁纸资源商城、修改合成器本身、**选 GPU / 切 MUX / 覆盖合成器的渲染设备**（选哪块卡是合成器的决定，niripaper 不提供任何选卡开关，也不读不写 BIOS/PRIME 相关配置）。

---

## 2. 已定决策（无需再议）

| 项 | 决定 |
| --- | --- |
| 名字 | `niripaper`（仓库 / crate / 二进制 / layer-shell 命名空间 / 配置目录同名） |
| 形态 | 独立守护进程 + **可选** Noctalia 集成插件；不依赖 Noctalia 也能用 |
| 许可证 | **GPL-3.0-or-later**（V2 链接 libmpv，而 libmpv 在本机是 GPL 构建） |
| 视频方案 | **V2：守护进程内嵌 libmpv**（`mpv_render_context` + OpenGL），不用外部 mpvpaper 进程 |
| 渲染栈 | **OpenGL / GLES + EGL**（不是 Vulkan/wgpu）：libmpv 的 render API 只有 `OPENGL` 与 `SW` |
| **选 GPU 的规则** | **跟随合成器的 dmabuf feedback `main_device`**，不是"输出所在的 GPU"。合成器在哪个 GPU 上合成，就在哪个 GPU 上渲染（M0b 实测：本机 DP-1 由 NVIDIA 输出，但 niri 在 AMD 上合成）。**"选哪块卡"完全不属于本项目**：不提供选卡开关，不写 niri 配置，不碰 MUX/BIOS。我们只负责"按它说的那张卡把缓冲做对"（§7 风险 1） |
| EGL vendor | 由目标 GPU 的 PCI vendor id 决定（`0x10de` → NVIDIA ICD，其余 → Mesa），在**本进程第一次 EGL 调用之前**用 `__EGL_VENDOR_LIBRARY_FILENAMES` 钉住（§5）。**一个进程只能钉一个 vendor** |
| 进度来源 | 守护进程**自己**连 `$NIRI_SOCKET` 读 event stream，不依赖外部推送 |
| 命名空间 / layer rule | 守护进程用 `niripaper`。**规则是可选的**（见下）：默认（不加规则）= 壁纸是**工作区背景**；加 `match namespace="niripaper"` + `place-within-backdrop true` = 壁纸进 **backdrop**。两者**模糊效果相同**（实测），区别只有两处：① backdrop 时总览背景整屏都是壁纸、且切工作区时壁纸不跟着滑；② 工作区背景时总览背景干净、壁纸出现在各工作区缩略图里（所有缩略图共用一张、同一偏移——layer 是 per-output，做不到按工作区独立视差） |
| Noctalia 交接 | 插件在某输出启用守护进程时调用 `setWallpaperEnabled(connector, false)` 撤掉原生壁纸层（保证 backdrop 一致）；插件启动时自愈式恢复 `true` |
| Noctalia UI | 不自己造选择器：镜像官方壁纸选择（Material You 取色继续生效）+ 状态栏开关 + 插件设置项 |
| 分发 | 源码 `cargo install` + AUR + GitHub Releases 预编译；Noctalia 插件为可选集成 |
| 默认参数 | `scale = 1.1`、`column_span = 6`、`workspace_span = 6`、视差 `ease-out-cubic 600 ms`、总览过渡 `spring 1.0/800/0.0001` + `zoom 0.96`、换图过渡 `fade 250 ms`、视频静音、`hwdec=auto-safe` |
| 动画配置 | **语义与写法对齐 niri 的 `animations { }`**：每个动画三选一——`off`、缓动（`duration_ms` + `curve`）、或 `spring`（`damping_ratio` / `stiffness` / `epsilon`，质量固定 1，与 niri 同）。`curve` 取 niri 的五个：`linear` / `ease-out-quad` / `ease-out-cubic` / `ease-out-expo` / `cubic-bezier`（后者配 `cubic_bezier = [x1,y1,x2,y2]`）。另有顶层 `off` 与 `slowdown`。当前两项：`animations.parallax`（§4.2.6：ease-out-cubic 600 ms）、`animations.overview-open-close`（`zoom` + 动画，默认用 **niri 自己的默认弹簧** 1.0/800/0.0001）。两种动画类型都能驱动任何被动画量。
| **跟随 niri 配置** | `animations.follow_niri`（默认 **true**）：读 niri 自己的 `config.kdl`（含 `include`），取用两者共享的 `overview-open-close` 与 `slowdown`；niri 的全局 `off` 只关掉总览过渡，**不动视差**（视差不是 niri 的动画）。优先级：**本文件显式值 > niri 配置 > 我们的默认值**，来源会写进启动日志（`from niri's config: …`），避免静默偏离。`follow_niri = false` 则完全不读 |
| 总览过渡 | 总览打开/关闭时画布做一次轻微后撤，由 `OverviewOpenedOrClosed` 驱动。zoom 实现为 **`scale` 的动画化乘数**，所以画布/overflow/视差行程/纹理采样全部自动跟随，shader 不用改、纹理不用重传（只采样子区域）。niri 不报告它自己的过渡进度，所以只能近似同步——把参数设成与 niri 配置里 `overview-open-close` 相同的弹簧，手感就基本一致 |

---

## 3. 架构

```
niripaper（Rust 守护进程）
├── gpu.rs      sysfs：connector → card → render node / vendor id；dev_t → /dev/dri 节点
├── niri.rs     已实现：连 $NIRI_SOCKET，写 "EventStream"，读一行一事件；解析 niri-ipc 的形状
│               （只认 §4.2.7 的 8 个事件，其余含多键信封一律丢弃）→ 喂给 motion.rs
├── motion.rs   已实现：§4 的运动规格（纯逻辑、无 IO，§4.3 全部用例 + §4.4 的实现注记）
├── ipc.rs      控制 socket：$XDG_RUNTIME_DIR/niripaper.sock（行协议：set/video/query/kill）
├── config.rs   已实现：~/.config/niripaper/config.toml（scale / column_span / workspace_span /
│               namespace + [outputs.<名>] 覆盖）；未知键与越界值都报错并指出键名
├── render/
│   ├── layer.rs   Wayland 客户端：registry / wl_output / wlr-layer-shell 表面（namespace = "niripaper"）
│   │              + dmabuf feedback（default 与 per-surface）+ wl_buffer 提交与 release 回收
│   ├── egl.rs     EGL 绑定到指定 GPU（GBM platform + GLVND vendor 钉住）+ dmabuf 导入
│   ├── gbm.rs     GBM 设备与 buffer（O_RDWR 打开 render node；plane fd/offset/stride/modifier）
│   ├── dmabuf.rs  Frame：GBM BO → EGLImage → GL 纹理 → FBO，可直接作为 wl_buffer 提交
│   ├── gl.rs      GL 子集（链 libGL，GLVND 按当前 EGL 上下文分发）+ 测试图案
│   ├── image.rs   已实现：解码 → cover 裁剪 → 缩放到画布（output × scale）→ 纹理
│   ├── video.rs   libmpv render API → 纹理
│   └── anim.rs    frame callback 驱动的动画：视差平移 + 总览 zoom（+ 未来的换图过渡）
└── cli.rs      niripaper {daemon|set|video|query|reload|kill}；另有 `watch`（打印视差目标，纯观测、不用 GPU）
```

**为什么自己连 Niri**：Niri IPC 很简单（连 socket → 写一行 JSON 请求 → 读一行 JSON；`"EventStream"` 先给全量状态再持续推增量）。自己连才能独立于任何桌面壳，并且省掉"Niri 事件 → 外部进程 → 再转发给渲染器"这条链的延迟（§4 的动画必须与窗口动画同时起步）。

**双后端、单循环**：静态图用自有纹理（便宜）；视频/动画格式用 libmpv 纹理；两者都由同一个 frame-callback 循环带着偏移绘制。换图过渡统一用"把上一帧内容快照成纹理再 crossfade"——任意组合（图↔视频↔视频）都能过渡，且任何时刻只需一个解码器。

**提交路径（M0b 已跑通）**：不用 `wl_egl_window`、不用 `eglSwapBuffers`。每帧：`gbm_bo_create_with_modifiers`（用合成器 feedback 里给出的 modifier）→ `eglCreateImageKHR(EGL_LINUX_DMA_BUF_EXT)` → `glEGLImageTargetTexture2DOES` → 挂到 FBO 上画 → `glFinish` → `zwp_linux_dmabuf_v1` 提交 → 等 `wl_buffer::release` 再复用。**只有这条路能保证渲染发生在指定 GPU 上。**

**技术选型（已锁定）**

| 用途 | 选择 | 备注 |
| --- | --- | --- |
| Wayland | `wayland-client` + `wayland-protocols`(unstable) + `wayland-protocols-wlr` | 已用；layer-shell v5 / linux-dmabuf v4 |
| EGL / GBM / GL | **裸 FFI**：`libEGL.so`（GLVND 加载器）、`libgbm.so`、`libGL.so`（GLVND 分发） | `src/render/{egl,gbm,gl}.rs`；无 EGL/GL crate |
| dmabuf 提交 | `zwp_linux_dmabuf_v1` + 自管 buffer | §7 风险 1 的唯一正路 |
| 视频 / 动画 | `libmpv2` / `libmpv-sys`（`MPV_RENDER_API_TYPE_OPENGL`） | 务必调 `mpv_render_context_report_swap()` |
| 图片解码 | `image` | 解码后立刻缩放到 `ceil(output × scale)` |
| 配置 / IPC | `serde` + `toml`；控制 socket 用行协议 | 行协议不需要框架 |

---

## 4. 运动规格（必须实现的语义）

参数与公式是对 Clavis（`StatIndet/quickshell`）的行为复刻（未复制其源码）。下表是规格来源，数值可直接使用：

| Clavis 位置 | 值 |
| --- | --- |
| `Common/Animations.qml:138` `wallpaperParallax` | `duration = durations.large`、`type = Easing.OutCubic` |
| `Common/Animations.qml:38` | `large = 600`（ms） |
| `Services/WallpaperSceneService.qml:237-248` | 动画只有 `animatedOffsetX/Y` 两个 QML `Behavior`（`NumberAnimation`） |
| `PersonalizationConfig` | `parallaxPreferredScale = 1.1`、`parallaxTiledColumnSpan = 6` |
| `Common/functions/WallpaperMath.js` | `focusedColumnProgress = rankIndex/(span-1)`、`workspaceProgress = activeIndex/(count-1)`、`wallpaperPosition = -overflow*progress`、`overflow = screen*(scale-1)` |

### 4.1 公式

```
horizontal = clamp01( rankOf(nearestColumn(columns, rememberedColumn[workspace])) / (span - 1) )
vertical   = activeIndex / (workspacesOnOutput - 1)
offset_px  = axisScreenSize × (scale - 1) × (progress - 0.5)     // 相对"居中"位置
canvas     = 画布按 scale 放大后裁剪铺满输出（若用 mpv 作参照，等价于 video-zoom = log2(scale)）
```

`column_span`（每工作区的列跨度）与 `workspace_span`（每输出的工作区跨度）各默认 6，`scale` 默认 1.1。两者**分开设**：它们数的是不同的东西，工作区多的人不该为此缩小列步长。参考换算（若与 mpv 单位对照：pan 单位 = 已缩放画布宽度）：
`pan = (scale-1)/scale × (0.5 - progress)`，`zoom = log2(scale)`——**新渲染器不要照搬这一行**，直接用上面的像素偏移。

数值例子（2560×1440、scale 1.1、两个 span 都取 6）：`overflow = 256 px` 横向、`144 px` 纵向；3 列时 rank 0/1/2 → progress 0 / 0.2 / 0.4 → **首列到末列共移动 102.4 px**（每列 51.2 px）；工作区 idx 1…4 → progress 0 / 0.2 / 0.4 / 0.6（每级 28.8 px）。两者都用固定跨度，见 §4.4 第 10 条。

### 4.2 事件与记忆规则

1. `column_span` 是**固定值**，不是"该工作区实际列数"（理由见 §7 风险 2）。**纵向同理**：`workspace_span` 是固定值，不是活动工作区数（见 §4.4 第 10 条）。两者独立配置。
2. 每个工作区**各自记忆焦点列**（存列号）；列消失时解析到**最近的现存列**（nearest，不是 lower-bound）。
3. 记忆只在**活动工作区**上更新；活动窗口是**浮动窗口**时保留上一个平铺列（不重置）。
4. 该工作区没有任何列 → `horizontal = 0.5`；**只有一列 → 0**（贴左边，属规格，不要"修正"为 0.5）。
5. 该输出上没有活动工作区 → `(0.5, 0.5)`。
6. 动画：`OutCubic`、600 ms；**收到与当前目标相同的目标必须忽略**（QML `Behavior` 只在源值变化时才动）；中途改目标时从"当前已到达的位置"重新缓出。
7. 只处理这些事件：`WorkspacesChanged` / `WorkspaceActivated` / `WorkspaceActiveWindowChanged` / `WindowsChanged` / `WindowOpenedOrChanged` / `WindowClosed` / `WindowFocusChanged` / `WindowLayoutsChanged`；其余事件（含多键信封）一律忽略，`apply()` 返回 false；上述事件 `apply()` 返回 true。

### 4.3 回归用例表（写 Rust 单测直接照抄）

辅助构造默认值：`workspace{idx=1, output="DP-1", is_active=true, is_focused=false, active_window_id=nil}`、`window{workspace_id=1, column=1, tile=1, is_focused=false, is_floating=false}`。

| 场景 | 期望 `(horizontal, vertical)` |
| --- | --- |
| ws1(active_window=1)、窗口列 {70,10,30,30(tile2)} | (0.4, 0.5) |
| ↳ `WindowFocusChanged(4)`（列 30） | (0.2, 0.5) |
| 单列（两窗口同列 8） | (0.0, 0.5) |
| `WindowsChanged` 为空 | (0.5, 0.5) |
| 双输出：DP-1 活动工作区 idx1；HDMI-A-1 idx2 活动、列为 {1,2} | DP-1 (0.0, 0.0)；HDMI-A-1 (0.2, 0.5) |
| 查询未知输出 | (0.5, 0.5) |
| ↳ `WindowFocusChanged(2)`（DP-1 列 2） | DP-1 (0.2, 0.0)；HDMI-A-1 仍为 (0.2, 0.5) |
| `WorkspaceActivated(2, focused=false)`（同输出另一工作区） | (0.0, 0.2) |
| ↳ 断言 | ws1.is_focused=true、ws2.is_focused=false、另一输出 ws3.is_active=true |
| ↳ `WorkspaceActivated(3, focused=true)` | ws1.is_focused=false、ws3.is_focused=true |
| 活动窗口记忆列（active_window=2、列 {1,2}） | (0.2, 0.0) |
| `WorkspacesChanged` 只剩别输出工作区 | 该输出 (0.5, 0.5)，旧工作区状态被丢弃 |
| 活动窗口是浮动窗口 | (0.0, 0.5) |
| 焦点列 3（列 {1,2,3}） | (0.4, 0.5) |
| ↳ `WindowClosed(1)`（列剩 {2,3}） | (0.2, 0.5) |
| ↳ `WindowClosed(3)`（焦点被关，记忆保留） | (0.0, 0.5)，且 `focused_id == nil` |
| ↳ `WorkspaceActiveWindowChanged(2)` + `WindowOpenedOrChanged(列 4)` | (0.0, 0.5) |
| `WindowLayoutsChanged` 批 {1→列3, 3→列1, 99→未知} | (0.4, 0.5)，未知窗口被忽略 |
| 焦点列 2（列 {1,2,3}） | (0.2, 0.5) |
| ↳ 焦点移到浮动窗口 | (0.2, 0.5) |
| ↳ 焦点回列 3 | (0.4, 0.5) |
| 工作区 idx {90,3,40}、活动=idx90、10 列 {10..100}、focus 列 100 | (1.0, 0.4) |
| ↳ focus 列 10 | (0.0, 0.4) |
| `WindowUrgencyChanged` | `apply()` 返回 false |
| 双键信封 | `apply()` 返回 false |
| `reset()` 之后 | (0.5, 0.5)，且 `focused_id == nil` |
| 聚焦第 2 列，`column_span` = 6 / 3 / 11 | (0.2, 0.0) / (0.5, 0.0) / (0.1, 0.0) |
| `panFor((0.5,0.5), 1.1)` | pan = (0, 0) |
| `panFor(_, 1.1)` 的 zoom | `log2(1.1)` |
| progress (0,0) → 像素（2560×1440、scale 1.1） | `x = +128`（首列位于中心右侧半个 overflow） |
| progress (1,1) → 像素 | `x = −128`、`y = −72` |
| progress (0.4,1) → 像素 | `x = 128 − 0.4×256`、`y = −72` |
| `panFor(_, 9)` | zoom 夹到 `log2(1.35)` |

---

### 4.4 实现注记（M1 期间确定，`src/motion.rs`）

§4.3 的表在几处**没有把前提写全**。以下是实现时必须定死、并且已按最一致的读法落地的细节（`src/motion.rs` 的 20 个单测全部覆盖 §4.3；如有异议，改这里再改代码）：

1. **纵向的退化情形**：`activeIndex / (count-1)` 在该输出只有 0 或 1 个工作区时是 0/0。定为 **`count <= 1` → 0.5**。表里期望 `0.5` 的行（单列、焦点列 3、活动窗口是浮动、双输出的 HDMI-A-1）都只有一个工作区；期望 `0.0` 的行（活动窗口记忆列、span 行、双输出的 DP-1）需要该输出有**两个**工作区（活动的是第一个）。`activeIndex` 按 **idx 排序**后的下标，不是 id。
2. **"记忆列"的更新时机**：每接受一个事件后重算一次，顺序是
   ① 每个**活动**工作区按其 `active_window_id` 指向的平铺窗口的列；
   ② 再用**焦点窗口**覆盖（仅当它是平铺窗口且所在工作区处于活动状态）。
   浮动窗口永不更新记忆（§4.2.3），所以"焦点移到浮动窗口"会保留上一个平铺列。
3. **没有任何记忆、也没有活动/焦点平铺窗口时**：取**最左列**（rank 0 → 0.0）。表里"活动窗口是浮动窗口 → (0.0, 0.5)"与双输出的 DP-1 行都与此一致。
4. **`offset_px` 的符号**：§4.1 的公式写作 `screen × (scale-1) × (progress - 0.5)`，但 §4.3 的像素行要求**相反**的符号（`progress 0 → x = +128`）。以**像素行为准**：`x = overflow × (0.5 - progress)`。
5. **scale 上限**：§4.3 的 `panFor(_, 9)` 行要求 zoom 夹到 `log2(1.35)`，即 scale ≤ **1.35**；`offset_px` 用同一上限夹紧。
6. **§4.3 里 `panFor(...)` 的三行不做**：它是 mpv 时代的表述（`video-pan-x`/`video-zoom`），§4.1 明确说新渲染器不要照搬。实现它会是死代码。
7. **"双键信封"那一行不属于 `motion.rs`**：那是 IPC 解析层的事——多键对象根本不会变成 `Event`，所以由 `niri.rs` 的解析器返回 `None` 来保证（`apply()` 只会收到 §4.2.7 的 8 个变体）。
8. **横向不随工作区切换平移**（对 §4.2.2 的修正）。原先"每个工作区各自记忆焦点列"意味着切工作区时 `horizontal` 也会跳到新工作区的列 ✗：竖直切换变成了**斜向** ✗。实测（修前）：`target h=0.500 v=1.000`，逐帧 `h` 与 `v` 的增量**完全相同**（0.1301/0.6301、0.2215/0.7215 …）——两个轴锁在一起走。修后：`target h=0.000 v=0.500`，`h` 一动不动。
   * 为什么不能只看事件类型：niri 切工作区发的是 `WorkspaceActivated` **紧跟一个** `WindowFocusChanged`（指向新工作区的窗口），而横向移动焦点**也**只发 `WindowFocusChanged` ✗。
   * 判据：**"焦点此前就在该输出的活动工作区上"** → 说明正在发生的是横向滚动，不是工作区带在滑动。其余情况（`WorkspaceActivated`、以及它后面那个 `WindowFocusChanged`）只改竖直。
   * 首次建立时机：状态有意义（已知焦点窗口）时建立一次。否则"切换早于首次横向移动"会回落到按新工作区重算 ✗，又变回斜向。
   * 实测（修后，八个动作段全部轴向纯净）：ws-down/up → Δh **+0.0 px**、Δv ±61…68 px；col-right/left → Δh ±46…47 px、Δv **+0.0 px**。
10. **纵向也用固定跨度**（对 §4.2/§4.3 的修正），即 `workspace_span`。原式 `activeIndex / (count - 1)` 用的是**活动工作区数** ✗，于是同一个活动工作区的位置随工作区总数变化 ✗：实测（修前）idx 2 在 3/4/5 个工作区时分别是 **72 / 48 / 36 px** —— 新建或关闭一个工作区，壁纸就跳一下 ✗，视差比例也跟着变 ✗。这与 §4.2.1 给列定"固定值"的理由**完全相同**，只是当初漏了这一轴 ✗。改为 `clamp01(activeIndex / (span - 1))`，保留"该输出只有 ≤1 个工作区时取 0.5"（没有可平移的对象 → 居中，§4.3 的单工作区行都依赖它）。
    实测（修后，用户 4 个工作区）：纵向目标 0.2 / 0.4 / 0.6（每级 28.8 px），八个动作段仍全部轴向纯净。
    两轴**各自独立**（`column_span` / `workspace_span`）：它们数的是不同的东西，工作区多的人不该为此缩小列步长。想让纵向像旧的"走满 overflow"那样大，把 `workspace_span` 设成工作区数即可，横向不受影响。
    同一次改动还修掉一个潜伏 bug：`Niri::connect()` 里写死 `Motion::new(DEFAULT_SPAN)`，**配置里的 span 从未接上线** ✗（默认值与当时配置里的值都是 6，所以看不出来 ✗）。现在由 daemon 把生效的跨度传进去；实测 `--config` 写 `column_span = 11` / `workspace_span = 3` → 启动日志 `spans 11/3`，每列 25.6 px、每级 72 px。
9. **过期的 `is_focused` 标志会把焦点抢回去**（修 8 时一并发现）。`WindowOpenedOrChanged` 对**标题/app_id 变化**也会触发，其载荷窗口并非焦点窗口 ✗；而 `adopt_reported_focus` 会在窗口列表里扫描 `is_focused` ✗ —— 我们自己从不清除旧标志 ✗，于是这类事件会把焦点（以及横向视差）拽回上一个焦点窗口 ✗。修正：只有**载荷窗口自称被聚焦**时才采纳（`WindowsChanged` 是完整列表，仍可整体扫描），采纳时把其余窗口的标志清掉（niri 的语义：一个窗口报告被聚焦即意味着其他都不是）。

---

## 5. 本机环境事实（可直接依赖）

| 事实 | 依据 |
| --- | --- |
| niri `26.04`、mpv `0.41.0`、libmpv `2.5.0`（`/usr/lib/libmpv.so` 存在，`pkg-config mpv` 可用） | `niri --version`、`mpv --version`、`ls /usr/lib/libmpv*` |
| 输出：**DP-1 = card0 = NVIDIA**（2560×1440 @ 180 Hz，logical scale 1.0）、**eDP-1 = card1 = AMD**（`status` 为 connected 但未启用） | `/sys/class/drm/card*-*/status`、`niri msg --json outputs` |
| 输出 → GPU 判定：`/sys/class/drm/cardN-<connector>/status == "connected"` → 取 `cardN` → 读 `cardN/device/vendor`（`0x10de` = NVIDIA、`0x1002` = AMD） | 同上 |
| **niri 在主 GPU 上合成一切，再把结果拷到输出所在 GPU**：每帧调 `gpu_manager.renderer(&self.primary_render_node, &device.render_node.unwrap_or(primary), fmt)`（`niri v26.04 src/backend/tty.rs:1872`），而 smithay 0.7 的签名是 `renderer(render_device, target_device, copy_format)`——文档原文 *"render_device: the gpu node rendering operations will take place upon"*、*"target_device: the gpu node the composited buffer will end up upon"*。本机 `primary_render_node = renderD128`（AMD），DP-1 的 `device.render_node = renderD129`（NVIDIA）→ **在 AMD 上画，拷到 NVIDIA 去扫描输出**。`MultiRenderer` 的职责是 *"transparently copies rendering results to another gpu, as well as transparently importing client buffers residing on different gpus"*（smithay `renderer/multigpu/mod.rs`） | niri 源码 + smithay 源码（rev `ff5fa7d`，v0.7.0） |
| 上述结论的运行期佐证：niri 启动日志 `using as the render node: "/dev/dri/renderD128"` + `initializing the primary renderer`；`/proc/<niri>/fd` 有 `card1`(AMD) ×4、`card0` ×1、`renderD129` ×2 与大量 `/dev/nvidia*`；`/proc/<niri>/maps` 同时有可执行的 Mesa `libgallium` 与 `libnvidia-eglcore`，且 nvidia `GLCache` 文件被打开 → **两个 GPU 上各有一个活着的 GL 上下文** | journalctl、`/proc/<niri>/{fd,maps}` |
| **dmabuf feedback 的 `main_device` 永远是主渲染节点**：default 与 per-surface feedback 都报 `dev 57984` = 226:128 = `renderD128`（AMD），XR24 可用 modifier 为 LINEAR + 10 个 AMD tiled | 探针输出；`niri/src/backend/tty.rs` `surface_dmabuf_feedback()`：`DmabufFeedbackBuilder::new(primary_render_node.dev_id(), primary_formats)` |
| niri 对"渲染节点 ≠ 主节点"的输出把 scanout tranche 限制为 **Linear**，注释原文："we're rendering on a different device" | 同上（`if surface_render_node != Some(primary_render_node)` 分支） |
| **按 GPU 建 EGL 上下文可行**：目标 GPU 由 PCI vendor 决定 GLVND vendor JSON（`0x10de` → `10_nvidia.json`，其余 → `50_mesa.json`），在第一次 EGL 调用前设 `__EGL_VENDOR_LIBRARY_FILENAMES` 钉住，再用 `EGL_PLATFORM_GBM_KHR` + `gbm_create_device(fd)` | `eglpin`：AMD 侧 `EGL 1.5 [Mesa Project]` / `GL 4.6 … radeonsi, renoir`；NVIDIA 侧 `EGL 1.5 [NVIDIA]` / `GL 3.3.0 NVIDIA` |
| **GLVND 按 platform 而不是按设备选 vendor**：不钉 vendor 时，`EGL_PLATFORM_GBM_KHR` 会被 NVIDIA ICD 接走并给 AMD 的 GBM 设备返回一个"能初始化但 0 config"的 display。`EGL_PLATFORM_DEVICE_EXT` 传裸 fd 得到 `EGL_NO_DISPLAY`；本机 GLVND 的 `libEGL.so.1` **不导出** `eglQueryDevicesEXT`（在 `libgallium` 里），所以拿不到 `EGLDeviceEXT` 去做设备路由 | 探针逐 platform 的失败记录 + `nm -D /usr/lib/libEGL.so.1` |
| **Mesa 的 GBM platform 只暴露 window（gbm_surface）config，不暴露 pbuffer config**：`EGL_SURFACE_TYPE=EGL_PBUFFER_BIT` → 0 configs，`EGL_WINDOW_BIT` 或无该属性 → 有 config。我们只画 FBO，surface type 无关紧要 | `eglpin` / `choose_config` 的尝试记录 |
| **NVIDIA 不能把 LINEAR dmabuf 当渲染目标**：`gbm_bo_create_with_modifiers([LINEAR])` 成功、`eglCreateImageKHR` 成功，但挂 FBO 得 `GL_FRAMEBUFFER_INCOMPLETE_ATTACHMENT`；NVIDIA EGL 自报 XR24 有 13 个 modifier、12 个 renderable，**没有 LINEAR**。NVIDIA 的 GBM 后端还要求显式 modifier（`gbm_bo_create` 不带 modifier 直接失败） | 探针输出（`eglQueryDmaBufModifiersEXT` + FBO 检查） |
| **M0a 的 "AMD 侧 EGL 失败" 是探针自己的 bug**：当时用 `File::open`（`O_RDONLY`）打开 render node，导致 `amdgpu_bo_cpu_map failed (-13)`；改成 `O_RDWR` 后 AMD 侧完全正常（§7 风险 8 已消除） | 重写后的 `eglpin`（`gbm::Device::open` 用 `OpenOptions::read(true).write(true)`） |
| `place-within-backdrop` 语义（Niri，`Since: 25.05`）：把 background 层放进 backdrop，在 Overview 与工作区之间可见；backdrop 内的层忽略输入。配合 `layout { background-color "transparent" }` 得到"壁纸不随工作区滑动" | Niri wiki：`Configuration:-Layer-Rules#place-within-backdrop`、`Overview#backdrop-customization` |
| 窗口模糊由 `background-effect { blur true }` 提供；`xray`（blur 打开时 niri 默认启用）让窗口背景"看穿到壁纸"。**壁纸放在哪它采哪**——放进 backdrop 或作为工作区背景都行，实测两者对窗口内部的影响只差 0.6/255 | 实测（纯红标记图，见 §8）|
| **总览里视差照样工作**：总览打开期间 niri 仍发 `WorkspaceActivated`，守护进程照常重画。实测切 workspace 1→3 时总览的整屏壁纸竖直位移 **72 px = 144 × Δv(0.5)**，且逐像素完美（mse 0.0） | 实测 |
| `place-within-backdrop` 换到的其实是"切工作区时壁纸不跟着滑"（壁纸属于工作区时会随工作区平移），以及"总览背景整屏是壁纸"；**不是**模糊的前提 | Niri wiki + 实测 |
| 本机 Niri 规则现状：`^noctalia-wallpaper*` 与 `^mpvpaper$` 两条均为 `place-within-backdrop true`；`layout { background-color "transparent" }`；另有若干应用 `background-effect { blur true }` | `~/.config/niri/rules.kdl`、`layout.kdl` |
| 静态壁纸由 Noctalia 自己渲染（其 `noctalia-wallpaper` 层，跑在 `renderD128`）；mpvpaper 只服务视频壁纸 | 官方 `noctalia/mpvpaper` 插件 README 与源码、`/proc/<noctalia>/fd` |
| 官方视频插件在分配视频后调 `setWallpaperEnabled(name, false)` 撤掉原生壁纸层（`mpvpaper_service.luau:451`），清除视频时恢复 `true` | 读源码 |
| Noctalia `v5.2.1`：插件 API 支持范围 **3–32**（越界拒绝加载）；`setWallpaperEnabled(connector, bool)` 是**运行时**开关，`onExit` 中恢复**不可靠** | `strings /usr/bin/noctalia` + 实测 |
| `niri msg --json outputs` 返回**以输出名为键的字典**：`modes[i] = {width,height,refresh_rate}`（**mHz**）、`current_mode` 是 `modes` 的**下标**、`logical = {width,height,scale}` | 实测 |
| `niri msg --json layers` 返回数组，每项含 `{namespace, layer, output, keyboard_interactivity}`；**不含 backdrop 归属** | 实测 |
| `niri msg --json windows`：`layout.tile_size` / `layout.window_size` 给几何；本机所有窗口都被 `default-column-width` 撑到接近满屏 | 实测 |
| **`wl_buffer::release` 只在缓冲被替换后到来**：只有一个缓冲常驻屏幕上时，合成器不会释放它。管线深度 = 已提交数 − 1 | M0b 实测（先按"每次 commit 后等 release"实现，超时；改为等 `N-1` 后 5 帧收到 4 次 release） |
| **mpvpaper 命名空间技巧有效**：`--namespace mpvpaper` 命中已存在的 `^mpvpaper$` layer-rule，零配置改动即可验证 backdrop 行为 | M0b 实测 |
| **跨设备缓冲只认 LINEAR，非 LINEAR 会"被接受但不出画面"**：把缓冲分配在 NVIDIA（`nv:0x0300000000606010`）提交给 DP-1 表面时，niri 不报错、`wl_buffer::release` 照常回来，但屏幕上**看不到内容**（模糊区仍是 Noctalia 壁纸）。对照：同一个缓冲放在 AMD 上用 AMD tiled（`amd:0x020000044051ba01`，2 plane）**正常显示**。这与 niri 源码里那条 HACK 一致："cross-device buffers produce a glitched scanout if the modifier is not Linear" | M0b 探针 + `grim` 四段均值对照 |
| niri 的 `MultiRenderer` 只在 `render_device != target_device` 时启用拷贝路径：`create_shared_dma_framebuffer` 用**渲染设备**的 allocator 分配，modifier 取两设备可导入集合的**交集**，再让目标设备 `import_dmabuf` | smithay `renderer/multigpu/mod.rs` |
| `debug { render-drm-device "<render node>" }` 只读一次（`Tty::new`），**热重载不生效，必须重启 niri**；`ignore-drm-device` 自 25.11 起可用 | niri `src/backend/tty.rs:466`、`docs/wiki/Configuration:-Debug-Options.md` |
| 混合显卡机型 niri 默认在 iGPU 上渲染是**官方设计**（省电），代价是外接屏内容要拷到 dGPU，官方承认高分辨率高刷下可能卡顿；解法是 UEFI 的 MUX 开关或上面的 debug 选项 | niri `docs/wiki/FAQ.md` |
| **niri 对图层表面的 frame callback 按 60 Hz 节奏**，即便 DP-1 当前是 180 Hz：实测 68 帧的 dt 分布 min 15.9 / 中位 16.7 / max 16.9 ms。600 ms 的缓动因此约 36 帧 | M1 daemon `--trace` |
| **`wl_surface.frame` 只在 `wl_surface.commit` 时生效**：单独发出请求、不 commit，回调永远不来（表现为"动画一帧都不走"）。正确做法是与 buffer 同一次 commit，或在没有新 buffer 时单独 commit 一次 | M1 实测（第一次实现踩到） |
| **缓冲池空时不能只"跳过绘制"**：那样既没有 commit 也没有新的帧请求，动画会卡死。必须补一次只带帧请求的 commit | M1 实测 |
| 测量用的条带：窗口上下沿的 10 px 外边距、左右 10 px 边距都是纯壁纸；**屏幕底部有 `noctalia-dock` 图层**、顶部有 bar，用它们做剖面会被污染 | M1 实测 |
| **做逐像素比对时要扣掉窗口阴影**：本机 `layout { shadow { on; softness 10; spread 4; color "#00000070" } }`，于是窗口外的 10 px 边距被阴影压暗——实测比值从离窗口远处 0.884 单调降到贴窗口处 0.823（纯乘性、有梯度）。这是配置效果，不是渲染 bug；要干净的比对就切到空工作区 | M2 实测 |
| niri **不会**因为 `focus-workspace <不存在的索引>` 而新建工作区（实测 5/6/9 均无效）；空工作区只有已有的那些，而 niri 会在最后一个空工作区被填满后自动补一个空工作区 | M1 实测 |
| 测 3 列位移时用 backdrop 放置（`--namespace mpvpaper` 命中既有规则）只是为了让**测量更干净**：工作区背景的壁纸会随工作区滚动一起平移，混进位移里 | M1 实测 |
| **配置键写了但没接上线**：`span` 曾只存在于配置与日志里，`Niri::connect()` 写死默认值 ✗ —— 因为默认值恰好等于当时配置的值，一直没暴露。改动配置管线时务必实机验证"改一个非默认值，行为跟着变" | 实测 |
| **竖直移动工作区时横向被带着走 → 斜向**：`horizontal` 若跟着新工作区重算，两个轴就锁在一起走 ✗（实测逐帧增量完全相同）。修复见 §4.4 第 8 条；判据是"焦点此前是否已在该输出的活动工作区上"，不是事件类型 | 实测 |
| **niri 不暴露它的动画参数**：`niri msg` 没有 dump 配置的命令、`niri validate` 只打印 "config is valid"、事件流里与配置有关的只有 `{"ConfigLoaded":{"failed":false}}`。唯一来源是它的 KDL 配置文件（含 `include`，本机动画在 `__custom__.kdl` 里） | `niri msg --help`、`niri validate`、事件流实测 |
| **niri 的配置路径顺序**（`src/main.rs`）：`-c/--config` > `$NIRI_CONFIG` > 用户配置（`$XDG_CONFIG_HOME/niri/config.kdl`，否则 `~/.config/niri/config.kdl`）> `/etc/niri/config.kdl`。注意 niri 启动后**会把 `NIRI_CONFIG` 从自己的环境里删掉**，子进程看不到 → 跟随它的唯一可靠办法是读 `/proc/<niri pid>/cmdline` 里的 `--config` | 读 niri 源码 + 实测 |
| `include` 的路径**相对于包含它的那个文件**解析（不是相对于主配置），绝对路径原样使用，且会**展开开头的 `~`**；递归有上限，自包含会报错 | niri `niri-config/src/lib.rs` 的 include 分支 |
| **没有自定义动画时**我们回落到内置默认，而内置默认就是 **niri 自己的默认值**（`overview-open-close: spring 1.0/800/0.0001`、`slowdown 1.0`），所以两边仍然一致；完全没有 niri 配置也不是错误 | 实测（临时配置只含 `input` 段 → 日志显示 `spring 1/800/0.0001`，无报错） |
| **niri 的配置是 KDL v1**（`niri-config` 依赖 `knuffel 3.2.0`），而 `kdl` crate 6.x 默认解析 KDL v2 → 直接解析会失败且 `Display` 只说 "Failed to parse KDL document"。crate 的 `v1-fallback` 特性正是为此；错误信息要用 `KdlError::diagnostics` 才可读 | 实测（读本机 config.kdl 失败 → 开特性后成功） |
| KDL 里 `400` 是 **Integer**、`0.5` 是 Float，`as_float()` 对前者返回 None；niri 的 `stiffness=400`、`cubic-bezier … 1` 都是整数写法 | 实测 |
| `--dump-stats` 是**抽样**输出（不能用来数帧率）；`nvidia-smi` 在桌面负载下噪声大（基线 26–65%），难以量测边际成本 | 实测 |
| 工具链：`cargo` / `rustc` / `gcc` / `grim` 在位；`socat` 未安装 | `command -v` |

---

## 6. 里程碑与验收

| 阶段 | 内容 | 验收标准 |
| --- | --- | --- |
| M0a ✔ | EGL 按 GPU 绑定 | 结论修正为"按合成器 main device 绑定"，机制见 §5；`eglpin` 可复现 |
| **M0b ✔** | 最小 layer 客户端 + 自建 dmabuf 交换链 | ① `niri msg --json layers` 出现 `{"namespace":"niripaper","output":"DP-1","layer":"Background"}`；② 用 `--namespace mpvpaper` 命中既有 rule 后，半透明窗口的模糊区域确实显示我们的画面（无 rule 时显示 Noctalia 壁纸）；③ 进程只打开 `/dev/dri/renderD128`；④ 静置 6 s：0 事件、0.0 ms CPU、0 字节写 socket、不请求 frame callback；⑤ 5 帧提交收到 4 次 `wl_buffer::release`（缓冲被回收，非泄漏） |
| **M1 ✔** | Niri IPC + `motion.rs`（§4.3 全部用例）+ frame callback 的 OutCubic 视差 + CLI + 配置文件 | **已验证**：`cargo test` 覆盖 §4.3（18 个用例）；逐帧单调（两段动画 30/35 帧，`--trace` 可复现）；静置 8 s：0 帧、0 目标变化、0 CPU 滴答；buffer 池 68 帧 / 68 release / 0 丢帧。**已实测切列位移**：3 列工作区（临时开 3 个终端造列，测完关闭）从第 1 列到第 3 列 = **102 px**（预期 102.4；相关性搜索是 1 px 粒度），纯壁纸条带上的 mse 160 vs 零位移 326；2 列时 Δrank=1 = **51–52 px**（预期 51.2）。**配置文件已实现**：`~/.config/niripaper/config.toml`，优先级 CLI > 文件 > 默认值；实测 `duration_ms = 150` 时动画段为 7/10 帧（600 ms 时约 36 帧），证明配置确实驱动到缓动。**缓冲必须跟着"当前" feedback 走**：`main_device` 可能运行时变化（会话恢复、设备变化），变了就要重建缓冲与 GL 导入；日志里必须记下"选了哪个设备 / vendor / modifier"（§7 风险 1 的失败是静默的） |
| **M2 进行中** | 静态图 ✔ / 总览过渡动画 ✔ / 换图过渡 / V2 视频（libmpv 纹理） | 静态图已实测：截图与"源图 → cover → 缩放 → 视差偏移"的独立复算**相关系数 0.9847**，偏移误差 ≤1 px。总览过渡实测：`overview opened → zoom 0.960`，动画 21 帧 ≈ 350 ms 逐帧单调；总览打开时按候选 zoom 复算整屏像素，误差在 z=0.96 处最小（6.21 vs z=1.00 的 12.33），即屏幕确实是 0.96。剩余验收：视频壁纸下切列时可见画面与**模糊区域**一起动；任意内容组合换图无空窗 |
| M3 | Noctalia 插件（镜像壁纸选择 + 设置 + 开关 + 原生层交接） | 关闭插件无残留进程；改壁纸 1 s 内生效；原生层恢复自愈 |
| M4 | 开源：README / 教程 / 示例 / CI（`cargo test` + `clippy` + `fmt`）、AUR / Releases | 新用户照 README 从零到出效果 |

---

## 7. 必须避免的失败模式与已知风险

1. **选错 GPU**（M0b 修正了这里的判断）：不是"输出在哪个 GPU 就在哪里渲染"。niri 用**主渲染节点**（本机是 AMD iGPU）合成所有输出，再把结果拷贝给"仅显示"设备（NVIDIA 的 DP-1）。所以正确规则是**跟随合成器的 dmabuf feedback `main_device`**：在 NVIDIA 上渲染 DP-1 反而会每帧多一次跨 GPU 导入+拷贝。判据：`surface feedback main device` 指向哪个 `dev_t`，就在哪个节点上开 GBM/EGL。**失败模式很隐蔽**：跨设备提交非 LINEAR 缓冲时 niri 不报错、`release` 也正常回来，但画面上什么都没有（§5 有对照实验）——排查时先量"模糊区/壁纸区四段均值"，不要只看有没有报错。
2. **一个进程只能钉一个 EGL vendor**：GLVND 只在第一次 EGL 调用时读 `__EGL_VENDOR_LIBRARY_FILENAMES`。合成器的 main device 是单一 GPU，所以够用；但**不要**设计成"一个进程同时渲染两块不同厂商的 GPU"。
3. **进度按"实际列数"归一化** → 开/关任何窗口都会让壁纸移动。必须用固定 `span`。（排查方法：开一个窗口，看壁纸是否动。）
4. **重复下发相同目标** → 缓动重启 → 画面在移动中"突然停住"。只在目标变化时下发；另外注意"播放器/客户端身份"不能用浮点 mtime 比较（末位抖动会导致每个 tick 都判成新客户端、每秒重发一次），要量化。
5. **中途改目标未从当前位置缓出** → 画面会跳一下。先取当前值，再换目标。
6. **`setWallpaperEnabled` 的恢复不可靠**：`onExit` 里调用不一定生效（运行时正在拆卸）。缓解：插件启动时对全部输出自愈式 `true`；文档写明"万一壁纸变纯色就重启 Noctalia"。
7. **backdrop 一致性有固有每帧成本**：壁纸每动一帧，backdrop 失效 → Niri 要重建并重跑模糊。这是"窗口背景必须跟随"的代价，无法消除。可省的是：额外的解码/渲染开销、跨 GPU 拷贝、外部进程往返、GPU 选错。
8. **backdrop 内的层序取决于创建顺序**：视差"失效"（壁纸不跟随）时，先查是否有别的层盖住（`niri msg --json layers` + §8 的位移测量），再怀疑算法。
9. **`wl_buffer::release` 的语义**：常驻缓冲永远不会被释放，必须按"替换后才回收"来设计池（管线深度 ≥ 1）。M1 做 buffer 池时注意。
10. **弹簧的"结束"判据不能用瞬时误差**：欠阻尼弹簧每半个周期穿过一次目标，`|当前 − 目标|` 会在第一次穿越时变成 0 —— 于是动画在过冲之前就被判定结束，用户永远看不到回弹（本机实测就是这样：轨迹一路走到 0.9600 直接停住）。判据要用**衰减包络**（ζ<1 时 `e^{-ζωt}/√(1-ζ²)`），它单调递减；值本身该过冲就过冲。修好后实测过冲 0.0065，与解析值 `e^{-πζ/√(1-ζ²)}×0.04` 一致
11. **LINEAR 是跨设备 scanout 的通行证**：niri 对"渲染节点 ≠ 主节点"的输出只接受 Linear 的 scanout tranche。我们选 LINEAR（Mesa 能渲染进 LINEAR）既省一次拷贝又保证可直扫。**NVIDIA 不能渲染 LINEAR**，所以若将来 main device 是 NVIDIA，必须改用它的 tiled modifier，并接受 niri 的跨设备拷贝。
11. **视频参与换图过渡需要 V2**：外部 mpvpaper 的帧拿不到，无法混合。
12. **指标可信性**：不要用 `--dump-stats`（抽样）或 `nvidia-smi`（噪声大）宣称性能收益；先用 §8 的方法确认指标可信。

---

## 8. 验证方法

**M0b 用过的四条**（可直接复跑）：

```sh
NIRI_SOCKET=$(ls -t /run/user/1000/niri.*.sock | head -1)      # shell 里的可能已过期

# ① 层注册
./target/debug/m0b --namespace niripaper &                     # 保持运行
niri msg --json layers | grep niripaper

# ③ 只开目标 GPU 节点：探针自己会打印 /proc/self/fd 里所有 /dev/dri 与 nvidia* 目标
# ④ 静置开销：探针 --idle-seconds N 会打印事件数 / CPU 时间 / socket 写入字节数
./target/debug/m0b --idle-seconds 6

# ⑤ 交换链回收
./target/debug/m0b --frames 5        # 期望 "5 commit(s), 4 buffer release(s)"
```

**测"壁纸实际移动了多少像素"**（M1 验收靠它）：

```sh
grim -o DP-1 /tmp/a.png     # 状态 A（例如焦点在第 1 列）
grim -o DP-1 /tmp/b.png     # 状态 B（例如焦点在第 3 列）
```

- **横向**：取一条只属于壁纸的**顶部横带**（窗口上方、bar 透明处，y≈3..12），压成 1 px 高的灰度剖面，在 ±250 px 内做**一维相关性**搜索（mse 最小处）。期望：列 1→列 3 = **102.4 px**（2560 宽、scale 1.1、span 6）。
- **纵向**：用**屏幕左边缘竖条**（x≈0..8）做剖面；期望每个工作区台阶 72 px。
- **测试图必须有唯一结构**：重复图案（棋盘格、等距条纹）会**混叠**——位移恰好等于图案周期时相关性给出 0，得出"没动"的假结论。用随机砖块/随机条形图。
- 若相关性给出 0 位移：先按 §7 风险 8 查层序，再怀疑算法。
- `grim` 单次约 20–40 ms，**不能**用来数帧率；要测帧率需在渲染循环里打点或录屏（本机无 `wf-recorder`）。

**判定"模糊到底采样谁"**（比 M0b 那套差分法可靠；M0b 的结论已被此法推翻）：

1. 做一张**纯色**标记图（例如纯红 `#ff0044`）当壁纸；
2. 分别用 `--namespace niripaper`（工作区背景）与 `--namespace mpvpaper`（backdrop）各跑一次，各截一张图，另截一张**不跑守护进程**的基线；
3. 只在**窗口内部**（基线里偏暗的像素）求均值：基线 (34.4, 45.8, 56.6)、工作区背景 (65.6, 25.5, 43.6)、backdrop (63.7, 22.7, 42.3) → R−B 从 −22 变成 +22 / +21，**两者只差 0.6**，即放置位置不影响模糊。
   ⚠️ 深色图案测不出这个差别（透过窗口只贡献约 10%，会落在噪声里）——这就是 M0b 当初误判的原因。

**测总览状态下的视差**：打开总览后用 `niri msg action focus-workspace N` 切换，对**缩略图之外**的背景竖条（x≈0..30）做竖直一维相关性；实测 72 px（= 144 × Δv），mse 0.0。

**旧的差分法**（窗口内容必须静止，仅供参照）：

1. 用 `--namespace mpvpaper`（命中既有 rule）跑一次，切到一个只有静态窗口（如终端）的工作区，`grim` 截图；
2. 换成 `--namespace niripaper`（无 rule）再截一次；
3. 比较**同一块窗口区域**的四分之一屏均值：有 rule 时四个象限会分别偏向红/绿/蓝/黄（我们的测试图案），无 rule 时四个象限颜色一致（Noctalia 壁纸）。
   实测（y=1100，rule − norule）：`(−62,−121,−136) / (−115,−71,−132) / (−124,−126,−90) / (−68,−75,−135)`。
4. 需要 Niri/Noctalia 的环境变量（`NIRI_SOCKET`、`WAYLAND_DISPLAY`、`XDG_RUNTIME_DIR`）才能从脚本里调它们的 CLI。

---

## 9. 参考资料

- Clavis（运动参数与公式的来源）：https://github.com/StatIndet/quickshell —— 见 §4 的定位表
- Niri 的 GPU 模型与 dmabuf feedback：`YaLTeR/niri` tag `v26.04`，`src/backend/tty.rs`（`surface_dmabuf_feedback`、`primary_render_node`、`GpuManager`）
- Niri layer rules：https://github.com/YaLTeR/niri/wiki/Configuration:-Layer-Rules#place-within-backdrop
- Niri backdrop 定制：https://github.com/YaLTeR/niri/wiki/Overview#backdrop-customization
- Niri IPC：https://github.com/YaLTeR/niri/wiki/IPC （连 `$NIRI_SOCKET`，写一行 JSON，读一行 JSON；`"EventStream"` 先全量后增量）
- linux-dmabuf feedback：`linux-dmabuf-v1.xml`（`main_device` / `format_table` / `tranche_*` 的语义）
- mpv：`man mpv`（`int frame` uniform、`MPV_RENDER_API_TYPE_OPENGL`、`--swapchain-depth`）
- mpvpaper（libmpv 嵌入的参照实现）：https://github.com/GhostNaN/mpvpaper
- awww / swww（Rust 壁纸守护进程；其"外部守护进程 + 桌面壳插件镜像选择 + 撤掉原生层"的集成模式可借鉴）：https://codeberg.org/LGFae/awww
