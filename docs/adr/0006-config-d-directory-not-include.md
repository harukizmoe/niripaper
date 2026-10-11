# 分层配置用 `config.d/` 目录，不自造 include 指令

配置是顶层 `~/.config/niripaper/config.toml` + `config.d/*.toml` **按文件名排序**逐个解析再深合并，而不是发明一条 `include` 指令。好处是来源归属由文件名直接给出：每份文件先单独反序列化，出错能指名是哪个文件、哪一行；Noctalia 插件独占 `config.d/noctalia.toml`，与手工配置互不打架。

## Considered Options

- **自造 `include` 指令**：否决——合并语义、路径解析、循环包含都要自己再定义一遍。
- **`config.d/` 目录 + 文件名顺序**：采用。

## Consequences

两条硬要求：解析失败**不许把 daemon 打挂**，失败时保留旧配置并打印原因；`output` / `namespace` **不能热改**（重建输出代价太大，改了也只当没看见）。
