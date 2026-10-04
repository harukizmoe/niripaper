//! niripaper — a wallpaper engine for the Niri compositor.
//!
//! Pure logic (motion maths, config) is kept separate from the pieces that talk
//! to real hardware, so the interesting parts stay testable without a GPU.
//! Nothing here is Linux-specific in principle, but the renderer is GL +
//! EGL + dmabuf by design: that is the only combination that lets a client
//! choose which GPU renders an output.

pub mod daemon;
pub mod gpu;
pub mod motion;
pub mod niri;
pub mod render;
