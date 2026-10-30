//! niripaper — a wallpaper engine for the Niri compositor.
//!
//! Pure logic (motion maths, config, schema, the control protocol) lives in
//! `niripaper-proto` and is re-exported here under the same module names: the
//! desktop app compiles against the same types, so daemon and UI cannot drift.
//! What stays here is everything that talks to real hardware: EGL + GBM +
//! dmabuf by design — that is the only combination that lets a client choose
//! which GPU renders an output — plus the motion state machines and the socket
//! server.

pub mod config;
pub mod daemon;
pub mod gpu;
pub mod ipc;
pub mod media;
pub mod motion;
pub mod niri;
pub mod render;
pub mod scene;
pub mod watch;

/// The shared half, re-exported under its historical module names so the
/// daemon's call sites and any external reader keep one spelling.
pub use niripaper_proto as proto;

/// The configuration's shape, for clients that must not hardcode it (§2).
pub mod schema {
    pub use niripaper_proto::schema::*;
}
