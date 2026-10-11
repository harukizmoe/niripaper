# 一个进程管所有输出：每个输出一个线程

多输出由**一个** daemon 进程负责（目标形态是 Wallpaper Engine 那样），实现方式是每个输出一个线程：`daemon::run` 作为监督者枚举输出（`--output` 指定一个，否则 niri 报告的全部）、起线程、独占 socket、路由请求。没有把单输出循环改造成 `Vec<Output>`——原循环本就按 `options.output` 参数化，所以线程间不共享任何东西（各自的 Wayland 连接、EGL 上下文、解码器、动画器），唯一共享的是"从监督者收命令"的那条 channel；这样既拿到单进程的好处（两屏同一张壁纸时只解码一次），又把爆炸半径和状态都压在各自的线程里。

## Considered Options

- **把单输出循环改造成 `Vec<Output>`**：否决——状态共享面变大，一处出错波及所有输出。
- **每个输出一个线程 + 共享 command channel**：采用。

## Consequences

监督者独占控制 socket、按输出路由命令，控制面因此从"每个输出一个 daemon"收敛为单一 socket（见 [单一控制 socket](0008-single-control-socket.md)）。
