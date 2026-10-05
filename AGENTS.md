# AGENTS.md — niripaper 开发约定

给在本仓库工作的 coding agent。**开发约束都在这里**；设计规格、已定决策、里程碑进度在维护者
本地的 `HANDOFF.md`（不进仓库，见下），代码注释里的 `§x.y` 指向它。

---

## 本机环境约束（硬性，不要违反）

- **不要改动 `~/.config/niri/**` 或 `~/.config/noctalia/**`。** 需要 layer rule 时：让维护者自己加，
  或**临时用 `--namespace mpvpaper`** 去命中本机已存在的 `^mpvpaper$` 规则（零配置改动地验证 backdrop 行为）。
- **这台桌面正在使用中。** 探针、临时文件、socket 都必须自己收尾：只杀自己起的进程、
  删自己建的文件，不留后台进程；临时切过工作区/焦点就要切回原处。
- **`$NIRI_SOCKET` 在 agent shell 里是过期的**（每次登录换名字）。取当前的：
  `NIRI_SOCKET=$(ls -t /run/user/1000/niri.*.sock | head -1)`，否则 `niri msg` 报 "No such file or directory"。
- `socat` 未安装（用 OpenBSD 版 `nc`）。
- **下结论前先确认指标可信**：`--dump-stats` 是抽样输出、`nvidia-smi` 在桌面日常负载下噪声极大
  （基线 26–65%）。不要用它们宣称"更省"或"更流畅"；可用的测量方法见本地 `HANDOFF.md` §8。

## 构建与自检

```sh
cargo fmt --check
cargo clippy --all-targets      # 必须 0 警告
cargo test
```

这三条**在 CI 里也会跑**（`.github/workflows/ci.yml`，push 到 `main`/`dev` 与所有 PR），
外加一个 release 构建 ✓。所以本地跑通不等于一定能过 —— CI 用的是**最新的 stable 工具链**，
偶尔会有新 lint 冒出来 ✓；`clippy` 在 CI 里带 `-D warnings` ✓，任何警告都是失败 ✓。

CI 里要装系统依赖 ✓，因为本项目**手写 FFI** 直接链接这些库 ✓：
`libmpv`（视频 render API）、`libEGL`/`libGL`（渲染器）、`libgbm`（dmabuf 分配）、
`libwayland-client`（图层表面）✓。测试是纯逻辑 ✓（无 GPU / 无合成器 / 无显示 ✓），
所以在什么显卡都没有的 runner 上照样跑 ✓。

装到 `PATH`：`cargo install --path . --locked --bin niripaper` —— **必须带 `--bin niripaper`**，
否则 `src/bin/` 下的那些诊断探针也会被一起装进 `~/.cargo/bin`。
**改完代码要重装**：`PATH` 上的 `niripaper` 与 `target/release/niripaper` 是两份拷贝 ✗，
自启动（`spawn-at-startup`）跑的是前者 ✗。实测踩过：新增配置键后只重建没重装，
自启动拿着旧二进制报 `unknown field` 直接退出，桌面连壁纸都没了 ✗（手动跑 `./target/release/…`
却一切正常，正好把问题掩盖掉 ✗）。

改动渲染路径或事件解析后，**必须实机跑一次**：启动 `./target/release/niripaper daemon --trace`，
观察逐帧的 `h` / `v` / `zoom` 与目标值，而不是只看编译通过。改动配置项时，用**非默认值**验证
它真的生效（历史上出现过"配置键写了但从未接上线"）。

## 工作流（git）

- `main` = 发布线，`dev` = 开发线。**绝不在 `main` 上开发**；`dev` 保留不删。
- **`main` 的历史必须干净、简洁、有意义**：一个里程碑一条提交，`main` 上**不出现**
  "改错别字 / 补文档"这类修补提交。里程碑发布后发现的小问题，改在 `dev` 上，
  随**下一个**里程碑 PR 一起 squash 进去，不要为它单独建 PR。
  PR 标题会成为 `main` 上那条提交的标题，所以要能独立说明这个里程碑做了什么。
