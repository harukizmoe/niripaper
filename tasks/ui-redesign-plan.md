# 施工单：桌面应用界面重做（MUI v9 + M3 配色）

> 2026-10-29。一次性施工单：完成后勾掉，不作为长期规范；长期约定归 `AGENTS.md` 与 `tasks/decisions.md`。
> 起因：现有界面是 `prototype-ui.html` 的逐块搬运 —— 1063 行手写 CSS、256 处内联样式、手写控件，
> MUI 与 Tailwind 装了但一处没用；文案夹着 SPEC 章节号、英文括注、配置键名与 emoji，
> 还有几处展示了守护进程并不存在的功能。目标是一个**完整、克制的桌面应用**，不是演示页。

## 一、已定事项

| 项 | 决定 | 依据 |
| --- | --- | --- |
| 外观方向 | Material（M3 配色角色 + M3 形状/字阶） | 维护者 2026-10-29 选定 |
| 组件库 | **MUI v9**（`@mui/material` 9.5，已装） | 组件覆盖最全（含 v9 新增 NumberField），无障碍扎实；React 生态没有维护中的 M3 组件库 |
| 关键事实 | MUI 实现的是 **Material Design 2**，M3 支持 on hold（mui/material-ui#29345），不会来 | 调研 2026-10-29；所以 M3 观感靠主题映射，不是靠组件 |
| `@material/web` | 不用 | 2024-06 起维护模式 |
| 图标 | `@mui/icons-material`（Rounded 变体） | 替换手写 `<svg>`、emoji、Unicode 符号三套来源 |
| 运行环境 | WebKitGTK 2.54（本机）支持 `color-mix()` 与相对颜色语法 | 仍以 2.40 为最低基线，见第三节"衍生色" |

## 二、主题层（第一步，其余一切的地基）

- **颜色角色**：`theme/materialYou.ts` 的输出从 `--m3-accent-lime` 这类**外观名**改为 M3 **角色名**
  `--md-sys-color-{primary,on-primary,primary-container,…,surface,surface-container-{lowest,low,,high,highest},on-surface,on-surface-variant,outline,outline-variant,error,…}`。
  取色算法换成官方 HCT（见第七节第 3 条），输出直接是 M3 角色。
- **`theme/mui.ts`**：`createTheme({ cssVariables: { colorSchemeSelector: 'class' }, colorSchemes: { light, dark } })`，
  palette 引用上面的变量（`primary.main: 'var(--md-sys-color-primary)'` …）。换壁纸 = 改 `<html>` 上的变量，
  不重建 theme、不重渲染。
- **衍生色**（hover/pressed/state layer）：在 JS 里随方案一起算好写成变量，不依赖 MUI 的 `nativeColor`
  （它要相对颜色语法，2.40 基线不保证）。
- **形状**：M3 圆角阶梯 4 / 8 / 12 / 16 / 28 / full，替换现有 6 / 10 / 16 / 22 / 32。
- **层级用色调不用阴影**：卡片用 `surface-container-*` 区分，去掉 `shadow-float` 这类重阴影。
- **字体**：`"Roboto Flex", "Roboto", "Noto Sans CJK SC", "Source Han Sans SC", sans-serif`；
  去掉 Google Sans（专有字体，多数机器上没有，实际一直在回退）。字阶按 M3 type scale 映射进 `typography`。
- **组件覆写**（`components.*.styleOverrides`，只在这里写样式）：Button（全圆角、M3 高度）、Switch（M3 轨道+拇指）、
  Slider、ToggleButtonGroup（当 segmented button 用）、Card（filled/outlined 两种）、ListItem、Select、Snackbar、Tooltip。

## 三、页面骨架与文案规范

**骨架**
- 左侧改为 **M3 navigation rail**（约 80px，图标 + 下方短标签），四个目的地不变；明暗切换做成 rail 底部的图标按钮。
- 每页 = 顶栏（只有页面标题，必要时右侧一个页面级操作）+ 若干**设置分组**。
- 设置分组 = 一张 `surface-container-low` 卡片，内部是**设置行**：左边标签 + 一行说明，右边控件。
  **不再卡片套卡片**。

**控件映射（按 schema 的类型选，不按页面各自发挥）**

| 值类型 | 控件 |
| --- | --- |
| 每一行 | 值 ≠ 默认值时显示"重置为默认值"图标按钮 |
| 布尔 | Switch（标签描述"是什么"，不在按钮上写"已开启/未开启"） |
| 枚举 ≤ 4 项 | Segmented（ToggleButtonGroup） |
| 枚举 > 4 项 | Select |
| 有界数值 | Slider + NumberField，单位写在输入框后缀 |
| 效果池（多选） | 可选卡片网格，选中态用 M3 checkbox/勾选，不另造"加入池"徽章 |

**文案规范**
- 只用中文；术语以 `CONTEXT.md` 为准。
- 不出现：`§x.y`、英文括注（"(Transitions)"）、配置键名（"(column_span)"）、emoji、营销式形容词
  （"极其克制工整""零侵略感"）、写死的本机环境（"由 Noctalia 原生壁纸接管"）。
