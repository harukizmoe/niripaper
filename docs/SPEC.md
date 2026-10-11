# SPEC: niripaper

> 系统规格。描述**现在是什么**（契约、行为、边界），不是待办清单。
> 代码注释里的 `§x.y` 指向本文件；**§1–§4 的编号是稳定的**，改动会让源码引用失效。

## 1. 项目定义

**niripaper = niri 的壁纸引擎**：独立守护进程，自己拥有壁纸图层与渲染循环，支持静态图 / 视频，**布局反应式视差**（随焦点列与活动工作区移动），换壁纸有过渡，并且**窗口模糊采样到的画面与可见壁纸始终一致**。

**不做（v1）**：shader / 粒子 / 交互式场景壁纸、音频或鼠标反应特效、壁纸资源商城、修改合成器本身、**选 GPU / 切 MUX / 覆盖合成器的渲染设备**（选哪块卡是合成器的决定，niripaper 不提供任何选卡开关，也不读不写 BIOS/PRIME 相关配置）。

术语见 [CONTEXT.md](../CONTEXT.md)。

---

## 2. 已定决策（无需再议）

标 **ADR** 的条目改动代价大、且从代码看不出来，理由单独成文；其余是本文件层面的规格决定。

| # | 项 | 决定 |
| --- | --- | --- |
| 2.1 | 名字 | `niripaper`（仓库 / crate / 二进制 / layer-shell 命名空间 / 配置目录同名） |
| 2.2 | 形态 | 独立守护进程；不依赖任何桌面壳也能用 |
| 2.3 | 许可证 | **GPL-3.0-or-later** — **ADR** [0005](./adr/0005-gpl3-or-later.md) |
| 2.4 | 视频方案 | 守护进程内嵌 libmpv（`mpv_render_context` + OpenGL），不用外部 mpvpaper 进程 |
| 2.5 | 渲染栈 | **OpenGL / GLES + EGL**，不是 Vulkan/wgpu — **ADR** [0001](./adr/0001-opengl-not-vulkan.md) |
| 2.6 | 选 GPU 的规则 | 跟随合成器的 dmabuf feedback **`main_device`**，不是"输出所在的 GPU" — **ADR** [0002](./adr/0002-follow-compositor-main-device.md) |
| 2.7 | EGL vendor | 由目标 GPU 的 PCI vendor id 决定（`0x10de` → NVIDIA ICD，其余 → Mesa），在**本进程第一次 EGL 调用之前**用 `__EGL_VENDOR_LIBRARY_FILENAMES` 钉住。**一个进程只能钉一个 vendor** |
| 2.8 | FFI 方式 | 裸 FFI 链 `libEGL` / `libgbm` / `libGL`，不用 EGL/GL crate — **ADR** [0003](./adr/0003-bare-ffi-no-egl-crates.md) |
| 2.9 | 进度来源 | 守护进程**自己**连 `$NIRI_SOCKET` 读 event stream，不依赖外部推送 |
| 2.10 | 多输出 | 一个进程管所有输出，**每个输出一个线程** — **ADR** [0004](./adr/0004-one-process-thread-per-output.md) |
| 2.11 | 控制 socket | 单一 `niripaper.sock`，协议 `set <output\|*> <path>` — **ADR** [0008](./adr/0008-single-control-socket.md) |
| 2.12 | 配置目录 | `config.toml` + `config.d/*.toml` 分层合并，不发明 include — **ADR** [0006](./adr/0006-config-d-directory-not-include.md) |
| 2.13 | 热重载 | 裸 inotify，**监视目录而不是文件** — 见 §3 |
| 2.14 | 换图过渡 | 屏幕空间混合 + 冻结快照 — **ADR** [0007](./adr/0007-screen-space-transition-with-frozen-snapshot.md) |
| 2.15 | 命名空间 / layer rule | 守护进程用 `niripaper`；**规则是可选的**（见 §2.15.1） |
| 2.16 | 默认参数 | `scale = 1.1`、`column_span = 6`、`workspace_span = 6`、视差 `ease-out-cubic 600 ms`、总览过渡 `spring 1.0/800/0.0001` + `zoom 0.96`、换图过渡 `fade 250 ms`、视频静音、`hwdec=auto-safe` |
| 2.17 | 动画配置 | 语义与写法对齐 niri 的 `animations { }` — 见 §2.17.1 |
| 2.18 | 跟随 niri 配置 | `animations.follow_niri`（默认 true）— 见 §2.18.1 |
| 2.19 | 总览过渡 | zoom 实现为 **`scale` 的动画化乘数** — 见 §2.19.1 |
| 2.20 | `fit` 的名字 | 借自 Wallpaper Engine — 见 §2.20.1 |
| 2.21 | `push` | 过渡期间画布被推开的比例，默认 **0.55** |
| 2.22 | 面板接口 | `schema` / `state` / 改配置热重载 — 见 §2.22.1 |
| 2.23 | 分发 | 源码 `cargo install` + AUR + GitHub Releases 预编译 |

