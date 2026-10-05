//! Layout-reactive parallax maths (`HANDOFF.md` §4).
//!
//! Pure logic: no IO, no clock, no GL. The compositor's event stream is applied
//! through [`Motion::apply`], which maintains the workspace/window state and the
//! per-workspace "remembered focus column"; [`Motion::progress`] then answers
//! "where should the wallpaper be for this output", and [`offset_px`] turns that
//! into pixels.
//!
//! Two things are deliberately *not* here:
//!
//! * The animation (§4.2.6: OutCubic, 600 ms, ignore identical targets, retarget
//!   from the current position) — that is frame-callback driven and lives in
//!   `render/anim.rs`. This module only produces targets.
//! * `panFor()` from the §4.3 table. It is the mpv-era formulation
//!   (`video-pan-x`/`video-zoom`), and §4.1 explicitly says not to use it: the
//!   renderer draws with a pixel offset. Implementing it would be dead code.

use std::collections::HashMap;

/// The parallax span for columns, fixed by spec (§4.2.1): *not* the workspace's
/// real column count, otherwise opening or closing any window would move the
/// wallpaper.
pub const DEFAULT_COLUMN_SPAN: usize = 6;
/// The parallax span for workspaces, fixed for the same reason: dividing by the
/// live workspace count moves the wallpaper whenever one is created or closed.
///
/// Separate from the column span because the two count different things —
/// columns within a workspace versus workspaces on an output — and a setup with
/// many workspaces should not have to shrink its column steps to say so.
pub const DEFAULT_WORKSPACE_SPAN: usize = 6;
/// Parallax scale: the canvas is enlarged by this factor and cropped.
pub const DEFAULT_SCALE: f64 = 1.1;
/// Upper bound on the scale, taken from the §4.3 `panFor(_, 9)` row.
pub const MAX_SCALE: f64 = 1.35;

/// A workspace as niri reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Workspace {
    /// Stable identifier (niri's `id`).
    pub id: u64,
    /// Display index (niri's `idx`) — the ordering key for the vertical axis.
    pub idx: u32,
    pub output: String,
    /// This is the workspace currently shown on its output.
    pub is_active: bool,
    /// This is the globally focused workspace.
    pub is_focused: bool,
    pub active_window_id: Option<u64>,
}

/// A window as far as parallax is concerned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Window {
    pub id: u64,
    pub workspace_id: u64,
    /// niri's `pos_in_scrolling_layout` column: an opaque ordering key.
    pub column: usize,
    /// Position within the column.
    pub tile: usize,
    pub is_floating: bool,
    pub is_focused: bool,
}

/// Events that affect parallax (§4.2.7). Anything else is ignored.
#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    WorkspacesChanged {
        workspaces: Vec<Workspace>,
    },
    WorkspaceActivated {
        id: u64,
        focused: bool,
    },
    WorkspaceActiveWindowChanged {
        workspace_id: u64,
        active_window_id: Option<u64>,
    },
    WindowsChanged {
        windows: Vec<Window>,
    },
    WindowOpenedOrChanged {
        window: Window,
    },
    WindowClosed {
        id: u64,
    },
    WindowFocusChanged {
        id: Option<u64>,
    },
    WindowLayoutsChanged {
        /// `(window id, column, tile)`; unknown ids are ignored.
        changes: Vec<(u64, usize, usize)>,
    },
}

/// `(horizontal, vertical)`, both 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub horizontal: f64,
    pub vertical: f64,
}

impl Progress {
    pub const CENTER: Self = Self {
        horizontal: 0.5,
        vertical: 0.5,
    };

    pub fn new(horizontal: f64, vertical: f64) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    /// Linear interpolation towards `to`, used by the easing in `render::anim`.
    pub fn lerp(self, to: Self, t: f64) -> Self {
        Self {
            horizontal: self.horizontal + (to.horizontal - self.horizontal) * t,
            vertical: self.vertical + (to.vertical - self.vertical) * t,
        }
    }
}

#[derive(Debug, Default)]
pub struct Motion {
    column_span: usize,
    workspace_span: usize,
    workspaces: Vec<Workspace>,
    windows: Vec<Window>,
    focused_id: Option<u64>,
    /// The workspace the focus was last *seen* on, kept when niri reports
    /// `WindowFocusChanged(None)` (the overview). It answers only one question —
    /// "was the focus on this output's active workspace?" — so that the
    /// horizontal can tell a focus move from a workspace switch. It must never
    /// drive the remembered column: in the overview the focus is `None` and the
    /// active window is the signal that moves.
    last_focus_workspace: Option<u64>,
    /// Per workspace (by id) focus column, remembered across column churn.
    remembered: HashMap<u64, usize>,
    /// Per output: the horizontal the wallpaper has settled on. Held across
    /// workspace switches — see [`Motion::refresh_horizontal`].
    horizontal: HashMap<String, f64>,
}

impl Motion {
    pub fn new(column_span: usize, workspace_span: usize) -> Self {
        // A span of 1 would divide by zero; the defaults are 6.
        Self {
            column_span: column_span.max(2),
            workspace_span: workspace_span.max(2),
            ..Default::default()
        }
    }

    pub fn column_span(&self) -> usize {
        self.column_span
    }

    /// Take new spans from a reloaded config. Both axes are fixed by design
    /// (§4.2.1), so this only ever changes the *scale* of the travel.
    pub fn set_spans(&mut self, column_span: usize, workspace_span: usize) {
        self.column_span = column_span.max(2);
        self.workspace_span = workspace_span.max(2);
    }

    pub fn workspace_span(&self) -> usize {
        self.workspace_span
    }

    pub fn focused_id(&self) -> Option<u64> {
        self.focused_id
    }

