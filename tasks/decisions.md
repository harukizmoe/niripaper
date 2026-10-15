# 决策记录：独立桌面应用

这份文件记录"niripaper 从守护进程 + 废弃插件，变成 niri 桌面的独立应用"这件事上
**已经定下来的**决策，以及每条为什么。

- 每条都写清了理由与实测依据 —— 它们之后会变成 `docs/adr/` 或 `docs/SPEC.md`。
- **本文件里没有待定项了**（2026-10-12；2026-10-16 工具包改回 mygo，见 **〇** —— 那是决定，不是待定项）：讨论收敛完毕，可以按 4.5 的顺序开工。
- 本文件是讨论的收敛点，不是规格。规格在 [docs/SPEC.md](../docs/SPEC.md)。

---

## 〇、2026-10-16：工具包改回 mygo（Rust/GPUI → Go）

原来定的是 `gpui-kit`（Rust，GPUI）。改成 **mygo**（Go，`github.com/egoist/mygo`，MIT，
文档称 v0.5.1）。**界面方式不变** —— 仍是原生控件、无 webview（上表"前端画布"一行不动）。

**为什么改**

| 依据 | 事实 |
| --- | --- |
| 托盘 | mygo 把**托盘 / 菜单 / 对话框 / 通知 / 全局快捷键 / 深链 / 文件关联 / 最近文档**都做成内置能力。gpui-kit 那边**没有托盘组件**，得自己用 `zbus` 接一套（见三）—— 这条缺口消失了 |
| 组件覆盖 | `ui` 包正好覆盖本应用要用的全部：`Grid view`（文档原话 "items in a grid of any size, **as a photo library**"）、`Image`/`Icon`、`Sidebar`/`Tabs`/`Split view`、`List`/`Table`/`Tree`/`Outline`、`Dialog`/`Alert`/`Popover`/`Tooltip`/`Toast`、`Form`、`Slider`/`Select`/`Combobox`/`Color picker`、`Charts`，以及 `motion`（弹簧 / 关键帧 / 时间轴）、`transitions`、`Transforms` |
| 可测性 | 视图**不开窗**就能跑（文档 `Testing`："as fast as a unit test"），另有 `Inspector` 看元素盒与每帧耗时 —— 与守护进程那边"纯逻辑可测"的口径一致 |
| 本机实测 | 2026-10-11 实测过：mygo 原生 UI 在 **niri + Wayland + NVIDIA** 上开窗并渲染成功（`app_id = probe`，GL 初始化干净） |
| 体积 | 文档给出的数字：单二进制 **~7 MB**、原生 UI 常驻 **~44 MB**、空闲 CPU **0%** —— 与"单二进制、无 WebKitGTK"的分发哲学一致 |
| 语言 | UI 与桌面 API 都是 Go 单语言，不必为 UI 再养一套 Rust 构建 —— gpui-kit 那 849 个 crate 的 CI 顾虑（见四）一并作废 |

**社区组件库：四个，2026-10-16 逐一核实过**（这是"改回 mygo"的主要理由之一）

| 库 | 是什么 | 对这个应用有用的部分 |
| --- | --- | --- |
| **`smallnest/uix`**（MIT，13★） | shadcn/ui 风格的 **mygo 原生**组件库，**~110 个组件**（含 BoardUI / shadcn expansions / onevra 三批移植） | `gridview` ✓、`item`（带媒体缩略图的列表行）✓、`carousel` ✓、`aspectratio` ✓、`scroll`/`scrollboth`/`infinitescroll` ✓、`command`（⌘K 面板）✓、`settings` ✓、`appshell` ✓、`image`/`icon`/`lucide` ✓、`split`/`sidebar`/`tree`/`list`/`table` ✓、`dialog`/`sheet`/`drawer`/`popover`/`toast` ✓、`code`/`markdown` ✓、**`webview`**（局部逃生口）✓ |
| **`oneo-me/onevra`**（MIT） | 原生 Go 组件库：主题控件 + 复杂组件 + **窗口 Shell** + Lucide 图标 + **Packer** + 起步模板 | 窗口 Shell（隐藏系统标题栏、跟随系统明暗）✓、**托盘起步模板**✓、**Packer**（dmg / zip / tar.gz，打包省一大截）✓、复杂组件（图表 / 命令面板 / 代码高亮 / Markdown / 对话）✓ |
| **MujicaUI**（`HycJack/MujicaUI`，fork 自 `ZacharyZhang-NY/MujicaUI`） | 按类别组织的 mygo 组件库，已迁移到 MyGo v0.3 的 value-type Element API | 已被一个真实应用采用（见下）；**许可证待确认** ✗ |
| **8bitMyGoUI**（`8bitui.zacharyzhang.com`） | 16-bit 像素风组件，同一份 Go 代码编到 WebAssembly | 风格取向，与本应用气质不合 ✗ —— 记下存在即可 |

