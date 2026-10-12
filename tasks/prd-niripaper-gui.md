# PRD: niripaper GUI —— 独立桌面壁纸应用

> ⚠️ **本 PRD 部分内容已被推翻，动手前先读 [decisions.md](./decisions.md)。**
>
> 2026-10-12 的讨论改动了几处根本前提：
>
> | 本文件里 | 现在是 |
> | --- | --- |
> | 「非目标」写不做自启管理、不做托盘 | **两者都做**；自启动取代 niri `spawn-at-startup`，GUI 静默启动到托盘 |
> | FR-6 / US-003：写 `config.d/niripaper-gui.toml` | **单一 `config.toml`**，只有 GUI 写（`toml_edit` + 原子替换）；`config.d/` 作废 |
> | US-008 / US-011 / FR-11 / FR-12：壁纸包 `manifest.toml` + 包参数 | **不做包**；库 = 一个目录里的媒体文件 + `list` + `thumbnail` |
> | `wallpaper` 是单张 | **列表**（轮播）+ `interval` + `order`，三者都在 `per_output` |
> | FR-7：`hot: false` 的键要标注 | **没有这种键了**（`namespace` 已从 schema 移除，它是唯一一个） |
> | 未提暂停 | **被窗口完全遮住时暂停整个渲染 + 解码** |
>
> 仍待定：v1 做设置页还是库网格（推荐①→②→③，见 decisions.md）、应用住哪、自启动通路、托盘职责与实现、守护进程不在时能否改配置、缩略图缓存、应用的名字。
>
> 本文件保留的价值：用户故事与验收标准的**写法**（可测、具体、带反例回归）。
> 落点变了的故事，以 `decisions.md` 为准。

## 概述

niripaper 已经有了完整的**引擎**：守护进程自己拥有壁纸图层与渲染循环，支持静态图与视频、布局反应式视差、换图过渡，并且把配置做成了机器可读的（`niripaper schema` / `niripaper state`）。它缺的只有**前端**：今天要换一张壁纸，用户得手动编辑 `~/.config/niripaper/config.toml`，或者敲 `niripaper set <path>` —— 没有缩略图、不知道有哪些图可选、也不知道当前屏幕上到底是什么。

niripaper 唯一存在过的前端是 Noctalia 插件，**该插件已废弃**，不会复活（见「非目标」）。本 PRD 描述它的替代品：

一个 **Wallpaper-Engine 式桌面应用**，以 niri 为第一目标：壁纸库网格、把壁纸应用到某个输出、属性面板、设置页。工具链是 **`gpui-kit`**（Rust，基于 GPUI），已在本机（niri + Wayland + NVIDIA）验证可跑。

一条决定性的架构前提：**渲染留在守护进程里，应用是守护进程控制 socket 上的薄客户端**。应用不解码媒体、不碰 GL/EGL/GBM、不自己画壁纸像素 —— 它只读 `schema` / `state` / `query`，只写**意图**（要哪个壁纸、参数是什么）。

- 系统契约（socket 协议、配置格式与合并规则、运动规格）：[`docs/SPEC.md`](../docs/SPEC.md)
- 领域词汇（canvas、overflow、transition、snapshot、output、layer…）：[`CONTEXT.md`](../CONTEXT.md)
- 已知失败模式：[`docs/gotchas.md`](../docs/gotchas.md)
- 验证方法：[`docs/verification.md`](../docs/verification.md)
- 仓库开发约定（构建与自检、本机硬性约束）：[`AGENTS.md`](../AGENTS.md)

## 目标

- 让"换壁纸"不再需要编辑任何文件：从库里选一张 → 应用到某个输出（或全部输出）→ 当场生效（有换图过渡）。
- 让"有哪些壁纸可选"可见：库网格 + 缩略图，缩略图**由守护进程**渲染（它已经有 libmpv 与 GL）。
- 让"现在到底是什么状态"可见且可信：`state` 的**实际生效值**、来源文件、屏幕上实际的 `wallpaper` / `kind` / `hwdec` / `fit`、`position` 与 `zoom`。
- 让全部 **28** 个配置键都能在图形界面里改：界面由 `schema` 驱动，不硬编码键名、取值与范围。
- 让"写配置"这件事**在结构上不可能**触发守护进程自激：应用只写意图，永不把 `state` 报出来的值写回配置。
- 保持引擎语义不变：视差、总览过渡、换图过渡、`fit`、按输出覆盖全部**原样暴露**，不新增行为、不新增配置键。

