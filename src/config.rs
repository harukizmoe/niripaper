//! `~/.config/niripaper/config.toml`.
//!
//! Precedence is CLI flag → config file → built-in default, so the file only
//! ever has to state what differs from §2's defaults.
//!
//! ```toml
//! wallpaper = "~/Pictures/wall.webp"
//! scale = 1.2          # canvas enlargement (§4.1); 1.0 ..= 1.35
//! column_span = 5      # fixed column span (§4.2.1); >= 2
//! workspace_span = 4   # fixed workspace span; >= 2
//! namespace = "niripaper"
//!
//! [animations]
//! follow_niri = true   # reuse niri's own animation settings
//!
//! [animations.parallax]
//! duration_ms = 600
//! curve = "ease-out-cubic"
//!
//! [animations.overview-open-close]
//! zoom = 0.96
//!
//! [outputs."eDP-1"]    # per-output overrides of any of the above
//! scale = 1.2
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::motion::{DEFAULT_COLUMN_SPAN, DEFAULT_SCALE, DEFAULT_WORKSPACE_SPAN, MAX_SCALE};
use crate::render::anim::{Animation, Curve, Spring};
use crate::render::transition::{Direction, Effect, Selection, Settings};

/// The daemon's layer-shell namespace, and the name users match in
/// `~/.config/niri/rules.kdl` (§2).
pub const DEFAULT_NAMESPACE: &str = "niripaper";

/// How much the canvas pulls back while the overview is open. Subtle on
/// purpose: with the wallpaper as a workspace background niri *also* scales it
/// down with the workspace, so a large value would double up.
pub const DEFAULT_OVERVIEW_ZOOM: f64 = 0.96;

/// niri's own default for `overview-open-close`. Using the same spring by
/// default makes the wallpaper feel like it belongs to the overview transition,
/// and because a spring is scale-invariant the same parameters work for any
/// zoom distance. Match your niri config's values for the closest feel.
pub const DEFAULT_OVERVIEW_ANIMATION: Animation = Animation::Spring(Spring {
    damping_ratio: 1.0,
    stiffness: 800.0,
    epsilon: 0.0001,
});

/// A validated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub scale: f64,
    pub column_span: usize,
    pub workspace_span: usize,
    /// Frame-rate cap for video wallpapers (`0` keeps the source's). niri hands
    /// layer surfaces frame callbacks at 60 Hz, and every drawn frame also
    /// invalidates the backdrop, so a 60 fps source sits exactly on the edge.
    pub video_fps: u32,
    pub namespace: String,
    /// Animation parameters, in niri's vocabulary.
    pub animations: Animations,
    /// The transition when the wallpaper changes.
    pub transition: Settings,
    /// Static wallpaper for every output that does not override it.
    pub wallpaper: Option<PathBuf>,
    /// Per-output overrides.
    pub outputs: BTreeMap<String, OutputOverride>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            column_span: DEFAULT_COLUMN_SPAN,
            workspace_span: DEFAULT_WORKSPACE_SPAN,
            video_fps: 0,
            namespace: DEFAULT_NAMESPACE.to_owned(),
            animations: Animations::default(),
            transition: Settings::default(),
            wallpaper: None,
            outputs: BTreeMap::new(),
        }
    }
}

/// Animation parameters, mirroring niri's `animations { }` section.
#[derive(Debug, Clone, PartialEq)]
pub struct Animations {
    /// The parallax follow (§4.2.6): 600 ms OutCubic by default.
    pub parallax: Animation,
    /// The overview transition, with its target zoom.
    pub overview_open_close: OverviewAnimation,
    /// niri's `slowdown`: divides elapsed time, so > 1 slows everything down.
    pub slowdown: f64,
    /// Whether the shared animations were read from niri's config.
    pub follow_niri: bool,
    /// The global `off`, as written. The resolved animations already carry it,
    /// but a client asking `state` has to be able to tell this switch apart from
    /// an animation that was turned off on its own.
    pub off: bool,
    /// Which of them actually came from there, for the startup log — a value
    /// that silently stops matching niri is the failure mode to avoid.
    pub from_niri: Vec<&'static str>,
    /// The niri config file those values were read from, if any.
    pub niri_config: Option<PathBuf>,
}

impl Default for Animations {
    fn default() -> Self {
        Self {
            parallax: Animation::easing(Curve::EaseOutCubic, crate::render::anim::DEFAULT_DURATION),
            overview_open_close: OverviewAnimation {
                zoom: DEFAULT_OVERVIEW_ZOOM,
                animation: DEFAULT_OVERVIEW_ANIMATION,
            },
            slowdown: 1.0,
            follow_niri: true,
            off: false,
            from_niri: Vec::new(),
            niri_config: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverviewAnimation {
    /// Canvas zoom multiplier while the overview is open (`1.0` = no change).
    pub zoom: f64,
    pub animation: Animation,
}

/// What one output may override. Everything else is global.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputOverride {
    pub scale: Option<f64>,
    pub column_span: Option<usize>,
    pub workspace_span: Option<usize>,
    pub wallpaper: Option<PathBuf>,
}

/// The parameters in effect for one output.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputParams {
    pub scale: f64,
    pub column_span: usize,
    pub workspace_span: usize,
    pub wallpaper: Option<PathBuf>,
}

impl Config {
    /// Load from the standard location, or fall back to defaults.
    pub fn load() -> Result<Self, String> {
        let Some(path) = default_path() else {
            return Ok(Self::default());
        };
        // No files at all is not an error: the built-in defaults are a valid
        // configuration.
        if !path.exists() && !path.with_extension("d").is_dir() {
            return Ok(Self::default());
        }
        Self::load_from(&path)
    }