**这套栈有实机旁证**：`HycJack/MujicaUI-agent-demo`（**Crux Agent**）是一个完整的 Codex 风格
桌面应用，就用 mygo 原生 UI + MujicaUI 写的 —— 三栏工作台、虚拟化目录树、语法高亮、真实
git diff、模态框、⌘K 命令面板、主题走语义令牌，而且**它的截图是无窗口渲染测试离线产出的**
（`MYGO_UI_SHOTS`），与本项目"纯逻辑可测"的口径一致。它按 `internal/{store,engine,app}` 分三层，
UI 层与逻辑层解耦 —— 与本项目"渲染全留在守护进程、应用是薄客户端"的分层是同一种口味。

**新增的已知缺口：没有视频组件**

**mygo 自带的组件表与上述四个库，都没有视频播放**（逐一核实：uix ~110 个组件里没有；
Onevra 的"复杂组件"四类里没有；MujicaUI、8bitMyGoUI 也没有），所以视频壁纸的**预览播放**
不能直接做。两条出路，都仍在"无 webview 分发"之内：

1. **抽帧**：`ffmpeg` 抽一帧当缩略图 / 静帧预览 —— Noctalia 插件时代已经这么干过，手法现成；
2. **局部 `ui.WebView`**：mygo 明确支持"原生 UI 之上叠一个 web 页"（文档 `Web view`：
   "a web page in the window, under the native UI painted after it"）—— 只在要播放的那一块嵌
   一个 `<video>`，其余仍是原生。缺口是**局部的**，不是全局的。

**因此作废 / 待清理**

| 东西 | 处置 |
| --- | --- |
| `tasks/prototype/`（Rust + GPUI 原型） | 技术栈作废。界面上的发现已单独记在 `tasks/ui-prototype-findings.md`，那份**与框架无关**，保留 |
| `.omp/skills/gpui-kit{,-design-guides}`（本地技能，29 个文件，未入库） | gpui API 部分作废；**design-guides 里的设计规范与框架无关**，值得原样留用 |
| 本文档三里的"托盘怎么用 `zbus` 接" | 作废（mygo 内置托盘） |
| 本文档四里的 `niripaper-app/ UI（gpui-kit / GPUI）`、849 crate 的 CI 说明 | 改成 Go 模块；CI 顾虑消失 |

---