- **PR 按里程碑建**：一个里程碑完成时才从 `dev` 向 `main` 建 PR，用 **squash** 合入
  （`main` 上只留一条提交）。里程碑之间的小修小改——文档措辞、拼写、注释、`.gitignore`、
  小 bug 修正——**直接提交本地 `dev` 分支即可，不建 PR**。
  里程碑：M0 渲染地基（EGL/GBM/dmabuf 交换链）、M1 niri IPC + 视差、M2 静态图 / 总览过渡 / 换图过渡、
  **M3 视频壁纸**（libmpv 硬解 + `video_fps` + 控制 socket）、M4 Noctalia 插件、
  M5 开源收尾（README / CI / AUR / Releases）。详见本地 `HANDOFF.md` §6。
- **每次 squash 合入之后都要 back-merge**：`git checkout dev && git merge origin/main`。
  squash 之后 `dev` 的提交不再是 `main` 的祖先，不做这一步下一个 PR 会带一堆旧提交并报 `CONFLICTING`。
  冲突通常只在 README / LICENSE / Cargo.toml 这类两边都改过的文件上，取 `dev` 的版本（它更新）：
  `git checkout --ours -- <files>`。判断是否需要：`git merge-base --is-ancestor origin/main origin/dev || echo 需要`。
- **提交信息**：`dev` 上的开发提交用**中文**，`type: 摘要` 形式（`feat:` / `fix:` / `docs:` /
  `chore:` / `refactor:`），正文写清"为什么"以及实测数据。**`main` 上的提交用英文**——
  它是面向外部读者的历史，而且那条提交的标题就是 PR 标题，所以 PR 标题要用英文写。
  （唯一的例外是 `ab4be5a`，它是 `dev` 与 `main` 的共同根，改成英文会让两边变成无关历史。）

## 代码与文档约定

- **按功能拆分模块，不要把代码堆进一个文件**：`daemon.rs` 只放 `Options`、事件循环与日志；
  媒体类型与路由在 `media.rs`，换图过渡在 `crossfade.rs`，一帧的绘制在 `scene.rs`。
- **绘制输入收成结构体**（`scene::Scene`）：加一个参数时不要让它波及每个调用点 ——
  实测过，加 fade 时 `daemon.rs` 与 `src/bin/m0b.rs` 都得跟着改。
- **别让模块互相知道对方的细节**：`media` 不暴露它有几个变体给 daemon（用 `content()`/`pump()`/
  `wakeup_fd()` 这些动词），`crossfade` 不碰缓冲池（快照由调用方填）。
- `HANDOFF.md` 是维护者本地的设计与进度文档，**不进仓库**（已在 `.gitignore` 里）。不要把它提交，
  也不要把它当成给外部读者的文档。
- 代码注释里的 `§x.y` 指向 `HANDOFF.md` 的章节，是内部可追溯约定，**保留**。
- 面向使用者的说明写在 `README.md`（英文）与 `README.zh-CN.md`（中文）里，两份**逐节对应**；
  改一份就要同步另一份。措辞用直白陈述，不要俏皮话，不要写未实现的功能。
- `src/bin/eglpin.rs`、`src/bin/m0b.rs` 是 M0 阶段的诊断探针（EGL 按设备绑定、dmabuf 通路），
  不是给使用者的工具，但**保留**：排查"这块 GPU 上 EGL/GBM 能不能用"时仍然有用。
- `src/bin/vidpin.rs` 也是：它把 mpv 解出来的一帧读回来看，排查"解码出来的画面对不对"用。
  `--fit cover|fit|stretch|center|tile` 可以逐个试填充模式，`--out` 存成 PNG —— 量黑边
  （逐行/逐列扫边缘）是验证 `fit` 最直接的办法，比截图快也不占屏幕。
- `src/bin/trprobe.rs` 同样保留：把每个换图过渡效果各渲一帧到离屏 dmabuf 并写成 PNG，
  `trprobe /dev/dri/renderD128 /tmp/tr`。**改 shader 之后用它看一眼** —— 单元测试看不见
  shader，而它已经抓到过三个真 bug：径向效果的 reach 误用整条对角线、honeycomb 用剪切取整
  得到的是平行四边形、以及纹理漏设 `GL_TEXTURE_MIN_FILTER` 导致快照采样恒为黑（后者意味着
  换图过渡一直在从黑淡入，只验 fade 数值是看不出来的）。
