//! The daemon connection's lifetime, shared across Tauri commands.
//!
//! Socket path resolution lives here so every command sees the same override
//! (`--socket`-style, from the frontend or an env var) and the same
//! "is the daemon running?" error wording.

use std::path::PathBuf;

use niripaper_proto::protocol;
use parking_lot::RwLock;

/// Where to find the daemon, and how to reach it.
#[derive(Debug)]
pub struct Connection {
    /// `None` = the standard session path. Overridable for testing, same as
    /// the daemon's own `--socket`.
    socket: RwLock<Option<PathBuf>>,
}

impl Connection {
    pub fn new() -> Self {
        Self {
            socket: RwLock::new(std::env::var_os("NIRIPAPER_SOCKET").map(PathBuf::from)),
        }
    }

    /// The socket to talk to, with the "is it running?" answer folded in.
    pub fn socket(&self) -> Result<PathBuf, String> {
        match self.socket.read().as_ref() {
            Some(path) => Ok(path.clone()),
            None => protocol::find_path(),
        }
    }

    pub fn set_socket(&self, path: Option<PathBuf>) {
        *self.socket.write() = path;
    }

    /// Send one request to the daemon.
    pub fn send(&self, request: &protocol::Request) -> Result<String, String> {
        let path = self.socket()?;
        niripaper_proto::client::send(&path, request)
    }
}

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}
