//! The Tauri command surface: one command per protocol verb the UI needs.
//!
//! These are the *only* bridge between the webview and the daemon. Schema comes
//! from the compiled-in `niripaper_proto::schema()` (decisions §4.7: the app can
//! build its panels and edit the config while the daemon is down), state/set/
//! query/kill go over the socket (PRD FR-2).

use tauri::State;

use crate::state::Connection;
use niripaper_proto::protocol::{Request, EVERY_OUTPUT};

/// Every config key a UI can offer, as JSON. Pure function, no daemon.
#[tauri::command]
pub fn daemon_schema() -> serde_json::Value {
    niripaper_proto::schema::schema()
}

/// What the daemon is doing right now, as JSON lines (one per output).
///
/// Errors come back as `Err(String)` — "the daemon is not running" is a state
/// the UI must render (PRD US-013), not a panic.
#[tauri::command]
pub fn daemon_state(
    connection: State<'_, Connection>,
    output: Option<String>,
) -> Result<String, String> {
    connection.send(&Request::State { output })
}

/// A one-line summary of what is on screen (`ok …` or `error …`).
#[tauri::command]
pub fn daemon_query(
    connection: State<'_, Connection>,
    output: Option<String>,
) -> Result<String, String> {
    connection.send(&Request::Query { output })
}

/// Switch one output's wallpaper (or every output with `*`).
#[tauri::command]
pub fn daemon_set(
    connection: State<'_, Connection>,
    output: String,
    path: String,
) -> Result<String, String> {
    let output = (output != EVERY_OUTPUT).then_some(output);
    connection.send(&Request::Set {
        output,
        path: path.into(),
    })
}

/// Shut the daemon down. Returns the daemon's reply.
#[tauri::command]
pub fn daemon_kill(connection: State<'_, Connection>) -> Result<String, String> {
    connection.send(&Request::Kill)
}

/// Point the app at a non-standard daemon socket (mirrors the daemon's own
/// `--socket`, for running a second instance in tests).
#[tauri::command]
pub fn daemon_socket(
    connection: State<'_, Connection>,
    path: Option<String>,
) -> Result<Option<String>, String> {
    let previous = connection.socket().ok().map(|p| p.display().to_string());
    connection.set_socket(path.map(std::path::PathBuf::from));
    Ok(previous)
}

/// The configuration file's path, for "open in editor" and for writing edits.
#[tauri::command]
pub fn config_path() -> Option<String> {
    niripaper_proto::config::default_path().map(|path| path.display().to_string())
}