## 使用者与场景

| 使用者 | 场景 |
| --- | --- |
| niri 用户（主要） | 装了 niripaper，想看壁纸都有哪些、点一下就换掉，不想再开编辑器 |
| 多输出用户 | DP-1 用这张、eDP-1 用那张；或者一次性把所有输出换掉 |
| 壁纸收藏者 | `~/Pictures/Wallpapers` 下有几十上百张图，需要缩略图、名字与标记来挑 |
| 调参用户 | 想改 `scale` / `column_span` / `workspace_span` / 过渡效果，又不想记键名与取值范围 |
| 排障的维护者 | 想知道"守护进程现在认为配置来自哪几个文件、实际在画什么"，而不是靠看日志猜 |

## 用户故事

### US-001: 应用外壳与守护进程连接

**描述：** 作为 niri 用户，我打开这个应用就能知道守护进程在不在跑，以及它管着哪些输出。

**验收标准：**
- [ ] 应用是一个 `gpui-kit` 桌面应用：窗口能打开，能渲染一个空的库视图（无壁纸时不崩）。
- [ ] 启动时按 `ipc::find_path()` 的既有规矩找 socket：只找唯一那份 `$XDG_RUNTIME_DIR/niripaper.sock`（一个进程管所有输出，socket 只有一份）；文件不存在 → 显示"守护进程没在运行"，并支持 `--socket PATH` 覆盖。
- [ ] 守护进程没在跑时：界面显示明确的空状态与原因（socket 路径 + "is the daemon running?" 的同类文案），**不是** panic、不是空白窗口。
- [ ] 连上以后，能从 `state` 得到输出清单，并在界面上列出来。
- [ ] 选择 `--output` 只影响请求的寻址，不影响守护进程本身（一个进程管所有输出，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.10）。
- [ ] 守护进程中途退出/重启：界面在 **5 秒内**变成"未连接"状态，恢复后能自动重连，不需要重启应用。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍并截图（守护进程开/关两种状态各一张）。

### US-002: `schema` 驱动的设置页

**描述：** 作为调参用户，我要在一个界面里改所有配置键，而且键名、取值范围、单位都由守护进程告诉我。

**验收标准：**
- [ ] 设置页的每一项都来自 `niripaper schema` 的 `keys`（当前 **28** 个键），代码里**没有**任何键名字面量、取值范围或选项列表。
- [ ] `Kind` 的 **9** 个变体各有对应控件：`bool` → 开关；`int` / `float` → 数字输入（带 `min` / `max` 限制，`max: null` 表示无上限）；`path` → 路径选择；`text` → 文本；`enum` → 下拉；`multi_enum` → 多选；`numbers` → `count` 个数字；`animation` → 三选一（`off` / 缓动 `duration_ms` + `curve` / `spring`）。
- [ ] 每个控件显示该键的 `about` 作为提示、在数值非平凡时显示 `unit`。
- [ ] 遇到不认识的 `type`：**跳过该键并显示一行说明**，不猜控件、不崩溃（`schema` 不版本化，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.22.1）。
- [ ] `hot: false` 的键（当前只有 `namespace`）在界面上明确标注"需要重启守护进程"，且不会被当成改完即可见。
- [ ] `enum` / `multi_enum` / `animation.curves` 的选项直接取 `schema` 给的列表（`transition.effect` 的 9 个效果、niri 的 5 条曲线等），不写死。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍并截图（含一个 `animation` 控件与一个 `enum` 控件）。

### US-003: 写意图到 GUI 独占的 drop-in 文件

**描述：** 作为用户，我改完设置后，守护进程当场按新值工作。

