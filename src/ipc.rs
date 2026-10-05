//! The control socket (`HANDOFF.md` §3): `$XDG_RUNTIME_DIR/niripaper.sock`.
//!
//! One line in, one line out. The protocol is deliberately tiny:
//!
//! ```text
//! set <output|*> <path>   switch one output's wallpaper (or every output)
//! query [<output>]        what is on screen right now
//! schema                  every config key a UI can offer, as JSON (`schema.rs`)
//! state [<output>]        what the daemon is doing right now, as JSON
//! kill                    shut the daemon down
//! ```
//!
//! The output name comes first on `set` so that a path containing spaces needs
//! no quoting rule: everything after the second word is the path, verbatim.
//! Omitting the output on `query`/`state` means "all of them", which is what a
//! panel wants — one call, every monitor.
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
    /// `set <output|*> <path>` — an absolute or relative path to an image or a
    /// video. `output: None` is `*`: every output.
    Set {
        output: Option<String>,
        path: PathBuf,
    },
    /// `query [<output>]` — `None` asks for a summary of all of them.
    Query {
        output: Option<String>,
    },
    /// The configuration's shape, so a panel never hardcodes a key name.
    Schema,
    /// Everything the daemon is doing: effective config, wallpaper, position.
    /// `None` is every output.
    State {
        output: Option<String>,
    },
    Kill,
}

/// The output name that means "every output". The daemon turns it into one
/// worker per output; a worker's own options always name a real output.
pub const EVERY_OUTPUT: &str = "*";

/// `*` is "every output"; anything else is a name.
fn output_name(word: &str) -> Option<String> {
    (word != EVERY_OUTPUT).then(|| word.to_owned())
}

impl Request {
    fn parse(line: &str) -> Result<Self, String> {
        // Tolerate surrounding whitespace: a shell or a test may hand it over
        // untrimmed, and that is not a protocol error.
        let line = line.trim();
        let mut parts = line.split_whitespace();
        let command = parts.next().unwrap_or("");
        match command {
            "set" => {
                let output = parts
                    .next()
                    .ok_or("set needs an output (or *) and a path")?
                    .to_owned();
                // Everything after the output word is the path, spaces included:
                // only the first two words are protocol.
                let rest = line[command.len()..].trim_start();
                let rest = rest[output.len()..].trim_start();
                if rest.is_empty() {
                    return Err("set needs a path".to_owned());
                }
                Ok(Self::Set {
                    output: output_name(&output),
                    path: PathBuf::from(rest),
                })
            }
            "query" => Ok(Self::Query {
                output: parts.next().and_then(output_name),
            }),
            "schema" => Ok(Self::Schema),
            "state" => Ok(Self::State {
                output: parts.next().and_then(output_name),
            }),
            "kill" => Ok(Self::Kill),
            "" => Err("empty request".to_owned()),
            other => Err(format!("unknown request {other:?}")),
        }
    }
}

/// The socket path: one per user session, whatever the number of outputs.
///
/// One process draws every output, so there is exactly one daemon to talk to and
/// a request says which output it is about. That is what lets a panel read every
/// monitor's state in one call.
pub fn default_path() -> Result<PathBuf, String> {
    let dir = std::env::var_os("XDG_RUNTIME_DIR")
        .ok_or("XDG_RUNTIME_DIR is not set — the daemon must run inside a session")?;
    Ok(PathBuf::from(dir).join("niripaper.sock"))
}

/// The socket path for a client, with the "is it running?" answer folded in.
pub fn find_path() -> Result<PathBuf, String> {
    let path = default_path()?;
    if !path.exists() {
        return Err(format!(
            "no niripaper socket at {} — is the daemon running?",
            path.display()
        ));
    }
    Ok(path)
}

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
    path: PathBuf,
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
    fn parses_the_requests() {
        assert_eq!(
            Request::parse("set DP-1 /tmp/wall.png").unwrap(),
            Request::Set {
                output: Some("DP-1".to_owned()),
                path: PathBuf::from("/tmp/wall.png")
            }
        );
        // `*` is every output, and so is saying nothing on `query`/`state`.
        assert_eq!(
            Request::parse("set * /tmp/wall.png").unwrap(),
            Request::Set {
                output: None,
                path: PathBuf::from("/tmp/wall.png")
            }
        );
        assert_eq!(
            Request::parse("query DP-1").unwrap(),
            Request::Query {
                output: Some("DP-1".to_owned())
            }
        );
        assert_eq!(
            Request::parse("query").unwrap(),
            Request::Query { output: None }
        );
        assert_eq!(
            Request::parse("state eDP-1").unwrap(),
            Request::State {
                output: Some("eDP-1".to_owned())
            }
        );
        assert_eq!(
            Request::parse("state").unwrap(),
            Request::State { output: None }
        );
        assert_eq!(Request::parse("schema").unwrap(), Request::Schema);
        assert_eq!(Request::parse("kill").unwrap(), Request::Kill);
        // Extra whitespace is a shell artefact, not a protocol error.
        assert_eq!(
            Request::parse("  query  ").unwrap(),
            Request::Query { output: None }
        );
        // Only the first two words are protocol: a path may contain spaces.
        assert_eq!(
            Request::parse("set * /tmp/my wall.png").unwrap(),
            Request::Set {
                output: None,
                path: PathBuf::from("/tmp/my wall.png")
            }
        );
    }

    #[test]
    fn rejects_malformed_requests() {
        // Each message has to name the problem: a silent no-op would look like
        // the daemon ignoring the client.
        assert!(Request::parse("set")
            .unwrap_err()
            .contains("needs an output"));
        assert!(Request::parse("set DP-1")
            .unwrap_err()
            .contains("needs a path"));
        assert!(Request::parse("").unwrap_err().contains("empty"));
        assert!(Request::parse("dance").unwrap_err().contains("unknown"));
    }
}
