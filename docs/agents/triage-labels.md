# Triage Labels

各 skill 用五个 **canonical triage 角色**说话。本文件把这些角色映射到本仓库 issue tracker 里实际使用的标签字符串。

| Label in mattpocock/skills | Label in our tracker | Meaning                                  |
| -------------------------- | -------------------- | ---------------------------------------- |
| `needs-triage`             | `needs-triage`       | 维护者需要评估这个 issue                    |
| `needs-info`               | `needs-info`         | 等报告者补充信息                            |
| `ready-for-agent`          | `ready-for-agent`    | 已完整定义，可以交给 AFK agent 干活           |
| `ready-for-human`          | `ready-for-human`    | 需要人来实现                                |
| `wontfix`                  | `wontfix`            | 不会处理                                   |

本仓库当前只存在 GitHub 默认标签（含 `wontfix`）；`needs-triage` / `needs-info` / `ready-for-agent` /
`ready-for-human` 会在 `triage` 第一次打标签时自动创建。

当某个 skill 提到一个角色（例如 "apply the AFK-ready triage label"），就用上表右列对应的标签字符串。

想换一套词汇（比如 tracker 里已经用 `bug:triage` 表示 `needs-triage`），直接改右列即可。