## 一、形态

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| 目标平台 | **niri 优先**，Linux | 引擎的视差/backdrop 依赖 niri IPC，这是它区别于通用壁纸工具的地方 |
| 形态 | **Wallpaper Engine 式桌面应用**：库网格 + 应用壁纸 + 属性面板 + 设置页 | 借用的是"应用的组织形式"，不是 WE 的文件格式与效果系统 |
| 交互性 | **静止优先，不做交互壁纸** | niri 上 `place-within-backdrop`（静止、总览整屏）与指针输入**互斥**——backdrop 内的层忽略全部输入。两者不能共存，选了静止 |
| 渲染位置 | **全部留在守护进程**；应用是控制 socket 上的薄客户端 | 守护进程已按 dmabuf feedback 的 `main_device` 开好 GBM/EGL 并钉好 EGL vendor；而"一个进程只能钉一个 vendor"（[ADR 0003](../docs/adr/0003-bare-ffi-no-egl-crates.md)）。应用再建一套渲染栈＝把踩平的坑重挖 |
| 工具包 | **`mygo`**（Go，`github.com/egoist/mygo`，原生 `ui` 包） | 2026-10-16 从 `gpui-kit` 改回，理由与实测依据见 **〇**。一句话：托盘/菜单/对话框/通知/全局快捷键**内置**（正好补掉 gpui-kit 唯一那条已知缺口），组件表覆盖本应用要用的全部，且已在本机 niri + Wayland + NVIDIA 实测开窗渲染成功。**新缺口：没有视频组件**（见 〇） |
| 前端画布 | **原生控件**（不是 webview） | 保持"单二进制、无 WebKitGTK"的分发哲学 |
| Noctalia 插件 | **废弃，不复活** | 见 [docs/SPEC.md](../docs/SPEC.md)「非目标」 |
| 视觉 | 现代、密度适中、信息优先；规范见 `tasks/ui-design-research.md` 与 `.omp/skills/gpui-kit-design-guides/references/design-guides.md` | 该 guide 是要求不是灵感。**它是设计规范、与框架无关**，所以换框架后仍然有效（那个技能目录的名字带着 gpui，但规范部分可以原样留用；技能里的 gpui API 部分随框架作废） |

---

## 二、配置（2026-10-12 大幅简化）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| **唯一配置来源** | **只有 `~/.config/niripaper/config.toml`** | |
| `config.d/` 目录 | **取消**（连排序一起） | 文件名当优先级是隐式规则，要用户靠 `90-` 前缀推理谁赢 |
| 多文件合并 | **取消** | 单一文件、单一写入者 → 优先级问题不存在 |
| 写入者 | **只有 GUI 写**；守护进程**永远只读** | 避免并发写与合并；并避开"把 `state` 报的值写回输入"的自激振荡（[docs/gotchas.md](../docs/gotchas.md) 风险 17） |
| GUI 写入方式 | **`toml_edit` 外科手术式改写** + **写临时文件再 `rename`** | |
| 已有值 | GUI 首次打开**载入文件里已有的值**，不用默认值覆盖 | |
| 注释 | **保留**（那份 201 行自解释配置照用） | |
| `set` 语义 | **不变**：内存临时覆盖，不落盘 | |

### 2.1 轮播与运行期状态（2026-10-12 决定）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| 轮播 | **v1 就做** | 用户明确要求 |
| **轮播进度** | **不持久化**——守护进程的运行期状态 | **死锁所迫**：轮播定时器必须在守护进程里（UI 关了也要继续），而"只有 GUI 能写"意味着守护进程写不了自己的进度。解法是把进度留在内存：重启守护进程从列表开头开始。WE 也是这个语义 |
| 配置键形状 | `wallpaper` → **列表** + `interval` + `order` | 轮播需要；这是**破坏性变更** |
| `schema` 扩展 | 新增 `Kind::Paths`（有序路径列表） | 现有 9 个变体里**没有**能装"用户自己的路径列表"的（`MultiEnum` 的选项由 schema 给死） |
| `state` 扩展 | 报"第几张 / 共几张" | UI 要显示进度 |
| 切换方式 | **复用 `[transition]`**，不另造 | |
| 作用域 | **每输出各自轮播**（`per_output` 加 `wallpapers` / `interval` / `order`） | 多屏用户对每块屏有不同列表 |
| 暂停 | **做** —— 见 2.3 | |

### 2.2 里程碑顺序（2026-10-12 决定）

**① 设置页 → ② 运行期状态 → ③ 库网格**，按此顺序。

| 块 | 内容 | 守护进程改动 |
| --- | --- | --- |
| **① 设置页** | `schema` 驱动的全部 27 键；`toml_edit` 写入；只读状态视图 | **0** |
| **② 运行期状态** | 轮播（定时器）+ 输出热插拔；`Kind::Paths`；`state` 扩展 | 新子系统 |
| **③ 库网格** | `list` + `thumbnail` + 缩略图缓存 | 2 个新动词 |

理由：① 几乎零新机制且直接消灭"手编辑配置"的痛点；② 与 UI 解耦、可独立验证；③ 是最重的 UI 工作。