**验收标准：**
- [ ] 设置写入**应用独占的**一份 drop-in：`~/.config/niripaper/config.d/niripaper-gui.toml`（文件名 = 应用的二进制名，与它独占这一份文件互为保障）。不修改 `config.toml`，不改别人的 drop-in（如 `noctalia.toml`）。
- [ ] 写出的文件符合配置合并规则：碎片文件**自己写表头**；顶层键写在第一个 `[表]` 之前（见 [`docs/SPEC.md`](../docs/SPEC.md) §5.2）。
- [ ] 改一个值后，守护进程热重载并生效：`state.config.values` 里该键变成新值，**不需要重启守护进程**。
- [ ] 写入的只有**意图**（用户选了什么），文件内容里不含任何来自 `state` / `query` / 守护进程日志的"当前值"。
- [ ] **回归（自激振荡）：** 界面开着、用户不动任何东西时，连续 10 秒内对该生成文件算哈希，**0 次变化**；相同条件下守护进程日志里 `config reloaded` 出现 **0 次**。
- [ ] 代码里不存在"读 `state` → 生成配置内容"的路径（`state` 只用于显示与校验）。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍：改一个值 → 观察 `niripaper state` 立即反映，并截图。

### US-004: 按 `per_output` 白名单路由写入

**描述：** 作为多输出用户，我要能只给一块屏设壁纸与参数，其余输出不受影响。

**验收标准：**
- [ ] 应用在启动时读 `schema.per_output`（当前 **5** 个键：`wallpaper` / `scale` / `column_span` / `workspace_span` / `fit`），并以它决定每个键写到哪里。
- [ ] 这 5 个键之外的任何键（如 `transition.push`、`animations.slowdown`）**永不**出现在 `[outputs."NAME"]` 表下，一律写在文件顶层。
- [ ] `[outputs."NAME"]` 表里的键名与 `state` 报出的输出名逐字一致；界面上的输出列表**只能来自** `state`，不能让用户手打一个不存在的名字。
- [ ] 反例回归：手工构造一份"把全局键写进 `[outputs."NAME"]`"的文件时，守护进程会**拒绝整个文件**（保留旧配置并报错）——应用必须不产生这种文件；至少一条测试钉住这条路由规则。
- [ ] 只给一块屏设了 `wallpaper` 时，另一块屏的壁纸不变（`state` 两块屏的值不同）。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍：只改一块输出的 `scale`，确认另一块的值不变，并截图。

### US-005: 只读状态视图（实际生效值与来源）

**描述：** 作为用户/维护者，我要一眼看到守护进程**实际**在用什么值，以及这个值是从哪来的。

**验收标准：**
- [ ] 展示 `state.config.values`（实际生效值）、`state.config.source`、`state.config.files`（这份配置由哪几个文件合成）、`state.config.from_niri`。
- [ ] 当"配置里的值"与"实际生效值"不一致时，界面必须显示**实际生效值**并说明来源（本文件显式值 > niri 配置 > 内置默认值，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.18.1）。
- [ ] 展示屏幕上实际的壁纸：`wallpaper.path`、`wallpaper.kind`、`wallpaper.hwdec`、`wallpaper.fit`（**实际**用的值，例如视频 + `tile` 会报 `fill`，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.20.1）。
- [ ] 展示 `position.horizontal` / `position.vertical` / `zoom` / `canvas`。
- [ ] 这个视图**没有**任何回写路径：在它上面点任何东西都不会改配置或调用 `set`。
- [ ] 值来自守护进程，不是本地缓存的旧值：守护进程改值后，视图在 **2 秒内**跟上。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍并截图（至少包含 `hwdec` 与 `from_niri` 两个字段）。

### US-006: 守护进程新增 `list` 动词

**描述：** 作为应用，我需要一份机器可读的"库里有哪些壁纸包"。

