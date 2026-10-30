//! The control socket's listening side (§3): `$XDG_RUNTIME_DIR/niripaper.sock`.
//!
//! The protocol grammar and the client half live in `niripaper-proto::protocol`
//! (re-exported as [`crate::proto::protocol`]); what stays here is what only a
//! daemon may do: own the socket file, evict a *stale* one, refuse to evict a
//! *live* daemon, and unlink on exit. The daemon owns the socket and unlinks it
//! on drop: a stale socket is worse than none, because the next start would
//! fail on `bind`.

use std::io::{BufRead, BufReader};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::time::Duration;

pub use niripaper_proto::protocol::{
    default_path, find_path, reply, request, Request, EVERY_OUTPUT,
};

/// A client cannot be allowed to stall the render loop, so reads are bounded.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

/// Is a daemon listening on `path`?
///
/// A socket nobody listens on refuses the connection (ECONNREFUSED), and only
/// that means "leftover from a daemon that died". Anything else counts as served
/// — evicting something that is running is the worse mistake.
fn served_by_daemon(path: &Path) -> bool {
    match UnixStream::connect(path) {
        Ok(mut probe) => {
            // Say something valid before closing, so the daemon we are declining
            // to replace does not log an empty request.
            let _ = probe.write_all(b"query\n");
            true
        }
        Err(_) => false,
    }
}

/// What a second daemon on the same output is told. One wording for two places:
/// the early check in the daemon, and the authoritative one in [`Server::bind`].
fn already_served(path: &Path) -> String {
    format!(
        "{} is served by a running daemon; kill it first, or use another --output",
        path.display()
    )
}

use std::io::Write;

/// Fail fast when `path` is already served.
///
/// [`Server::bind`] refuses for the same reason, but a daemon only reaches that
/// after building a layer surface, loading the media and creating a Vulkan
/// device — work a duplicate start must never do, and a second wallpaper must
/// never flash on the output while it does it.
pub fn refuse_if_served(path: &Path) -> Result<(), String> {
    if served_by_daemon(path) {
        return Err(already_served(path));
    }
    Ok(())
}

pub struct Server {
    listener: UnixListener,
    path: std::path::PathBuf,
}

impl Server {
    pub fn bind(path: &Path) -> Result<Self, String> {
        // Two different things can be sitting at this path, and they need
        // opposite answers.
        //
        // A leftover socket from a daemon that died would make `bind` fail with
        // EADDRINUSE, so it has to go. A *live* daemon on the same output must
        // not be evicted: it keeps drawing (the compositor still holds its
        // surface) but becomes unreachable, and when it eventually exits its
        // `Drop` unlinks *our* socket instead — leaving both unaddressable.
        // That is not hypothetical: it is how a stray second daemon on `DP-1`
        // went unnoticed while it was doubling the work on one output.
        //
        // `connect` is what tells them apart. A socket nobody listens on
        // refuses the connection; only that means "stale".
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            use std::os::unix::fs::FileTypeExt;
            if !meta.file_type().is_socket() {
                return Err(format!(
                    "{} exists and is not a socket; refusing to remove it",
                    path.display()
                ));
            }
            if served_by_daemon(path) {
                return Err(already_served(path));
            }
            let _ = std::fs::remove_file(path);
        }
        let listener =
            UnixListener::bind(path).map_err(|e| format!("binding {}: {e}", path.display()))?;
        Ok(Self {
            listener,
            path: path.to_owned(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn fd(&self) -> RawFd {
        self.listener.as_raw_fd()
    }

    /// Accept one client and read its request. The stream comes back so the
    /// caller can reply after acting on it.
    pub fn accept(&self) -> Result<(Request, UnixStream), String> {
        let (stream, _) = self.listener.accept().map_err(|e| format!("accept: {e}"))?;
        stream
            .set_read_timeout(Some(READ_TIMEOUT))
            .map_err(|e| format!("set_read_timeout: {e}"))?;
        let mut reader = BufReader::new(
            stream
                .try_clone()
                .map_err(|e| format!("cloning the client stream: {e}"))?,
        );
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("reading the request: {e}"))?;
        let request = Request::parse(line.trim())?;
        Ok((request, stream))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Leave no socket behind: the next daemon must be able to bind.
        let _ = std::fs::remove_file(&self.path);
    }
}
