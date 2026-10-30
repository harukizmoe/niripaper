# Issue tracker: GitHub

本仓库的 issue 与规格都放在 GitHub Issues 里（`github.com/harukizmoe/niripaper`）。所有操作走 `gh` CLI。

## 约定

- **建 issue**：`gh issue create --title "..." --body "..."`。多行正文用 heredoc。
- **读 issue**：`gh issue view <number> --comments`，用 `jq` 过滤评论，并一并取标签。
- **列 issue**：`gh issue list --state open --json number,title,body,labels,comments --jq '[.[] | {number, title, body, labels: [.labels[].name], comments: [.comments[].body]}]'`，配合 `--label` / `--state` 过滤。
- **评论**：`gh issue comment <number> --body "..."`
- **加 / 去标签**：`gh issue edit <number> --add-label "..."` / `--remove-label "..."`
- **关闭**：`gh issue close <number> --comment "..."`

仓库由 `git remote -v` 推断；在 clone 里跑 `gh` 会自动认出来。

## 关于分支（本仓库特有）

`main` 是发布线，`dev` 是开发线，**不在 `main` 上开发**。issue 里提到的代码改动默认落在 `dev`；
PR 只按里程碑从 `dev` 向 `main` 建、用 squash 合入——细节见 `AGENTS.md` 的「工作流（git）」一节。
本文件只规定 issue 读写，不改变上面那套分支规矩。

## PR 作为 triage 入口

**PRs as a request surface: no.**（若本仓库把外部 PR 也当作需求入口，改成 `yes`；`/triage` 读这个标志。）

设为 `yes` 时，PR 走与 issue 相同的标签与状态，用对应的 `gh pr` 命令：

- **读 PR**：`gh pr view <number> --comments`，`gh pr diff <number>` 看 diff。
- **列出待 triage 的外部 PR**：`gh pr list --state open --json number,title,body,labels,author,authorAssociation,comments`，只保留 `authorAssociation` 为 `CONTRIBUTOR` / `FIRST_TIME_CONTRIBUTOR` / `NONE` 的（丢掉 `OWNER` / `MEMBER` / `COLLABORATOR`）。
- **评论 / 打标签 / 关闭**：`gh pr comment`、`gh pr edit --add-label` / `--remove-label`、`gh pr close`。

GitHub 的 issue 与 PR 共用一个编号空间，所以裸 `#42` 可能两者之一：先用 `gh pr view 42` 解析，再回退到 `gh issue view 42`。

## 当某个 skill 说 "publish to the issue tracker"

建一个 GitHub issue。

## 当某个 skill 说 "fetch the relevant ticket"

跑 `gh issue view <number> --comments`。

## Wayfinding 操作

供 `/wayfinder` 使用。**map** 是一个 issue，**child** issue 是它的 ticket。

- **Map**：单个打了 `wayfinder:map` 标签的 issue，正文里放 Notes / Decisions-so-far / Fog。`gh issue create --label wayfinder:map`。
- **Child ticket**：作为 map 的子 issue 关联（`gh api` 走 sub-issues 端点）。在未启用 sub-issues 的地方，把 child 加进 map 正文的任务列表，并在 child 正文顶部写 `Part of #<map>`。标签：`wayfinder:<type>`（`research` / `prototype` / `grilling` / `task`）。认领后把 ticket 指派给负责的开发者。
- **阻塞关系**：用 GitHub **原生 issue 依赖**，这是唯一能在 UI 里看见的表示。加边用 `gh api --method POST repos/<owner>/<repo>/issues/<child>/dependencies/blocked_by -F issue_id=<blocker-db-id>`，其中 `<blocker-db-id>` 是阻塞者的数字 **database id**（`gh api repos/<owner>/<repo>/issues/<n> --jq .id`，_不是_ `#number`，也不是 `node_id`）。GitHub 用 `issue_dependencies_summary.blocked_by` 报告（只含未关闭的阻塞者，是实时的门）。依赖功能不可用时，回退到在 child 正文顶部写 `Blocked by: #<n>, #<n>`。所有阻塞者都关闭时，ticket 才算解除阻塞。
- **前沿查询**：列出 map 下所有未关闭的 child（`gh issue list --state open`，限定在 map 的 sub-issues / 任务列表内），丢掉任何存在未关闭阻塞者的（`issue_dependencies_summary.blocked_by > 0`，或 `Blocked by` 行里还有未关闭 issue）、以及已有指派的；按 map 顺序取第一个。
- **认领**：`gh issue edit <n> --add-assignee @me`，这是本次会话的第一次写入。
- **解决**：先 `gh issue comment <n> --body "<answer>"`，再 `gh issue close <n>`，然后把上下文指针（要点 + 链接）追加到 map 的 Decisions-so-far。