    pub fn workspaces(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn window(&self, id: u64) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Forget everything (niri restarted, or the connection was re-established).
    pub fn reset(&mut self) {
        self.workspaces.clear();
        self.windows.clear();
        self.focused_id = None;
        self.last_focus_workspace = None;
        self.remembered.clear();
        self.horizontal.clear();
    }

    /// Apply one event. Returns whether it is one of the events §4.2.7 lists.
    pub fn apply(&mut self, event: &Event) -> bool {
        // Per output, before the event: which workspace is displayed, and which
        // one the focus sits on. The horizontal only pans when neither moved in
        // the wrong way — see [`Motion::refresh_horizontal`].
        let before: Vec<(String, Option<u64>, Option<u64>)> = self
            .outputs()
            .into_iter()
            .map(|output| {
                let active = self.active_workspace(&output).map(|w| w.id);
                let focus = self.last_focus_workspace_on(&output);
                (output, active, focus)
            })
            .collect();
        match event {
            Event::WorkspacesChanged { workspaces } => {
                let keep: Vec<u64> = workspaces.iter().map(|w| w.id).collect();
                self.workspaces = workspaces.clone();
                // State for workspaces that no longer exist is dropped, and so
                // are windows that lived on them.
                self.windows.retain(|w| keep.contains(&w.workspace_id));
                self.remembered.retain(|id, _| keep.contains(id));
                if self
                    .focused_id
                    .is_some_and(|id| !self.windows.iter().any(|w| w.id == id))
                {
                    self.focused_id = None;
                }
            }
            Event::WorkspaceActivated { id, focused } => {
                let Some(output) = self
                    .workspaces
                    .iter()
                    .find(|w| w.id == *id)
                    .map(|w| w.output.clone())
                else {
                    return true;
                };
                for workspace in &mut self.workspaces {
                    if workspace.output == output {
                        workspace.is_active = workspace.id == *id;
                    }
                    if *focused {
                        workspace.is_focused = workspace.id == *id;
                    }
                }
            }
            Event::WorkspaceActiveWindowChanged {
                workspace_id,
                active_window_id,
            } => {
                if let Some(workspace) = self.workspaces.iter_mut().find(|w| w.id == *workspace_id)
                {
                    workspace.active_window_id = *active_window_id;
                }
            }
            Event::WindowsChanged { windows } => {
                self.windows = windows.clone();
                if self
                    .focused_id
                    .is_some_and(|id| !self.windows.iter().any(|w| w.id == id))
                {
                    self.focused_id = None;
                }
                self.adopt_reported_focus();
            }
            Event::WindowOpenedOrChanged { window } => {
                match self.windows.iter_mut().find(|w| w.id == window.id) {
                    Some(existing) => *existing = window.clone(),
                    None => self.windows.push(window.clone()),
                }
                // This event also fires for plain changes (a title, an app id),
                // whose payload is not focused. Only a window that reports
                // itself focused is news about the focus.
                if window.is_focused {
                    self.adopt_focus(window.id);
                }
            }
            Event::WindowClosed { id } => {
                self.windows.retain(|w| w.id != *id);
                if self.focused_id == Some(*id) {
                    self.focused_id = None;
                }
                // The remembered column stays: that is what "记忆保留" means.
            }
            Event::WindowFocusChanged { id } => {
                // `focused_id` keeps the compositor's literal answer, including
                // `None`: the overview reports no focused window while it is
                // open, and *instead* reports what it is working with as
                // `WorkspaceActiveWindowChanged` — so a remembered focus here
                // would mask the very signal the overview moves with.
                //
                // `last_focus_workspace` is the separate, sticky answer the
                // horizontal's test needs (see the field's comment).
                self.focused_id = *id;
                if let Some(id) = id {
                    self.remember_focus_workspace(*id);
                }
            }
            Event::WindowLayoutsChanged { changes } => {
                for (id, column, tile) in changes {
                    if let Some(window) = self.windows.iter_mut().find(|w| w.id == *id) {
                        window.column = *column;
                        window.tile = *tile;
                    }
                }
            }
        }
        self.refresh_memory();
        self.refresh_horizontal(&before);
        true
    }

    /// A window that reports itself focused is the focused window: niri
    /// documents that all other windows are no longer focused in that case.
    /// `WindowFocusChanged` remains the primary signal.
    fn adopt_reported_focus(&mut self) {
        if let Some(id) = self.windows.iter().find(|w| w.is_focused).map(|w| w.id) {
            self.adopt_focus(id);
        }
    }

    /// Adopt `id` as the focused window and clear every other window's flag.
    ///
    /// `is_focused` is only ever true for the one focused window, so a window
    /// reporting itself focused makes every *other* window's flag stale — and a
    /// stale flag is what [`Motion::adopt_reported_focus`] would otherwise pick
    /// up, dragging the focus (and with it the horizontal) back to whichever
    /// window was focused last.
    fn adopt_focus(&mut self, id: u64) {
        for window in &mut self.windows {
            window.is_focused = window.id == id;
        }
        self.focused_id = Some(id);
        self.remember_focus_workspace(id);
    }

    /// Record where the focus was last seen, for the horizontal's test only.
    fn remember_focus_workspace(&mut self, window_id: u64) {
        if let Some(workspace) = self
            .windows
            .iter()
            .find(|w| w.id == window_id)
            .and_then(|window| self.workspaces.iter().find(|w| w.id == window.workspace_id))
            .map(|workspace| workspace.id)
        {
            self.last_focus_workspace = Some(workspace);
        }
    }

    /// Where the focus was last seen, but only while that workspace is on
    /// `output` — `None` if the focus has never been seen there.
    fn last_focus_workspace_on(&self, output: &str) -> Option<u64> {
        let workspace = self.last_focus_workspace?;
        self.workspaces
            .iter()
            .find(|w| w.id == workspace && w.output == output)
            .map(|w| w.id)
    }

    /// Re-derive the remembered focus column of every displayed workspace.
    ///
    /// Runs after every event because all three of these can move it: the
    /// reported active window, the focused window, and a column change that
    /// relocates either of them. The focused window wins when both are known.
    fn refresh_memory(&mut self) {
        let active: Vec<(u64, u64)> = self
            .workspaces
            .iter()
            .filter(|w| w.is_active)
            .filter_map(|w| w.active_window_id.map(|id| (w.id, id)))
            .collect();
        for (workspace_id, window_id) in active {
            if let Some(window) = self
                .windows
                .iter()
                .find(|w| w.id == window_id && !w.is_floating)
            {
                self.remembered.insert(workspace_id, window.column);
            }
        }

        // Only tiled windows on a workspace that is actually displayed count
        // (§4.2.3); focusing a floating window keeps the previous column.
        let Some(id) = self.focused_id else { return };
        let Some(window) = self.windows.iter().find(|w| w.id == id) else {
            return;
        };
        if window.is_floating {
            return;
        }
        let Some(workspace) = self.workspaces.iter().find(|w| w.id == window.workspace_id) else {
            return;
        };
        if !workspace.is_active {
            return;
        }
        self.remembered.insert(workspace.id, window.column);
    }

    /// Every output any workspace lives on, deduplicated.
    fn outputs(&self) -> Vec<String> {
        let mut outputs: Vec<String> = self.workspaces.iter().map(|w| w.output.clone()).collect();
        outputs.sort();
        outputs.dedup();
        outputs
    }

    /// The workspace currently shown on `output`.
    fn active_workspace(&self, output: &str) -> Option<&Workspace> {
        self.workspaces
            .iter()
            .find(|w| w.output == output && w.is_active)
    }

    /// Re-derive the settled horizontal for each output the focus moved *within*.
    ///
    /// Switching workspaces is a **vertical** move: niri slides the workspace
    /// strip, and the new workspace's own scroll position is not a movement of
    /// the viewport. Panning the wallpaper horizontally along with it makes both
    /// axes advance in lockstep — the diagonal glide this rule exists to
    /// prevent, which reads as neither a horizontal nor a vertical move.
    ///
    /// So the horizontal is refreshed only while the focus was *already* on this
    /// output's active workspace: that is precisely when a horizontal move of
    /// the view is what is happening (a column focus change, a window closing,
    /// a column relocation). Everything else — a workspace activation, and the
    /// `WindowFocusChanged` niri sends right after it naming the new
    /// workspace's window — leaves the horizontal where it is and moves the
    /// vertical alone.
    fn refresh_horizontal(&mut self, before: &[(String, Option<u64>, Option<u64>)]) {
        for (output, active_before, focus_before) in before {
            let Some(active) = self.active_workspace(output) else {
                continue;
            };
            let active_id = active.id;
            // A different workspace is displayed now: that is the workspace
            // strip sliding, a vertical move. Nothing horizontal happened, so
            // the value holds — even in the overview, where the focus is
            // reported as absent and would otherwise pass the test below.
            if *active_before != Some(active_id) {
                continue;
            }
            // The focus was on this output's active workspace and still is:
            // whatever moved was the horizontal scroll, not the workspace strip.
            //
            // No focus before *and* after counts too: that is the overview,
            // which reports its selection as `WorkspaceActiveWindowChanged`
            // instead of as a focus change, and reports no focus while it is
            // open. Holding there would freeze the horizontal for the whole
            // overview.
            //
            // Both ends matter. A workspace switched while nothing was focused
            // (say the focused window had just been closed) is followed by niri
            // focusing a window in the new workspace; `None` → `Some` is that
            // focus arriving, not a horizontal move, and panning there would
            // drag the horizontal along with the vertical again.
            let pans = *focus_before == Some(active_id) || focus_before.is_none();
            // Establish it once the state is meaningful (a window is focused),
            // so that a workspace switch arriving before the first horizontal
            // move freezes the right value instead of re-deriving one for the
            // workspace being switched to.
            let fresh = !self.horizontal.contains_key(output) && self.focused_id.is_some();
            if !pans && !fresh {
                continue;
            }
            let columns = self.columns_of(active_id);
            let horizontal = self.horizontal_for(active_id, &columns);
            self.horizontal.insert(output.clone(), horizontal);
        }
    }

    /// §4.2.4: the horizontal for a workspace, from its remembered column.
    fn horizontal_for(&self, workspace_id: u64, columns: &[usize]) -> f64 {
        if columns.is_empty() {
            // No columns at all → centred (§4.2.4).
            return 0.5;
        }
        let Some(workspace) = self.workspaces.iter().find(|w| w.id == workspace_id) else {
            return 0.5;
        };
        let column = self.resolved_column(workspace, columns);
        let rank = columns.iter().position(|c| *c == column).unwrap_or(0);
        // A single column ranks 0 → 0.0, i.e. flush left. That is the spec
        // (§4.2.4), not a bug to "fix" to 0.5.
        (rank as f64 / (self.column_span - 1) as f64).clamp(0.0, 1.0)
    }

    /// `(horizontal, vertical)` for `output` (§4.1).
    pub fn progress(&self, output: &str) -> Progress {
        let mut on_output: Vec<&Workspace> = self
            .workspaces
            .iter()
            .filter(|w| w.output == output)
            .collect();
        let Some(active) = on_output.iter().find(|w| w.is_active).copied() else {
            // No active workspace on this output (§4.2.5).
            return Progress::CENTER;
        };
        on_output.sort_by_key(|w| w.idx);
        let vertical = if on_output.len() <= 1 {
            // Nothing to pan between, so centre — which is what the §4.3 rows
            // with a single workspace expect (`(_, 0.5)`).
            0.5
        } else {
            let index = on_output
                .iter()
                .position(|w| w.id == active.id)
                .unwrap_or(0);
            // Fixed spread, for the same reason as the columns (§4.2.1):
            // dividing by the live workspace count moves the wallpaper —
            // and changes the parallax ratio — whenever a workspace is
            // created or closed, for a workspace that did not change. Measured
            // before this: the same active workspace sat at 72 / 48 / 36 px
            // depending on whether 3 / 4 / 5 workspaces existed.
            (index as f64 / (self.workspace_span - 1) as f64).clamp(0.0, 1.0)
        };

        let horizontal = match self.horizontal.get(output) {
            Some(value) => *value,
            // Nothing has established one yet (before the first event): derive
            // it from the current state.
            None => self.horizontal_for(active.id, &self.columns_of(active.id)),
        };
        Progress::new(horizontal, vertical)
    }

    /// Distinct columns of a workspace, ascending. Floating windows are ignored:
    /// they have no column.
    fn columns_of(&self, workspace_id: u64) -> Vec<usize> {
        let mut columns: Vec<usize> = self
            .windows
            .iter()
            .filter(|w| w.workspace_id == workspace_id && !w.is_floating)
            .map(|w| w.column)
            .collect();
        columns.sort_unstable();
        columns.dedup();
        columns
    }

    /// The column to rank: the remembered one resolved to the nearest column
    /// that still exists (§4.2.2), else the active window's, else the leftmost.
    fn resolved_column(&self, workspace: &Workspace, columns: &[usize]) -> usize {
        if let Some(remembered) = self.remembered.get(&workspace.id) {
            return nearest(columns, *remembered);
        }
        if let Some(id) = workspace.active_window_id {
            if let Some(window) = self.windows.iter().find(|w| w.id == id && !w.is_floating) {
                return nearest(columns, window.column);
            }
        }
        if let Some(id) = self.focused_id {
            if let Some(window) = self
                .windows
                .iter()
                .find(|w| w.id == id && w.workspace_id == workspace.id && !w.is_floating)
            {
                return nearest(columns, window.column);
            }
        }
        columns[0]
    }
}

/// Nearest value in a sorted, non-empty list; ties go to the lower one.
fn nearest(sorted: &[usize], target: usize) -> usize {
    let mut best = sorted[0];
    let mut best_distance = best.abs_diff(target);
    for candidate in &sorted[1..] {
        let distance = candidate.abs_diff(target);
        if distance < best_distance {
            best = *candidate;
            best_distance = distance;
        }
    }
    best
}

/// Pixel offset of the canvas relative to centre (§4.1), for `screen` in pixels.
///
/// `progress 0` puts the leftmost column to the *right* of centre by half the
/// overflow, `progress 1` mirrors that: the §4.3 pixel rows are the authority
/// here (`(0,0) → x = +128`), which is the opposite sign from the formula as it
/// is written in §4.1.
pub fn offset_px(progress: Progress, screen: (f64, f64), scale: f64) -> (f64, f64) {
    let scale = scale.clamp(1.0, MAX_SCALE);
    let overflow_x = screen.0 * (scale - 1.0);
    let overflow_y = screen.1 * (scale - 1.0);
    (
        overflow_x * (0.5 - progress.horizontal),
        overflow_y * (0.5 - progress.vertical),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- fixtures from the §4.3 helper comment -----------------------------

    /// `workspace{idx=1, output="DP-1", is_active=true, is_focused=false,
    /// active_window_id=nil}`
    fn default_workspace() -> Workspace {
        Workspace {
            id: 1,
            idx: 1,
            output: "DP-1".to_owned(),
            is_active: true,
            is_focused: false,
            active_window_id: None,
        }
    }

    /// `window{workspace_id=1, column=1, tile=1, is_focused=false,
    /// is_floating=false}`
    fn window(id: u64, column: usize, tile: usize) -> Window {
        window_on(id, 1, column, tile)
    }

    fn window_on(id: u64, workspace_id: u64, column: usize, tile: usize) -> Window {
        Window {
            id,
            workspace_id,
            column,
            tile,
            is_floating: false,
            is_focused: false,
        }
    }

    fn floating(id: u64, column: usize) -> Window {
        Window {
            workspace_id: 1,
            is_floating: true,
            ..window(id, column, 1)
        }
    }

    fn motion_with(workspaces: Vec<Workspace>, windows: Vec<Window>) -> Motion {
        let mut motion = Motion::new(DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN);
        motion.apply(&Event::WorkspacesChanged { workspaces });
        motion.apply(&Event::WindowsChanged { windows });
        motion
    }

    fn assert_progress(motion: &Motion, output: &str, horizontal: f64, vertical: f64) {
        let got = motion.progress(output);
        assert!(
            (got.horizontal - horizontal).abs() < 1e-9 && (got.vertical - vertical).abs() < 1e-9,
            "{output}: got ({}, {}), want ({horizontal}, {vertical})",
            got.horizontal,
            got.vertical
        );
    }

    // --- the §4.3 table, row by row ----------------------------------------

    #[test]
    fn active_window_column_is_ranked_against_fixed_span() {
        // ws1(active_window=1)、窗口列 {70,10,30,30(tile2)} → (0.4, 0.5)
        let mut workspace = default_workspace();
        workspace.active_window_id = Some(1);
        let mut motion = motion_with(
            vec![workspace],
            vec![
                window(1, 70, 1),
                window(2, 10, 1),
                window(3, 30, 1),
                window(4, 30, 2),
            ],
        );
        assert_progress(&motion, "DP-1", 0.4, 0.5);

        // ↳ WindowFocusChanged(4)（列 30） → (0.2, 0.5)
        motion.apply(&Event::WindowFocusChanged { id: Some(4) });
        assert_progress(&motion, "DP-1", 0.2, 0.5);
    }

    #[test]
    fn single_column_is_flush_left() {
        // 单列（两窗口同列 8） → (0.0, 0.5)
        let mut workspace = default_workspace();
        workspace.active_window_id = Some(1);
        let motion = motion_with(vec![workspace], vec![window(1, 8, 1), window(2, 8, 2)]);
        assert_progress(&motion, "DP-1", 0.0, 0.5);
    }

    #[test]
    fn no_workspaces_is_centred() {
        // WindowsChanged 为空 → (0.5, 0.5)
        let motion = motion_with(vec![default_workspace()], vec![]);
        assert_progress(&motion, "DP-1", 0.5, 0.5);
    }

    #[test]
    fn two_outputs_are_independent() {
        // 双输出：DP-1 活动工作区 idx1；HDMI-A-1 idx2 活动、列为 {1,2}
        // → DP-1 (0.0, 0.0)；HDMI-A-1 (0.2, 0.5)
        // DP-1 needs two workspaces for its vertical to be 0.0; HDMI-A-1 has a
        // single one, which is the 0.5 case.
        let mut dp1 = default_workspace();
        dp1.active_window_id = Some(1);
        let dp1_other = Workspace {
            id: 3,
            idx: 3,
            is_active: false,
            ..default_workspace()
        };
        let hdmi = Workspace {
            id: 2,
            idx: 2,
            output: "HDMI-A-1".to_owned(),
            is_active: true,
            is_focused: true,
            active_window_id: Some(4),
        };
        let windows = vec![
            window_on(1, 1, 1, 1), // DP-1, column 1 — the active window
            window_on(2, 1, 2, 1), // DP-1, column 2
            window_on(3, 2, 1, 1), // HDMI-A-1, column 1
            window_on(4, 2, 2, 1), // HDMI-A-1, column 2 — the active window
        ];
        let mut motion = motion_with(vec![dp1, hdmi, dp1_other], windows);
        assert_progress(&motion, "DP-1", 0.0, 0.0);
        assert_progress(&motion, "HDMI-A-1", 0.2, 0.5);

        // 查询未知输出 → (0.5, 0.5)
        assert_progress(&motion, "eDP-1", 0.5, 0.5);

        // ↳ WindowFocusChanged(2)（DP-1 列 2） → DP-1 (0.2, 0.0)；HDMI-A-1 仍为 (0.2, 0.5)
        motion.apply(&Event::WindowFocusChanged { id: Some(2) });
        assert_progress(&motion, "DP-1", 0.2, 0.0);
        assert_progress(&motion, "HDMI-A-1", 0.2, 0.5);
    }

    // --- the horizontal must not ride along with a vertical move -------------
    //
    // niri sends `WorkspaceActivated` followed by a `WindowFocusChanged` naming
    // the new workspace's window, so the event *types* cannot tell a workspace
    // switch from a horizontal move. What can: whether the focus was already on
    // this output's active workspace. Without that, both axes advance in
    // lockstep and the wallpaper glides diagonally — reproduced live before
    // this rule (h and v advanced by the same delta every frame).

    /// Two workspaces whose focus columns rank differently, plus a third with
    /// no columns at all.
    /// Two workspaces on one output; the first is active, has columns {1,2,3}
    /// and the focus is on column 3.
    fn focus_on_column_three(column_span: usize, workspace_span: usize) -> Motion {
        let ws1 = Workspace {
            is_focused: true,
            active_window_id: Some(4),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            ..default_workspace()
        };
        let mut motion = Motion::new(column_span, workspace_span);
        motion.apply(&Event::WorkspacesChanged {
            workspaces: vec![ws1, ws2],
        });
        motion.apply(&Event::WindowsChanged {
            windows: vec![
                window(1, 1, 1),
                window(2, 2, 1),
                Window {
                    is_focused: true,
                    ..window(4, 3, 1)
                },
            ],
        });
        motion
    }

    fn two_workspaces_with_different_columns() -> Motion {
        let ws1 = Workspace {
            is_focused: true,
            active_window_id: Some(4),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            active_window_id: Some(3),
            ..default_workspace()
        };
        let ws3 = Workspace {
            id: 3,
            idx: 3,
            is_active: false,
            ..default_workspace()
        };
        motion_with(
            vec![ws1, ws2, ws3],
            vec![
                // ws1 columns {1,2,3}, focus on column 3 → rank 2 → 0.4
                window(1, 1, 1),
                window(2, 2, 1),
                Window {
                    is_focused: true,
                    ..window(4, 3, 1)
                },
                // ws2: a single column → rank 0 → 0.0
                window_on(3, 2, 1, 1),
            ],
        )
    }

    #[test]
    fn switching_workspaces_does_not_pan_horizontally() {
        let mut motion = two_workspaces_with_different_columns();
        assert_progress(&motion, "DP-1", 0.4, 0.0);

        motion.apply(&Event::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        // The vertical moves to ws2's slot; the horizontal holds ws1's 0.4
        // rather than jumping to ws2's 0.0 (a 102 px sideways slide).
        assert_progress(&motion, "DP-1", 0.4, 0.2);

        // …and niri's follow-up focus change into the new workspace is part of
        // the same vertical move, so it must not pan either.
        motion.apply(&Event::WindowFocusChanged { id: Some(3) });
        assert_progress(&motion, "DP-1", 0.4, 0.2);

        // A focus move *within* that workspace is a horizontal move: it pans.
        motion.apply(&Event::WindowOpenedOrChanged {
            window: window_on(5, 2, 4, 1),
        });
        motion.apply(&Event::WindowFocusChanged { id: Some(5) });
        // ws2's columns are now {1,4}, focus on 4 → rank 1 → 0.2.
        assert_progress(&motion, "DP-1", 0.2, 0.2);
    }

    #[test]
    fn switching_to_a_workspace_without_columns_does_not_pan_either() {
        // §4.2.4 makes a columnless workspace horizontal 0.5. Panning to it from
        // 0.4 is a 128 px slide for a purely vertical move.
        let mut motion = two_workspaces_with_different_columns();
        motion.apply(&Event::WorkspaceActivated {
            id: 3,
            focused: true,
        });
        assert_progress(&motion, "DP-1", 0.4, 0.4);
        motion.apply(&Event::WindowFocusChanged { id: None });
        assert_progress(&motion, "DP-1", 0.4, 0.4);

        // Coming back, the horizontal is still ws1's.
        motion.apply(&Event::WorkspaceActivated {
            id: 1,
            focused: true,
        });
        assert_progress(&motion, "DP-1", 0.4, 0.0);
        // …and a focus move inside it pans, as it does anywhere else. (The
        // `None` above no longer forgets window 4, so this is a real horizontal
        // move: column 3 → column 1.)
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.0);
    }

    #[test]
    fn the_two_spans_are_independent() {
        // The two axes count different things (columns within a workspace,
        // workspaces on an output), so one must not scale the other.
        // Fixture: ws1 active with columns {1,2,3} and the focus on column 3,
        // ws2 second on the same output.

        let mut motion = focus_on_column_three(6, 4);
        // Column 3 of {1,2,3} → rank 2; column_span 6 → 2/5.
        assert_progress(&motion, "DP-1", 0.4, 0.0);
        motion.apply(&Event::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        // workspace_span 4 → the second of two workspaces is 1/3, not 1/5.
        assert_progress(&motion, "DP-1", 0.4, 1.0 / 3.0);

        // Same fixture, column_span 3: only the horizontal changes.
        let narrow = focus_on_column_three(3, 4);
        assert_progress(&narrow, "DP-1", 1.0, 0.0);
    }

    #[test]
    fn creating_or_closing_a_workspace_does_not_move_the_wallpaper() {
        // The vertical spread is fixed for the same reason as the columns
        // (§4.2.1): with the live workspace count, the same active workspace sat
        // at 72 / 48 / 36 px depending on whether 3 / 4 / 5 workspaces existed,
        // so creating or closing one jerked the wallpaper for a workspace that
        // did not change.
        let positions = |count: u32| {
            let mut motion = Motion::new(DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN);
            let workspaces: Vec<Workspace> = (1..=count)
                .map(|idx| Workspace {
                    id: idx as u64,
                    idx,
                    is_active: idx == 2,
                    ..default_workspace()
                })
                .collect();
            motion.apply(&Event::WorkspacesChanged { workspaces });
            motion.progress("DP-1").vertical
        };
        assert_eq!(positions(4), 1.0 / 5.0);
        assert_eq!(
            positions(5),
            positions(4),
            "a workspace appearing changed it"
        );
        assert_eq!(positions(3), positions(4), "a workspace closing changed it");
    }

    #[test]
    fn a_plain_window_change_does_not_steal_the_focus_back() {
        // `WindowOpenedOrChanged` fires for title/app-id changes too, whose
        // payload is not focused. A stale `is_focused` on the previously focused
        // window used to make those events re-adopt it, yanking the focus (and
        // the horizontal) back.
        let mut motion = two_workspaces_with_different_columns();
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.0);

        let mut renamed = window(1, 1, 1);
        renamed.column = 1;
        motion.apply(&Event::WindowOpenedOrChanged { window: renamed });
        assert_eq!(motion.focused_id(), Some(1), "focus stayed on window 1");
        assert_progress(&motion, "DP-1", 0.0, 0.0);
    }

    #[test]
    fn a_focus_move_within_the_workspace_still_pans() {
        // The guard against over-suppressing: same workspace, different column.
        let mut motion = two_workspaces_with_different_columns();
        assert_progress(&motion, "DP-1", 0.4, 0.0);
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.0);
        motion.apply(&Event::WindowFocusChanged { id: Some(4) });
        assert_progress(&motion, "DP-1", 0.4, 0.0);
    }

    #[test]
    fn workspace_activation_sets_active_and_focused() {
        // WorkspaceActivated(2, focused=false)（同输出另一工作区） → (0.0, 1.0)
        // ↳ 断言 ws1.is_focused=true、ws2.is_focused=false、另一输出 ws3.is_active=true
        // ↳ WorkspaceActivated(3, focused=true) → ws1.is_focused=false、ws3.is_focused=true
        let ws1 = Workspace {
            is_focused: true,
            active_window_id: Some(1),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            ..default_workspace()
        };
        let ws3 = Workspace {
            id: 3,
            idx: 3,
            output: "HDMI-A-1".to_owned(),
            is_active: true,
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1, ws2, ws3],
            vec![window(1, 1, 1), window_on(2, 2, 1, 1)],
        );
        assert_progress(&motion, "DP-1", 0.0, 0.0);

        motion.apply(&Event::WorkspaceActivated {
            id: 2,
            focused: false,
        });
        // Two workspaces on DP-1, the second active → 1/(span-1) = 0.2.
        assert_progress(&motion, "DP-1", 0.0, 0.2);
        let by_id = |motion: &Motion, id: u64| {
            motion
                .workspaces()
                .iter()
                .find(|w| w.id == id)
                .cloned()
                .unwrap()
        };
        assert!(by_id(&motion, 1).is_focused);
        assert!(!by_id(&motion, 2).is_focused);
        assert!(by_id(&motion, 3).is_active);

        motion.apply(&Event::WorkspaceActivated {
            id: 3,
            focused: true,
        });
        assert!(!by_id(&motion, 1).is_focused);
        assert!(by_id(&motion, 3).is_focused);
    }