- 说明文字一句话讲清"调它会怎样"，不讲实现（"GPU 屏幕空间着色器混合"不给用户看）。

## 四、删改清单

| 现状 | 处理 | 理由 |
| --- | --- | --- |
| 过渡效果里的「Push 弹性推挤」卡片 | 删 | 守护进程没有这个效果；`push` 只是 portal 的参数 |
| 效果列表缺 `none` | 加 | 守护进程支持 |
| 抽屉里的亮度/对比度/饱和度/色相/暗角/胶片颗粒 | 删 | 只是 CSS 预览，守护进程没有这些键 |
| 视差页"纵横仿真沙盒"（上/下切工作区、打开总览、试演） | 删 | 纯模拟，且背景图从 unsplash 远程加载 |
| 视差页两个重复的"启用视差"按钮 | 合成一个 Switch | 同一件事出现两次 |
| `App.tsx` 写死的显示器列表 | 改为读 `state` 的 outputs | 占位数据 |
| 设置页写死的 Noctalia 文案 | 改为中性描述 | 本机事实不进界面 |
| 切页时左下角的"切换至：xxx"提示 | 删 | 无信息量，且遮挡明暗切换 |
| 写死的颜色（`#4d6006`、`rgba(214,248,88,…)` 等） | 全部改为角色变量 | 否则不跟随动态取色 |
| `video_fps`、`fit` 在界面上没有（或不完整的）控件 | 补上 | schema 里有、用户可调 |

## 五、迁移顺序（每步一个 `dev` 提交，每步结束应用都能跑）

1. [x] **主题层 + 外壳**：卸 Tailwind；装 `@material/material-color-utilities`；`theme/color.ts`、`theme/mui.ts`、ThemeProvider、navigation rail、动态取色接线。
2. [x] **系统设置页**：最简单（几乎全是开关），用它把"设置分组 + 设置行"模式定下来。
3. [x] **视差页**：套用模式；删沙盒与重复开关。
4. [x] **过渡页**：效果池网格 + 当前效果的参数分组；删 Push、加 none。
5. [x] **壁纸库 + 抽屉**：瀑布流结构保留（它是现在最好的一页），换掉筛选条、来源切换、输出选择、角标；删假滤镜。
6. **收尾**：删掉 `index.css` 里已无引用的旧样式与全部内联颜色；更新 `tasks/decisions.md`（MUI v7→v9，
   M3 只做配色映射这一事实）与 `AGENTS.md` 的前端栈描述。

## 六、验收

1. `npm run build`、`npm run lint`、`cargo clippy --all-targets -p niripaper-app -- -D warnings` 全过。
2. `grep` 组件源码：无 `§`、无 emoji、无十六进制/rgba 颜色字面量（主题文件除外）。
3. 浅色/深色 × 至少 3 张色相差异大的壁纸截图：所有控件都跟着变色，没有残留的固定绿/紫。
4. 键盘可走完每一页（Tab 顺序合理、焦点可见）。
5. Tauri 实机连守护进程：每页至少改一个**非默认值**，确认写进 `config.toml` 且守护进程生效
   （AGENTS.md："配置键写了但从未接上线"的教训）。

## 七、维护者答复（2026-10-29）

1. **保存模型：改了立即生效。** 每次改动立即写 `config.toml`（守护进程热重载），不设保存按钮。
   每个设置行在值 ≠ 默认值时显示"重置为默认值"按钮，默认值取自 `schema`（不在前端另写一份）。
   滑块/输入框拖动中不落盘，松手或停止输入后再写，避免一次拖动写几十次文件。
2. **Tailwind：卸掉。** 只保留 MUI 一套样式系统。
3. **取色算法：这次一起换成官方 `@material/material-color-utilities`（HCT）。**
   - 方案直接用库里的 `SchemeTonalSpot / SchemeContent / SchemeFruitSalad / SchemeRainbow / SchemeMonochrome / SchemeFidelity`
     （`faithful` 对应 Fidelity），并可顺势补上 Vibrant / Expressive / Neutral。
   - 源色提取改用库里的 `QuantizerCelebi` + `Score`：之前 `schemes.ts` 注释里"vibrant/muted 会换一个源色"
     正是 Score 的行为，自己的近似算法复刻不了，官方库可以。
   - 与 Noctalia 对齐：迁移后用同几张壁纸对比 `noctalia theme --scheme <name>` 的输出，记录偏差；
     `scripts/compare-schemes.mts` 改为对比官方库输出。
   - 风险：0.4.0 的 ESM 包内 import 不带扩展名，Node 直接加载会失败；Vite 打包正常。
     `compare-schemes.mts` 用一个 resolve hook 补 `.js`。
   - **实测（第一步完成后，4 张壁纸）**：noctalia 的 `m3-tonal-spot / m3-content / m3-fruit-salad /
     m3-rainbow / m3-monochrome` 与本应用输出**逐字节一致**（误差 0，原近似算法最大 11°）。
     noctalia 的 `faithful / vibrant / soft / muted / dysfunctional` 是它自己的算法、不是这个库，
     复刻不了；库里的 Fidelity/Vibrant/Expressive/Neutral 用 `m3-` 前缀命名，避免与之同名不同色。
