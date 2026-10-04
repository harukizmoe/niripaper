//! Niri IPC: the event stream that drives parallax.
//!
//! Connect to `$NIRI_SOCKET`, write `"EventStream"` as one JSON line, then read
//! one JSON event per line. The first events are the full state
//! (`WorkspacesChanged`, `WindowsChanged`), after which niri only sends
//! increments, so a client can never desync.
//!
//! niri sends ~20 kinds of events; §4.2.7 says only eight of them matter, and
//! everything else — including a multi-key envelope — must be ignored without
//! disturbing the state. That decision lives here: [`parse_event`] returns
//! `None` for anything that is not one of the eight, so [`Motion::apply`] never
//! sees it.
//!
//! Event shapes are from `niri-ipc` (`Event`, tag `v26.04`), and the tests below
//! use lines captured from a live socket.

use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::motion::{Event, Motion, Window, Workspace, DEFAULT_SPAN};

/// The socket the compositor is listening on.
///
/// Note: an inherited `$NIRI_SOCKET` can be stale (the socket name changes every
/// session), which shows up as a confusing `No such file or directory`.
pub fn socket_path() -> Result<PathBuf, String> {
    let path = std::env::var_os("NIRI_SOCKET").ok_or(
        "NIRI_SOCKET is not set — niripaper must run inside the niri session (or set it manually)",
    )?;
    Ok(PathBuf::from(path))
}

/// A connected event stream.
pub struct Niri {
    reader: BufReader<UnixStream>,
    pub motion: Motion,
    pub path: PathBuf,
    /// Lines read that were not parallax events (`{"Ok":...}`, urgency, casts…).
    pub ignored: u64,
    /// Whether niri's overview is open. Not a parallax input, but the daemon
    /// animates the wallpaper on it.
    pub overview_open: bool,
}

impl Niri {
    /// Connect using `$NIRI_SOCKET`.
    pub fn connect() -> Result<Self, String> {
        Self::connect_path(&socket_path()?)
    }

    pub fn connect_path(path: &Path) -> Result<Self, String> {
        let mut stream = UnixStream::connect(path)
            .map_err(|e| format!("connecting to {}: {e}", path.display()))?;
        stream
            .write_all(b"\"EventStream\"\n")
            .and_then(|()| stream.flush())
            .map_err(|e| format!("requesting the event stream: {e}"))?;
        Ok(Self {
            reader: BufReader::new(stream),
            motion: Motion::new(DEFAULT_SPAN),
            path: path.to_owned(),
            ignored: 0,
            overview_open: false,
        })
    }

    /// The socket fd, for an event loop that wants to poll it.
    pub fn fd(&self) -> std::os::fd::RawFd {
        self.reader.get_ref().as_raw_fd()
    }

    /// Apply the events waiting on the socket. Returns the last parallax event
    /// seen, if any.
    ///
    /// The caller is expected to have polled [`Niri::fd`] and seen it readable,
    /// so the first read does not block. Only *complete* lines are consumed
    /// after that: a partial line would block, and it can wait for the rest.
    pub fn drain(&mut self) -> Result<Option<Event>, String> {
        let mut last = self.read_line()?;
        while self.reader.buffer().contains(&b'\n') {
            if let Some(event) = self.read_line()? {
                last = Some(event);
            }
        }
        Ok(last)
    }

    /// Block until the next parallax event. Ignored lines are counted in
    /// [`Niri::ignored`] and skipped.
    pub fn next_event(&mut self) -> Result<Event, String> {
        loop {
            if let Some(event) = self.read_line()? {
                return Ok(event);
            }
        }
    }

    /// Block until the initial full state has arrived.
    ///
    /// niri sends the complete state up front — `WorkspacesChanged` and then
    /// `WindowsChanged` — so waiting for the latter means the first `progress()`
    /// reflects reality instead of an empty world.
    pub fn wait_for_full_state(&mut self) -> Result<(), String> {
        loop {
            if let Event::WindowsChanged { .. } = self.next_event()? {
                return Ok(());
            }
        }
    }

