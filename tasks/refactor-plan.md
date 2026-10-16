# 重构计划：niripaper → workspace + Tauri 桌面应用

> 2026-10-16。依据：`tasks/decisions.md` §〇（Tauri v2 + web 前端）与 §4.6（Cargo workspace），
> PRD `tasks/prd-niripaper-gui.md`（FR-1 薄客户端、FR-2 只走控制 socket）。
> 本文档是一次性施工单：完成后勾掉，不作为长期规范；长期约定归 `AGENTS.md` 与 ADR。

## 一、目标目录树（最终形态）

```
niripaper/
├── Cargo.toml                  # [workspace]：default-members = ["."（根包）, "crates/proto"]
├── Cargo.lock                  # workspace 共用一份
├── crates/
│   └── proto/                  # niripaper-proto：UI 与守护进程共用的纯逻辑，零 IO 之外的依赖
│       ├── Cargo.toml          # serde / serde_json / toml（toml 仅为 shared 编辑需要时再加）
│       └── src/
│           ├── lib.rs          # pub use 汇总
│           ├── domain.rs       # ← render/anim.rs 的 Curve/Animation/Spring、render/transition.rs
│           │                   #   的 Effect/Selection/Direction/Settings、render/mod.rs 的 Fit、
│           │                   #   motion.rs 的 DEFAULT_* / MAX_SCALE 常量（值类型与名字，无渲染数学）
│           ├── config.rs       # ← src/config.rs（Config/Animations/OutputOverride/OutputParams/
│           │                   #   解析器/validator/default_path/describe_source）
│           ├── schema.rs       # ← src/schema.rs（schema()/keys()/Kind/Key/PER_OUTPUT）
│           ├── protocol.rs     # ← src/ipc.rs 的 Request/parse/reply/request/默认与查找 socket 路径
│           ├── state.rs        # ← daemon.rs 的 state_json()（Snapshot → JSON 的纯函数）
│           └── client.rs       # UnixStream socket 客户端（一行进一行出），UI 的唯一 IO 通路
├── src/                        # 守护进程 niripaper（EGL/GBM/dmabuf/libmpv/wayland），模块名不变
│   ├── bin/                    # niripaper CLI + 4 个诊断探针（AGENTS.md：保留）
│   └── …
├── app/                        # niripaper-app：Tauri v2 壳（原型 tasks/prototype-tauri 迁入）
│   ├── Cargo.toml              # tauri 2 (tray-icon) + niripaper-proto
│   ├── build.rs  tauri.conf.json  capabilities/default.json  icons/  gen/
│   ├── src/
│   │   ├── main.rs             # Builder + 托盘（US-014：打开主窗口 / 退出）
│   │   ├── state.rs            # 连接生命周期：socket 路径、重连、库根目录
│   │   └── commands.rs         # #[tauri::command]：schema / state / set / query / list / thumbnail
│   └── ui/                     # 前端：Vite + React + TypeScript + Tailwind v4 + MUI v7
│       ├── index.html  package.json  package-lock.json  vite.config.ts  tsconfig.json
│       └── src/
│           ├── main.tsx  App.tsx  index.css
│           ├── theme/          # prototype-ui.html 的 M3 Expressive 令牌 → MUI theme + CSS vars
│           ├── components/     # Rail / SettingsPage / StatePage / LibraryGrid / PropertyPanel …
│           ├── hooks/          # useDaemon（invoke/listen 封装、连接状态）
│           └── lib/            # schema 类型镜像、请求封装
├── tests/                      # 现空，保留
├── docs/  CONTEXT.md  AGENTS.md  README*.md   # 不动（AGENTS.md 的 workflow 节随后补 workspace 说明）
├── packaging/aur/              # 后续：niripaper-app 打包（不在本次范围）
└── tasks/                      # PRD / decisions / 一次性原型保留 prototype-ui.html 作设计参照
```

## 二、搬移清单（旧 → 新）

