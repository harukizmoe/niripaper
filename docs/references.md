# 外部参考资料

niripaper 依赖或借鉴的外部资料，以及项目内部的约定入口。每条一行：**是什么 / 为什么看它**。随机器走的事实不在这里。

## 运动规格的来源

- **Clavis**（`StatIndet/quickshell`）：https://github.com/StatIndet/quickshell —— 本项目的运动参数与公式是对它**行为**的复刻（未复制其源码）；"Clavis 位置 → 本项目取值"的对照表在 [`SPEC.md`](./SPEC.md) 的 §4。

## niri 与 Wayland 协议

- **niri 的 GPU 模型与 dmabuf feedback**：`YaLTeR/niri` tag `v26.04`，`src/backend/tty.rs`（`surface_dmabuf_feedback()`、`primary_render_node`、`GpuManager`）—— 合成器在哪块 GPU 上画、又把结果拷到哪里，是 [`adr/0002`](./adr/0002-follow-compositor-main-device.md) 那条规则的一手来源；来源文件：https://github.com/YaLTeR/niri/blob/v26.04/src/backend/tty.rs
- **niri layer rules**：https://github.com/YaLTeR/niri/wiki/Configuration:-Layer-Rules#place-within-backdrop —— `place-within-backdrop` 的语义与版本要求，用户给 niripaper 加规则时照它写。
- **niri backdrop 定制**：https://github.com/YaLTeR/niri/wiki/Overview#backdrop-customization —— backdrop 里怎么摆层、层序与创建顺序的关系。
- **niri IPC**：https://github.com/YaLTeR/niri/wiki/IPC —— 连 `$NIRI_SOCKET`，写一行 JSON、读一行 JSON；`"EventStream"` 先给全量状态再持续推增量。
- **linux-dmabuf feedback**：`linux-dmabuf-v1.xml`（`main_device` / `format_table` / `tranche_*` 的语义）—— 本项目选渲染设备与可用 modifier 的依据（[`SPEC.md`](./SPEC.md) §2.6、[`adr/0002`](./adr/0002-follow-compositor-main-device.md)）；上游文件：https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/stable/linux-dmabuf/linux-dmabuf-v1.xml

## 渲染与视频

- **mpv 手册**（`man mpv`）：`int frame` uniform（用户 shader 里全局可见的帧计数）、`MPV_RENDER_API_TYPE_OPENGL`（本项目用的 render API 类型）、`--swapchain-depth`（在飞帧数上限，默认 2，直接决定帧延迟与流水线）—— 内嵌 libmpv 时按这三项查；在线副本：https://mpv.io/manual/master/
- **mpvpaper**：https://github.com/GhostNaN/mpvpaper —— 用 libmpv 做壁纸的参照实现（怎么建 render context、怎么按输出摆放与释放）。
- **awww / swww**：https://codeberg.org/LGFae/awww —— Rust 壁纸守护进程；其"外部守护进程 + 桌面壳插件镜像用户的选择 + 撤掉原生壁纸层"的集成模式可借鉴。同一作者的前身 swww（已归档）：https://github.com/LGFae/swww

## 项目内部

- **GPUI Kit**（计划中的 GUI）：https://github.com/longbridge/gpui-kit —— 组件库（`gpui-kit` crate）；项目级 skills 在 `.omp/skills/`（`gpui-kit`、`gpui-kit-design-guides`），**只在本地、被 `.gitignore` 忽略**，动 UI 之前先读它们的 `SKILL.md`。
- **规格工作流 skills**（本机 agent 环境提供，不进仓库）：`/prd` → `/prd-to-spec` → `/to-issues`（PRD / SPEC 落在 `tasks/`，issue 落在 GitHub Issues，agent 配置见 [`agents/`](./agents/)）；实现收尾走 `/review-it` → `/note-it` → `/ship-it`，批量实现用 `/loop-it`，术语与 ADR 由 `/domain-modeling` 维护。
