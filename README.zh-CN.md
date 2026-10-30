# niripaper

[![CI](https://github.com/harukizmoe/niripaper/actions/workflows/ci.yml/badge.svg)](https://github.com/harukizmoe/niripaper/actions/workflows/ci.yml)

[English](README.md) · **简体中文**

**会跟着你的布局一起动的壁纸。**

niripaper 是 [niri](https://github.com/niri-wm/niri) 合成器的视差壁纸守护进程。
在窗口之间、工作区之间移动时，它会把壁纸朝同一方向移动几十像素，
让壁纸保持在窗口背后，而不是定住不动。

它自己绘制壁纸，用一个 layer-shell 背景。总览过渡的时序取自 niri 自己的动画设置，
所以它和工作区是同步动的。

## 特性

- **视差。** 在窗口之间、工作区之间移动时，壁纸朝同一方向移动几十像素。
- **自己绘制壁纸。** 不需要 mpv，不需要 shell 脚本，也没有辅助进程：一个二进制，
  在 background 层放一个 layer-shell 表面，直接往里渲染。
- **视频壁纸。** mp4、webm、mkv、mov、m4v、avi、gif，**硬件解码**。
  （gif 按视频播，所以会动 —— Wallpaper Engine 也是这么处理的。）
  [mpv](https://mpv.io) 在这里是**库**：没有 mpv 进程、没有窗口、没有 IPC、不读它自己的配置；
  它把解码好的帧画进我们自己的纹理，于是视频和静态图一样，按视差偏移采样。
- **总览过渡跟随 niri。** 默认 `follow_niri = true`：它的弹簧或缓动曲线从 niri 的配置里读，
  所以壁纸和工作区是同步动的。
- **总览缩放。** 打开 niri 总览时，壁纸跟着一起缩小。
- **按输出分别配置。** 缩放、跨度、壁纸都可以每个输出不一样。
- **桌面应用。** 一个 Tauri 窗口，做壁纸守护进程不该做的事：浏览本地库或 Wallhaven、
  带着滤镜预览、选择显示器，以及改过渡、视差和轮播。它自己一个壁纸像素都不画 ——
  通过控制 socket 发 `set`/`next`，并就地改写配置文件。
- **资源占用低。** 没有东西在动时不出帧，也测不到 CPU 占用。

## 状态

版本 0.1.0，早期。

现在能用：静态图像（PNG / JPEG / WebP）、**视频壁纸**（硬件解码）、视差、总览过渡、
换图过渡（九个效果）、`fit`（源与画布宽高比不一致时的五种放法）、单一配置文件
与热重载、按输出配置、给前端用的 `schema`/`state`、给运行中的守护进程换壁纸，
以及一个浏览本地库与 Wallhaven、按显示器应用壁纸、编辑配置的桌面应用（`app/`）。

还没有：AUR 包。

## 环境要求

- **niri。** 开发时用的是 26.04。
- **libmpv** —— 视频那条路链接它。Arch 上就是 `mpv` 这个包。
- **Rust**（较新的 stable 工具链），如果你要自己编译。
- **桌面应用还需要：** WebKitGTK 4.1 的开发文件（Arch 上是 `webkit2gtk-4.1`），以及
  Node.js 与 npm。`ffmpeg` 是可选的 —— 没有它，本地视频壁纸就没有预览帧。

## 安装

从源码编译。需要先装 `libmpv`、`libEGL`、`libGL`、`libgbm`、`libwayland-client` ——
守护进程是**手写 FFI** 直接链接它们的，所以编译时要有对应的开发文件
（Arch 上是 `mpv`、`libglvnd`、`mesa`、`wayland`）：

```bash
git clone https://github.com/harukizmoe/niripaper.git
cd niripaper
cargo install --path . --locked --bin niripaper
```

**`--bin niripaper` 不能省**：不写它，`src/bin/` 下的诊断探针也会一起装进
`~/.cargo/bin`。

桌面应用用同一份检出构建。它的前端会被编译进 app 二进制，所以先构建前端，再构建外壳：

```bash
cd app/ui && npm ci && npm run build && cd ../..
cargo build -p niripaper-app --release
```

产物是 `target/release/niripaper-app`。crate 名就是这个；窗口和托盘图标显示的是 NiriPaper。
没有任何步骤会替你把它装到 `PATH` 上。

`packaging/aur/` 里有一份给 AUR 用的 `PKGBUILD`（**还没发布到 AUR** —— AUR 现在
不收新账号，见 `packaging/aur/README.md`）。

Release 里带**两个** `x86_64` 压缩包。它们按**守护进程链接的库**命名，不按构建它的发行版
命名 —— 因为真正有差别的只有这一件事：链接 `libmpv` 的守护进程被它的 soname 钉住。
`mpv --version` 报 0.35 或更新的 —— Arch、Fedora、Debian 12 及更新 —— 取 `-mpv0.41`；
报 0.34 的（Ubuntu 22.04）取 `-mpv0.34`。两者都要 glibc 2.34 或更新（两份构建的这个
数字相同）。

每个包里都有两个程序：`niripaper`（守护进程）与 `niripaper-app`（桌面应用）。应用在
运行时还需要 WebKitGTK 4.1 与 libayatana-appindicator；守护进程不需要，所以只跑守护进程
就不必装这两个库。每个包里的 `INSTALL.txt` 写出确切要求，且是 **从二进制里量出来的**
而不是写死的。从源码构建则在任何地方都可用。

## 用法

```bash
niripaper daemon                          # 绘制壁纸
niripaper daemon --wallpaper ~/wall.webp  # …指定图像
niripaper watch                           # 只打印视差目标值，不绘制
```

`watch` 是排查工具：niri 布局变化时它会打印目标位置，
有助于判断问题出在 niri 的事件上还是渲染上。

### 给运行中的守护进程换壁纸

守护进程监听一个小控制 socket，**所有输出共用一个**
（`$XDG_RUNTIME_DIR/niripaper.sock`）：

```bash
niripaper set ~/Pictures/wall.webp    # 换壁纸，图片或视频都行
niripaper next                        # 立刻切到轮播列表的下一张
niripaper query                       # 现在屏幕上是什么
niripaper schema                      # 全部配置项，JSON
niripaper state                       # 守护进程当前状态，JSON
niripaper kill                        # 让它退出
```

`set` 会**先加载成功再换** —— 路径写错就回报错误并保持原壁纸，而不是把屏幕搞空。

`set` 是**临时覆盖**：它只改屏幕上是什么，不改配置；任何一次配置文件的重载都会
把配置里的壁纸放回来。想留住一张壁纸，就把它写进配置。

`schema` 与 `state` 是给前端（面板，或桌面应用）用的。`schema` 列出每一个配置项
及其类型、取值范围、默认值、单位、以及改动后能否即时生效；`state` 用**同一套名字**报出生效值，
外加输出名、画布尺寸、**实际**在屏的壁纸与视差位置。前端启动时问一次 schema 来生成界面，
之后只需要 `state` —— 所以在这里加一个配置项，前端不用改。

`state` 里的 `config.values` 是**配置里写的**，而 `wallpaper.path` 是**实际在屏的**：
`set` 只换画面不动配置，所以两者本来就可能不一样。

### 桌面应用

`niripaper-app` 是图形前端。它是薄客户端：通过控制 socket 发 `set` 和 `next`，
并就地改写 `~/.config/niripaper/config.toml`（保留注释）；文件一变，守护进程就重载。

- **壁纸** —— 两个来源。本地库是 `$XDG_PICTURES_DIR/Wallpapers`（没有就用
  `~/Pictures/Wallpapers`），递归读取；卡片可以按类型（图片 / 视频）、按明暗（暗色 / 亮色）、
  按是否在轮播列表里筛选。明暗是从图片本身量的（平均感知亮度），按文件缓存 —— 之后新加的壁纸
  在第一次读取库时就会被分类。Wallhaven 是实时检索：关键词、分类、纯度、排序，以及一个按
  所选显示器的比例与分辨率生成的「匹配这块屏」筛选。结果是瀑布流，滚到接近末尾时一次再取
  两页（约 48 张）；缩略图缓存在 `~/.cache/niripaper/wallhaven/`，滑回去不会重新下载。
- **应用** —— 选显示器（或全部）和 `fit`，然后应用。Wallhaven 的图会先整张下载进本地库，
  所以应用它的同时也留下了它；同一份下载也可以单独请求而不应用；Wallhaven 的图还能直接加进轮播。
- **滤镜** —— 守护进程提供的预设加上逐项滑杆，拖动时预览同步变化。预览是 shader 的
  CSS 近似，不是逐像素一致。
- **轮播、过渡、视差** 各有一页；写的都是命令行读的同一批配置键。
- **设置** 包括守护进程连接、登录时自启、窗口配色（从当前壁纸取），以及两个列表行为：
  悬停时虚化其它卡片、只在滚到附近时加载预览图。

### 随 niri 启动

在 `~/.config/niri/config.kdl` 里加上：

```kdl
spawn-at-startup "niripaper" "daemon"
```

需要二进制在 `PATH` 上 —— 所以上面那行 `install` 是有必要的。

### 让壁纸进总览的 backdrop

不做任何额外配置时，壁纸是**工作区背景**：它随工作区一起滚动，总览里出现在各工作区的缩略图内。
想让它静止不动、铺满整个总览背景，就在 niri 的配置里加一条 layer rule ——
这是 niri 自己的机制，所有壁纸工具都这么用（`swaybg` 也一样）：

```kdl
layer-rule {
    match namespace="^niripaper$"
    place-within-backdrop true
}
```

它匹配的命名空间固定是 `niripaper`，**不是设置项** —— 改了它，这条规则就匹配不上了。
两种情况下的窗口模糊效果相同 —— 这条规则只改变总览显示什么、以及壁纸是否随工作区移动。

### 命令行参数

| 参数 | 含义 |
| --- | --- |
| `--output NAME` | 只在某一个输出上绘制（默认：niri 报告的所有输出） |
| `--config PATH` | 读取的配置文件（默认 `~/.config/niripaper/config.toml`） |
| `--namespace NAME` | layer-shell 命名空间（默认 `niripaper`；决定哪条 niri layer-rule 能匹配到） |
| `--wallpaper PATH` | 要绘制的图像 |
| `--scale F` | 画布放大倍数，`1.0`–`1.35` |
| `--column-span N`、`--workspace-span N` | 视差铺开多少级 |
| `--pattern blocks\|bands` | 内置测试图案，未指定壁纸时使用 |
| `--socket PATH` | 用哪个控制 socket（默认 `$XDG_RUNTIME_DIR/niripaper.sock`） |
| `--output NAME` | 这条请求针对哪个输出；`*`（默认）表示所有输出 |
| `--trace` | 逐帧打印（位置、缩放、耗时）—— 排查用 |

## 配置

配置文件位于 `~/.config/niripaper/config.toml`，它就是**全部**配置 —— 只有这一个文件。
它是可选的：完全没有这个文件时用内置默认值，此时画程序化测试图案。守护进程**只读**它；
写它的是桌面应用（原地改写、保留那些逐键说明的注释）。运行中改文件会即时生效；解析失败会报出原因并**继续用
运行的配置**。取值优先级是 **命令行 → 配置文件 → 内置默认值**，
所以文件里只需要写和默认不同的部分。

```toml
wallpaper = "~/Pictures/wall.webp"   # 图片或视频（按扩展名路由）
video_fps = 25                       # 视频帧率上限（0 = 跟着源帧率走）

scale = 1.1           # 画布放大倍数；越大，可移动的余地越大
column_span = 6       # 横向视差铺开多少列
workspace_span = 6    # 纵向视差铺开多少工作区

[animations]
follow_niri = true    # 总览过渡的设置取自 niri

[animations.parallax]
duration_ms = 600
curve = "ease-out-cubic"

[animations.overview-open-close]
zoom = 0.96           # 1.0 即关闭该效果

[transition]
selection = "rotate"                  # fixed | rotate | random
effects = ["portal", "iris", "dissolve"]
duration_ms = 1500
curve = "ease-out-cubic"
softness = 0.3                        # 0 是硬边，1 很软
on_start = true                       # 启动时也播一次

# 按输出覆盖 —— 这里没写的项都继承上面的全局值。
[outputs."DP-1"]
scale = 1.2
```

### 配置项

| 配置项 | 含义 |
| --- | --- |
| `wallpaper` | 要绘制的图像或视频（PNG/JPEG/WebP，或 mp4/webm/mkv/mov/m4v/avi/gif） |
| `video_fps` | 视频帧率上限（默认 `0`，即跟着源帧率走） |
| `scale` | 画布放大倍数（默认 `1.1`，上限 `1.35`） |
| `parallax` | 壁纸是否跟随焦点与工作区移动（默认 `true`）。设为 `false` 时壁纸居中不动，但 `scale` 仍然生效，总览缩放照常工作 |
| `column_span` | 横向视差的固定列跨度（默认 `6`，最小 `2`） |
| `workspace_span` | 纵向视差的固定工作区跨度（默认 `6`，最小 `2`） |
| `fit` | 源与画布宽高比不一致时怎么放：`fill`（默认）、`fit`、`stretch`、`center`、`tile` |
| `[filter]` | 后期滤镜，见下。每一项默认都是“无效果”，不改就没有开销 |
| `[playlist] enabled`、`interval_mins`、`order`、`items` | 按定时器在列表里轮换壁纸（默认关闭、`15` 分钟、`rotate`；`random` 不会重复当前这张） |
| `[animations] follow_niri` | 总览过渡的设置取自 niri（默认 `true`） |
| `[animations] slowdown` | 把所有动画的时间轴拉长 |
| `[animations.parallax]` | 移动时壁纸怎么动 |
| `[animations.overview-open-close]` | 总览过渡：一个 `zoom` 加一个弹簧或缓动 |
| `[transition] selection` | 每次怎么挑效果：`fixed`、`rotate` 或 `random` |
| `[transition] effect`、`effects` | `fixed` 时用的效果，以及 `rotate`/`random` 挑选的范围 |
| `[transition] duration_ms`、`curve` | 过渡多长、怎么缓动 |
| `[transition] cubic_bezier` | `curve = "cubic-bezier"` 的控制点，与 CSS 同义 |
| `[transition] softness` | 移动边缘多宽（`0` 硬边，`1` 很软） |
| `[transition] center` | 径向效果的起点：`[x, y]` 为屏幕比例，`[0, 0]` 是**左上角** |
| `[transition] direction` | wipe/stripes/slide 的方向 |
| `[transition] stripes` | `stripes` 把边缘拆成多少条带子 |
| `[transition] push` | `portal` 把旧画面往外推多远；`0` 就等于 `iris` |
| `[transition] start_radius` | 过渡开始时圆已经有的大（屏幕高度的比例） |
| `[transition] hold_ms` | 第一帧停住多久才开始动（算在 `duration_ms` 之内） |
| `[transition] allow_overshoot`、`on_start` | 允许曲线回弹；启动时也播一次 |
| `[outputs."NAME"]` | 按输出覆盖上面任意一项 |

动画用的是 niri 自己那套词汇：每一个动画要么是 `off`，要么是缓动
（`duration_ms` 加 `curve`），要么是弹簧（`damping_ratio`、`stiffness`、`epsilon`）。
`curve` 的取值也是 niri 那五个：`linear`、`ease-out-quad`、`ease-out-cubic`、
`ease-out-expo`、`cubic-bezier`。

换图过渡自成一段，不放进 `[animations]` —— 那张表镜像 niri 的词汇，而 niri 没有
换图动画可镜像。效果共九个：`portal`、`iris`、`dissolve`、`wipe`、`stripes`、`zoom`、`slide`、
`fade`、`none`。

所有效果都被同一个约束塑形：**旧的一侧是冻结的快照，新的一侧是活的** ——
任何时刻只有一个解码器在跑。所以"视频从一个扩张的圆盘里透出来、并且已经在动"
是能做到的；也是因此，每个效果无论看起来多华丽，代价都只是一个 fragment pass。
`softness` 加宽移动的边缘 —— 唯独 `slide` 例外，它的两帧正好相邻，接缝是硬边。

`iris` 与 `portal` 是同一个圆：`portal` 额外把旧画面向外推，所以观感是你**穿过去**，
而不是看着它被挖掉。差别只有这一处，`push` 就是推多远。

圆本身也可以调：`center` 是它从哪里开 —— `[x, y]` 为屏幕比例，**从左上角读起**，
和看截图的习惯一致 —— `start_radius` 是过渡开始时它已经有的大。留 `0` 就是从一点长起；
给个 `0.08` 左右就会一上来就看得见一个**小小的完整的圆**。
无论放在哪，它都会一直长到盖住离 `center` 最远的那个角。

`hold_ms` 让第一帧停住一会儿再动 —— 那是**停顿，不是放慢**：半径一动不动，
整段过渡会被切成两截。想让小圆仍然能被看清，更好的答案是**让曲线开头慢**：
圆一直在动，只是开头动得慢。真的想要一个"顿一下"的节拍时再用 `hold_ms`。
它算在 `duration_ms` 之内，不是额外加的。

niri 那五个缓动都是"开头快、结尾慢"，正好和一个"张开"想要的相反。想要
**先慢 → 中间快 → 最后慢**，用贝塞尔：`curve = "cubic-bezier"` 配
`cubic_bezier = [0.42, 0, 0.58, 1]`，也就是 CSS 的 `ease-in-out`
（解法和浏览器一样：解 `x(u) = t`）。

`fit` 决定壁纸宽高比和画布不一致时怎么办，**名字用的是 Wallpaper Engine 那一套**
（也就是 Windows 桌面背景的）：`fill`（默认）放大铺满、裁掉多余；`fit` 完整显示、其余
补黑边 —— 对电影画幅的源值得用，因为 2.35:1 的画面被 `fill` 裁掉会损失约四分之一宽度；
`stretch` 拉伸变形；`center` 完全不缩放，按原始像素居中；`tile` 按原始尺寸从左上角
平铺重复。（Windows 的 `span` 没做：跨显示器铺一张，对一个只画一个输出的守护进程
没有意义。）

名字是**故意**借来的：从 Wallpaper Engine 过来的人已经知道每个是什么，换个拼法就是
要人学两遍。`tile` 是视频唯一做不到的一种 —— mpv 把视频缩放进画框，不会重复它 ——
所以视频配 `tile` 会退回 `fill`，`state` 里的 `wallpaper.fit` 会报出实际生效的那个，
和配置里的值并排放着。

它**按输出**生效，而且**图片与视频走同一套**，所以壁纸不会因为"恰好是图片还是视频"
而变形状。

`niripaper schema` 会列出上面每一个配置项，连同类型、取值范围、以及运行中改文件
是否即时生效。

开着 `follow_niri = true` 时，总览过渡的参数取自 niri 的配置，
所以改 niri 的设置也会改变壁纸的运动。本文件里显式写的值优先于 niri 的值。

视频会**硬件解码**，并缩放到画布（输出尺寸 × `scale`）—— 从不按源分辨率出图。
`video_fps` 决定每秒呈现多少次：niri 给图层表面的帧回调是 60 Hz，所以 60 fps 的源
正好卡在那个临界点上。本机实测 25 足够顺滑、不丢帧；换更快的硬件值得往上调。

### 滤镜

`[filter]`（单个输出用 `[outputs."NAME".filter]`）在 GPU 上调整画面，图片与视频通用。
颜色相关的运算在线性光里进行。

| 配置项 | 范围 | 默认 | 含义 |
| --- | --- | --- | --- |
| `brightness` | `0.5`–`1.5` | `1.0` | 曝光 |
| `contrast` | `0.5`–`1.6` | `1.0` | S 曲线对比度 |
| `saturation` | `0`–`2` | `1.0` | 颜色浓度 |
| `vibrance` | `0`–`2` | `1.0` | 类似饱和度，但平淡的颜色提得多、鲜艳的提得少 |
| `temperature` | `-1`–`1` | `0` | 偏冷到偏暖；`0` 不改变颜色 |
| `hue` | `0`–`360` | `0` | 所有色相整体旋转，单位度 |
| `vignette` | `0`–`1` | `0` | 压暗四角 |
| `grain`、`grain_size` | `0`–`1`、`1`–`4` px | `0`、`1.5` | 胶片颗粒，中间调最明显 |
| `aberration` | `0`–`1` | `0` | 靠近边缘的色散 |
| `tone_amount`、`tone_shadow`、`tone_highlight` | `0`–`1`、色相（度） | `0` | 分离色调：给暗部和亮部各染一点色，不改变亮度 |

桌面应用里有若干预设（电影质感、黑白胶片、赛博霓虹、暖阳日落、动漫清透、复古胶片、
暗夜护眼）。它们只是这些配置项的取值，定义在 `crates/proto` 的 `Filter::presets()`。
`trprobe NODE OUT WALLPAPER` 会把每个预设渲染到一张图上，不用占用屏幕就能看效果。

## 排查

- **什么都不显示。** 输出名必须和 niri 的一致：用 `niri msg outputs` 查，然后传 `--output NAME`。
- **启动后立刻退出。** 配置文件解析失败 —— 报错会指明出问题的键和行号。
- **壁纸不动。** 如果 niri 配置里全局关掉了动画（`animations { off }`），
  总览过渡在这里也是关的。视差不受影响 —— 它不是 niri 的动画之一。
- **你的图层规则匹配的是别的名字。** 图层规则按 layer-shell 命名空间匹配，
  而它是固定的 `niripaper`（不是配置键 —— 改了它，你写的那条规则就匹配不上了）。
  一次性运行可以用命令行覆盖：`--namespace NAME`。

## 许可证

GPL-3.0-or-later © 2026 harukizmoe。见 [LICENSE](LICENSE)。

## 致谢

- **[niri](https://github.com/niri-wm/niri)** —— 本项目的目标合成器，
  也是它跟随的事件流的来源。