**验收标准：**
- [ ] 控制 socket 接受 `list`，返回**一行** JSON（协议一行进一行出，见 [`docs/SPEC.md`](../docs/SPEC.md) §5.1）：每个包至少含 `id`、`name`、`type`、`tags`、目录路径。
- [ ] `id` 稳定、无空格（协议按空格切词），可由 `thumbnail <id>` 直接引用。
- [ ] 守护进程不在库目录时返回空列表而不是报错；库目录不存在不算失败。
- [ ] `schema` / `state` / `set` / `query` / `kill` 的行为逐字不变（现有 CLI 与测试全绿）。
- [ ] CLI 侧可读：`niripaper list` 打印缩进后的 JSON（与 `schema` / `state` 的 CLI 处理一致）。
- [ ] `cargo test` 全绿；`cargo clippy --all-targets` 0 警告。
- [ ] 对比 `niripaper list` 的输出与库目录的实际内容，两者逐项一致。

### US-007: 守护进程新增 `thumbnail <id> [w h]` 动词

**描述：** 作为应用，我要缩略图，但我不想在自己的进程里解码媒体或碰 GL。

**验收标准：**
- [ ] 控制 socket 接受 `thumbnail <id> [w h]`，渲染**一帧**并写出 PNG，返回**一行**（PNG 路径或明确错误）。
- [ ] 缩略图由守护进程渲染：它已经有 libmpv 与 GL，且已经按合成器反馈钉好了 GPU 与 EGL vendor（一个进程只能钉一个 vendor，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.7）。应用进程**不**链接 libmpv、不做图片解码、不建 EGL/GL 上下文。
- [ ] 指定 `w h` 时输出尺寸就是它；省略时输出默认尺寸（并把这个默认值写进 `docs/SPEC.md` §5.1）。
- [ ] 生成缩略图**不打扰屏幕上的壁纸**：同一时刻正在跑的视差/换图过渡不中断，`state` 里屏幕上壁纸的 `path` 不变。
- [ ] 未知 `id`、非法 `w h` 返回明确错误（不是 panic、不是空 PNG）。
- [ ] 连续请求同一个 `id` 两次不泄漏：文件句柄与显存量级不增长（按 [`docs/verification.md`](../docs/verification.md) 的 PSS 方法测）。
- [ ] `cargo test` 全绿；`cargo clippy --all-targets` 0 警告。
- [ ] 本机实跑：对库里一个视频包与一个图片包各出一张 PNG，人工看一眼并记录尺寸。

### US-008: 壁纸包格式（`manifest.toml`）

**描述：** 作为壁纸收藏者，我要把一个文件夹放进库就算一个"包"，里面自带名字、标记和参数。

**验收标准：**
- [ ] 一个包 = 一个目录，内含 `manifest.toml` + 资源文件。
- [ ] `manifest.toml` 至少有这四个键：`type`、`name`、`tags`、`params`。
- [ ] **`params` 的形状与 `schema::Key` 一致**：因此属性面板能用**同一条代码路径**渲染内置键与包参数（同一套 `Kind` 到控件的映射）。
- [ ] 缺 `manifest.toml`、`manifest.toml` 无法解析、`params` 形状不对的目录：**跳过并显示原因**，不影响其他包，不使 `list` 整体失败。
- [ ] `type` 未知（例如声明了一个本项目不支持的壁纸类型）：跳过并显示原因，不尝试渲染。
- [ ] 同一份 manifest 连续解析两次得到逐字相同的 `params` 顺序与取值。
- [ ] `cargo test` 覆盖：`manifest.toml` 的正常解析、`params` 形状错误、缺文件三种情况。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。

### US-009: 库网格视图

**描述：** 作为用户，我要在一个网格里看缩略图挑壁纸。

**验收标准：**
- [ ] 网格里的每一项显示：缩略图（来自 `thumbnail <id>`）、`name`、`tags`；项的顺序稳定（同一份库两次打开顺序相同）。
- [ ] 缩略图不在 UI 线程上同步等：先出占位，PNG 到位后再替换（界面在等待期间可交互）。
- [ ] 虚拟化：库里放 **200** 个包时滚动流畅，不一次性渲染 200 张图（用 gpui-kit 的虚拟化列表承载）。
- [ ] 缩略图有缓存，重复滚动不重复请求同一个 `id`。
- [ ] 库为空时显示空状态文案（含库目录路径），不是空白。
- [ ] 包目录在应用运行时被增删：**10 秒内**反映到网格（重新 `list`），或明确提供刷新动作。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍并截图（含 200 个包的滚动场景）。