### 2.3 轮播、暂停与 `set`（2026-10-12 决定）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| 轮播间隔 | **per-output**（与 `wallpapers` 一致） | 多屏用户对每块屏有不同列表 |
| `per_output` | 5 键 → **8 键**（加 `wallpapers` / `interval` / `order`） | |
| 轮播的过渡 | **复用 `[transition]`**，不新增配置 | 用户要求；间隔本身由用户设 |
| `set` 与轮播 | **`set` 暂停轮播**，reload 恢复 | 与现有"`set` 被 reload 盖掉"语义一致 |
| 重启后的起点 | GUI 点选某张 = **把它移到列表首位** | "选哪张"变成**配置事实**（列表顺序）而非运行期记忆 → 不撞"守护进程不能写配置"的死锁 |
| 暂停判据 | **被窗口完全遮住**（不只全屏） | 比"全屏"更通用；判据相同。全屏只是"没人看"的一种 |
| 暂停范围 | **整个渲染 + 解码**（不只停轮播） | 用户要求；被遮住时解码纯属浪费 |
| 暂停的落点 | `src/daemon.rs` 那个唯一的帧请求条件（`animator.is_moving() \|\| zoom.is_moving() \|\| transition.is_moving() \|\| video_present`）加一项 | 它本来就是"还要不要画下一帧"的唯一决策，也是"静置 0 CPU"的实现 |
| 恢复语义 | **跳到最后**（丢弃暂停期间的视频帧），不补播 | 否则恢复瞬间会"时间旅行"，从被遮住那刻继续播 |
| `state` | 新增"暂停中 + 原因"字段，UI 要显示 | 否则用户会问"我的视频壁纸为什么不转了" |

#### 判据的实测依据（2026-10-12）

`is_fullscreen` **niri 不报告** —— `niri msg --json windows` 与原始 EventStream 的窗口字段都是
`app_id / focus_timestamp / id / is_floating / is_focused / is_urgent / layout / pid / title / workspace_id`，
`layout` 里也没有。所以判据只能靠几何。

实测（DP-1 = 2560×1440）：

| 状态 | `window_size` |
| --- | --- |
| 普通平铺（最大化，被 gaps 10 + 顶栏留出） | `2540×1390` |
| **全屏** | **`2560×1440`**（**精确等于**输出尺寸） |

→ 判据是**精确相等**，**不需要阈值**，也不新增任何数据（守护进程已有输出尺寸与窗口 `window_size`）。
平铺布局里窗口不互相重叠，所以"遮满"只可能是**一个全屏窗口**，不用算并集。

### 2.4 `namespace` 与 backdrop（2026-10-12 决定）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| backdrop | **保留**（总览整屏壁纸 + 切工作区不滑） | 用户确认"静止优先"，而 backdrop 是它的实现方式 |
| 谁写 layer-rule | **用户自己**加一条到 niri 配置 | 这是 niri 唯一能进 backdrop 的机制（`place-on-workspace` 只在 issue 里，未实现）。所有壁纸工具都这么用，`swaybg` 也一样 —— **不是"应用去改别的应用的配置"，是用户按 niri 的规矩配置 niri** |
| **`namespace`** | **从 `schema` 删除**，降级为编译期常量 + `--namespace` 排障标志 | 它是**内部契约**不是用户设置：**改了它，用户写的 layer-rule 就匹配不上**，backdrop 静默失效 |
| 附带效果 | `schema` **27 键**，且**不再有 `hot: false`** 的键 | FR-7（"标注需要重启的键"）随之消失 |

实测（2026-10-12）：用一条匹配不到任何规则的 namespace 起守护进程，**层照常注册**（`namespace: niripaper-norule-test | layer: Background | output: DP-1`）、零报错 —— 只是不进 backdrop。即 backdrop 与"能否工作"解耦。

### 为什么 `toml_edit` 是硬要求（实测，2026-10-12）

带注释的配置**不是问题**——问题在工具。`toml` crate 只能 parse，序列化回去会丢注释；
`toml_edit` 只替换被改的节点，其余字节原样输出。拿本机那份 201 行配置实测：

```
改 scale 1.2 → 1.35、transition.effects 追加一项：
diff 只有那两行
201 行 → 201 行，152 注释行 → 152 注释行
```

