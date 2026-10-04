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
- **总览过渡跟随 niri。** 默认 `follow_niri = true`：它的弹簧或缓动曲线从 niri 的配置里读，
  所以壁纸和工作区是同步动的。
- **总览缩放。** 打开 niri 总览时，壁纸跟着一起缩小。
- **按输出分别配置。** 缩放、跨度、壁纸都可以每个输出不一样。
- **资源占用低。** 没有东西在动时不出帧，也测不到 CPU 占用。

## 状态

版本 0.1.0，早期。

现在能用：静态图像（PNG / JPEG / WebP）、视差、总览过渡、配置文件、按输出配置。

还没有：视频壁纸，以及换图时的交叉淡入。

## 环境要求

- **niri。** 开发时用的是 26.04。
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
| `--trace` | 逐帧打印（位置、缩放、耗时）—— 排查用 |

## 配置

配置文件位于 `~/.config/niripaper/config.toml`。它是可选的：完全没有这个文件时，
用内置默认值。取值优先级是 **命令行 → 配置文件 → 内置默认值**，
所以文件里只需要写和默认不同的部分。

```toml
wallpaper = "~/Pictures/wall.webp"

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

# 按输出覆盖 —— 这里没写的项都继承上面的全局值。
[outputs."DP-1"]
scale = 1.2
```

### 配置项

| 配置项 | 含义 |
| --- | --- |
| `wallpaper` | 要绘制的图像（PNG / JPEG / WebP） |
| `scale` | 画布放大倍数（默认 `1.1`，上限 `1.35`） |
| `column_span` | 横向视差的固定列跨度（默认 `6`，最小 `2`） |
| `workspace_span` | 纵向视差的固定工作区跨度（默认 `6`，最小 `2`） |
| `namespace` | layer-shell 命名空间（默认 `niripaper`） |
| `[animations] follow_niri` | 总览过渡的设置取自 niri（默认 `true`） |
| `[animations] slowdown` | 把所有动画的时间轴拉长 |
| `[animations.parallax]` | 移动时壁纸怎么动 |
| `[animations.overview-open-close]` | 总览过渡：一个 `zoom` 加一个弹簧或缓动 |
| `[outputs."NAME"]` | 按输出覆盖上面任意一项 |

动画用的是 niri 自己那套词汇：每一个动画要么是 `off`，要么是缓动
（`duration_ms` 加 `curve`），要么是弹簧（`damping_ratio`、`stiffness`、`epsilon`）。
`curve` 的取值也是 niri 那五个：`linear`、`ease-out-quad`、`ease-out-cubic`、
`ease-out-expo`、`cubic-bezier`。

开着 `follow_niri = true` 时，总览过渡的参数取自 niri 的配置，
所以改 niri 的设置也会改变壁纸的运动。本文件里显式写的值优先于 niri 的值。

## 排查

- **什么都不显示。** 输出名必须和 niri 的一致：用 `niri msg outputs` 查，然后传 `--output NAME`。
- **启动后立刻退出。** 配置文件解析失败 —— 报错会指明出问题的键和行号。
- **壁纸不动。** 如果 niri 配置里全局关掉了动画（`animations { off }`），
  总览过渡在这里也是关的。视差不受影响 —— 它不是 niri 的动画之一。
- **你已经为 `mpvpaper` 写过图层规则。** 把 `namespace = "mpvpaper"` 设上，
  现有规则就直接适用于这个守护进程。

## 许可证

GPL-3.0-or-later © 2026 harukizmoe。见 [LICENSE](LICENSE)。

## 致谢

- **[niri](https://github.com/niri-wm/niri)** —— 本项目的目标合成器，
  也是它跟随的事件流的来源。
- **[mpvpaper](https://github.com/GhostNaN/mpvpaper)** —— `mpvpaper`
  这个 layer-shell 命名空间约定来自它。