    #[test]
    fn active_window_supplies_the_remembered_column() {
        // 活动窗口记忆列（active_window=2、列 {1,2}） → (0.2, 0.0)
        let ws1 = Workspace {
            active_window_id: Some(2),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            ..default_workspace()
        };
        let motion = motion_with(vec![ws1, ws2], vec![window(1, 1, 1), window(2, 2, 1)]);
        assert_progress(&motion, "DP-1", 0.2, 0.0);
    }

    #[test]
    fn workspaces_changed_drops_stale_state() {
        // WorkspacesChanged 只剩别输出工作区 → 该输出 (0.5, 0.5)，旧工作区状态被丢弃
        let ws1 = default_workspace();
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            ..default_workspace()
        };
        let mut motion = motion_with(vec![ws1, ws2], vec![window(1, 1, 1)]);
        assert_progress(&motion, "DP-1", 0.0, 0.0);

        let other = Workspace {
            id: 5,
            idx: 1,
            output: "HDMI-A-1".to_owned(),
            is_active: true,
            ..default_workspace()
        };
        motion.apply(&Event::WorkspacesChanged {
            workspaces: vec![other],
        });
        assert_progress(&motion, "DP-1", 0.5, 0.5);
        assert_eq!(
            motion.window(1),
            None,
            "windows of gone workspaces are dropped"
        );
    }

    #[test]
    fn floating_active_window_keeps_previous_column() {
        // 活动窗口是浮动窗口 → (0.0, 0.5)
        let mut workspace = default_workspace();
        workspace.active_window_id = Some(1);
        let mut motion = motion_with(vec![workspace], vec![window(1, 1, 1), floating(2, 5)]);
        motion.apply(&Event::WindowFocusChanged { id: Some(2) });
        assert_progress(&motion, "DP-1", 0.0, 0.5);
    }

    #[test]
    fn closing_windows_resolves_to_the_nearest_remaining_column() {
        // 焦点列 3（列 {1,2,3}） → (0.4, 0.5)
        let ws1 = Workspace {
            active_window_id: Some(3),
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1],
            vec![window(1, 1, 1), window(2, 2, 1), window(3, 3, 1)],
        );
        assert_progress(&motion, "DP-1", 0.4, 0.5);

        // ↳ WindowClosed(1)（列剩 {2,3}） → (0.2, 0.5)
        motion.apply(&Event::WindowClosed { id: 1 });
        assert_progress(&motion, "DP-1", 0.2, 0.5);

        // ↳ WindowClosed(3)（焦点被关，记忆保留） → (0.0, 0.5)，且 focused_id == nil
        motion.apply(&Event::WindowFocusChanged { id: Some(3) });
        motion.apply(&Event::WindowClosed { id: 3 });
        assert_progress(&motion, "DP-1", 0.0, 0.5);
        assert_eq!(motion.focused_id(), None);

        // ↳ WorkspaceActiveWindowChanged(2) + WindowOpenedOrChanged(列 4) → (0.0, 0.5)
        motion.apply(&Event::WorkspaceActiveWindowChanged {
            workspace_id: 1,
            active_window_id: Some(2),
        });
        motion.apply(&Event::WindowOpenedOrChanged {
            window: window(4, 4, 1),
        });
        assert_progress(&motion, "DP-1", 0.0, 0.5);
    }

    #[test]
    fn layout_changes_move_columns_and_ignore_unknown_windows() {
        // WindowLayoutsChanged 批 {1→列3, 3→列1, 99→未知} → (0.4, 0.5)，未知窗口被忽略
        let ws1 = Workspace {
            active_window_id: Some(1),
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1],
            vec![window(1, 1, 1), window(2, 2, 1), window(3, 3, 1)],
        );
        assert_progress(&motion, "DP-1", 0.0, 0.5);

        motion.apply(&Event::WindowLayoutsChanged {
            changes: vec![(1, 3, 1), (3, 1, 1), (99, 2, 1)],
        });
        assert_progress(&motion, "DP-1", 0.4, 0.5);
        assert_eq!(motion.window(99), None);
    }

    #[test]
    fn focus_moves_between_tiled_and_floating_windows() {
        // 焦点列 2（列 {1,2,3}） → (0.2, 0.5)
        let ws1 = Workspace {
            active_window_id: Some(2),
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1],
            vec![
                window(1, 1, 1),
                window(2, 2, 1),
                window(3, 3, 1),
                floating(4, 9),
            ],
        );
        assert_progress(&motion, "DP-1", 0.2, 0.5);

        // ↳ 焦点移到浮动窗口 → (0.2, 0.5)
        motion.apply(&Event::WindowFocusChanged { id: Some(4) });
        assert_progress(&motion, "DP-1", 0.2, 0.5);

        // ↳ 焦点回列 3 → (0.4, 0.5)
        motion.apply(&Event::WindowFocusChanged { id: Some(3) });
        assert_progress(&motion, "DP-1", 0.4, 0.5);
    }

    #[test]
    fn sparse_workspace_indices_are_ordered_not_dense() {
        // 工作区 idx {90,3,40}、活动=idx90、10 列 {10..100}、focus 列 100 → (1.0, 1.0)
        let ws90 = Workspace {
            id: 90,
            idx: 90,
            active_window_id: Some(10),
            ..default_workspace()
        };
        let ws3 = Workspace {
            id: 3,
            idx: 3,
            is_active: false,
            ..default_workspace()
        };
        let ws40 = Workspace {
            id: 40,
            idx: 40,
            is_active: false,
            ..default_workspace()
        };
        let windows: Vec<Window> = (1..=10)
            .map(|i| window_on(i, 90, i as usize * 10, 1))
            .collect();
        let mut motion = motion_with(vec![ws90, ws3, ws40], windows);
        assert_progress(&motion, "DP-1", 1.0, 0.4);

        // ↳ focus 列 10 → (0.0, 1.0)
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.4);
    }

    #[test]
    fn the_eight_listed_events_are_accepted() {
        // §4.2.7 lists exactly eight events, and all of them must be accepted.
        // The other half of that rule — everything else (including multi-key
        // envelopes) being ignored — is decided by the parser in `niri.rs`,
        // where such an event never becomes an `Event` at all.
        let listed = [
            Event::WorkspacesChanged { workspaces: vec![] },
            Event::WorkspaceActivated {
                id: 1,
                focused: false,
            },
            Event::WorkspaceActiveWindowChanged {
                workspace_id: 1,
                active_window_id: None,
            },
            Event::WindowsChanged { windows: vec![] },
            Event::WindowOpenedOrChanged {
                window: window(1, 1, 1),
            },
            Event::WindowClosed { id: 1 },
            Event::WindowFocusChanged { id: None },
            Event::WindowLayoutsChanged { changes: vec![] },
        ];
        assert_eq!(listed.len(), 8);
        for event in listed {
            let mut motion = Motion::new(DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN);
            assert!(motion.apply(&event), "{event:?} must be accepted");
        }
    }

    #[test]
    fn a_workspace_switch_with_nothing_focused_does_not_pan() {
        // The nasty `None` → `Some` case: the focus had been on ws1, the
        // focused window closed (niri reports `None`), then the user switched
        // workspaces and niri focused a window in the new one. That arriving
        // focus is part of a vertical move, not a horizontal one — panning
        // there would drag the horizontal along with the vertical again, which
        // is exactly the diagonal this rule exists to prevent.
        let ws1 = Workspace {
            is_focused: true,
            active_window_id: Some(1),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            active_window_id: Some(4),
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1, ws2],
            vec![
                window(1, 1, 1),
                window(2, 2, 1),
                window(3, 3, 1),
                window_on(4, 2, 3, 1),
            ],
        );
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.0);

        // The focused window goes away.
        motion.apply(&Event::WindowClosed { id: 1 });
        assert_eq!(motion.focused_id(), None);

        // Switch workspaces, then niri focuses a window in the new one (its
        // column 3 would read 0.4 if the horizontal followed it).
        motion.apply(&Event::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        motion.apply(&Event::WindowFocusChanged { id: Some(4) });
        assert_progress(&motion, "DP-1", 0.0, 0.2);
    }

    #[test]
    fn the_overview_pans_horizontally_but_switches_workspaces_vertically() {
        // The exact events niri emits with the overview open, captured from a
        // live session:
        //
        //   ← → : WorkspaceActiveWindowChanged (the workspace's active window
        //         cycles) — the overview moves horizontally
        //   ↑ ↓ : WorkspaceActivated — it switches workspaces
        //
        // The overview reports no focused window while it is open, so the
        // active-window signal is the only one carrying the horizontal move.
        // Letting a remembered focus override it froze the horizontal.
        let ws1 = Workspace {
            is_focused: true,
            active_window_id: Some(1),
            ..default_workspace()
        };
        let ws2 = Workspace {
            id: 2,
            idx: 2,
            is_active: false,
            active_window_id: None,
            ..default_workspace()
        };
        let mut motion = motion_with(
            vec![ws1, ws2],
            vec![window(1, 1, 1), window(2, 2, 1), window(3, 3, 1)],
        );
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        assert_progress(&motion, "DP-1", 0.0, 0.0);

        // The overview opens: niri reports no focused window.
        motion.apply(&Event::WindowFocusChanged { id: None });
        assert_eq!(motion.focused_id(), None);

        // ← → : the workspace's active window cycles right and back.
        for (active, horizontal) in [(2u64, 0.2f64), (3, 0.4), (1, 0.0)] {
            motion.apply(&Event::WorkspaceActiveWindowChanged {
                workspace_id: 1,
                active_window_id: Some(active),
            });
            assert_progress(&motion, "DP-1", horizontal, 0.0);
        }

        // ↑ ↓ : switching workspaces still moves the vertical alone.
        motion.apply(&Event::WorkspaceActivated {
            id: 2,
            focused: true,
        });
        assert_progress(&motion, "DP-1", 0.0, 0.2);
        motion.apply(&Event::WorkspaceActivated {
            id: 1,
            focused: true,
        });
        assert_progress(&motion, "DP-1", 0.0, 0.0);
    }

    #[test]
    fn reset_clears_everything() {
        // reset() 之后 → (0.5, 0.5)，且 focused_id == nil
        let ws1 = Workspace {
            active_window_id: Some(1),
            ..default_workspace()
        };
        let mut motion = motion_with(vec![ws1], vec![window(1, 1, 1)]);
        motion.apply(&Event::WindowFocusChanged { id: Some(1) });
        motion.reset();
        assert_progress(&motion, "DP-1", 0.5, 0.5);
        assert_eq!(motion.focused_id(), None);
    }

    #[test]
    fn span_changes_the_horizontal_scale() {
        // 聚焦第 2 列，span = 6 / 3 / 11 → (0.2, 0.0) / (0.5, 0.0) / (0.1, 0.0)
        for (span, horizontal) in [(6usize, 0.2f64), (3, 0.5), (11, 0.1)] {
            let ws1 = Workspace {
                active_window_id: Some(2),
                ..default_workspace()
            };
            let ws2 = Workspace {
                id: 2,
                idx: 2,
                is_active: false,
                ..default_workspace()
            };
            let mut motion = Motion::new(span, DEFAULT_WORKSPACE_SPAN);
            motion.apply(&Event::WorkspacesChanged {
                workspaces: vec![ws1, ws2],
            });
            motion.apply(&Event::WindowsChanged {
                windows: vec![window(1, 1, 1), window(2, 2, 1), window(3, 3, 1)],
            });
            assert_progress(&motion, "DP-1", horizontal, 0.0);
        }
    }

    // --- pixel offsets ------------------------------------------------------

    #[test]
    fn pixel_offsets_match_the_table() {
        // progress (0,0) → 像素（2560×1440、scale 1.1）| x = +128
        let screen = (2560.0, 1440.0);
        let (x, y) = offset_px(Progress::new(0.0, 0.0), screen, 1.1);
        assert!((x - 128.0).abs() < 1e-9, "x = {x}");
        assert!((y - 72.0).abs() < 1e-9, "y = {y}");

        // progress (1,1) → x = −128、y = −72
        let (x, y) = offset_px(Progress::new(1.0, 1.0), screen, 1.1);
        assert!((x + 128.0).abs() < 1e-9, "x = {x}");
        assert!((y + 72.0).abs() < 1e-9, "y = {y}");

        // progress (0.4,1) → x = 128 − 0.4×256、y = −72
        let (x, y) = offset_px(Progress::new(0.4, 1.0), screen, 1.1);
        assert!((x - (128.0 - 0.4 * 256.0)).abs() < 1e-9, "x = {x}");
        assert!((y + 72.0).abs() < 1e-9, "y = {y}");

        // centred → no offset
        let (x, y) = offset_px(Progress::CENTER, screen, 1.1);
        assert!((x, y) == (0.0, 0.0), "({x}, {y})");
    }

    #[test]
    fn scale_is_clamped_to_the_documented_maximum() {
        // panFor(_, 9) → zoom 夹到 log2(1.35): the same ceiling applies here.
        let (x, _) = offset_px(Progress::new(0.0, 0.5), (2560.0, 1440.0), 9.0);
        assert!(
            (x - 2560.0 * (MAX_SCALE - 1.0) * 0.5).abs() < 1e-9,
            "x = {x}"
        );
    }

    #[test]
    fn column_rank_is_clamped_to_one() {
        // 10 columns with span 6 rank past the end; §4.3 expects 1.0.
        let ws1 = Workspace {
            active_window_id: Some(10),
            ..default_workspace()
        };
        let windows: Vec<Window> = (1..=10).map(|i| window(i, i as usize * 10, 1)).collect();
        let motion = motion_with(vec![ws1], windows);
        assert_progress(&motion, "DP-1", 1.0, 0.5);
    }
}