**那份注释是独立资产**：`schema.about` 装不下它（`about` 是"一行说明"，注释里是成段推导——
`center` 的示例坐标、"`hold_ms` 别用"的理由、`cubic_bezier` 为什么选 `[0.42,0,0.58,1]`）。
它是给"正在调参的人"看的，UI tooltip 放不下也不该放。

### 为什么单一文件不会让守护进程"没配置"

守护进程自启（见三）时 GUI 不在，它从**同一个文件**读。没有配置时画 `Pattern::Blocks`
测试图案——**这是好失败模式**：屏幕上有东西，且一眼知道还没配。不要改成黑屏或退出。

### 改动半径（已核实）

| 位置 | 改动 |
| --- | --- |
| `src/config.rs` `config_files()` | 删掉 `read_dir(config.d)` + `sort()`；只返回主文件 |
| `src/config.rs` 单测 | `a_later_file_overrides_an_earlier_one` 作废；`config.d/bad.toml` 的断言改指主文件 |
| `src/daemon.rs` `config_dirs` | 只留 `config.toml` 所在目录（去掉 `config.d`） |
| `src/watch.rs` | **不动** —— 它本就监视目录并认 `IN_MOVED_TO`，正是原子写入要的路径 |
| `docs/SPEC.md` §5.2 | 合并规则整段重写 |
| `CONTEXT.md` | `config.d drop-in` 术语删除 |
| **ADR 0006** | **作废**，被"单一配置文件"取代 |
| PRD FR-6 / US-003 | 改为"写唯一的 `config.toml`" |

---

## 三、生命周期（自启动与托盘）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| 启动方式 | **不用 niri 的 `spawn-at-startup`**，交给**自启动** | 生命周期不该写死在桌面配置里 |
| 启动的是谁 | **守护进程**（必须） | 壁纸要先于任何窗口出现在屏幕上 |
| GUI | **静默启动**（不弹主窗口，只留托盘图标） | 用户明确要求 |
| 开关 | 由**用户在应用里选择**，默认不开 | 不写死 |
| 通路 | **systemd user service**（+ `.desktop` 兜底）—— 见 4.1 | |

已核实：本机 `xdg-desktop-autostart.target` **active**，`Clash Verge` / `NetworkManager Applet` /
`Fcitx 5` 等确实由它拉起；systemd user session 也在跑。niri 自己只认 `spawn-at-startup`，
**XDG autostart 是经 systemd 生成器 + `xdg-desktop-autostart.target` 实现的**。

---

## 四、其余各项（**标「已定」的已定，其余在定下来之前不要动手**）

### 4.1 自启动用哪条通路 —— **已定**（2026-10-12）

**用 systemd user service**（`niripaper.service`），另打包一份 `.desktop` 给非 systemd 的 niri 用户。

选 systemd 是因为它一次解决三件事：

- **崩溃自动重启** —— 守护进程挂了壁纸自己回来，不需要用户手动救；
- **日志进 journald** —— 顺手干掉现在那个 `spawn-at-startup "sh" "-c" "… >~/.cache/niripaper.log"` 的手工重定向，改看 `journalctl --user -u niripaper`；
- **依赖顺序** —— `After=graphical-session.target`。

`.desktop` 兜底是为了不排除没有 systemd 的 niri 用户（XDG autostart 是标准）。
开关由用户**在应用里**选择（`systemctl --user enable/disable`，或写/删 `.desktop`），**默认不开**。

### 4.2 托盘管什么 —— **已定**（2026-10-12）

**只做"打开主窗口 / 退出应用"。** 不碰守护进程，因此**不新增协议动词** ——
PRD 的"只有 `list` + `thumbnail` 两个新动词"这条约定保持不变。控制壁纸留给主窗口。

### 4.3 托盘怎么实现 —— **已定**（2026-10-12）

> ⚠ **2026-10-16 作废，见 〇**：工具包换成 mygo 之后，托盘是**框架内置能力**，不必自己接
> `zbus`。以下保留为当时的推理记录。

**自写 StatusNotifierItem + `zbus`。**