### 2.15.1 命名空间与 layer rule

默认（不加规则）= 壁纸是**工作区背景**；加 `match namespace="niripaper"` + `place-within-backdrop true` = 壁纸进 **backdrop**。两者**模糊效果相同**（实测），区别只有两处：① backdrop 时总览背景整屏都是壁纸、且切工作区时壁纸不跟着滑；② 工作区背景时总览背景干净、壁纸出现在各工作区缩略图里（所有缩略图共用一张、同一偏移 —— layer 是 per-output，做不到按工作区独立视差）。

### 2.17.1 动画配置的语义

每个动画三选一 —— `off`、缓动（`duration_ms` + `curve`）、或 `spring`（`damping_ratio` / `stiffness` / `epsilon`，质量固定 1，与 niri 同）。`curve` 取 niri 的五个：`linear` / `ease-out-quad` / `ease-out-cubic` / `ease-out-expo` / `cubic-bezier`（后者配 `cubic_bezier = [x1,y1,x2,y2]`）。另有顶层 `off` 与 `slowdown`。当前两项：`animations.parallax`（§4.2.6：ease-out-cubic 600 ms）、`animations.overview-open-close`（`zoom` + 动画，默认用 **niri 自己的默认弹簧** 1.0/800/0.0001）。两种动画类型都能驱动任何被动画量。

### 2.18.1 跟随 niri 配置

读 niri 自己的 `config.kdl`（含 `include`），取用两者共享的 `overview-open-close` 与 `slowdown`；niri 的全局 `off` 只关掉总览过渡，**不动视差**（视差不是 niri 的动画）。优先级：**本文件显式值 > niri 配置 > 我们的默认值**，来源会写进启动日志（`from niri's config: …`），避免静默偏离。`follow_niri = false` 则完全不读。

### 2.19.1 总览过渡的实现

总览打开/关闭时画布做一次轻微后撤，由 `OverviewOpenedOrClosed` 驱动。zoom 实现为 **`scale` 的动画化乘数**，所以画布/overflow/视差行程/纹理采样全部自动跟随，shader 不用改、纹理不用重传（只采样子区域）。niri 不报告它自己的过渡进度，所以只能近似同步 —— 把参数设成与 niri 配置里 `overview-open-close` 相同的弹簧，手感就基本一致。

### 2.20.1 `fit` 的取值

`fill` / `fit` / `stretch` / `center` / `tile`，跳过 `span`。顶层键 + 按输出可覆盖（`OutputParams.fit`）。**`tile` 是视频唯一做不到的一种** → 对视频退回 `fill`，`state` 报**实际**用的值而不是请求的值。换 `fit` 会重新装载媒体。

### 2.22.1 面板接口

三条：`niripaper schema`（机器可读的配置说明，含 Enum 取值）、`niripaper state`（**当前实际**在用的值 + 来源）、以及"改配置文件让它热重载"。**`set <path>` 是临时覆盖**：任何一次热重载都会盖掉它，别拿它当持久设置。