| 旧位置 | 新位置 | 动作 |
| --- | --- | --- |
| `render/anim.rs` 的 Curve/Animation/Spring/Lerp/Animator | domain（值类型）/ 守护进程保留 Animator 等**带状态的** | **拆分**：纯值类型进 proto，运行期状态机留 render/ |
| `render/transition.rs` 的 Effect/Selection/Direction/Settings | proto::domain | 同上，Transition 状态机留守护进程 |
| `render/mod.rs` 的 Fit | proto::domain（image.rs 等仍用） | 移动 + re-export 过渡期由守护进程 `pub use` |
| `motion.rs` 的 DEFAULT_SCALE 等常量 | proto::domain | 移动；Motion 状态机留守护进程 |
| `src/config.rs` 整体 | proto::config | 移动；`load()` 的 default_path 留 proto（纯 env 逻辑） |
| `src/schema.rs` 整体 | proto::schema | 移动（它的 crate::config 依赖随迁自然满足） |
| `src/ipc.rs` 的 Request/parse/reply/request | proto::protocol | 移动；Server（UnixListener accept 循环）留守护进程 |
| `daemon.rs` 的 state_json() | proto::state | 移动为纯函数：入参从 `&Snapshot` 改为显式字段结构 |
| `tasks/prototype-tauri/` | `app/` | git mv（内容入库）；tauri.conf frontendDist → `ui/dist` |
| `tasks/prototype-ui.html` | 保留原位 | 设计参照；令牌移植进 `app/ui/src/theme/` 后仍不删 |

## 三、删掉的东西（已弃用/无关）

| 项 | 理由 |
| --- | --- |
| `tasks/prototype-native/`（Go 手绘原型） | mygo 路线已作废（decisions §〇之一 → §〇） |
| `tasks/prototype-webui/`（Go webui 包装） | 同上；tauri 原型已验证 webview 通路 |
| `tasks/prototype-tauri/target/`、`gen/` 生成物 | 构建产物不进仓库；gen/ 由 tauri-build 重新生成 |
| `tasks/prototype-webui/niripaper-webui`、`prototype-native/niripaper-prototype` | 9.8MB + 13.4MB 编译产物 |

## 四、验收标准（全过才算完）

1. `cargo fmt --check` / `cargo clippy --all-targets -- -D warnings` / `cargo test` 在 workspace
   default-members（根包 + proto）全绿 —— CI 语义不变，runner 无 webkit2gtk 也能过。
2. `cargo build -p niripaper-app` 在本机通过（原型已证依赖齐全）；前端 `npm run build` 过。
3. daemon 行为不变：`./target/release/niripaper daemon` + `niripaper schema` / `state` / `set` 冒烟
   （改动横跨 config/ipc/schema，必须实机跑 —— AGENTS.md 硬性要求）。
4. schema 输出与重构前逐字节一致（`niripaper schema` 的 JSON diff 为空）——证明抽离没有改变对外契约。
5. CI yml 增加 app job（webkit2gtk + node，`cargo clippy -p niripaper-app` + `npm run build`）。
6. dev 提交（中文，`refactor: …`），decisions.md 补一条组件库选型记录；AGENTS.md 工作流节补
   workspace 构建说明（`-p niripaper-app` / default-members 语义 / 前端构建）。

---

## 五、执行记录（2026-10-16 深夜）

全部完成并验证：

- [x] workspace 三 crate 落地（`default-members = [".", "crates/proto"]`）
- [x] proto：config/schema 原样搬入；anim/transition 值类型拆出（状态机留守护进程）；
      protocol/client 拆出；niri KDL 读取器随迁；state.rs（state_json + config_values）新建
- [x] 守护进程 re-export shim 保持 `crate::config`/`crate::schema`/`render::Fit` 路径不变
- [x] **契约验证**：`niripaper schema` 输出与重构前逐字节一致（离线 dump + 实机各 diff 一次）
- [x] 实机冒烟（release 二进制，`--socket /tmp/niripaper-refactor-test.sock`）：
      `schema` / `state` / `set`（切真壁纸，`query` 回 `h/v`）/ 坏路径（日志报错、画面不变）/
      `kill`（socket 清理）全过
- [x] app/：main.rs（托盘 US-014）+ state.rs（parking_lot RwLock）+ commands.rs（7 个 command）；
      capabilities/default.json；tauri.conf 指向 ui/dist
- [x] app/ui：Vite + React 19 + TS + Tailwind v4 + MUI v7；M3 令牌全量移植；Rail/四个 section；
      useDaemon hook。`npm run build` 过；**实机截图验证**（niri 窗口、托盘、未连接态、M3 观感）
- [x] 守护进程死重清理：src/niri_config.rs（已迁 proto）、根包 kdl/toml 依赖移除
- [x] CI：check job 不动；app job（webkit2gtk + Node 24 + npm ci/build + clippy -D warnings）
- [x] 全绿：fmt ✓ / clippy（默认成员 + app）✓ / cargo test 132+55 ✓ / release build ✓
