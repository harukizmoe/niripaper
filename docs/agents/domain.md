# Domain Docs

工程类 skill 在探索本仓库代码时，应当如何消费领域文档。

## 探索之前先读这些

- 仓库根部的 **`CONTEXT.md`**；
- 若存在仓库根部的 **`CONTEXT-MAP.md`**：它指向每个 context 各一份的 `CONTEXT.md`。读取与当前主题相关的每一份；
- **`docs/adr/`**：读与即将动手的区域相关的 ADR。多 context 仓库里还要看 `src/<context>/docs/adr/` 里 context 级的决策。

任何一个文件不存在时，**静默继续**。不要提示它缺失，也不要主动建议先创建它们。`/domain-modeling` skill
（经 `/grill-with-docs` 与 `/improve-codebase-architecture` 触发）会在术语或决策真正定下来时才按需创建。

## 文件结构

单 context 仓库（绝大多数仓库，**本仓库就是这种**）：

```
/
├── CONTEXT.md
├── docs/adr/
│   ├── 0001-event-sourced-orders.md
│   └── 0002-postgres-for-write-model.md
└── src/
```

多 context 仓库（根部存在 `CONTEXT-MAP.md`）：

```
/
├── CONTEXT-MAP.md
├── docs/adr/                          ← 系统级决策
└── src/
    ├── ordering/
    │   ├── CONTEXT.md
    │   └── docs/adr/                  ← context 级决策
    └── billing/
        ├── CONTEXT.md
        └── docs/adr/
```

## 用 glossary 的词汇

当输出里要提到某个领域概念时（issue 标题、重构提案、假设、测试名），使用 `CONTEXT.md` 里定义的词。
不要漂到 glossary 明确避开的同义词上。

如果需要的概念还不在 glossary 里，那是个信号：要么你在发明项目不使用的语言（重新考虑），
要么这里是真的空缺（记下来交给 `/domain-modeling`）。

## 与本仓库既有文档的关系

- **`AGENTS.md`** 是本仓库的开发约定（硬性规矩），优先于本文档；
- **本机环境事实不进仓库**（当前是哪块 GPU、哪条 path、哪个版本、实测数值）。那是每台机器各自的，
  维护者本地另有记录（仓库旁的 `~/workspace/rust/niripaperdocs/`）。仓库文档只写**稳定**的事。
- 面向使用者的说明在 `README.md`（英文）与 `README.zh-CN.md`（中文），两份逐节对应，改一份要同步另一份。

## 标记 ADR 冲突

如果输出与既有 ADR 相矛盾，显式指出来，不要悄悄覆盖：

> _与 ADR-0007（event-sourced orders）冲突，但值得重开，因为……_