### US-010: 把壁纸应用到输出

**描述：** 作为多输出用户，我要把这面墙的壁纸应用到某一块屏（或者所有屏）。

**验收标准：**
- [ ] 网格里选中一个包后，能把它的壁纸应用到：某一个输出、或全部输出（`set <output|*> <path>`）。
- [ ] 额外交互**必须**区分两种语义，且界面上写清楚：**临时覆盖**（走 `set`，任何一次热重载都会盖掉它，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.22.1）与**持久设置**（写 drop-in，见 US-003）。
- [ ] 走 `set` 时，含空格的路径不需要任何引号规则（输出名先出现，第二个词之后都是路径，见 [`docs/SPEC.md`](../docs/SPEC.md) §5.1）。
- [ ] `set` 生效当场可见：换图过渡照常播（不因为换壁纸路径而退化）。
- [ ] 点一个不存在的输出：显示守护进程报的错，不静默失败。
- [ ] 界面上"现在屏幕上是哪张"与 `state.wallpaper.path` 一致，而不只是与刚才点了什么一致。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍：临时改一块屏 + 持久改一块屏，两种都截图。

### US-011: 属性面板（内置键与包参数同一条代码路径）

**描述：** 作为调参用户，我要选中一个壁纸后就地调它的参数，而不是先跳到设置页。

**验收标准：**
- [ ] 选中包后，属性面板渲染**两组**参数：该包 `params` 里的参数、以及 `schema` 相关的内置键（至少 `fit`、`scale`，多输出时含 `[outputs."NAME"]` 覆盖）。
- [ ] 两组参数**共用**同一套 `Kind` → 控件映射（US-002 的那一套），没有第二条渲染路径。
- [ ] 能预览**临时**效果与**持久**效果（对应 US-010 的两种语义），并在界面上标明哪个是哪个。
- [ ] 参数改动后 `state.config.values`（持久）或 `state.wallpaper`（临时）在 **2 秒内**反映出来。
- [ ] 面板上的值变化**不会**引发 US-003 的振荡回归（空闲 10 秒文件哈希 0 次变化）。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍并截图（含一个包参数与一个内置键）。

### US-012: 现代外观与主题

**描述：** 作为用户，我要一个看起来 2026 年而不是 1996 年的界面。

**验收标准：**
- [ ] 明暗两套主题都能切换，切换后所有界面文字与背景的对比度可读（不出现白底白字）。
- [ ] 界面只用 gpui-kit 的组件与主题 token（`cx.theme()` 里的 `background` / `foreground` / `border` / `surface` / `muted` / `primary` …），不硬编码颜色值。
- [ ] 组件选型与布局遵循项目本地的 `gpui-kit-design-guides`（`.omp/skills/`，不进仓库），不发明组件 API。
- [ ] 重复元素的 id 由领域值派生（包 id、输出的 connector 名），不是列表下标。
- [ ] 单独跑窗口时不产生告警刷屏（无 GPU 相关的报错循环）。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里真跑一遍，明暗两种主题各截图一张。

### US-013: 异常与空状态

**描述：** 作为用户，出问题时我要看到一句人话，而不是一个空白窗口。

**验收标准：**
- [ ] 至少这些状态各有明确界面：守护进程没在跑、socket 文件陈旧（存在但没人应答）、库为空、某个包 manifest 坏掉、`thumbnail` 失败、`state` 里没有该输出。
- [ ] 每种状态都写明**接下来能做什么**（例如"启动守护进程"的命令行），而不只是报错。
- [ ] 守护进程拒绝一份 drop-in（解析失败、保留旧配置）时，界面能显示这次失败，并保持"当前生效值"与 `state` 一致，不显示刚写进去的错值。
- [ ] 任何异常都不使应用退出；恢复条件满足后界面自动回到正常态。
- [ ] `cargo clippy --all-targets` 0 警告；`cargo fmt --check` 通过。
- [ ] 在本机 niri 会话里逐个制造这些状态并截图。

## 功能需求