    fn read_line(&mut self) -> Result<Option<Event>, String> {
        let mut line = String::new();
        let read = self
            .reader
            .read_line(&mut line)
            .map_err(|e| format!("reading the event stream: {e}"))?;
        if read == 0 {
            return Err("niri closed the event stream".to_owned());
        }
        let line = line.trim();
        if line.is_empty() {
            self.ignored += 1;
            return Ok(None);
        }
        match parse_line(line)? {
            Parsed::Event(event) => {
                self.motion.apply(&event);
                Ok(Some(event))
            }
            Parsed::Overview { is_open } => {
                self.overview_open = is_open;
                self.ignored += 1;
                Ok(None)
            }
            Parsed::Ignored => {
                self.ignored += 1;
                Ok(None)
            }
        }
    }
}

/// What one event-stream line turned out to be.
#[derive(Debug, Clone, PartialEq)]
pub enum Parsed {
    /// One of §4.2.7's eight parallax events.
    Event(Event),
    /// `OverviewOpenedOrClosed`: not a parallax input, but the daemon animates
    /// the wallpaper when it opens and closes.
    Overview { is_open: bool },
    /// Everything else: urgency, casts, config reloads, multi-key envelopes.
    Ignored,
}

/// Parse one event line.
pub fn parse_line(line: &str) -> Result<Parsed, String> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| format!("unparsable event {line:?}: {e}"))?;
    let serde_json::Value::Object(object) = value else {
        // `{"Ok":"Handled"}` is an object; anything else is not an event.
        return Ok(Parsed::Ignored);
    };
    // A multi-key envelope is not a valid single event (§4.2.7) — ignore it
    // rather than guess which half applies.
    if object.len() != 1 {
        return Ok(Parsed::Ignored);
    }
    let (name, payload) = object.into_iter().next().expect("len == 1");
    let event = match name.as_str() {
        "WorkspacesChanged" => {
            let raw: RawWorkspacesChanged = from(payload)?;
            Event::WorkspacesChanged {
                workspaces: raw.workspaces.into_iter().map(Into::into).collect(),
            }
        }
        "WorkspaceActivated" => {
            let raw: RawWorkspaceActivated = from(payload)?;
            Event::WorkspaceActivated {
                id: raw.id,
                focused: raw.focused,
            }
        }
        "WorkspaceActiveWindowChanged" => {
            let raw: RawWorkspaceActiveWindowChanged = from(payload)?;
            Event::WorkspaceActiveWindowChanged {
                workspace_id: raw.workspace_id,
                active_window_id: raw.active_window_id,
            }
        }
        "WindowsChanged" => {
            let raw: RawWindowsChanged = from(payload)?;
            Event::WindowsChanged {
                windows: raw.windows.into_iter().map(Into::into).collect(),
            }
        }
        "WindowOpenedOrChanged" => {
            let raw: RawWindowOpenedOrChanged = from(payload)?;
            Event::WindowOpenedOrChanged {
                window: raw.window.into(),
            }
        }
        "WindowClosed" => {
            let raw: RawWindowClosed = from(payload)?;
            Event::WindowClosed { id: raw.id }
        }
        "WindowFocusChanged" => {
            let raw: RawWindowFocusChanged = from(payload)?;
            Event::WindowFocusChanged { id: raw.id }
        }
        "WindowLayoutsChanged" => {
            let raw: RawWindowLayoutsChanged = from(payload)?;
            Event::WindowLayoutsChanged {
                changes: raw
                    .changes
                    .into_iter()
                    .map(|(id, layout)| {
                        let [column, tile] = layout.pos_in_scrolling_layout.unwrap_or([0, 0]);
                        (id, column, tile)
                    })
                    .collect(),
            }
        }
        "OverviewOpenedOrClosed" => {
            let raw: RawOverviewOpenedOrClosed = from(payload)?;
            return Ok(Parsed::Overview {
                is_open: raw.is_open,
            });
        }
        // Everything else: urgency, casts, config, timestamps…
        _ => return Ok(Parsed::Ignored),
    };
    Ok(Parsed::Event(event))
}

/// Convenience for callers that only care about parallax events.
pub fn parse_event(line: &str) -> Result<Option<Event>, String> {
    Ok(match parse_line(line)? {
        Parsed::Event(event) => Some(event),
        _ => None,
    })
}

fn from<T: for<'de> Deserialize<'de>>(payload: serde_json::Value) -> Result<T, String> {
    serde_json::from_value(payload).map_err(|e| format!("bad event payload: {e}"))
}