    /// Load the configuration for `path`: the file itself, then every `*.toml`
    /// in the sibling `<stem>.d/` directory, in filename order. Later files
    /// override earlier ones.
    ///
    /// That is what lets a tool own its own file (`config.d/noctalia.toml`)
    /// without touching the user's hand-commented `config.toml`, and removing
    /// the file is a clean undo (`HANDOFF.md` §2).
    pub fn load_from(path: &Path) -> Result<Self, String> {
        let files = Self::config_files(path)?;
        let mut sources = Vec::new();
        for file in &files {
            let text = std::fs::read_to_string(file)
                .map_err(|e| format!("reading {}: {e}", file.display()))?;
            sources.push((file.clone(), text));
        }
        Self::load_sources(&sources, true)
    }

    /// The files that make up the configuration for `path`.
    ///
    /// The main file has to exist — an explicit path with a typo should say so.
    /// The `.d` directory is optional.
    pub fn config_files(path: &Path) -> Result<Vec<PathBuf>, String> {
        // `config.toml` -> `config.d`
        let dir = path.with_extension("d");
        // Nothing at all is a typo worth reporting; a missing main file with a
        // `config.d` next to it is a legitimate setup.
        if !path.exists() && !dir.is_dir() {
            return Err(format!("{} does not exist", path.display()));
        }
        let mut files = Vec::new();
        if path.exists() {
            files.push(path.to_owned());
        }
        if dir.is_dir() {
            let mut extra: Vec<PathBuf> = std::fs::read_dir(&dir)
                .map_err(|e| format!("reading {}: {e}", dir.display()))?
                .flatten()
                .map(|entry| entry.path())
                .filter(|entry| {
                    entry.extension().and_then(|e| e.to_str()) == Some("toml") && entry.is_file()
                })
                .collect();
            extra.sort();
            files.extend(extra);
        }
        Ok(files)
    }

    /// Merge and validate already-read sources.
    ///
    /// Every file is deserialized *on its own* first: with several files in
    /// play, "unknown field" without a filename is a puzzle, and the whole
    /// point of `config.d` is that a tool writes files the user never looks at.
    pub fn load_sources(sources: &[(PathBuf, String)], read_niri: bool) -> Result<Self, String> {
        let mut parsed = Vec::with_capacity(sources.len());
        for (path, text) in sources {
            let value: toml::Value =
                toml::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
            if let Err(err) = toml::from_str::<RawConfig>(text) {
                return Err(format!("{}: {err}", path.display()));
            }
            parsed.push(value);
        }
        let mut merged = toml::Value::Table(toml::map::Map::new());
        for value in parsed {
            merge(&mut merged, value);
        }
        let raw: RawConfig = merged
            .try_into()
            .map_err(|e: toml::de::Error| e.to_string())?;
        Self::from_raw(raw, read_niri)
    }

    /// Parse and validate. Kept separate from the file handling so it is
    /// testable without touching the filesystem.
    /// Reads niri's config when `animations.follow_niri` is on, so tests use
    /// [`Config::parse_without_niri`] to stay hermetic.
    pub fn parse(text: &str) -> Result<Self, String> {
        Self::parse_inner(text, true)
    }

    /// Like [`Config::parse`] but never reads niri's config.
    pub fn parse_without_niri(text: &str) -> Result<Self, String> {
        Self::parse_inner(text, false)
    }

    fn parse_inner(text: &str, read_niri: bool) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        Self::from_raw(raw, read_niri)
    }

