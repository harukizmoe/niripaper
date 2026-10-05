//! The control socket (`HANDOFF.md` §3): `$XDG_RUNTIME_DIR/niripaper.sock`.
//!
//! One line in, one line out. The protocol is deliberately tiny:
//!
//! ```text
//! set <path>   switch the wallpaper (image or video, routed by extension)
//! query        what is on screen right now
//! schema       every config key a UI can offer, as JSON (`schema.rs`)
//! state        what the daemon is doing right now, as JSON
//! kill         shut the daemon down
//! ```
//!
//! This is what makes a wallpaper *changeable* without restarting, which is both
//! what the cross-fade needs to be observable and what the Noctalia plugin (M3)
//! talks to.
//!
//! The daemon owns the socket file and unlinks it on exit: a stale socket is
//! worse than none, because the next start would fail on `bind`.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// A client cannot be allowed to stall the render loop, so reads are bounded.
const READ_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Request {
    /// `set <path>` — an absolute or relative path to an image or a video.
    Set(PathBuf),
    Query,
    /// The configuration's shape, so a panel never hardcodes a key name.
    Schema,
    /// Everything the daemon is doing: effective config, wallpaper, position.
    State,
    Kill,
}

impl Request {
    fn parse(line: &str) -> Result<Self, String> {
        // Tolerate surrounding whitespace: a shell or a test may hand it over
        // untrimmed, and that is not a protocol error.
        let line = line.trim();
        let mut parts = line.splitn(2, char::is_whitespace);
        let command = parts.next().unwrap_or("").trim();
        let argument = parts.next().unwrap_or("").trim();
        match command {
            "set" => {
                if argument.is_empty() {
                    return Err("set needs a path".to_owned());
                }
                Ok(Self::Set(PathBuf::from(argument)))
            }
            "query" => Ok(Self::Query),
            "schema" => Ok(Self::Schema),
            "state" => Ok(Self::State),
            "kill" => Ok(Self::Kill),
            "" => Err("empty request".to_owned()),
            other => Err(format!("unknown request {other:?}")),
        }
    }
}

/// The default socket path: per-user, per-session, **per output**.
///
/// There is one daemon per output, so one socket per output — a single path
/// would have the second daemon treat the first one's socket as stale and
/// unlink it (`Server::bind` is allowed to remove a socket).
pub fn default_path(output: &str) -> Result<PathBuf, String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is not set — the daemon must run inside a session")?;
    Ok(PathBuf::from(dir).join(format!("niripaper-{output}.sock")))
}

/// Find the socket when the caller did not name an output: exactly one is
/// unambiguous, several need `--output`.
pub fn find_path() -> Result<PathBuf, String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is not set — the daemon must run inside a session")?;
    let dir = PathBuf::from(dir);
    let mut found: Vec<PathBuf> = std::fs::read_dir(&dir)
        .map_err(|e| format!("reading {}: {e}", dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("niripaper-") && name.ends_with(".sock"))
        })
        .collect();
    found.sort();
    match found.len() {
        0 => Err(format!(
            "no niripaper socket in {} — is the daemon running?",
            dir.display()
        )),
        1 => Ok(found.remove(0)),
        _ => {
            let names: Vec<String> = found
                .iter()
                .map(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or_default()
                        .to_owned()
                })
                .collect();
            Err(format!(
                "several daemons are running ({}); pass --output NAME",
                names.join(", ")
            ))
        }
    }
}

pub struct Server {
    listener: UnixListener,
    path: PathBuf,
}

impl Server {
    pub fn bind(path: &Path) -> Result<Self, String> {
        // A leftover socket from a killed daemon would make `bind` fail with
        // EADDRINUSE. Only ever remove a socket: if something else lives there,
        // refusing is the right answer.
        if let Ok(meta) = std::fs::symlink_metadata(path) {
            use std::os::unix::fs::FileTypeExt;
            if meta.file_type().is_socket() {
                let _ = std::fs::remove_file(path);
            } else {
                return Err(format!(
                    "{} exists and is not a socket; refusing to remove it",
                    path.display()
                ));
            }
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

/// Send one request and return the reply. Used by the CLI.
pub fn request(path: &Path, request: &str) -> Result<String, String> {
    let mut stream = UnixStream::connect(path).map_err(|e| {
        format!(
            "connecting to {}: {e} — is the daemon running?",
            path.display()
        )
    })?;
    stream
        .write_all(format!("{request}\n").as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|e| format!("sending {request:?}: {e}"))?;
    let mut reply = String::new();
    BufReader::new(&stream)
        .read_line(&mut reply)
        .map_err(|e| format!("reading the reply: {e}"))?;
    Ok(reply.trim_end().to_owned())
}

/// Write one line back to a client. Failures are the client's problem, not ours.
pub fn reply(stream: &UnixStream, text: &str) {
    let mut stream = stream;
    let _ = stream.write_all(format!("{text}\n").as_bytes());
    let _ = stream.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_three_requests() {
        assert_eq!(
            Request::parse("set /tmp/wall.png").unwrap(),
            Request::Set(PathBuf::from("/tmp/wall.png"))
        );
        assert_eq!(Request::parse("query").unwrap(), Request::Query);
        assert_eq!(Request::parse("kill").unwrap(), Request::Kill);
        // Extra whitespace is a shell artefact, not a protocol error.
        assert_eq!(Request::parse("  query  ").unwrap(), Request::Query);
        // A path may contain spaces: only the first run splits.
        assert_eq!(
            Request::parse("set /tmp/my wall.png").unwrap(),
            Request::Set(PathBuf::from("/tmp/my wall.png"))
        );
    }

    #[test]
    fn rejects_malformed_requests() {
        // Each message has to name the problem: a silent no-op would look like
        // the daemon ignoring the client.
        assert!(Request::parse("set").unwrap_err().contains("needs a path"));
        assert!(Request::parse("").unwrap_err().contains("empty"));
        assert!(Request::parse("dance").unwrap_err().contains("unknown"));
    }
}