- FR-1: 应用是守护进程的**薄客户端**：壁纸的渲染、解码、GL/EGL/GBM 与 dmabuf 全部留在守护进程；应用进程不得链接 libmpv、不得解码媒体、不得自己绘制壁纸像素，预览一律来自 `thumbnail` 输出的 PNG。
- FR-2: 应用只通过既有控制 socket（`$XDG_RUNTIME_DIR/niripaper.sock`，一行进一行出，全会话一份，请求里带输出名）与守护进程通信；socket 发现遵循 `ipc::find_path()` 的既有行为（不存在即"守护进程没在跑"），并支持 `--socket PATH` 覆盖。
- FR-3: 应用的界面**必须**由 `niripaper schema` 驱动：键名、类型、范围、单位、默认值、说明、可选项全部取自 `schema`，代码中不得硬编码任何键名或取值列表；不认识的 `type` 跳过而不是猜。
- FR-4: 应用**必须**按 `schema.per_output` 路由写入：只有 `per_output` 列出的键（当前 `wallpaper` / `scale` / `column_span` / `workspace_span` / `fit`）可以写在 `[outputs."NAME"]` 之下；其他所有键必须写在文件顶层。理由（硬约束）：把全局键写进 `[outputs."NAME"]` 会让守护进程**拒绝整个 drop-in 文件**。
- FR-5: 应用**必须只写意图**，永不把守护进程报出来的值写回它自己的输入。`state` / `query` 的返回值只用于显示与校验，不得成为生成配置内容的来源。（这是已实测过的自激振荡失败模式：连续每 ~2 秒翻转、10 秒内 7 次 `config reloaded`；见 [`docs/gotchas.md`](../docs/gotchas.md)。）
- FR-6: 应用**必须**把自己的设置写在一份独立的 `~/.config/niripaper/config.d/niripaper-gui.toml` 里，不改 `config.toml`、不碰别的工具的 drop-in；写出时遵守 TOML 与合并规则（碎片文件自带表头、顶层键在第一个 `[表]` 之前）。
- FR-7: 应用**必须**把 `hot: false` 的键标注为"需要重启守护进程"，并且不把它们当作即时生效。
- FR-8: 应用**必须**在界面上区分两种改壁纸的语义：`set`（临时覆盖，任何一次热重载都会盖掉它）与写 drop-in（持久）。
- FR-9: 守护进程**必须**新增 `list` 动词：返回库里的壁纸包，一行 JSON。它只有这一个新职责；`schema` / `state` / `set` / `query` / `kill` 的行为不变。
- FR-10: 守护进程**必须**新增 `thumbnail <id> [w h]` 动词：渲染一帧并写出 PNG。理由：守护进程已经有 libmpv、GL 与"按合成器反馈钉好的目标 GPU / EGL vendor"，缩略图属于那里，不属于 GUI 进程。
- FR-11: 应用**必须**按"目录 + `manifest.toml`（`type` / `name` / `tags` / `params`）+ 资源"识别一个壁纸包；`params` 的形状与 `schema::Key` 一致。
- FR-12: 属性面板**必须**用同一条代码路径渲染包的 `params` 与内置键（同一套 `Kind` → 控件的映射）。
- FR-13: 界面上可供选择的输出**必须**来自 `state` 的输出列表，不得由用户手打；应用到不存在输出时必须显示守护进程返回的错误。
- FR-14: 应用**必须**原样暴露既有的运动与过渡键（`scale`、`column_span`、`workspace_span`、`animations.*`、`transition.*`、`fit`），不得新增行为、不得新增配置键、不得在应用侧重新解释这些语义（视差规格见 [`docs/SPEC.md`](../docs/SPEC.md) §4）。
- FR-15: 应用**不得**修改 niri 或任何桌面壳的配置（`~/.config/niri/**`、`~/.config/noctalia/**`）、不得写入 layer rule、不得代管 `place-within-backdrop` 或"工作区背景"这两种放置方式；它们继续由用户与 niri 决定，行为不变。
- FR-16: 应用**不得**提供任何 GPU / 渲染设备 / MUX 选择项（选哪块卡是合成器的决定）。
- FR-17: 守护进程断开或重启时，应用**必须**显示明确状态并在恢复后自动重连，不得退出、不得显示上一份可能过期的值当作当前值。
- FR-18: 应用**必须**在写完后核对效果：读回 `state`，确认该值确实生效；若守护进程保留了旧配置（解析失败），向用户报出这次失败并把界面回到实际生效值。
- FR-19: 应用窗口空闲时**不得**轮询守护进程到产生持续负载；静止状态下的请求频率必须有上界（阈值在实现时确定并写进 `docs/SPEC.md`）。
- FR-20: 本仓库的构建与自检约定同样适用于应用代码：`cargo fmt --check`、`cargo clippy --all-targets`（0 警告）、`cargo test` 全绿（见 [`AGENTS.md`](../AGENTS.md)）。