---

## 3. 架构

```
niripaper（Rust 守护进程）
├── gpu.rs      sysfs：connector → card → render node / vendor id；dev_t → /dev/dri 节点
├── niri.rs     连 $NIRI_SOCKET，写 "EventStream"，读一行一事件；解析 niri-ipc 的形状
│               （只认 §4.2.7 的 8 个事件，其余含多键信封一律丢弃）→ 喂给 motion.rs
├── motion.rs   §4 的运动规格（纯逻辑、无 IO）
├── ipc.rs      控制 socket：$XDG_RUNTIME_DIR/niripaper.sock（行协议）
├── config.rs   ~/.config/niripaper/config.toml（含 [outputs.<名>] 覆盖）
│               未知键与越界值都报错并指出键名
├── schema.rs   schema 输出：每个 UI 能提供的键（类型 / 边界 / 默认 / 单位 / 是否热改）
├── watch.rs    inotify 热重载
├── media.rs    媒体路由：静态图 / 视频
├── scene.rs    一帧的绘制输入（收成结构体，加参数不波及调用点）
├── crossfade.rs 换图过渡
├── daemon.rs   Options、事件循环与日志、多输出监督
└── render/
    ├── layer.rs   Wayland 客户端：registry / wl_output / wlr-layer-shell 表面
    │              + dmabuf feedback（default 与 per-surface）+ wl_buffer 提交与 release 回收
    ├── egl.rs     EGL 绑定到指定 GPU（GBM platform + GLVND vendor 钉住）+ dmabuf 导入
    ├── gbm.rs     GBM 设备与 buffer（O_RDWR 打开 render node；plane fd/offset/stride/modifier）
    ├── dmabuf.rs  Frame：GBM BO → EGLImage → GL 纹理 → FBO，可直接作为 wl_buffer 提交
    ├── gl.rs      GL 子集（链 libGL，GLVND 按当前 EGL 上下文分发）+ 测试图案
    ├── image.rs   解码 → cover 裁剪 → 缩放到画布（output × scale）→ 纹理
    ├── video.rs   libmpv render API → 纹理
    ├── anim.rs    frame callback 驱动的动画：视差平移 + 总览 zoom + 换图过渡
    └── transition.rs  换图过渡的九个效果
```

**为什么自己连 niri**：niri IPC 很简单（连 socket → 写一行 JSON 请求 → 读一行 JSON；`"EventStream"` 先给全量状态再持续推增量）。自己连才能独立于任何桌面壳，并且省掉"niri 事件 → 外部进程 → 再转发给渲染器"这条链的延迟（§4 的动画必须与窗口动画同时起步）。

**双后端、单循环**：静态图用自有纹理（便宜）；视频用 libmpv 纹理；两者都由同一个 frame-callback 循环带着偏移绘制。换图过渡统一用"把上一帧内容快照成纹理再混合"——任意组合（图↔视频↔视频）都能过渡，且任何时刻只需一个解码器。

**提交路径**：不用 `wl_egl_window`、不用 `eglSwapBuffers`。每帧：`gbm_bo_create_with_modifiers`（用合成器 feedback 里给出的 modifier）→ `eglCreateImageKHR(EGL_LINUX_DMA_BUF_EXT)` → `glEGLImageTargetTexture2DOES` → 挂到 FBO 上画 → `glFinish` → `zwp_linux_dmabuf_v1` 提交 → 等 `wl_buffer::release` 再复用。**只有这条路能保证渲染发生在指定 GPU 上。**

**热重载**：`watch.rs` 把 inotify fd 直接进 `poll()`，**监视目录而不是文件**（编辑器多为"写临时文件再 rename"，盯文件会跟丢）。必须**解析事件**：`IN_CREATE` 早于内容落盘，所以重载只在 `IN_CLOSE_WRITE` / `IN_MOVED_TO` / `IN_DELETE` / `IN_MOVED_FROM` 上做，`IN_CREATE` 只在 `IN_ISDIR` 时算。`Watcher` **不依赖"记住"目录** —— `inotify_add_watch` 幂等，每轮无脑重加，目录被删了再建也能重新盯上。