实测：`zbus` **已经在 gpui-kit 的依赖树里**（`cargo generate-lockfile` 验过），所以是纯 Rust、
无新 C 依赖。另一条路 `libayatana-appindicator3`（本机有 0.6.0）会引入一个 GTK 时代的 C 库。
gpui-kit 自己**没有**托盘组件（搜遍 SKILL.md 与 30 篇 references：`tray`/`systray`/
`StatusNotifierItem`/`indicator` 零命中），所以无论哪条路都要自己接。

### 4.4 "壁纸包"这个概念要不要 —— **已定：不做**（2026-10-12）

**决定：v1 不做壁纸包。** 库就是"一个目录里的媒体文件"。

理由（轮播定下来之后更清楚）：轮播已经让 `wallpaper` 变成**列表**，所以"多张壁纸"这件事
由列表解决了；而包格式剩下的价值只在**多文件 + 带参数**的壁纸（shader / 场景），
那正是非目标。给单张图/视频套一层目录 + manifest 是纯开销。

将来真加 shader 类型时再引入包格式 —— 那时 `schema` 驱动的 UI 会自动接住（FR-3 已保证）。

**连带**：PRD 的 US-008（`manifest.toml`）、US-011（包参数）、FR-11、FR-12 **作废**。
库网格退化为"扫描一个目录 + `list` + `thumbnail`"。

### 4.5 v1 的顺序 —— **已定**（2026-10-12）

**① 设置页 → ② 运行期状态 → ③ 库网格。**

| 块 | 内容 | 守护进程改动 |
| --- | --- | --- |
| ① 设置页 | `schema` 驱动的全部 27 键；`toml_edit` 写入；只读状态视图 | **0** |
| ② 运行期状态 | 轮播（定时器）+ 输出热插拔 + 遮满暂停；`Kind::Paths`；`state` 扩展 | 新子系统 |
| ③ 库网格 | `list` + `thumbnail` + 缩略图缓存 | 2 个新动词 |

① 几乎零新机制，且直接消灭"手编辑配置"这个最大痛点；② 与 UI 解耦、可独立验证；③ 最重。

### 4.6 应用住在哪 —— **已定**（2026-10-12）

**一个 Cargo workspace，三个 crate：**

```
niripaper-proto/   纯逻辑，零 IO 依赖（serde/serde_json/toml）
                   ├─ socket 客户端（一行进一行出）
                   ├─ Schema / State / Key / Kind 类型
                   ├─ schema()            ← 守护进程「生成」它，UI 也用它
                   └─ per_output 路由规则  ← UI 与守护进程共用同一条，写不错
niripaper/         守护进程（wayland / EGL / libmpv）—— 现状不变
niripaper-app/     UI（mygo / Go）          ← 2026-10-16 改，见 〇
```

> ⚠ **2026-10-16 作废（见 〇）**：应用改用 Go 之后，这条 cargo workspace 的顾虑整个不存在了
> （`niripaper-app` 是 Go 模块，根本不进 cargo）。下面的推理保留为当时的记录。

**CI 是硬约束，而它正好被解掉**：`.github/workflows/ci.yml` 用的是 `cargo test --locked`
（**没有** `--workspace`），所以加了 workspace 之后**根包仍是 `niripaper`，测试与 release
build 默认仍只覆盖它** —— gpui-kit 那 849 个 crate 不会进守护进程的 CI（现在 `timeout-minutes: 20`，
进了一定超时）。

### 4.7 守护进程不在时，应用还能不能改配置 —— **已定：能**（2026-10-12）

**schema 来自编译期，不走 socket。** 关键事实：`schema()` 是**纯函数**
（`src/schema.rs:379`，只依赖 `Config::default()`，无 IO、无运行期状态），所以
**守护进程能算的，UI 编译时也算得出来，不会漂移**。而它很小：27 键 = **8.3 KB** JSON。

于是 UI 建界面**不需要**守护进程：

| 场景 | 现在能怎样 |
| --- | --- |
| 守护进程没跑 | 能建界面、能改配置（写文件）✓ |
| **壁纸路径写错、守护进程起不来** | **正是最需要改配置的时刻** —— 现在能改 ✓ |

