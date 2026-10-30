# niripaper

本文件是 niripaper 的领域词汇表：只收录这个项目自己的词，不解释通用编程概念。读代码或写 issue 时用这里的词，不要漂到 `_Avoid_` 列出的同义词上；项目定义与架构见 [`docs/SPEC.md`](./docs/SPEC.md)。

## Language

### 布局

**output**:
niripaper 画的每一个显示输出，也是 CLI、配置与 socket 的寻址单位（`--output DP-1`）。多输出时由一个进程给每个输出起一条独立的渲染线程。
_Avoid_: 屏、显示器、monitor

**connector**:
output 在 DRM 里的名字（`DP-1`、`eDP-1`），niri 也用它标识输出。它只是名字不是设备：同一块屏换到别的 GPU 上时名字不变，卡号会变。
_Avoid_: GPU、device、显卡

**workspace**:
niri 的工作区，输出上纵向排列的单位；纵向视差按活动工作区移动。
_Avoid_: 桌面、tag

**column**:
niri 滚动布局里的一列，横向视差的移动单位。每个工作区各自记忆自己的焦点列（见 **记住的焦点列**）。
_Avoid_: 窗口、tab

**tile**:
列内部一个窗口的位置（niri 的 `tile` 坐标），同一列可以叠多个 tile。
_Avoid_: 窗口

**记住的焦点列**:
每个工作区**各自**记住的焦点列号；只在活动工作区上更新，列消失时解析到最近的现存列（nearest，不是 lower-bound）。它与“上一个已知的焦点工作区”是两个分开的字段，不能互相代替。
_Avoid_: 记忆列、最后焦点列、全局焦点列、每工作区共享一张表

### 画布与视差

**canvas**:
壁纸渲染用的那张图，尺寸是输出按 **scale** 放大后的结果（`ceil(output × scale)`）；源图解码后立刻被裁剪、缩放铺满它。
_Avoid_: 屏幕、surface、buffer

**scale**:
画布相对输出放大的倍数（默认 1.1，上限 1.35），同时也决定了视差能移动多远。
_Avoid_: zoom、放大率

**zoom**:
总览打开/关闭时**叠在 scale 上**的动画化乘数（后撤到 0.96）。它是乘法关系，所以画布、overflow、视差行程与纹理采样全部自动跟随，shader 不用改、纹理不用重传。
_Avoid_: scale

**overflow**:
画布比输出多出来的部分（`screen × (scale - 1)`），即视差可用的最大行程。
_Avoid_: 黑边

**parallax**:
布局反应式视差：画布随焦点列（横向）与活动工作区（纵向）移动。两个轴各自独立、各自用固定跨度。
_Avoid_: pan（`pan` / `video-pan-x` 是 mpv 时代的单位换算，新渲染器直接用像素偏移，不要照搬）

**progress**:
视差的归一化位置 `(horizontal, vertical)`，取值 0..1；画布的像素偏移由它算出。
_Avoid_: 百分比、索引

**column_span**:
横向视差的固定跨度，默认 6。它**不是**该工作区的实际列数——按实际列数归一化的话，开关任何一个窗口都会让壁纸移动。
_Avoid_: 列数、实际列数、span

**workspace_span**:
纵向视差的固定跨度，默认 6，理由与 `column_span` 完全相同。两者分开配置：它们数的是不同的东西（工作区内的列 vs 输出上的工作区），工作区多的人不该为此缩小列步长。
_Avoid_: 工作区数、span

### 换图过渡

**transition**:
换壁纸时的整段过渡，配置里自成 `[transition]` 段，不跟随 niri。它做的是**屏幕空间混合**：旧的一侧是冻结快照、新的一侧是活的，于是任意内容组合（静态图 ↔ 视频）都能过渡，任何时刻也只需一个解码器。
_Avoid_: 换图动画、crossfade

