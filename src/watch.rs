//! Watching the configuration for changes (`HANDOFF.md` §2).
//!
//! Raw inotify rather than a crate: the job is "tell me when something in these
//! directories changed", and the fd drops straight into the daemon's existing
//! `poll()` — the same shape as the niri socket and the control socket.
//!
//! **Directories are watched, not files.** Editors and tools write atomically
//! (write a temp file, then rename it over the target), which a per-file watch
//! would miss entirely, and a tool may create `config.d` long after the daemon
//! started.

use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

/// What counts as "the configuration changed": a write that finished, a file
/// appearing or disappearing, and a rename landing on top of a file. The
/// daemon reloads on any of them, and a reload is idempotent — so a burst of
/// events from one save just means a few extra parses, not a debounce timer.
const MASK: u32 = libc::IN_CLOSE_WRITE
    | libc::IN_CREATE
    | libc::IN_DELETE
    | libc::IN_MOVED_TO
    | libc::IN_MOVED_FROM;

pub struct Watcher {
    fd: OwnedFd,
    /// Directories already watched, so a newly created `config.d` can be added
    /// without adding the same watch twice.
    watched: Vec<PathBuf>,
    buffer: Vec<u8>,
}

impl Watcher {
    pub fn new() -> Result<Self, String> {
        let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
        if fd < 0 {
            return Err(format!(
                "inotify_init1: {}",
                std::io::Error::last_os_error()
            ));
        }
        Ok(Self {
            fd: unsafe { OwnedFd::from_raw_fd(fd) },
            watched: Vec::new(),
            buffer: vec![0u8; 4096],
        })
    }

    /// Watch every directory that exists and is not watched yet. Call it again
    /// after each reload: that is how a `config.d` created later starts being
    /// watched.
    pub fn watch(&mut self, dirs: &[PathBuf]) -> Result<(), String> {
        for dir in dirs {
            if !dir.is_dir() || self.watched.contains(dir) {
                continue;
            }
            let path = CString::new(dir.as_os_str().as_bytes())
                .map_err(|e| format!("{}: {e}", dir.display()))?;
            let handle =
                unsafe { libc::inotify_add_watch(self.fd.as_raw_fd(), path.as_ptr(), MASK) };
            if handle < 0 {
                return Err(format!(
                    "inotify_add_watch {}: {}",
                    dir.display(),
                    std::io::Error::last_os_error()
                ));
            }
            self.watched.push(dir.clone());
        }
        Ok(())
    }

    pub fn fd(&self) -> RawFd {
        self.fd.as_raw_fd()
    }

    /// Read everything pending. `true` means "reload". EAGAIN (no more events)
    /// is the normal way out.
    pub fn drain(&mut self) -> bool {
        let mut anything = false;
        loop {
            let read = unsafe {
                libc::read(
                    self.fd.as_raw_fd(),
                    self.buffer.as_mut_ptr().cast(),
                    self.buffer.len(),
                )
            };
            if read <= 0 {
                return anything;
            }
            anything = true;
        }
    }
}
