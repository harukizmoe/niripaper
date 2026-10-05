# niripaper

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
- **视频壁纸。** mp4、webm、mkv、mov、m4v、avi，**硬件解码**。
  [mpv](https://mpv.io) 在这里是**库**：没有 mpv 进程、没有窗口、没有 IPC、不读它自己的配置；
  它把解码好的帧画进我们自己的纹理，于是视频和静态图一样，按视差偏移采样。
- **总览过渡跟随 niri。** 默认 `follow_niri = true`：它的弹簧或缓动曲线从 niri 的配置里读，
  所以壁纸和工作区是同步动的。
- **总览缩放。** 打开 niri 总览时，壁纸跟着一起缩小。
- **按输出分别配置。** 缩放、跨度、壁纸都可以每个输出不一样。
- **资源占用低。** 没有东西在动时不出帧，也测不到 CPU 占用。

## 状态

版本 0.1.0，早期。

现在能用：静态图像（PNG / JPEG / WebP）、**视频壁纸**（硬件解码）、视差、总览过渡、
配置文件、按输出配置，以及给运行中的守护进程换壁纸。

还没有：换图时的交叉淡入。

## 环境要求

- **niri。** 开发时用的是 26.04。
- **libmpv** —— 视频那条路链接它。Arch 上就是 `mpv` 这个包。
- **Rust**（较新的 stable 工具链），如果你要自己编译。

## 安装

从源码编译：

```bash
git clone https://github.com/harukizmoe/niripaper.git
cd niripaper
cargo build --release
install -Dm755 target/release/niripaper ~/.local/bin/niripaper
```

## 用法

```bash
niripaper daemon                          # 绘制壁纸
niripaper daemon --wallpaper ~/wall.webp  # …指定图像
niripaper watch                           # 只打印视差目标值，不绘制
```

`watch` 是排查工具：niri 布局变化时它会打印目标位置，
有助于判断问题出在 niri 的事件上还是渲染上。

### 给运行中的守护进程换壁纸

守护进程监听一个小控制 socket，**每个输出一个**
（`$XDG_RUNTIME_DIR/niripaper-DP-1.sock`）：

```bash
niripaper set ~/Pictures/wall.webp    # 换壁纸，图片或视频都行
niripaper query                       # 现在屏幕上是什么
niripaper schema                      # 全部配置项，JSON
niripaper state                       # 守护进程当前状态，JSON
niripaper kill                        # 让它退出
```

`set` 会**先加载成功再换** —— 路径写错就回报错误并保持原壁纸，而不是把屏幕搞空。

`schema` 与 `state` 是给面板（Noctalia 插件，或别的什么）用的。`schema` 列出每一个配置项
及其类型、取值范围、默认值、单位、以及改动后能否即时生效；`state` 用**同一套名字**报出生效值，
外加输出名、画布尺寸、**实际**在屏的壁纸与视差位置。面板启动时问一次 schema 来生成界面，
之后只需要 `state` —— 所以在这里加一个配置项，面板不用改。

`state` 里的 `config.values` 是**配置里写的**，而 `wallpaper.path` 是**实际在屏的**：
`set` 只换画面不动配置，所以两者本来就可能不一样。

### 随 niri 启动

在 `~/.config/niri/config.kdl` 里加上：

```kdl
spawn-at-startup "niripaper" "daemon"
```

需要二进制在 `PATH` 上 —— 所以上面那行 `install` 是有必要的。

### 命令行参数

| 参数 | 含义 |
| --- | --- |
| `--output NAME` | 在哪个输出上绘制（默认：niri 报告的第一个） |
| `--config PATH` | 读取的配置文件（默认 `~/.config/niripaper/config.toml`） |
| `--namespace NAME` | layer-shell 命名空间（默认 `niripaper`） |
| `--wallpaper PATH` | 要绘制的图像 |
| `--scale F` | 画布放大倍数，`1.0`–`1.35` |
| `--column-span N`、`--workspace-span N` | 视差铺开多少级 |
| `--pattern blocks\|bands` | 内置测试图案，未指定壁纸时使用 |
| `--socket PATH` | 用哪个控制 socket（默认 `$XDG_RUNTIME_DIR/niripaper-<输出名>.sock`） |
| `--output NAME` | 找哪个输出的守护进程；只有多个在跑时才需要 |
| `--trace` | 逐帧打印（位置、缩放、耗时）—— 排查用 |

## 配置

配置文件位于 `~/.config/niripaper/config.toml`。它是可选的：完全没有这个文件时，
用内置默认值。它旁边的 `config.d/` 目录也会被读取 —— 里面每个 `*.toml` 按文件名
顺序合并到主配置之上（靠后的覆盖靠前的）。工具把自己的设置放在这里，不用动你的
配置：删掉文件即还原。**碎片文件要自己写表头**（`[transition]` 之类）——
它是被合并的，不是文本拼接；键写错层级就是未知键，会直接报错。运行中改文件会即时生效；解析失败会报出原因并**继续用正在
运行的配置**。取值优先级是 **命令行 → 配置文件 → 内置默认值**，
所以文件里只需要写和默认不同的部分。

```toml
wallpaper = "~/Pictures/wall.webp"   # 图片或视频（按扩展名路由）
video_fps = 25                       # 视频帧率上限（0 = 跟着源帧率走）

scale = 1.1           # 画布放大倍数；越大，可移动的余地越大
column_span = 6       # 横向视差铺开多少列
workspace_span = 6    # 纵向视差铺开多少工作区
namespace = "niripaper"

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
| `wallpaper` | 要绘制的图像或视频（PNG/JPEG/WebP，或 mp4/webm/mkv/mov/m4v/avi） |
| `video_fps` | 视频帧率上限（默认 `0`，即跟着源帧率走） |
| `scale` | 画布放大倍数（默认 `1.1`，上限 `1.35`） |
| `column_span` | 横向视差的固定列跨度（默认 `6`，最小 `2`） |
| `workspace_span` | 纵向视差的固定工作区跨度（默认 `6`，最小 `2`） |
| `namespace` | layer-shell 命名空间（默认 `niripaper`） |
| `[animations] follow_niri` | 总览过渡的设置取自 niri（默认 `true`） |
| `[animations] slowdown` | 把所有动画的时间轴拉长 |
| `[animations.parallax]` | 移动时壁纸怎么动 |
| `[animations.overview-open-close]` | 总览过渡：一个 `zoom` 加一个弹簧或缓动 |
| `[transition] selection` | 每次怎么挑效果：`fixed`、`rotate` 或 `random` |
| `[transition] effect`、`effects` | `fixed` 时用的效果，以及 `rotate`/`random` 挑选的范围 |
| `[transition] duration_ms`、`curve` | 过渡多长、怎么缓动 |
| `[transition] softness` | 移动边缘多宽（`0` 硬边，`1` 很软） |
| `[transition] center` | 径向效果的起点：`[x, y]` 为屏幕比例，`[0, 0]` 是**左上角** |
| `[transition] direction` | wipe/stripes/slide 的方向 |
| `[transition] stripes`、`cell` | `stripes` 的条数；`honeycomb` 的六边形大小 |
| `[transition] push` | `portal` 把旧画面往外推多远；`0` 就等于 `iris` |
| `[transition] start_radius` | 过渡开始时圆已经有的大（屏幕高度的比例） |
| `[transition] allow_overshoot`、`on_start` | 允许曲线回弹；启动时也播一次 |
| `[outputs."NAME"]` | 按输出覆盖上面任意一项 |

动画用的是 niri 自己那套词汇：每一个动画要么是 `off`，要么是缓动
（`duration_ms` 加 `curve`），要么是弹簧（`damping_ratio`、`stiffness`、`epsilon`）。
`curve` 的取值也是 niri 那五个：`linear`、`ease-out-quad`、`ease-out-cubic`、
`ease-out-expo`、`cubic-bezier`。

换图过渡自成一段，不放进 `[animations]` —— 那张表镜像 niri 的词汇，而 niri 没有
换图动画可镜像。效果共十个：`portal`、`iris`、`dissolve`、`wipe`、`stripes`、
`honeycomb`、`zoom`、`slide`、`fade`、`none`。

所有效果都被同一个约束塑形：**旧的一侧是冻结的快照，新的一侧是活的** ——
任何时刻只有一个解码器在跑。所以"视频从一个扩张的圆盘里透出来、并且已经在动"
是能做到的；也是因此，每个效果无论看起来多华丽，代价都只是一个 fragment pass。
`softness` 加宽移动的边缘 —— 唯独 `slide` 例外，它的两帧正好相邻，接缝是硬边。

`iris` 与 `portal` 是同一个圆：`portal` 额外把旧画面向外推，所以观感是你**穿过去**，
而不是看着它被挖掉。差别只有这一处，`push` 就是推多远。

圆本身也可以调：`center` 是它从哪里开 —— `[x, y]` 为屏幕比例，**从左上角读起**，
和看截图的习惯一致 —— `start_radius` 是过渡开始时它已经有的大。留 `0` 就是从一点长起；
给个 `0.25` 左右就会**一上来就看得见一个完整的圆**，形状从第一帧就认得出来。
无论放在哪，它都会一直长到盖住离 `center` 最远的那个角。

`niripaper schema` 会列出上面每一个配置项，连同类型、取值范围、以及运行中改文件
是否即时生效。

开着 `follow_niri = true` 时，总览过渡的参数取自 niri 的配置，
所以改 niri 的设置也会改变壁纸的运动。本文件里显式写的值优先于 niri 的值。

视频会**硬件解码**，并缩放到画布（输出尺寸 × `scale`）—— 从不按源分辨率出图。
`video_fps` 决定每秒呈现多少次：niri 给图层表面的帧回调是 60 Hz，所以 60 fps 的源
正好卡在那个临界点上。本机实测 25 足够顺滑、不丢帧；换更快的硬件值得往上调。

## 排查

- **什么都不显示。** 输出名必须和 niri 的一致：用 `niri msg outputs` 查，然后传 `--output NAME`。
- **启动后立刻退出。** 配置文件解析失败 —— 报错会指明出问题的键和行号。
- **壁纸不动。** 如果 niri 配置里全局关掉了动画（`animations { off }`），
  总览过渡在这里也是关的。视差不受影响 —— 它不是 niri 的动画之一。
- **你的图层规则匹配的是别的名字。** 图层规则是按命名空间匹配的，
  把 `namespace` 设成你现有规则已经在用的名字即可。

## 许可证

GPL-3.0-or-later © 2026 harukizmoe。见 [LICENSE](LICENSE)。

## 致谢

- **[niri](https://github.com/niri-wm/niri)** —— 本项目的目标合成器，
  也是它跟随的事件流的来源。