**effect**:
过渡的九选一形状：`none` / `fade` / `dissolve` / `wipe` / `stripes` / `iris` / `portal` / `zoom` / `slide`（`honeycomb` 已删除）。九个效果各自是一遍屏幕空间混合，最花哨的和最简单的同价。
_Avoid_: 特效、动画、honeycomb（已删除，编号已整体前移）

**selection**:
每次换图怎么挑 `effect`：`fixed`（固定一个）/ `rotate`（轮换，连续两次不重复）/ `random`（随机）。
_Avoid_: 模式、mode

**push**:
过渡期间画布被推开的比例，默认 0.55。
_Avoid_: 位移、偏移

**snapshot**:
换图过渡里旧的那一侧：**已渲染帧**的冻结副本，不是源帧。正因为它冻的是渲染结果，视差与总览 zoom 已经烘在里面，视频也能在过渡中照常解码而不卡住。
_Avoid_: 源帧、上一张图

### 放法

**fit**:
源与画布宽高比不一致时的放法：`fill` / `fit` / `stretch` / `center` / `tile`。名字借自 Wallpaper Engine，并**故意跳过** `span`；视频唯一做不到的是 `tile`（会退回 `fill`，`state` 报的是实际用的值而不是请求的值），换 `fit` 会重新装载媒体。
_Avoid_: span（Wallpaper Engine 有、这里不给）、cover / contain（CSS 的叫法）

### 放置与图层

**layer**:
niripaper 以 Wayland layer-shell 客户端的身份把自己的表面放进合成器的某一层（默认 Background），并自己拥有它的渲染循环。**backdrop 内的层序取决于创建顺序**——视差“失效”时先查是不是有别的层盖住。
_Avoid_: 窗口、surface

**namespace**:
layer-shell 表面报给合成器的身份（固定 `niripaper`），niri 的 layer rule 按它匹配。**不是配置键** —— 改了它，用户写的规则就匹配不上。用户不做任何配置时它是工作区背景，加规则命中才进 **backdrop**。
_Avoid_: 应用名、app_id

**backdrop**:
niri 的总览背景那一层；`place-within-backdrop` 把 background 层放进去，它在总览与工作区之间都可见且忽略输入，切工作区时壁纸也不跟着滑。
_Avoid_: 工作区背景（那是另一种放置，见 **工作区背景**）

**工作区背景**:
壁纸的默认放置（用户不加 layer rule 时）：壁纸层随工作区滚动一起平移，因此总览背景保持干净、壁纸出现在各工作区缩略图里。
_Avoid_: backdrop

### 合成与设备

**main device**:
合成器在 linux-dmabuf feedback 里指明的渲染设备——niri 在哪块 GPU 上合成，niripaper 就在哪块上开 GBM/EGL 并把缓冲做对。它**不是**“输出所在的 GPU”。
_Avoid_: 输出所在的 GPU（这条规则被项目明确否掉）

### 配置

**动画**:
一项可被驱动的量，三选一：`off`、缓动（`duration_ms` + `curve`）、或 `spring`（`damping_ratio` / `stiffness` / `epsilon`，质量固定 1）。语义与写法对齐 niri 的 `animations { }`；当前两项是 `animations.parallax` 与 `animations.overview-open-close`。
_Avoid_: 过渡（换图过渡自成一段 `[transition]`，不是 animation）

**follow_niri**:
`animations.follow_niri`（默认 true）：读 niri 自己的 `config.kdl`（含它的 `include`），只取用两者共享的 `overview-open-close` 与 `slowdown`；niri 的全局 `off` 只关掉总览过渡，不动视差。优先级是**本文件显式值 > niri 配置 > 自己的默认值**，来源写进启动日志。
_Avoid_: 镜像 niri 配置（只跟随明确共享的那几个键）

**意图 vs 镜像**:
意图 = 用户选了什么（GUI 写进配置文件的那些）；镜像 = 守护进程报出来的实际状态（`state` / `query`）。**只有意图写进配置**，把镜像写回去会让守护进程的输出变成它自己的输入，来回翻转。
_Avoid_: 把 `state` 的值写回配置、把"当前在画什么"当设置