    /// Validate and resolve a parsed (and possibly merged) raw config.
    fn from_raw(raw: RawConfig, read_niri: bool) -> Result<Self, String> {
        let mut config = Self::default();
        if let Some(scale) = raw.scale {
            config.scale = check_scale("scale", scale)?;
        }
        if let Some(fps) = raw.video_fps {
            config.video_fps = fps;
        }
        if let Some(span) = raw.column_span {
            config.column_span = check_span("column_span", span)?;
        }
        if let Some(span) = raw.workspace_span {
            config.workspace_span = check_span("workspace_span", span)?;
        }
        // niri exposes nothing over IPC (`niri msg` has no config dump, `niri
        // validate` only reports validity), so its config file is the only
        // source. A config we cannot read is an error rather than something to
        // paper over: the whole point is that the two stay in step.
        // `animations.follow_niri = false` is the way out.
        let follow_niri = raw
            .animations
            .follow_niri
            .unwrap_or(Animations::default().follow_niri);
        let niri = if follow_niri && read_niri {
            crate::niri_config::animations()?
        } else {
            None
        };
        let niri_config = niri.as_ref().and_then(|n| n.path.clone());
        config.animations = resolve_animations(&raw.animations, niri)?;
        config.animations.niri_config = niri_config;
        if let Some(wallpaper) = raw.wallpaper {
            config.wallpaper = Some(check_wallpaper("wallpaper", wallpaper)?);
        }
        if let Some(namespace) = raw.namespace {
            if namespace.is_empty() {
                return Err("namespace must not be empty".to_owned());
            }
            config.namespace = namespace;
        }
        // The global `off` is an animation switch, and the wallpaper transition
        // is an animation: turning everything off has to turn this off too.
        config.transition =
            resolve_transition(&raw.transition, raw.animations.off.unwrap_or(false))?;
        for (name, output) in raw.outputs {
            if let Some(scale) = output.scale {
                check_scale(&format!("outputs.{name}.scale"), scale)?;
            }
            if let Some(span) = output.column_span {
                check_span(&format!("outputs.{name}.column_span"), span)?;
            }
            if let Some(span) = output.workspace_span {
                check_span(&format!("outputs.{name}.workspace_span"), span)?;
            }
            if let Some(wallpaper) = &output.wallpaper {
                check_wallpaper(&format!("outputs.{name}.wallpaper"), wallpaper.clone())?;
            }
            config.outputs.insert(name, output);
        }
        Ok(config)
    }

    /// The motion parameters for one output.
    pub fn output(&self, name: &str) -> OutputParams {
        let over = self.outputs.get(name);
        OutputParams {
            scale: over.and_then(|o| o.scale).unwrap_or(self.scale),
            column_span: over.and_then(|o| o.column_span).unwrap_or(self.column_span),
            workspace_span: over
                .and_then(|o| o.workspace_span)
                .unwrap_or(self.workspace_span),
            wallpaper: over
                .and_then(|o| o.wallpaper.clone())
                .or_else(|| self.wallpaper.clone()),
        }
    }
}

/// How to name the configuration a daemon is running from: the startup log,
/// `state`, and anywhere else that has to say it out loud.
pub fn describe_source(path: Option<&Path>) -> String {
    match path {
        Some(path) => path.display().to_string(),
        None => "built-in defaults (no config file)".to_owned(),
    }
}

/// Where the config lives: `$XDG_CONFIG_HOME/niripaper/config.toml`, else
/// `~/.config/niripaper/config.toml`.
pub fn default_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir).join("niripaper/config.toml"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/niripaper/config.toml"))
}

// --- validation -----------------------------------------------------------
/// Deep-merge `from` into `into`: tables merge key by key, anything else is
/// replaced. That is what makes "later file wins" work for nested tables like
/// `[animations.parallax]` without either file having to repeat the other's keys.
fn merge(into: &mut toml::Value, from: toml::Value) {
    match (into, from) {
        (toml::Value::Table(into), toml::Value::Table(from)) => {
            for (key, value) in from {
                match into.get_mut(&key) {
                    Some(existing) => merge(existing, value),
                    None => {
                        into.insert(key, value);
                    }
                }
            }
        }
        (into, from) => *into = from,
    }
}

fn check_scale(key: &str, scale: f64) -> Result<f64, String> {
    if !scale.is_finite() {
        return Err(format!("{key} must be a finite number, got {scale}"));
    }
    if scale < 1.0 || scale > MAX_SCALE {
        return Err(format!(
            "{key} must be between 1.0 and {MAX_SCALE} (the canvas cannot be smaller than the \
             output, and past {MAX_SCALE} the travel grows without bound), got {scale}"
        ));
    }
    Ok(scale)
}

fn check_span(key: &str, span: usize) -> Result<usize, String> {
    if span < 2 {
        return Err(format!(
            "{key} must be at least 2 (the progress is divided by span - 1), got {span}"
        ));
    }
    Ok(span)
}

/// The file has to exist: a wallpaper that silently does not load is exactly
/// the kind of failure this project keeps getting bitten by.
fn check_wallpaper(key: &str, path: PathBuf) -> Result<PathBuf, String> {
    let path = expand_home(&path);
    if !path.exists() {
        return Err(format!("{key} {} does not exist", path.display()));
    }
    Ok(path)
}

/// Expand a leading `~` the way a shell would, so `~/Pictures/wall.png` works in
/// the config file. niri expands `~` in its own paths, so people write it; a
/// config value is not a shell word, so nobody else will do it for us.
fn expand_home(path: &Path) -> PathBuf {
    expand_home_at(path, std::env::var_os("HOME").map(PathBuf::from).as_deref())
}

fn expand_home_at(path: &Path, home: Option<&Path>) -> PathBuf {
    let Some(rest) = path.to_str().and_then(|text| text.strip_prefix('~')) else {
        return path.to_owned();
    };
    match home {
        Some(home) => home.join(rest.trim_start_matches('/')),
        None => path.to_owned(),
    }
}