**配置合并**：`config.toml` + `config.d/*.toml` **按文件名排序**逐个解析再深合并。每份先单独反序列化（出错能指名是哪个文件、哪一行），失败**保留旧配置**并打印原因。两条硬要求：**解析失败不许把 daemon 打挂**；**`output` / `namespace` 不能热改**（重建输出代价太大，改了也只当没看见）。

**技术选型**

| 用途 | 选择 | 备注 |
| --- | --- | --- |
| Wayland | `wayland-client` + `wayland-protocols`(unstable) + `wayland-protocols-wlr` | layer-shell v5 / linux-dmabuf v4 |
| EGL / GBM / GL | **裸 FFI**：`libEGL.so`（GLVND 加载器）、`libgbm.so`、`libGL.so`（GLVND 分发） | `src/render/{egl,gbm,gl}.rs`；无 EGL/GL crate |
| dmabuf 提交 | `zwp_linux_dmabuf_v1` + 自管 buffer | 见 `docs/gotchas.md` 的设备选择一节 |
| 视频 | `libmpv`（`MPV_RENDER_API_TYPE_OPENGL`） | 务必调 `mpv_render_context_report_swap()` |
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

`pan = (scale-1)/scale × (0.5 - progress)`，`zoom = log2(scale)` —— **新渲染器不要照搬这一行**，直接用上面的像素偏移。

数值例子（2560×1440、scale 1.1、两个 span 都取 6）：`overflow = 256 px` 横向、`144 px` 纵向；3 列时 rank 0/1/2 → progress 0 / 0.2 / 0.4 → **首列到末列共移动 102.4 px**（每列 51.2 px）；工作区 idx 1…4 → progress 0 / 0.2 / 0.4 / 0.6（每级 28.8 px）。两者都用固定跨度，见 §4.4 第 10 条。

### 4.2 事件与记忆规则

1. `column_span` 是**固定值**，不是"该工作区实际列数"（理由见 `docs/gotchas.md`）。**纵向同理**：`workspace_span` 是固定值，不是活动工作区数（见 §4.4）。两者独立配置。
2. 每个工作区**各自记忆焦点列**（存列号）；列消失时解析到**最近的现存列**（nearest，不是 lower-bound）。
3. 记忆只在**活动工作区**上更新；活动窗口是**浮动窗口**时保留上一个平铺列（不重置）。
4. 该工作区没有任何列 → `horizontal = 0.5`；**只有一列 → 0**（贴左边，属规格，不要"修正"为 0.5）。
5. 该输出上没有活动工作区 → `(0.5, 0.5)`。
6. 动画：`OutCubic`、600 ms；**收到与当前目标相同的目标必须忽略**（QML `Behavior` 只在源值变化时才动）；中途改目标时从"当前已到达的位置"重新缓出。
7. 只处理这些事件：`WorkspacesChanged` / `WorkspaceActivated` / `WorkspaceActiveWindowChanged` / `WindowsChanged` / `WindowOpenedOrChanged` / `WindowClosed` / `WindowFocusChanged` / `WindowLayoutsChanged`；其余事件（含多键信封）一律忽略，`apply()` 返回 false；上述事件 `apply()` 返回 true。

### 4.3 回归用例表

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

