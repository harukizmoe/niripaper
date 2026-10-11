# 控制面收敛成单一 `niripaper.sock`，协议里带输出参数

多输出改由"一个进程管所有输出"的监督者独占控制 socket 之后，路径不再按输出命名，而是单一的 `$XDG_RUNTIME_DIR/niripaper.sock`，命令里带上输出：`set <output|*> <path>` / `query [<output>]` / `state [<output>]`。CLI 对外仍是 `--output NAME`（翻译成协议参数），所以既有的 `set … --output X` 调用不用改；点一个不存在的输出会明确报错。

## Status

accepted — 取代 `$XDG_RUNTIME_DIR/niripaper-{output}.sock` 的旧设计（每输出一个 socket，配上 `find_path()` 的规矩：0 个 → "没在跑"、1 个 → 直接用、>1 个 → 要求 `--output NAME`，为的是多输出各跑一个 daemon 时不互相抢）。