/// The zoom must stay positive; the daemon clamps `scale × zoom` to ≥ 1.0 as a
/// second line of defence (a canvas smaller than the output would leave holes).
fn check_overview_zoom(zoom: f64) -> Result<f64, String> {
    if !zoom.is_finite() || zoom <= 0.0 {
        return Err(format!("zoom must be a positive number, got {zoom}"));
    }
    Ok(zoom)
}

fn resolve_animations(
    raw: &RawAnimations,
    niri: Option<crate::niri_config::NiriAnimations>,
) -> Result<Animations, String> {
    let defaults = Animations::default();
    let follow_niri = raw.follow_niri.unwrap_or(defaults.follow_niri);
    let mut from_niri = Vec::new();

    let global_off = raw.off.unwrap_or(false);
    let niri_off = niri.as_ref().and_then(|n| n.off).unwrap_or(false);

    // Precedence: our explicit value → niri's config → our default.
    let slowdown = match raw.slowdown {
        Some(value) => {
            if !value.is_finite() || value <= 0.0 {
                return Err(format!(
                    "animations.slowdown must be a positive number, got {value}"
                ));
            }
            value
        }
        None => match niri.as_ref().and_then(|n| n.slowdown) {
            Some(value) => {
                from_niri.push("slowdown");
                value
            }
            None => defaults.slowdown,
        },
    };

    let parallax = match explicit_animation("animations.parallax", raw.parallax.as_ref())? {
        Some(animation) => animation,
        None if global_off => Animation::Off,
        None => defaults.parallax,
    };

    let overview = raw.overview_open_close.as_ref();
    let zoom = match overview.and_then(|o| o.zoom) {
        Some(zoom) => check_overview_zoom(zoom)?,
        None => defaults.overview_open_close.zoom,
    };
    // niri's `off` only switches off the transition it owns: the parallax is not
    // a niri animation, so niri's global `off` does not touch it.
    let overview_animation = match explicit_animation(
        "animations.overview-open-close",
        overview.map(|o| &o.animation),
    )? {
        Some(animation) => animation,
        None if global_off => Animation::Off,
        None => match niri.as_ref().and_then(|n| n.overview_open_close) {
            Some(animation) => {
                from_niri.push("overview-open-close");
                animation
            }
            None if niri_off => Animation::Off,
            None => defaults.overview_open_close.animation,
        },
    };

    Ok(Animations {
        parallax,
        overview_open_close: OverviewAnimation {
            zoom,
            animation: overview_animation,
        },
        slowdown,
        follow_niri,
        off: global_off,
        from_niri,
        niri_config: None,
    })
}

/// Turn one niri-style animation block into an [`Animation`].
///
/// The three shapes are niri's: `off`, an easing (`duration_ms` + `curve`), or a
/// `spring`. Mixing them is an error rather than a silent precedence rule.
/// `Ok(None)` means the block is absent, so a lower-priority source applies.
fn explicit_animation(key: &str, raw: Option<&RawAnimation>) -> Result<Option<Animation>, String> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    if raw.off == Some(true) {
        if raw.duration_ms.is_some() || raw.curve.is_some() || raw.spring.is_some() {
            return Err(format!(
                "{key}: `off` cannot be combined with other settings"
            ));
        }
        return Ok(Some(Animation::Off));
    }

    if let Some(spring) = &raw.spring {
        if raw.duration_ms.is_some() || raw.curve.is_some() || raw.cubic_bezier.is_some() {
            return Err(format!(
                "{key}: a spring and an easing are alternatives — remove duration_ms/curve"
            ));
        }
        return Ok(Some(Animation::spring(
            check_damping_ratio(key, spring.damping_ratio)?,
            check_stiffness(key, spring.stiffness)?,
            check_epsilon(key, spring.epsilon)?,
        )));
    }

    match (raw.duration_ms, &raw.curve) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(format!(
            "{key}: `curve` is required when `duration_ms` is set"
        )),
        (None, Some(_)) => Err(format!(
            "{key}: `duration_ms` is required when `curve` is set"
        )),
        (Some(duration_ms), Some(curve)) => {
            if duration_ms == 0 {
                return Err(format!(
                    "{key}: `duration_ms` must be greater than 0 (use off = true)"
                ));
            }
            let curve = Curve::parse(curve, raw.cubic_bezier)?;
            Ok(Some(Animation::easing(
                curve,
                std::time::Duration::from_millis(duration_ms),
            )))
        }
    }
}

fn check_damping_ratio(key: &str, ratio: f64) -> Result<f64, String> {
    // niri documents 0.1 ..= 10.0, and warns that > 1.0 is unstable.
    if !(0.1..=10.0).contains(&ratio) {
        return Err(format!(
            "{key}: damping_ratio must be between 0.1 and 10.0, got {ratio}"
        ));
    }
    Ok(ratio)
}

fn check_stiffness(key: &str, stiffness: f64) -> Result<f64, String> {
    if !stiffness.is_finite() || stiffness <= 0.0 {
        return Err(format!(
            "{key}: stiffness must be a positive number, got {stiffness}"
        ));
    }
    Ok(stiffness)
}

fn check_epsilon(key: &str, epsilon: f64) -> Result<f64, String> {
    if !epsilon.is_finite() || epsilon <= 0.0 {
        return Err(format!(
            "{key}: epsilon must be a positive number, got {epsilon}"
        ));
    }
    Ok(epsilon)
}

