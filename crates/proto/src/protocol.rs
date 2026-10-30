//! The control socket's line protocol (§3): one line in, one line out.
//!
//! This half is what a client needs: the request grammar, the socket path
//! conventions, and how to send one request. The listening side (`Server`,
//! stale-socket eviction, unlink-on-drop) stays in the daemon crate.
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

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

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
    pub fn parse(line: &str) -> Result<Self, String> {
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

/// Send one request and return the reply. Used by the CLI and by the app.
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
