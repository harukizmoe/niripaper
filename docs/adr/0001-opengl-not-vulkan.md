# 渲染栈用 OpenGL / GLES + EGL，不用 Vulkan/wgpu

渲染栈锁定为 **OpenGL / GLES + EGL**（不是 Vulkan/wgpu）：libmpv 的 render API 只有 `OPENGL` 与 `SW` 两种后端，没有 Vulkan/wgpu 入口。于是视频用 libmpv 的 OpenGL 纹理、静态图用自有纹理，两者由同一个 frame-callback 循环带着视差偏移绘制。

## Considered Options

- `MPV_RENDER_API_TYPE_SW`：libmpv 的另一条路，帧不落在 GL 纹理上。
- `MPV_RENDER_API_TYPE_OPENGL` + EGL：采用。
