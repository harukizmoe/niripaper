//! niripaper-proto — the pure half of niripaper, shared by the daemon and the
//! desktop app (§5, decisions §4.6).
//!
//! Everything here is free of GPU, Wayland, and mpv: the daemon "generates" the
//! schema and the app compiles against the same types, so the two cannot drift.
//! IO is limited to the control socket's line protocol and the XDG path
//! conventions — the app's only way to reach the daemon (PRD FR-2).
//!
//! What deliberately does **not** live here: the renderers (EGL/GBM/dmabuf/GL),
//! the motion state machines, and mpv. Those stay in the daemon crate; only
//! their value types and names, which config files and the schema speak, are
//! mirrored below.

pub mod animation;
pub mod client;
pub mod config;
pub mod domain;
pub mod niri;
pub mod protocol;
pub mod schema;
pub mod state;
pub mod transition;

pub use animation::{Animation, Curve, Spring};
pub use config::{Animations, Config, OutputOverride, OutputParams, OverviewAnimation};
pub use protocol::{Request, EVERY_OUTPUT};
pub use transition::{Direction, Effect, Selection, Settings};