// --- wire types (field names and shapes from `niri-ipc`) -------------------

#[derive(Deserialize)]
struct RawWorkspacesChanged {
    workspaces: Vec<RawWorkspace>,
}

#[derive(Deserialize)]
struct RawWorkspace {
    id: u64,
    idx: u32,
    /// `null` for a workspace that is not on any output.
    output: Option<String>,
    is_active: bool,
    is_focused: bool,
    active_window_id: Option<u64>,
}

impl From<RawWorkspace> for Workspace {
    fn from(raw: RawWorkspace) -> Self {
        Self {
            id: raw.id,
            idx: raw.idx,
            // An output-less workspace is never displayed, and no connector is
            // named "", so it simply never matches.
            output: raw.output.unwrap_or_default(),
            is_active: raw.is_active,
            is_focused: raw.is_focused,
            active_window_id: raw.active_window_id,
        }
    }
}

#[derive(Deserialize)]
struct RawWorkspaceActivated {
    id: u64,
    focused: bool,
}

#[derive(Deserialize)]
struct RawWorkspaceActiveWindowChanged {
    workspace_id: u64,
    active_window_id: Option<u64>,
}

#[derive(Deserialize)]
struct RawWindowsChanged {
    windows: Vec<RawWindow>,
}

#[derive(Deserialize)]
struct RawWindowOpenedOrChanged {
    window: RawWindow,
}

#[derive(Deserialize)]
struct RawWindowClosed {
    id: u64,
}

#[derive(Deserialize)]
struct RawOverviewOpenedOrClosed {
    is_open: bool,
}

#[derive(Deserialize)]
struct RawWindowFocusChanged {
    id: Option<u64>,
}

#[derive(Deserialize)]
struct RawWindowLayoutsChanged {
    changes: Vec<(u64, RawWindowLayout)>,
}

#[derive(Deserialize)]
struct RawWindowLayout {
    /// `[column, tile]`, 1-based; `null` for floating windows.
    pos_in_scrolling_layout: Option<[usize; 2]>,
}

#[derive(Deserialize)]
struct RawWindow {
    id: u64,
    /// `null` for a window that is not on a workspace.
    workspace_id: Option<u64>,
    is_focused: bool,
    is_floating: bool,
    layout: RawWindowLayout,
}

