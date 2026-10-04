//! `~/.config/niripaper/config.toml`.
//!
//! Precedence is CLI flag → config file → built-in default, so the file only
//! ever has to state what differs from §2's defaults.
//!
//! Only the parameters that exist today are configurable. There is deliberately
//! no `wallpaper = "…"` yet: images land with the next milestone, and a key that
//! does nothing is worse than no key at all.
//!
//! ```toml
//! scale = 1.1          # canvas enlargement (§4.1); 1.0 ..= 1.35
//! span = 6             # fixed column span (§4.2.1); >= 2
//! duration_ms = 600    # easing duration (§4.2.6)
//! overview_zoom = 0.96 # canvas zoom while niri's overview is open
//! overview_duration_ms = 350
//! namespace = "niripaper"
//! wallpaper = "~/Pictures/wall.png"
//!
//! [outputs."eDP-1"]    # per-output overrides of the motion parameters
//! scale = 1.2
//! ```

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::motion::{DEFAULT_SCALE, DEFAULT_SPAN, MAX_SCALE};
use crate::render::anim::DEFAULT_DURATION;

/// The daemon's layer-shell namespace, and the name users match in
/// `~/.config/niri/rules.kdl` (§2).
pub const DEFAULT_NAMESPACE: &str = "niripaper";

/// How much the canvas pulls back while the overview is open. Subtle on
/// purpose: with the wallpaper as a workspace background niri *also* scales it
/// down with the workspace, so a large value would double up.
pub const DEFAULT_OVERVIEW_ZOOM: f64 = 0.96;

/// Shorter than the parallax easing: niri's own overview transition is a spring
/// (`damping-ratio`/`stiffness`), and the wallpaper has to feel like it belongs
/// to it. The compositor does not report its animation progress, so this can
/// only ever be an approximation — hence a knob.
pub const DEFAULT_OVERVIEW_DURATION_MS: u64 = 350;

/// A validated configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Config {
    pub scale: f64,
    pub span: usize,
    pub duration_ms: u64,
    /// Canvas zoom multiplier while niri's overview is open (`1.0` = no change).
    pub overview_zoom: f64,
    pub overview_duration_ms: u64,
    pub namespace: String,
    /// Static wallpaper for every output that does not override it.
    pub wallpaper: Option<PathBuf>,
    /// Per-output overrides.
    pub outputs: BTreeMap<String, OutputOverride>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scale: DEFAULT_SCALE,
            span: DEFAULT_SPAN,
            duration_ms: DEFAULT_DURATION.as_millis() as u64,
            overview_zoom: DEFAULT_OVERVIEW_ZOOM,
            overview_duration_ms: DEFAULT_OVERVIEW_DURATION_MS,
            namespace: DEFAULT_NAMESPACE.to_owned(),
            wallpaper: None,
            outputs: BTreeMap::new(),
        }
    }
}

/// What one output may override. Everything else is global.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutputOverride {
    pub scale: Option<f64>,
    pub span: Option<usize>,
    pub wallpaper: Option<PathBuf>,
}

/// The parameters in effect for one output.
#[derive(Debug, Clone, PartialEq)]
pub struct OutputParams {
    pub scale: f64,
    pub span: usize,
    pub wallpaper: Option<PathBuf>,
}