// --- the file's shape -----------------------------------------------------

/// niri's `animations { }` section.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnimations {
    /// niri's global `off`.
    off: Option<bool>,
    /// niri's `slowdown <factor>`.
    slowdown: Option<f64>,
    /// Read the shared animations from niri's own config (default true).
    follow_niri: Option<bool>,
    parallax: Option<RawAnimation>,
    #[serde(rename = "overview-open-close")]
    overview_open_close: Option<RawOverviewAnimation>,
}

/// One animation block: `off`, an easing, or a spring.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAnimation {
    off: Option<bool>,
    duration_ms: Option<u64>,
    curve: Option<String>,
    /// Control points for `curve = "cubic-bezier"`.
    cubic_bezier: Option<[f64; 4]>,
    spring: Option<RawSpring>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpring {
    damping_ratio: f64,
    stiffness: f64,
    epsilon: f64,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTransition {
    selection: Option<String>,
    effect: Option<String>,
    effects: Option<Vec<String>>,
    duration_ms: Option<u64>,
    curve: Option<String>,
    cubic_bezier: Option<[f64; 4]>,
    allow_overshoot: Option<bool>,
    softness: Option<f64>,
    center: Option<[f64; 2]>,
    direction: Option<String>,
    stripes: Option<u32>,
    cell: Option<f64>,
    push: Option<f64>,
    start_radius: Option<f64>,
    on_start: Option<bool>,
}

/// Validate `[transition]`. Every message names the key, because this is the
/// table a panel writes and a typo there has to be findable.
fn resolve_transition(raw: &RawTransition, global_off: bool) -> Result<Settings, String> {
    let defaults = Settings::default();
    let key = "transition";

    let selection = match &raw.selection {
        Some(name) => Selection::parse(name).map_err(|e| format!("{key}.selection: {e}"))?,
        None => defaults.selection,
    };
    let effect = match &raw.effect {
        Some(name) => Effect::parse(name).map_err(|e| format!("{key}.effect: {e}"))?,
        None => defaults.effect,
    };
    let effects = match &raw.effects {
        Some(names) => names
            .iter()
            .map(|name| Effect::parse(name).map_err(|e| format!("{key}.effects: {e}")))
            .collect::<Result<Vec<_>, _>>()?,
        None => defaults.effects.clone(),
    };
    if selection != Selection::Fixed && effects.is_empty() {
        return Err(format!(
            "{key}.effects is empty, but selection = {:?} has to pick from it",
            selection.name()
        ));
    }

    let duration = match raw.duration_ms {
        Some(0) => {
            return Err(format!(
                "{key}.duration_ms must be greater than 0 (use effect = \"none\" for a hard cut)"
            ))
        }
        Some(ms) => std::time::Duration::from_millis(ms),
        None => defaults.duration,
    };
    let curve = match &raw.curve {
        Some(name) => {
            Curve::parse(name, raw.cubic_bezier).map_err(|e| format!("{key}.curve: {e}"))?
        }
        None => defaults.curve,
    };

    let softness = raw.softness.unwrap_or(defaults.softness);
    if !(0.0..=1.0).contains(&softness) {
        return Err(format!(
            "{key}.softness must be between 0 (a hard edge) and 1, got {softness}"
        ));
    }
    let center = raw.center.unwrap_or([defaults.center.0, defaults.center.1]);
    if center.iter().any(|axis| !(0.0..=1.0).contains(axis)) {
        return Err(format!(
            "{key}.center is a fraction of the screen, so both axes must be between 0 and 1, got {:?}",
            center
        ));
    }
    let direction = match &raw.direction {
        Some(name) => Direction::parse(name).map_err(|e| format!("{key}.direction: {e}"))?,
        None => defaults.direction,
    };
    let stripes = raw.stripes.unwrap_or(defaults.stripes);
    if !(2..=64).contains(&stripes) {
        return Err(format!(
            "{key}.stripes must be between 2 and 64, got {stripes}"
        ));
    }
    let cell = raw.cell.unwrap_or(defaults.cell);
    if !(0.02..=0.5).contains(&cell) {
        return Err(format!(
            "{key}.cell is a fraction of the screen's height; 0.02 to 0.5, got {cell}"
        ));
    }

    let push = raw.push.unwrap_or(defaults.push);
    if !(0.0..=1.5).contains(&push) {
        return Err(format!(
            "{key}.push is a multiple of the distance from the centre; 0 (an iris) to 1.5, got {push}"
        ));
    }

    let start_radius = raw.start_radius.unwrap_or(defaults.start_radius);
    if !(0.0..=1.0).contains(&start_radius) {
        return Err(format!(
            "{key}.start_radius is a fraction of the screen's height; 0 to 1, got {start_radius}"
        ));
    }

    Ok(Settings {
        // A hard cut is the honest reading of "no animations": nothing to pick.
        selection: if global_off {
            Selection::Fixed
        } else {
            selection
        },
        effect: if global_off { Effect::None } else { effect },
        effects,
        duration,
        curve,
        allow_overshoot: raw.allow_overshoot.unwrap_or(defaults.allow_overshoot),
        softness,
        center: (center[0], center[1]),
        direction,
        stripes,
        cell,
        push,
        start_radius,
        on_start: raw.on_start.unwrap_or(defaults.on_start),
    })
}

/// The overview transition: an animation block plus its target zoom.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawOverviewAnimation {
    zoom: Option<f64>,
    #[serde(flatten)]
    animation: RawAnimation,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    scale: Option<f64>,
    column_span: Option<usize>,
    workspace_span: Option<usize>,
    video_fps: Option<u32>,
    namespace: Option<String>,
    #[serde(default)]
    animations: RawAnimations,
    #[serde(default)]
    transition: RawTransition,
    wallpaper: Option<PathBuf>,
    #[serde(default)]
    outputs: BTreeMap<String, OutputOverride>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(
            Config::parse_without_niri("").expect("parses"),
            Config::default()
        );
    }

    #[test]
    fn reads_every_parameter() {
        let config = Config::parse_without_niri(
            r#"
            scale = 1.2
            column_span = 8
            workspace_span = 5
            namespace = "custom"

            [animations.parallax]
            duration_ms = 250
            curve = "ease-out-expo"

            [outputs."DP-1"]
            scale = 1.3

            [outputs."eDP-1"]
            column_span = 4
            wallpaper = "/etc/hostname"
            "#,
        )
        .expect("parses");
        assert_eq!(config.scale, 1.2);
        assert_eq!(config.column_span, 8);
        assert_eq!(config.workspace_span, 5);
        assert_eq!(
            config.animations.parallax,
            Animation::easing(Curve::EaseOutExpo, std::time::Duration::from_millis(250))
        );
        assert_eq!(config.namespace, "custom");
        assert_eq!(config.output("DP-1").scale, 1.3);
        assert_eq!(config.output("DP-1").column_span, 8);
        assert_eq!(config.output("DP-1").workspace_span, 5);
        assert_eq!(config.output("eDP-1").scale, 1.2);
        assert_eq!(config.output("eDP-1").column_span, 4);
        // The per-output section does not override what it does not mention.
        assert_eq!(config.output("eDP-1").workspace_span, 5);
        // The per-output wallpaper wins over the global one.
        assert_eq!(
            config.output("eDP-1").wallpaper.as_deref(),
            Some(Path::new("/etc/hostname"))
        );
        // An output with no section gets the global values.
        assert_eq!(config.output("HDMI-A-1").scale, 1.2);
        assert_eq!(config.output("HDMI-A-1").column_span, 8);
        assert_eq!(config.output("HDMI-A-1").workspace_span, 5);
        assert_eq!(config.output("HDMI-A-1").wallpaper, None);
    }

    #[test]
    fn a_leading_tilde_in_the_wallpaper_expands() {
        // niri expands `~` in its own paths, so people write it here too. A
        // config value is not a shell word, so nobody else will.
        let home = Path::new("/home/someone");
        assert_eq!(
            expand_home_at(Path::new("~/Pictures/wall.png"), Some(home)),
            PathBuf::from("/home/someone/Pictures/wall.png")
        );
        // Anything else is left exactly as written.
        assert_eq!(
            expand_home_at(Path::new("/abs/wall.png"), Some(home)),
            PathBuf::from("/abs/wall.png")
        );
        assert_eq!(
            expand_home_at(Path::new("rel/wall.png"), Some(home)),
            PathBuf::from("rel/wall.png")
        );
        // No home to expand into: leave it alone rather than invent a path.
        assert_eq!(
            expand_home_at(Path::new("~/wall.png"), None),
            PathBuf::from("~/wall.png")
        );
    }

    #[test]
    fn a_later_file_overrides_an_earlier_one() {
        // This is what lets a tool own `config.d/<tool>.toml` without touching
        // the user's hand-commented `config.toml`.
        let main = (
            PathBuf::from("config.toml"),
            "scale = 1.1\n[animations.parallax]\nduration_ms = 600\ncurve = \"linear\"".to_owned(),
        );
        let extra = (
            PathBuf::from("config.d/noctalia.toml"),
            "[animations.parallax]\ncurve = \"ease-out-expo\"".to_owned(),
        );
        let config = Config::load_sources(&[main, extra], false).expect("merges");
        assert_eq!(config.scale, 1.1, "the untouched key survives");
        assert_eq!(
            config.animations.parallax,
            Animation::easing(Curve::EaseOutExpo, std::time::Duration::from_millis(600)),
            "the later file wins for the key it sets"
        );
    }

    #[test]
    fn rejects_a_push_outside_its_range() {
        for push in [1.6, -0.1] {
            let err = Config::parse_without_niri(&format!("[transition]\npush = {push}"))
                .expect_err("out of range");
            assert!(err.contains("transition.push"), "{err}");
        }
        // The ends are allowed: 0 is an iris, and 1.5 is as far as it goes.
        for push in [0.0, 1.5] {
            Config::parse_without_niri(&format!("[transition]\npush = {push}")).expect("in range");
        }
    }

    #[test]
    fn an_unknown_key_names_the_file_that_has_it() {
        // With several files in play, "unknown field" without a filename is a
        // puzzle — and the files a tool writes are ones the user never reads.
        let main = (PathBuf::from("config.toml"), "scale = 1.1".to_owned());
        let extra = (
            PathBuf::from("config.d/bad.toml"),
            "nonsense = 1".to_owned(),
        );
        let err = Config::load_sources(&[main, extra], false).unwrap_err();
        assert!(err.contains("config.d/bad.toml"), "{err}");
        assert!(err.contains("nonsense"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_values() {
        // Each message has to name the key, otherwise a typo is unsearchable.
        let scale = Config::parse_without_niri("scale = 0.5").unwrap_err();
        assert!(scale.contains("scale") && scale.contains("1.0"), "{scale}");
        let high = Config::parse_without_niri("scale = 9").unwrap_err();
        assert!(high.contains("scale"), "{high}");
        let span = Config::parse_without_niri("column_span = 1").unwrap_err();
        assert!(span.contains("column_span"), "{span}");
        let workspace_span = Config::parse_without_niri("workspace_span = 0").unwrap_err();
        assert!(
            workspace_span.contains("workspace_span"),
            "{workspace_span}"
        );
        let duration = Config::parse_without_niri(
            "[animations.parallax]\nduration_ms = 0\ncurve = \"linear\"",
        )
        .unwrap_err();
        assert!(duration.contains("duration_ms"), "{duration}");
        let zoom =
            Config::parse_without_niri("[animations.overview-open-close]\nzoom = 0").unwrap_err();
        assert!(zoom.contains("zoom"), "{zoom}");
        let per_output = Config::parse_without_niri("[outputs.\"DP-1\"]\nscale = 9").unwrap_err();
        assert!(per_output.contains("outputs.DP-1.scale"), "{per_output}");
        let namespace = Config::parse_without_niri("namespace = \"\"").unwrap_err();
        assert!(namespace.contains("namespace"), "{namespace}");
        let wallpaper =
            Config::parse_without_niri("wallpaper = \"/nope/missing.png\"").unwrap_err();
        assert!(wallpaper.contains("wallpaper"), "{wallpaper}");
    }

    #[test]
    fn rejects_unknown_keys() {
        // A typo like `scal` must not silently do nothing.
        let err = Config::parse_without_niri("scal = 1.2").unwrap_err();
        assert!(err.contains("scal"), "{err}");
        let err = Config::parse_without_niri("[outputs.\"DP-1\"]\nspam = 1").unwrap_err();
        assert!(err.contains("spam"), "{err}");
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let config = Config::load_from(Path::new("/nonexistent/niripaper/config.toml"));
        assert!(config.is_err(), "an explicitly requested file must exist");
        // …but `load()` falls back to defaults when the default path is absent.
        let path = default_path().expect("HOME is set in tests");
        assert!(path.ends_with("niripaper/config.toml"));
    }
}

#[cfg(test)]
mod animation_tests {
    use super::*;
    use std::time::Duration;

    fn parallax(config: &Config) -> Animation {
        config.animations.parallax
    }

    #[test]
    fn animations_default_to_the_spec_and_to_niri() {
        let config = Config::default();
        // §4.2.6: OutCubic, 600 ms.
        assert_eq!(
            parallax(&config),
            Animation::easing(Curve::EaseOutCubic, Duration::from_millis(600))
        );
        // niri's own default for `overview-open-close`.
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::spring(1.0, 800.0, 0.0001)
        );
        assert_eq!(config.animations.overview_open_close.zoom, 0.96);
        assert_eq!(config.animations.slowdown, 1.0);
    }

    #[test]
    fn easing_blocks_need_both_duration_and_curve() {
        let err =
            Config::parse_without_niri("[animations.parallax]\nduration_ms = 250").unwrap_err();
        assert!(err.contains("curve"), "{err}");
        let err =
            Config::parse_without_niri("[animations.parallax]\ncurve = \"linear\"").unwrap_err();
        assert!(err.contains("duration_ms"), "{err}");
    }

    #[test]
    fn spring_and_easing_are_alternatives() {
        let err = Config::parse_without_niri(
            "[animations.parallax]\nduration_ms = 250\ncurve = \"linear\"\nspring = { damping_ratio = 1.0, stiffness = 800.0, epsilon = 0.0001 }",
        )
        .unwrap_err();
        assert!(err.contains("alternatives"), "{err}");
    }

    #[test]
    fn springs_are_validated_like_niri_documents() {
        for (spring, key) in [
            (
                "damping_ratio = 0.0, stiffness = 800.0, epsilon = 0.0001",
                "damping_ratio",
            ),
            (
                "damping_ratio = 1.0, stiffness = 0.0, epsilon = 0.0001",
                "stiffness",
            ),
            (
                "damping_ratio = 1.0, stiffness = 800.0, epsilon = 0.0",
                "epsilon",
            ),
        ] {
            let text = format!("[animations.parallax]\nspring = {{ {spring} }}");
            let err = Config::parse_without_niri(&text).unwrap_err();
            assert!(err.contains(key), "{err}");
        }
    }

    #[test]
    fn off_can_be_global_or_per_animation() {
        let config = Config::parse_without_niri("[animations]\noff = true").expect("parses");
        assert_eq!(parallax(&config), Animation::Off);
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::Off
        );

        let config =
            Config::parse_without_niri("[animations.parallax]\noff = true").expect("parses");
        assert_eq!(parallax(&config), Animation::Off);
        // …while the other animation keeps its default.
        assert_ne!(
            config.animations.overview_open_close.animation,
            Animation::Off
        );

        let err =
            Config::parse_without_niri("[animations.parallax]\noff = true\nduration_ms = 100")
                .unwrap_err();
        assert!(err.contains("cannot be combined"), "{err}");
    }

    #[test]
    fn slowdown_must_be_positive() {
        assert_eq!(
            Config::parse_without_niri("[animations]\nslowdown = 3.0")
                .unwrap()
                .animations
                .slowdown,
            3.0
        );
        let err = Config::parse_without_niri("[animations]\nslowdown = 0").unwrap_err();
        assert!(err.contains("slowdown"), "{err}");
    }

    #[test]
    fn the_overview_block_takes_a_zoom_and_an_animation() {
        let config = Config::parse_without_niri(
            "[animations.overview-open-close]\nzoom = 0.9\nspring = { damping_ratio = 0.5, stiffness = 400.0, epsilon = 0.001 }",
        )
        .expect("parses");
        assert_eq!(config.animations.overview_open_close.zoom, 0.9);
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::spring(0.5, 400.0, 0.001)
        );

        // Easing works there too.
        let config = Config::parse_without_niri(
            "[animations.overview-open-close]\nduration_ms = 350\ncurve = \"ease-out-cubic\"",
        )
        .expect("parses");
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::easing(Curve::EaseOutCubic, Duration::from_millis(350))
        );
    }

    #[test]
    fn cubic_bezier_takes_four_control_points() {
        let config = Config::parse_without_niri(
            "[animations.parallax]\nduration_ms = 250\ncurve = \"cubic-bezier\"\ncubic_bezier = [0.05, 0.7, 0.1, 1.0]",
        )
        .expect("parses");
        assert_eq!(
            parallax(&config),
            Animation::easing(
                Curve::CubicBezier([0.05, 0.7, 0.1, 1.0]),
                Duration::from_millis(250)
            )
        );
    }
}