impl From<RawWindow> for Window {
    fn from(raw: RawWindow) -> Self {
        let [column, tile] = raw.layout.pos_in_scrolling_layout.unwrap_or([0, 0]);
        Self {
            id: raw.id,
            // 0 means "no workspace": niri's ids start at 1.
            workspace_id: raw.workspace_id.unwrap_or(0),
            column,
            tile,
            is_floating: raw.is_floating,
            is_focused: raw.is_focused,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real lines captured from the live socket (`nc -U $NIRI_SOCKET`).
    const WORKSPACES_CHANGED: &str = r#"{"WorkspacesChanged":{"workspaces":[{"id":22,"idx":4,"name":null,"output":"DP-1","is_urgent":false,"is_active":false,"is_focused":false,"active_window_id":null},{"id":15,"idx":2,"name":null,"output":"DP-1","is_urgent":false,"is_active":true,"is_focused":true,"active_window_id":180}]}}"#;
    const WINDOWS_CHANGED: &str = r#"{"WindowsChanged":{"windows":[{"id":180,"title":"niripaper — lib.rs","app_id":"dev.zed.Zed","pid":674550,"workspace_id":15,"is_focused":false,"is_floating":false,"is_urgent":false,"layout":{"pos_in_scrolling_layout":[2,1],"tile_size":[2540.0,1384.0],"window_size":[2540,1384],"tile_pos_in_workspace_view":null,"window_offset_in_tile":[0.0,0.0]},"focus_timestamp":{"secs":50301,"nanos":638326872}}]}}"#;

    fn parsed(line: &str) -> Event {
        parse_event(line).expect("parses").expect("is an event")
    }

    #[test]
    fn parses_the_full_state_events() {
        let Event::WorkspacesChanged { workspaces } = parsed(WORKSPACES_CHANGED) else {
            panic!("wrong variant")
        };
        assert_eq!(workspaces.len(), 2);
        assert_eq!(
            workspaces[1],
            Workspace {
                id: 15,
                idx: 2,
                output: "DP-1".to_owned(),
                is_active: true,
                is_focused: true,
                active_window_id: Some(180),
            }
        );

        let Event::WindowsChanged { windows } = parsed(WINDOWS_CHANGED) else {
            panic!("wrong variant")
        };
        assert_eq!(
            windows[0],
            Window {
                id: 180,
                workspace_id: 15,
                column: 2,
                tile: 1,
                is_floating: false,
                is_focused: false,
            }
        );
    }

    #[test]
    fn parses_incremental_events() {
        assert_eq!(
            parsed(r#"{"WorkspaceActivated":{"id":18,"focused":true}}"#),
            Event::WorkspaceActivated {
                id: 18,
                focused: true
            }
        );
        assert_eq!(
            parsed(
                r#"{"WorkspaceActiveWindowChanged":{"workspace_id":18,"active_window_id":134}}"#
            ),
            Event::WorkspaceActiveWindowChanged {
                workspace_id: 18,
                active_window_id: Some(134)
            }
        );
        assert_eq!(
            parsed(r#"{"WindowClosed":{"id":139}}"#),
            Event::WindowClosed { id: 139 }
        );
        assert_eq!(
            parsed(r#"{"WindowFocusChanged":{"id":null}}"#),
            Event::WindowFocusChanged { id: None }
        );
        assert_eq!(
            parsed(
                r#"{"WindowLayoutsChanged":{"changes":[[180,{"pos_in_scrolling_layout":[3,2]}]]}}"#
            ),
            Event::WindowLayoutsChanged {
                changes: vec![(180, 3, 2)]
            }
        );
        assert_eq!(
            parsed(
                r#"{"WindowLayoutsChanged":{"changes":[[180,{"pos_in_scrolling_layout":null}]]}}"#
            ),
            Event::WindowLayoutsChanged {
                changes: vec![(180, 0, 0)]
            }
        );
    }

    #[test]
    fn surfaces_the_overview_state() {
        // Not a parallax event, but the daemon animates on it.
        assert_eq!(
            parse_line(r#"{"OverviewOpenedOrClosed":{"is_open":true}}"#).expect("parses"),
            Parsed::Overview { is_open: true }
        );
        assert!(
            parse_event(r#"{"OverviewOpenedOrClosed":{"is_open":false}}"#)
                .expect("parses")
                .is_none()
        );
    }

    #[test]
    fn ignores_everything_that_is_not_a_parallax_event() {
        // The request acknowledgement, and events §4.2.7 does not list.
        for line in [
            r#"{"Ok":"Handled"}"#,
            r#"{"WindowUrgencyChanged":{"id":1,"urgent":true}}"#,
            r#"{"ConfigLoaded":{"failed":false}}"#,
            r#"{"KeyboardLayoutsChanged":{"keyboard_layouts":{"names":[]}}}"#,
            r#"{"CastsChanged":{"casts":[]}}"#,
            r#"{"WindowFocusTimestampChanged":{"id":1,"focus_timestamp":null}}"#,
        ] {
            assert!(parse_event(line).expect("parses").is_none(), "{line}");
        }
    }

    #[test]
    fn ignores_multi_key_envelopes() {
        // §4.3: 双键信封 → apply() 返回 false. It never becomes an Event here.
        let line = r#"{"WindowFocusChanged":{"id":1},"WindowClosed":{"id":2}}"#;
        assert!(parse_event(line).expect("parses").is_none());
    }

    #[test]
    fn rejects_malformed_json() {
        assert!(parse_event("{").is_err());
        assert!(parse_event(r#"{"WorkspaceActivated":{"id":"x"}}"#).is_err());
    }

    #[test]
    fn state_follows_a_realistic_sequence() {
        let mut motion = Motion::new(DEFAULT_SPAN);
        for line in [WORKSPACES_CHANGED, WINDOWS_CHANGED] {
            let event = parse_event(line).expect("parses").expect("is an event");
            motion.apply(&event);
        }
        // DP-1 has two workspaces and the active one (ws15, idx 2) sorts first
        // → vertical 0.0; ws15 has a single column, which ranks 0 → 0.0.
        let progress = motion.progress("DP-1");
        assert!((progress.horizontal - 0.0).abs() < 1e-9, "{progress:?}");
        assert!((progress.vertical - 0.0).abs() < 1e-9, "{progress:?}");
    }
}
