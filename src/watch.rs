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
    /// Directories currently watched, with the handle the kernel gave them. Only
    /// used to decide whether a watch is *new* (so it can be reported once): the
    /// watches themselves are re-added on every call, because a list that never
    /// forgets a directory that went away is exactly how a `config.d` recreated
    /// after an `rmdir` ends up silently unwatched.
    watched: Vec<(PathBuf, i32)>,
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
            if !dir.is_dir() {
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
            // Only the *report* is deduplicated. The watch itself is added every
            // time, because `inotify_add_watch` on a path that is already watched
            // returns the same handle instead of adding a second watch — and
            // because remembering is what broke this: `rmdir` makes the kernel drop
            // the watch, and a list that never forgets then skips re-adding it,
            // leaving the directory silently unwatched for good. (Found the hard
            // way: a demo that created and removed `config.d` repeatedly left the
            // daemon blind to every later write into it.)
            if !self.watched.iter().any(|(known, _)| known == dir) {
                self.watched.push((dir.clone(), handle));
                crate::daemon::log_public(&format!("watching {}", dir.display()));
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `rmdir` makes the kernel drop the watch on a directory. If `watch` trusts a
    /// list of what it has seen, it skips re-adding it — and the directory then
    /// stays silently unwatched for good. That is exactly what happened to a
    /// `config.d` which a demo created and removed repeatedly: the daemon went
    /// blind to every write into it, and only a restart brought it back.
    #[test]
    fn a_recreated_directory_is_watched_again() {
        let dir = std::env::temp_dir().join(format!("niripaper-watch-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");

        let mut watcher = Watcher::new().expect("inotify");
        watcher.watch(std::slice::from_ref(&dir)).expect("watch");
        std::fs::remove_dir(&dir).expect("remove");
        std::fs::create_dir_all(&dir).expect("recreate");
        watcher
            .watch(std::slice::from_ref(&dir))
            .expect("watch again");

        // Clear whatever the removal and recreation left behind.
        watcher.drain();

        std::fs::write(dir.join("x.toml"), "fit = \"tile\"\n").expect("write");
        let mut seen = false;
        for _ in 0..100 {
            if watcher.drain() {
                seen = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert!(seen, "the recreated directory is not being watched");
    }
}
