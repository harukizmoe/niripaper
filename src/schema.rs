//! The configuration's shape — moved to `niripaper-proto::schema` (the desktop
//! app builds its panels from the same function). This module re-exports it
//! for the daemon's historical `crate::schema::…` paths.

pub use niripaper_proto::schema::*;