## 非目标（明确不做）

- **不做** shader / 粒子 / 交互式场景壁纸。
- **不做**音频反应、鼠标反应特效。
- **不做**壁纸资源商城 / store（库是本地目录）。
- **不做**可视化编辑器（不编辑包、不做效果设计器）。
- **不做** Wallpaper Engine 格式兼容（不读它的包与项目文件；只借 `fit` 的名字，见 [`docs/SPEC.md`](../docs/SPEC.md) §2.20.1）。
- **不做**任何 GPU / MUX 选择。
- **不改**守护进程的运动语义：视差、总览过渡、换图过渡照 [`docs/SPEC.md`](../docs/SPEC.md) §4 执行；应用只是它们的遥控器。
- **不**复活、**不**修改 Noctalia 插件。
- **不**替代合成器自己的壁纸处理：`place-within-backdrop`、静态 backdrop 与 `layout { background-color }` 的现状不变，应用不写 layer rule、不改 `~/.config/niri/**`。
- **不**在首版做：开机自启管理、系统托盘常驻、库目录的写入/导入工具、把 `set` 当成持久设置的兼容层。

## 设计考虑

形态照 Wallpaper Engine 的桌面应用（以 niri 为第一目标），四个区域：

1. **库网格**：缩略图 + `name` + `tags`，虚拟化滚动，选中即进属性面板。
2. **应用到输出**：输出清单来自 `state`；可"只这一块"或"全部"；临时与持久两条路各自可见（FR-8）。
3. **属性面板**：内置键与包 `params` 同一条渲染路径（FR-12）；`schema` 的 `about` 直接当提示、`unit` 直接当单位。
4. **设置页**：全部 28 个键，按 `schema` 的顺序分节；`per_output` 的键在选中某个输出时出现（FR-4）；`hot: false` 有明确标注（FR-7）。

组件与主题用 `gpui-kit`：应用只依赖 `gpui-kit` 一个 crate（`use gpui_kit::*;`），控件取 `gpui_kit::component`（`Button` / `ButtonGroup` / `Input` / `Select` / `Switch` / `Checkbox` / `Icon` / `Dialog` / `Notification` / `Tabs` / `Tooltip` / `Form` 的 `v_form` `h_form` `field` / 虚拟化列表），布局用 `h_flex` / `v_flex`，主题走 `cx.theme()` 与 `Theme::change(ThemeMode::…)`。**不许凭 React/CSS/旧版 GPUI 的例子类推 API**：签名按项目本地 `.omp/skills/gpui-kit/references/` 的 `conventions.md` 与 `usage.md` 核对，设计取舍按 `gpui-kit-design-guides`（AGENTS.md 与 CI 之外，这两份 guide 是要求而不是灵感）。

视觉目标：现代、密度适中、信息优先 —— 缩略图是主体，参数用表单，不用模态对话框承载日常操作。重复元素用领域 id（包 id、输出名），不用列表下标。

## 技术考虑

