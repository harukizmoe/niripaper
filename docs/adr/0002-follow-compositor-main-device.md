# 跟随合成器的 dmabuf feedback `main_device`，而不是输出所在的 GPU

缓冲的分配与渲染都落在**合成器实际合成的那张 GPU** 上：判据是 dmabuf feedback 报出的 `main_device` 指向哪个 `dev_t`，就在那个 render node 上开 GBM/EGL。合成器合成就行——它未必是你以为的"输出所在的那张卡"：混显机上 niri 默认在 iGPU 上合成所有输出、再把结果拷贝给只负责扫描输出的 dGPU，此时在 dGPU 上渲染该输出反而每帧多一次跨 GPU 导入 + 拷贝。（当前这台机器是单卡，`main_device` 就是那块卡；规则本身不变。）

## Considered Options

- **按输出所在的 GPU 渲染**（"输出在 NVIDIA 就在 NVIDIA 上画"）：最初就是这么判断的，已否——它把画面推出了合成器的渲染节点。
- **跟随 per-surface feedback 的 `main_device`**：采用。与合成器同节点，不需要那次跨 GPU 导入 + 拷贝。

## Consequences

**渲染节点与主节点不同时**，niri 对该输出的 scanout tranche 只接受 **Linear** modifier——LINEAR 因此是跨设备 scanout 的通行证：选它既省一次拷贝，又保证可直扫（Mesa 能渲染进 LINEAR）。选错的失败是**静默**的：跨设备提交非 LINEAR 缓冲时 niri 不报错、`wl_buffer::release` 也照常回来，但屏幕上什么内容都没有——排查时先量模糊区/壁纸区的四段均值，不要只看有没有报错（见 [已知坑](../gotchas.md)）。"选哪块卡"也因此完全不属于本项目：不提供选卡开关、不写 niri 配置、不碰 MUX/BIOS，只负责按它说的那张卡把缓冲做对。