### 4.4 实现注记

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
7. **"双键信封"那一行不属于 `motion.rs`**：那是 IPC 解析层的事 —— 多键对象根本不会变成 `Event`，所以由 `niri.rs` 的解析器返回 `None` 来保证（`apply()` 只会收到 §4.2.7 的 8 个变体）。
8. **横向不随工作区切换平移**（对 §4.2.2 的修正）。原先"每个工作区各自记忆焦点列"意味着切工作区时 `horizontal` 也会跳到新工作区的列 ✗：竖直切换变成了**斜向** ✗。实测（修前）：`target h=0.500 v=1.000`，逐帧 `h` 与 `v` 的增量**完全相同**（0.1301/0.6301、0.2215/0.7215 …）—— 两个轴锁在一起走。修后：`target h=0.000 v=0.500`，`h` 一动不动。
   * 为什么不能只看事件类型：niri 切工作区发的是 `WorkspaceActivated` **紧跟一个** `WindowFocusChanged`（指向新工作区的窗口），而横向移动焦点**也**只发 `WindowFocusChanged` ✗。
   * 判据：**"焦点此前就在该输出的活动工作区上"** → 说明正在发生的是横向滚动，不是工作区带在滑动。其余情况（`WorkspaceActivated`、以及它后面那个 `WindowFocusChanged`）只改竖直。
   * 首次建立时机：状态有意义（已知焦点窗口）时建立一次。否则"切换早于首次横向移动"会回落到按新工作区重算 ✗，又变回斜向。
   * 实测（修后，八个动作段全部轴向纯净）：ws-down/up → Δh **+0.0 px**、Δv ±61…68 px；col-right/left → Δh ±46…47 px、Δv **+0.0 px**。
9. **过期的 `is_focused` 标志会把焦点抢回去**（修 8 时一并发现）。`WindowOpenedOrChanged` 对**标题/app_id 变化**也会触发，其载荷窗口并非焦点窗口 ✗；而 `adopt_reported_focus` 会在窗口列表里扫描 `is_focused` ✗ —— 我们自己从不清除旧标志 ✗，于是这类事件会把焦点（以及横向视差）拽回上一个焦点窗口 ✗。修正：只有**载荷窗口自称被聚焦**时才采纳（`WindowsChanged` 是完整列表，仍可整体扫描），采纳时把其余窗口的标志清掉（niri 的语义：一个窗口报告被聚焦即意味着其他都不是）。
10. **纵向也用固定跨度**（对 §4.2/§4.3 的修正），即 `workspace_span`。原式 `activeIndex / (count - 1)` 用的是**活动工作区数** ✗，于是同一个活动工作区的位置随工作区总数变化 ✗：实测（修前）idx 2 在 3/4/5 个工作区时分别是 **72 / 48 / 36 px** —— 新建或关闭一个工作区，壁纸就跳一下 ✗，视差比例也跟着变 ✗。这与 §4.2.1 给列定"固定值"的理由**完全相同**，只是当初漏了这一轴 ✗。改为 `clamp01(activeIndex / (span - 1))`，保留"该输出只有 ≤1 个工作区时取 0.5"（没有可平移的对象 → 居中，§4.3 的单工作区行都依赖它）。

    实测（修后，用户 4 个工作区）：纵向目标 0.2 / 0.4 / 0.6（每级 28.8 px），八个动作段仍全部轴向纯净。

    两轴**各自独立**（`column_span` / `workspace_span`）：它们数的是不同的东西，工作区多的人不该为此缩小列步长。想让纵向像旧的"走满 overflow"那样大，把 `workspace_span` 设成工作区数即可，横向不受影响。

    同一次改动还修掉一个潜伏 bug：`Niri::connect()` 里写死 `Motion::new(DEFAULT_SPAN)`，**配置里的 span 从未接上线** ✗（默认值与当时配置里的值都是 6，所以看不出来 ✗）。现在由 daemon 把生效的跨度传进去；实测 `--config` 写 `column_span = 11` / `workspace_span = 3` → 启动日志 `spans 11/3`，每列 25.6 px、每级 72 px。