- **后端已经就位，不需要改**（首版除两个新动词外）：`niripaper schema`（28 个键，机器可读）、`niripaper state`（实际生效值 + 来源 + 屏幕上是什么）、`set <output|*> <path>`、`query`、以及靠 `~/.config/niripaper/config.d/` 的热重载。CLI 仍用 `--output NAME`，多输出由一个进程按输出寻址。
- **两个新动词是唯一的守护进程改动**：`list` 与 `thumbnail`（FR-9、FR-10），落在 `src/ipc.rs` 的请求枚举、CLI 与 [`docs/SPEC.md`](../docs/SPEC.md) §5.1。协议是一行进一行出，所以两者都只能回一行。
- **`set` 是临时覆盖**：任何一次热重载都会盖掉它（[`docs/SPEC.md`](../docs/SPEC.md) §2.22.1）。持久设置只有一条路 —— 写 drop-in。
- **为什么缩略图必须在守护进程**：它已经按合成器 dmabuf feedback 的 `main_device` 开好了 GBM/EGL，并已钉好 EGL vendor；而"一个进程只能钉一个 vendor"。GUI 进程再建一套渲染栈等于把这条已经踩平的坑重新挖一遍（[`docs/SPEC.md`](../docs/SPEC.md) §2.6、§2.7，[`docs/gotchas.md`](../docs/gotchas.md)）。
- **振荡失败模式的教训直接约束数据流**：意图（用户选了什么）与镜像（守护进程报了什么）必须是两张表，只有意图写进配置（FR-5）。验收用"空闲时文件哈希 0 次变化"来钉。
- **配置文件是逐份解析的**：出错能指名文件与行，失败保留旧配置并打印原因，且解析失败不许把守护进程打挂（[`docs/SPEC.md`](../docs/SPEC.md) §5.2）。应用因此必须能显示"这次写入被拒了"（FR-18）。
- **工具链已验证可用**：`gpui-kit` 在本机（niri + Wayland + NVIDIA）跑通；项目本地 skills 在 `.omp/skills/`（不进仓库）。
- 应用是**独立进程**，因此与守护进程的"一个进程一个 EGL vendor"约束互不干扰；但这也意味着应用绝不能把自己变成第二个渲染器（FR-1）。
- 库与缩略图的目录位置、缓存策略见「开放问题」；确定后写进 `docs/SPEC.md`。

## 成功指标

- 换一张壁纸：**0** 次手动编辑文件，**≤3** 次点击/按键（库 → 选包 → 选输出）。
- 新增一个配置键：应用侧 **0** 行 UI 代码改动（`schema` 驱动，FR-3）。
- 自激振荡回归：空闲 10 秒生成文件哈希 **0** 次变化、`config reloaded` **0** 次。
- 库 **200** 个包时滚动不掉帧到不可用（虚拟化生效，US-009）。
- 守护进程断开到界面反映：**≤5** 秒（US-001）；守护进程改值到界面跟上：**≤2** 秒（US-005）。
- 属性面板与设置页渲染同一批键时不产生两套控件实现（FR-12：一份 `Kind` 映射）。
- 仓库自检全绿：`cargo fmt --check`、`cargo clippy --all-targets`（0 警告）、`cargo test`。

## 开放问题

1. **应用住在哪里**：本仓库新增一个 crate/binary（与守护进程共享 `schema`/协议类型的定义），还是另开一个仓库？决定后写进 `docs/SPEC.md`（本 PRD 只要求"薄客户端 + 复用既有动词"不变）。
2. **库根目录**：需要一个约定路径（建议 `$XDG_DATA_HOME/niripaper/library`，或让用户配置），以及"库目录不存在"时的行为。
3. **缩略图缓存**：落在哪（XDG cache 目录？）、什么键失效（包内容 mtime？），以及库目录变大时的清理策略。
4. **`manifest.toml` 的 `type` 允许哪些取值**：首版是否只允许图片与视频两类；未知 `type` 的显示方式（US-008 已要求"跳过并说明"）。
5. **应用是否负责拉起守护进程**：建议**不**（只提示 + 给出命令行），避免应用与 systemd/自启动两套生命周期打架 —— 需要确认。
6. **`thumbnail` 的默认尺寸**与是否需要"无头输出"的降级路径（守护进程没画任何输出时能否出图）。
7. **空闲轮询上界**（FR-19）：具体间隔与是否改用事件驱动（守护进程何时需要告知"配置变了"）—— 若需要新动词，要重新评估 FR-9 的"只有两个新动词"。
