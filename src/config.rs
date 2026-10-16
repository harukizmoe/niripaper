//! Configuration — moved to `niripaper-proto::config` (the desktop app reads
//! and writes the same file). This module re-exports it for the daemon's
//! historical `crate::config::…` paths.

pub use niripaper_proto::config::*;