#[cfg(test)]
mod niri_follow_tests {
    use super::*;
    use crate::niri_config::NiriAnimations;
    use std::time::Duration;

    fn niri_spring() -> NiriAnimations {
        NiriAnimations {
            overview_open_close: Some(Animation::spring(0.5, 400.0, 0.001)),
            slowdown: Some(2.0),
            ..NiriAnimations::default()
        }
    }

    #[test]
    fn niri_is_followed_when_we_do_not_override() {
        let out =
            resolve_animations(&RawAnimations::default(), Some(niri_spring())).expect("resolves");
        assert_eq!(
            out.overview_open_close.animation,
            Animation::spring(0.5, 400.0, 0.001)
        );
        assert_eq!(out.slowdown, 2.0);
        // …and it says so, so a silent divergence is impossible.
        assert_eq!(out.from_niri, vec!["slowdown", "overview-open-close"]);
        assert!(out.follow_niri);
    }

    #[test]
    fn our_explicit_values_win_over_niri() {
        let raw: RawAnimations = toml::from_str(
            "slowdown = 3.0\n[overview-open-close]\nduration_ms = 350\ncurve = \"linear\"",
        )
        .expect("parses");
        let out = resolve_animations(&raw, Some(niri_spring())).expect("resolves");
        assert_eq!(out.slowdown, 3.0);
        assert_eq!(
            out.overview_open_close.animation,
            Animation::easing(Curve::Linear, Duration::from_millis(350))
        );
        assert!(out.from_niri.is_empty(), "nothing came from niri");
    }

    #[test]
    fn niris_global_off_only_touches_the_transition_it_owns() {
        let niri = NiriAnimations {
            off: Some(true),
            ..NiriAnimations::default()
        };
        let out = resolve_animations(&RawAnimations::default(), Some(niri)).expect("resolves");
        assert_eq!(out.overview_open_close.animation, Animation::Off);
        // The parallax is not a niri animation.
        assert_ne!(out.parallax, Animation::Off);
    }

    #[test]
    fn without_niri_the_defaults_stand() {
        let out = resolve_animations(&RawAnimations::default(), None).expect("resolves");
        assert_eq!(
            out.overview_open_close.animation,
            Animations::default().overview_open_close.animation
        );
        assert!(out.from_niri.is_empty());
    }

    #[test]
    fn following_niri_can_be_switched_off() {
        let raw: RawAnimations = toml::from_str("follow_niri = false").expect("parses");
        let out = resolve_animations(&raw, None).expect("resolves");
        assert!(!out.follow_niri);
        assert_eq!(
            out.overview_open_close.animation,
            Animation::spring(1.0, 800.0, 0.0001),
            "falls back to niri's own default, which is our default"
        );
    }
}
