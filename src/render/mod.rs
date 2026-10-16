//! Rendering: EGL/GBM on a chosen GPU, GL, dmabufs, and the Wayland layer.

pub mod anim;
pub mod dmabuf;
pub mod egl;
pub mod gbm;
pub mod gl;
pub mod image;
pub mod layer;
pub mod transition;
pub mod video;

/// How a source whose aspect ratio does not match the canvas is placed in it.
///
/// The names are Wallpaper Engine's (and Windows' before it): `fill`, `fit`,
/// `stretch`, `center`, `tile`. Matching them is deliberate — anyone arriving
/// from Wallpaper Engine already knows what each one means, and a config that
/// spells them differently is a config they have to learn twice. `span` is the
/// one Windows mode left out: it spreads a picture across several monitors,
/// which cannot mean anything to a daemon that draws on exactly one output.
///
/// The type itself lives in `niripaper-proto::domain` (the schema reports it),
/// re-exported here under its historical path.
pub use niripaper_proto::domain::Fit;
