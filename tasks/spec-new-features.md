# 规格说明书（SPEC）与任务拆分：新功能管线

> 对应 GitHub Issues: #17, #18, #19, #20  
> 日期：2026-10-30  
> 状态：Ready for Implementation  
> 模式：Spec-Driven Development

---

## 目录
1. [功能一（#17）：应用明暗主题支持跟随系统自动切换](#功能一17应用明暗主题支持跟随系统自动切换)
2. [功能二（#18）：视差跟随与总览动画解耦并提供独立开关](#功能二18视差跟随与总览动画解耦并提供独立开关)
3. [功能三（#19）：壁纸后期滤镜系统（着色器、配置与抽屉）](#功能三19壁纸后期滤镜系统着色器配置与抽屉)
4. [功能四（#20）：壁纸定时轮播与播放列表机制](#功能四20壁纸定时轮播与播放列表机制)
5. [执行路线与依赖关系](#执行路线与依赖关系)

---

## 功能一（#17）：应用明暗主题支持跟随系统自动切换

### 1.1 现状与问题
- 目前 `App.tsx` 仅在组件初次加载时通过 `matchMedia('(prefers-color-scheme: dark)')` 读取一次系统明暗偏好，随后写入 `localStorage('niripaper_dark')`。
- 用户只能在侧栏底部通过点击按钮在硬编码的浅色与深色之间二选一。
- 系统或桌面环境（如 Noctalia 太阳日程、系统设置）切换明暗时，应用毫无感知，无法自动跟随。

### 1.2 架构与设计规格
1. **状态模型**：
   - 引入三态主题模式：`ThemeMode = 'system' | 'light' | 'dark'`。
   - 持久化键名：`localStorage('niripaper_theme_mode')`，默认值 `'system'`。兼容迁移已有 `niripaper_dark`。
   - 系统动态状态：`systemDark: boolean`，通过监听 `window.matchMedia('(prefers-color-scheme: dark)')` 的 `change` 事件实时更新。
   - 解析计算：`isDark = themeMode === 'system' ? systemDark : themeMode === 'dark'`。
2. **交互体验**：
   - **侧栏底部快捷按钮**：图标与提示响应三态（`system` 显示自动/跟随系统图标，`light` 显示太阳图标，`dark` 显示月亮图标），点击在 `system` -> `dark` -> `light` 间快速轮转。
   - **设置页「界面外观」分组**：新增「外观模式」设置行，提供 Segmented Button（分段控制器）：`[跟随系统 | 浅色 | 深色]`。

### 1.3 任务拆分
- [x] **Task 1.1**：在 `App.tsx` 中建立 `ThemeMode` 三态状态机与 `matchMedia` 事件监听器。
- [x] **Task 1.2**：在 `Rail.tsx` 中升级侧栏主题按钮，支持三态循环切换与 M3 提示。
- [x] **Task 1.3**：在 `SettingsView.tsx` 增加外观模式分段控制行，与全局状态同步。
- [x] **Task 1.4**：自检与回归测试（验证系统主题动态触发时页面即时重绘，无闪烁）。

---

## 功能二（#18）：视差跟随与总览动画解耦并提供独立开关

### 1.1 现状与问题
- 在 `ParallaxView.tsx` 中，「跟随窗口移动壁纸」开关通过执行 `scale = 1.0` 来停用视差。
- 在 `src/scene.rs` 的 `Scene::view` 中，总览缩放计算公式为：
  ```rust
  let scale = (scale * zoom).max(1.0);
  ```
  当 `scale == 1.0` 时，总览打开时的 `zoom`（例如 0.96）相乘后被强制 clamp 在 1.0，导致关掉视差移动时，**总览动画也被连带彻底掐死**。
- 此外，「总览」分组缺乏显式的总览动画开关，且视差关闭时整个页面被错误地灰显禁用。

### 1.2 架构与设计规格
1. **守护进程与配置层（`crates/proto` + `src/`）**：
   - 在配置中引入显式布尔字段：`parallax: bool`（配置键 `parallax`，默认 `true`）。
   - 守护进程 `Scene::view` 或 `motion` 计算：
     - 当 `parallax == false` 时，视差位移量恒定为 0（`progress` 固定为居中或 `offset_px` 归零），壁纸不随焦点窗口或工作区平移。
     - 画布缩放保持 `scale` 配置值（如 1.15），为总览缩小（zoom 0.96）预留足够的画布外延。总览动画正常进行。
   - 在 `crates/proto/src/schema.rs` 暴露 `parallax`（Bool），热重载即时生效。
2. **总览动画独立开关**：
   - 支持独立开关控制总览动画。如果关闭总览动画，`zoom` 固定为 1.0，不再触发缩放。
3. **前端 UI（`ParallaxView.tsx`）**：
   - 「移动」分组的「跟随窗口移动壁纸」改为控制 `parallax` 键，不再强行写入 `scale = 1.0`。
   - 「移动幅度」等几何参数在 `parallax` 开启时可调。
   - 「总览」分组拥有独立的「启用总览缩放动画」Switch 开关，不再受视差开关影响。两者完全解耦。

### 1.3 任务拆分
- [ ] **Task 2.1**：在 `crates/proto` 配置与 schema 中新增 `parallax: bool` 字段及读写验证。
- [ ] **Task 2.2**：在 `src/daemon.rs` 与 `src/scene.rs` 中应用 `parallax` 配置：当 `parallax` 关闭时视差位移归零，总览 zoom 保持独立运算。
- [ ] **Task 2.3**：在 `crates/proto` 与守护进程中为总览缩放提供显式启闭开关逻辑。
- [ ] **Task 2.4**：重构 `ParallaxView.tsx`，将移动开关与总览开关解耦，解除不必要的联动禁用。
- [ ] **Task 2.5**：单元测试与实机验证（验证关闭窗口跟随移动时，总览依然能够流畅缩放）。

---

## 功能三（#19）：壁纸后期滤镜系统（着色器、配置与抽屉）

### 1.1 现状与问题
- `tasks/backlog-wallpaper-features.md` 记录：旧抽屉里的滤镜（亮度、对比度、饱和度、色相、暗角、颗粒）只是前端 CSS 预览，点应用后从未生效。
- 维护者要求滤镜在**真正的系统桌面壁纸上生效**。

### 1.2 架构与设计规格
1. **数据模型（按规格收敛）**：
   - 采用单一全局配置 `[filter]`，支持 per-output 覆盖（`[outputs.<name>.filter]`）。
   - 六个维度与默认值（默认值严格等于无效果）：
     - `brightness: f32` (默认 1.0, 范围 0.5..1.5)
     - `contrast: f32` (默认 1.0, 范围 0.5..1.6)
     - `saturation: f32` (默认 1.0, 范围 0.0..2.0)
     - `hue: f32` (默认 0.0, 范围 0.0..360.0 度)
     - `vignette: f32` (默认 0.0, 范围 0.0..1.0)
     - `grain: f32` (默认 0.0, 范围 0.0..1.0)
2. **着色器渲染管线（`src/render/gl.rs`）**：
   - 在壁纸合成片段着色器中注入滤镜逻辑。
   - **性能门控（重要）**：当全部参数处于默认值时，着色器使用单一分支直接跳过滤镜运算，保证 0 性能损耗。
   - 静态图与 mpv 视频壁纸统一在此管线处理。
3. **协议与配置（`crates/proto`）**：
   - 增加 `filter.*` schema 字段，支持 hot reload。
4. **前端抽屉交互（`WallpaperDrawer.tsx`）**：
   - 新增「后期滤镜」折叠或设置行分组，提供滑块微调与「恢复默认滤镜」按键。
   - 滑块调节即时触发预览/热重载。

### 1.3 任务拆分
- [ ] **Task 3.1**：在 `crates/proto` 中设计 `FilterConfig` 结构体、默认值、验证逻辑与 schema 导出。
- [ ] **Task 3.2**：在 `src/render/gl.rs` 中编写滤镜算法（RGB-HSV 色相调整、亮度对比度饱和度矩阵、暗角径向衰减与伪随机噪点），并增加 uniforms。
- [ ] **Task 3.3**：在 `src/scene.rs` 和 `src/daemon.rs` 中装配滤镜配置并传入着色器。
- [ ] **Task 3.4**：在 `WallpaperDrawer.tsx` 中实现滤镜调节 UI，接入通用配置读写与重置功能。
- [ ] **Task 3.5**：使用 `src/bin/trprobe.rs` 及真机验证滤镜绘制效果及默认状态性能基线。

---

## 功能四（#20）：壁纸定时轮播与播放列表机制

### 1.1 现状与问题
- `tasks/backlog-wallpaper-features.md` 记录：旧抽屉里的「加入轮播列表」无后台支撑。
- 守护进程缺乏定时轮换壁纸的自主循环。

### 1.2 架构与设计规格
1. **数据模型与配置**：
   - 在配置中增加 `[playlist]` 结构：
     - `items: Vec<PathBuf>`：轮播壁纸路径列表。
     - `interval_mins: u32`：轮播间隔时间（分钟，默认 15）。
     - `order: "rotate" | "random"`：轮播顺序。
     - `enabled: bool`：轮播总开关。
2. **守护进程轮播定时器**：
   - 在守护进程事件循环中维护轮播定时器。
   - 到期自动调用现有的 `switch_wallpaper`，无缝衔接当前配置的换图过渡动画（portal/fade/wipe等）。
3. **控制协议（`crates/proto/src/protocol.rs`）**：
   - 增加请求类型：`Request::PlaylistAdd { path }`, `Request::PlaylistRemove { path }`, `Request::PlaylistNext`。
4. **前端交互**：
   - 抽屉「加入轮播列表」接入 `PlaylistAdd` 指令。
   - 壁纸库顶部工具栏或专属入口展示当前播放列表弹窗/面板，支持调整轮播间隔、移除壁纸与手动下一张。

### 1.3 任务拆分
- [ ] **Task 4.1**：在 `crates/proto` 中定义 `PlaylistConfig` 与控制协议指令。
- [ ] **Task 4.2**：在 `src/daemon.rs` 中实现轮播定时推进器，到期自动调度过渡切图。
- [ ] **Task 4.3**：在 `app/src/commands.rs` 封装轮播操作指令供前端调用。
- [ ] **Task 4.4**：在 `WallpaperDrawer.tsx` 中接通「加入轮播列表」。
- [ ] **Task 4.5**：在 `WallpaperView.tsx` 中增加轮播管理界面（查看列表、设置间隔、立即切换）。
- [ ] **Task 4.6**：端到端轮播验证（确认到达时间后平滑播放换图动画并切到下一张）。

---

## 执行路线与依赖关系

```
[Issue #17: 主题自动跟随] (纯前端独立，无后端阻碍，优先执行)
           │
           ▼
[Issue #18: 视差与总览动画解耦] (涉及 proto 与 daemon 小幅解耦重构)
           │
           ▼
[Issue #19: 壁纸后期滤镜系统] (涉及 GPU GL Shader 与滤镜控制)
           │
           ▼
[Issue #20: 壁纸定时轮播机制] (涉及 daemon 定时状态机与播放列表)
```

每个 Issue 作为一个独立的里程碑阶段在 `dev` 分支递进开发，完成后按规范进行 Code Review 并准备合入。
