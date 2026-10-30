//! The control socket client, from the app's side.
//!
//! Thin on purpose: one request, one reply, per [`niripaper_proto::protocol`].
//! The app is a thin client (PRD FR-1/FR-2) — this module is its entire IO
//! surface to the daemon.

use std::path::Path;

use crate::protocol::{find_path, Request};

/// Send one request and return the daemon's reply line.
pub fn send(path: &Path, request: &Request) -> Result<String, String> {
    crate::protocol::request(path, &render(request))
}

/// Send one request to the daemon at the standard session path.
pub fn send_default(request: &Request) -> Result<String, String> {
    send(&find_path()?, request)
}

/// Render a request back to its wire form. The grammar is line-oriented, so a
/// client that builds requests programmatically needs the exact spelling —
/// including the rule that a path may contain spaces (everything after the
/// second word is the path, verbatim).
pub fn render(request: &Request) -> String {
    match request {
        Request::Set { output, path } => format!(
            "set {} {}",
            output.as_deref().unwrap_or(crate::protocol::EVERY_OUTPUT),
            path.display()
        ),
        Request::Query { output } => match output {
            Some(output) => format!("query {output}"),
            None => "query".to_owned(),
        },
        Request::Next { output } => match output {
            Some(output) => format!("next {output}"),
            None => "next".to_owned(),
        },
        Request::Schema => "schema".to_owned(),
        Request::State { output } => match output {
            Some(output) => format!("state {output}"),
            None => "state".to_owned(),
        },
        Request::Kill => "kill".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::default_path;
    use std::path::PathBuf;

    #[test]
    fn renders_the_grammar_the_daemon_parses() {
        // Round-trips through `Request::parse`, so client and daemon cannot
        // drift apart: whatever this renders, the daemon must parse back to
        // the same request.
        let cases = [
            Request::Set {
                output: Some("DP-1".to_owned()),
                path: PathBuf::from("/tmp/wall.png"),
            },
            Request::Set {
                output: None,
                path: PathBuf::from("/tmp/my wall.png"),
            },
            Request::Query {
                output: Some("eDP-1".to_owned()),
            },
            Request::Query { output: None },
            Request::Next { output: None },
            Request::Next {
                output: Some("DP-1".to_owned()),
            },
            Request::Schema,
            Request::State { output: None },
            Request::State {
                output: Some("DP-1".to_owned()),
            },
            Request::Kill,
        ];
        for case in cases {
            let line = render(&case);
            assert_eq!(&Request::parse(&line).unwrap(), &case, "line: {line:?}");
        }
    }

    #[test]
    fn default_path_names_the_session_socket() {
        // Not the value — that is per machine — but that it comes from
        // XDG_RUNTIME_DIR and is shared with the daemon's convention.
        if std::env::var_os("XDG_RUNTIME_DIR").is_some() {
            let path = default_path().unwrap();
            assert!(path.ends_with("niripaper.sock"), "{path:?}");
        }
    }
}
