# EGL / GBM / GL 走裸 FFI，不引入 EGL/GL crate

`src/render/{egl,gbm,gl}.rs` 直接链 `libEGL.so`（GLVND 加载器）、`libgbm.so`、`libGL.so`（GLVND 分发），没有 EGL/GL crate 这一层。它买到的是对加载器与扩展入口的完全控制：在**本进程第一次 EGL 调用之前**用 `__EGL_VENDOR_LIBRARY_FILENAMES` 按目标 GPU 的 PCI vendor id 钉住 vendor（`0x10de` → NVIDIA ICD，其余 → Mesa），用 `EGL_PLATFORM_GBM_KHR` + `gbm_create_device(fd)` 绑定指定设备，再用 `eglCreateImageKHR` / `glEGLImageTargetTexture2DOES` 把合成器给的 dmabuf 导入成可挂 FBO 的纹理。

## Consequences

GLVND 只在第一次 EGL 调用时读 `__EGL_VENDOR_LIBRARY_FILENAMES`，所以**一个进程只能钉一个 vendor**：合成器的 main device 是单一 GPU，因此够用；但不要设计成"一个进程同时渲染两块不同厂商的 GPU"（见 [已知坑](../gotchas.md)）。