11. **总览的"左右"走的是"活动窗口"，不是焦点**。实测抓到的真实事件（总览打开期间）：
    * `Mod+←/→`（`focus-column-or-monitor-*`）→ **`WorkspaceActiveWindowChanged`**（该工作区的活动窗口在换）
    * `Mod+↑/↓`（`focus-window-or-workspace-*`）→ **`WorkspaceActivated`**（切工作区）
    * 打开总览时先发一次 **`WindowFocusChanged(None)`**

    总览里唯一携带横向变化的信号是**活动窗口** ✗。由此定死（`src/motion.rs`）：
    ① `focused_id` **照实记**（`None` 就清空）→ 它驱动记忆列（§4.4 第 2 条的优先级：焦点只在**真有焦点**时覆盖活动窗口）。**不要**"保留最后焦点"来驱动记忆列：那样过期的焦点会盖住活动窗口信号，整个总览横向冻结 ✗。
    ② 另存一个 **`last_focus_workspace`**（跨 `WindowFocusChanged(None)` 保留），**只**回答"焦点此前是不是在这个输出的活动工作区上" ✗。它**绝不能**参与记忆列 ✗ —— 两个字段分开正是为了这个 ✗。
    ③ 横向刷新判据：**该输出的活动工作区没变** && （`last_focus_workspace` 就在该输出上 || 从未见过焦点）。
    ④ 为什么"从未见过焦点"要放行：焦点**第一次**出现（`None → Some`）必须平移，§4.3 的多行依赖它 ✗。
    ⑤ 为什么 ② 的保留是必须的：`None → Some` 有两种含义 —— "焦点第一次出现"（要平移 ✗）和"切工作区后 niri 补发的焦点"（必须保持 ✗）。只有"上一个已知焦点在哪个工作区"能把两者分开；缺了它，`focus-window-or-workspace-*` 这类切换会把横向一起带走，斜向回归 ✗（单测 `a_workspace_switch_with_nothing_focused_does_not_pan` 就是钉这个的）。
    ⑥ ③ 里的"活动工作区没变"也是必须的：否则总览里 ↑↓（只有 `WorkspaceActivated`、没有焦点）会走进 ③ 的"没有焦点"那一支，把横向跳到新工作区的值 ✗（单测抓到过 0.0 → 0.5 ✗）。

    实测（用户的确切按键，scale 1.2 → overflow 512×288 px；普通视图与总览各测一遍）：

    | 动作 | 普通视图 | 总览 |
    | --- | --- | --- |
    | `focus-column-or-monitor-right/left` | Δh ±125 px、Δv +0.0 px | Δh ±125 px、Δv +0.0 px |
    | `move-column-right/left` | Δh ±118 px、Δv +0.0 px | Δh ±118 px、Δv +0.0 px |
    | 切工作区（`focus-workspace`） | Δh +0.0 px、Δv ±91 px | Δh +0.0 px、Δv ±91 px |

---

## 5. 协议与配置

### 5.1 控制 socket

`$XDG_RUNTIME_DIR/niripaper.sock`。一行进，一行出：

```
set <output|*> <path>   切换一个输出的壁纸（或全部）
query [<output>]        屏幕上现在是什么
schema                  每个 UI 能提供的配置键，JSON
state [<output>]        守护进程现在在做什么，JSON
kill                    关闭守护进程
list                    库里所有壁纸包
thumbnail <id> [w h]    渲一帧 PNG
```

输出名放在 `set` 的第一位，于是含空格的路径不需要引号规则：第二个词之后的所有内容都是路径，原样。`query` / `state` 省略输出名表示"全部"，这正是面板想要的 —— 一次调用，每块屏。

守护进程拥有 socket 文件并在退出时 unlink：陈旧的 socket 比没有更糟，因为下一次启动会在 `bind` 上失败。

### 5.2 配置

`~/.config/niripaper/config.toml` + `config.d/*.toml`（按文件名排序深合并）。优先级：**CLI > 配置文件 > 内置默认值**。

**TOML 规则**：顶层键必须写在第一个 `[表]` 之前；写在表头之后会被当成那个表的字段并报 unknown field。碎片文件必须**自己写表头**（它是被合并的，不是文本拼接）。

`schema` 与 `state` 见 `niripaper schema` / `niripaper state` 的实际输出；`schema` 的 `per_output` 列出可以写在 `[outputs."NAME"]` 下的键，其余是全局键。