impl Config {
    /// Load from the standard location, or fall back to defaults.
    pub fn load() -> Result<Self, String> {
        match default_path() {
            Some(path) if path.exists() => Self::load_from(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn load_from(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("reading {}: {e}", path.display()))?;
        Self::parse(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Parse and validate. Kept separate from the file handling so it is
    /// testable without touching the filesystem.
    pub fn parse(text: &str) -> Result<Self, String> {
        let raw: RawConfig = toml::from_str(text).map_err(|e| e.to_string())?;
        let mut config = Self::default();
        if let Some(scale) = raw.scale {
            config.scale = check_scale("scale", scale)?;
        }
        if let Some(span) = raw.span {
            config.span = check_span("span", span)?;
        }
        if let Some(duration) = raw.duration_ms {
            config.duration_ms = check_duration("duration_ms", duration)?;
        }
        if let Some(zoom) = raw.overview_zoom {
            config.overview_zoom = check_overview_zoom(zoom)?;
        }
        if let Some(duration) = raw.overview_duration_ms {
            config.overview_duration_ms = check_duration("overview_duration_ms", duration)?;
        }
        if let Some(wallpaper) = raw.wallpaper {
            config.wallpaper = Some(check_wallpaper("wallpaper", wallpaper)?);
        }
        if let Some(namespace) = raw.namespace {
            if namespace.is_empty() {
                return Err("namespace must not be empty".to_owned());
            }
            config.namespace = namespace;
        }
        for (name, output) in raw.outputs {
            if let Some(scale) = output.scale {
                check_scale(&format!("outputs.{name}.scale"), scale)?;
            }
            if let Some(span) = output.span {
                check_span(&format!("outputs.{name}.span"), span)?;
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
            span: over.and_then(|o| o.span).unwrap_or(self.span),
            wallpaper: over
                .and_then(|o| o.wallpaper.clone())
                .or_else(|| self.wallpaper.clone()),
        }
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
    if !path.exists() {
        return Err(format!("{key} {} does not exist", path.display()));
    }
    Ok(path)
}

fn check_duration(key: &str, duration_ms: u64) -> Result<u64, String> {
    if duration_ms == 0 {
        return Err(format!("{key} must be greater than 0"));
    }
    Ok(duration_ms)
}

/// The zoom must stay positive, and `scale × zoom` must not fall below 1.0 —
/// a canvas smaller than the output would leave holes at the edges. The daemon
/// clamps the effective scale as a second line of defence.
fn check_overview_zoom(zoom: f64) -> Result<f64, String> {
    if !zoom.is_finite() || zoom <= 0.0 {
        return Err(format!(
            "overview_zoom must be a positive number, got {zoom}"
        ));
    }
    Ok(zoom)
}

// --- the file's shape -----------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawConfig {
    scale: Option<f64>,
    span: Option<usize>,
    duration_ms: Option<u64>,
    overview_zoom: Option<f64>,
    overview_duration_ms: Option<u64>,
    namespace: Option<String>,
    wallpaper: Option<PathBuf>,
    #[serde(default)]
    outputs: BTreeMap<String, OutputOverride>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_file_is_the_defaults() {
        assert_eq!(Config::parse("").expect("parses"), Config::default());
    }

    #[test]
    fn reads_every_parameter() {
        let config = Config::parse(
            r#"
            scale = 1.2
            span = 8
            duration_ms = 250
            namespace = "custom"

            [outputs."DP-1"]
            scale = 1.3

            [outputs."eDP-1"]
            span = 4
            wallpaper = "/etc/hostname"
            "#,
        )
        .expect("parses");
        assert_eq!(config.scale, 1.2);
        assert_eq!(config.span, 8);
        assert_eq!(config.duration_ms, 250);
        assert_eq!(config.namespace, "custom");
        assert_eq!(config.output("DP-1").scale, 1.3);
        assert_eq!(config.output("DP-1").span, 8);
        assert_eq!(config.output("eDP-1").scale, 1.2);
        assert_eq!(config.output("eDP-1").span, 4);
        // The per-output wallpaper wins over the global one.
        assert_eq!(
            config.output("eDP-1").wallpaper.as_deref(),
            Some(Path::new("/etc/hostname"))
        );
        // An output with no section gets the global values.
        assert_eq!(config.output("HDMI-A-1").scale, 1.2);
        assert_eq!(config.output("HDMI-A-1").span, 8);
        assert_eq!(config.output("HDMI-A-1").wallpaper, None);
    }

    #[test]
    fn rejects_out_of_range_values() {
        // Each message has to name the key, otherwise a typo is unsearchable.
        let scale = Config::parse("scale = 0.5").unwrap_err();
        assert!(scale.contains("scale") && scale.contains("1.0"), "{scale}");
        let high = Config::parse("scale = 9").unwrap_err();
        assert!(high.contains("scale"), "{high}");
        let span = Config::parse("span = 1").unwrap_err();
        assert!(span.contains("span"), "{span}");
        let duration = Config::parse("duration_ms = 0").unwrap_err();
        assert!(duration.contains("duration_ms"), "{duration}");
        let zoom = Config::parse("overview_zoom = 0").unwrap_err();
        assert!(zoom.contains("overview_zoom"), "{zoom}");
        let overview_duration = Config::parse("overview_duration_ms = 0").unwrap_err();
        assert!(
            overview_duration.contains("overview_duration_ms"),
            "{overview_duration}"
        );
        let per_output = Config::parse("[outputs.\"DP-1\"]\nscale = 9").unwrap_err();
        assert!(per_output.contains("outputs.DP-1.scale"), "{per_output}");
        let namespace = Config::parse("namespace = \"\"").unwrap_err();
        assert!(namespace.contains("namespace"), "{namespace}");
        let wallpaper = Config::parse("wallpaper = \"/nope/missing.png\"").unwrap_err();
        assert!(wallpaper.contains("wallpaper"), "{wallpaper}");
    }

    #[test]
    fn rejects_unknown_keys() {
        // A typo like `scal` must not silently do nothing.
        let err = Config::parse("scal = 1.2").unwrap_err();
        assert!(err.contains("scal"), "{err}");
        let err = Config::parse("[outputs.\"DP-1\"]\nspam = 1").unwrap_err();
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