被否掉的替代：**UI 每次启动缓存一份 schema** —— 首次启动没有缓存，且与守护进程版本可能差一步。
编译期链接没有这两个问题。

**已知代价**：若守护进程更新而 UI 没重编，UI 会显示一个守护进程不认识的新键 →
写进去 → 守护进程报错。这条**本来就有**（[SPEC §5.2](../docs/SPEC.md)：解析失败保留旧配置
并报错），UI 把这个错误弹出来即可。

### 4.8 缩略图缓存 —— **已定**（2026-10-12）

**位置**：`$XDG_CACHE_HOME/niripaper/thumbnails/`（可 `rm -rf` 重建）。
**键**：`路径 + 大小 + mtime`，**不用纯内容哈希** —— 哈希要读整个文件，而图片可能很大；
`mtime` 会被 `cp`/`rsync`/解压改掉，所以**把路径与大小一起算进去**来消掉大部分误重渲。
**懒加载 + 渐进生成**：只为**可见**的网格项渲缩略图，后台补。
（视频要解码，不可一次性全渲。）

### 4.9 应用的名字 —— **已定**（2026-10-12）

| 项 | 值 |
| --- | --- |
| 显示名 | **NiriPaper** |
| 二进制 | `niripaper-app`（守护进程已占 `niripaper` 这个命令名） |
| `.desktop` / 托盘 | `niripaper-app.desktop`，图标名 `niripaper` |
| 包名 | `niripaper-app`（AUR 不能与已有 `niripaper` 冲突） |

**约束**：显示名是 **NiriPaper**，而**配置目录、socket、crate、守护进程命令名都仍是
小写 `niripaper`** —— 那是既有的、稳定的标识，不要因为改了显示名去动它们。

### 4.10 库目录 —— **已定**（2026-10-12）

**默认 `~/Pictures/Wallpapers`**（尊重 `$XDG_PICTURES_DIR`，即 `<PICTURES>/Wallpapers`）。
目录不存在 → 库为空，界面显示空状态（不是错误）。GUI 里可改。

### 4.11 支持的格式 —— **已定**（2026-10-12）

| 类别 | 扩展名 | 走哪条路 |
| --- | --- | --- |
| 图片 | `png` / `jpg` / `jpeg` / `webp` | `image` crate（按内容探测） |
| 视频 | `mp4` / `webm` / `mkv` / `mov` / `m4v` / `avi` | mpv |
| **GIF** | `gif` | **mpv**（当视频）—— 见下 |

**为什么 GIF 当视频**：`image` crate 编译时没开 gif feature，所以动图当图片走是**硬错误**
（实测原文：`The image format Gif is not supported`），而 mpv 能播它（含动画）。
这也与 Wallpaper Engine 一致。

**硬要求**：`list` 只报扩展名在**支持列表**里的文件，且**支持列表是单一来源**
（`is_video` 与"是不是图片"都从同一个地方派生）—— 否则会出现"UI 说能点、点了把守护进程搞挂"。
用户壁纸目录里一定有杂物（`.txt`/`.md`/`.svg`/`.psd`），**扫进库就是让用户一点就崩**。

---

### 4.12 两件顺手的（**已定**，2026-10-12）

| 项 | 决定 | 为什么 |
| --- | --- | --- |
| 单实例 | UI 第二次启动**聚焦已有窗口**，不开第二个 | 否则两个 UI 同时写同一份配置 |
| 写入前备份 | UI 保存前留一份 `config.toml.bak` | `toml_edit` 万一出问题，用户能退回 |

---

## 五、已作废的旧设定

| 旧设定 | 现状 |
| --- | --- |
| niri `spawn-at-startup` 拉起守护进程 | **作废**，改自启动 |
| `config.d/` + 按文件名排序合并（ADR 0006） | **作废**，改单一 `config.toml` |
| Noctalia 插件镜像壁纸选择 | **作废**，插件废弃 |
| `config.d/noctalia.toml` | **作废**（连 `config.d` 一起） |
| PRD 非目标里的"不做自启管理、不做托盘" | **作废**，两者都要做 |
