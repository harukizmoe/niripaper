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
use crate::render::anim::{Animation, Curve, Spring};

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
    pub span: usize,
    pub namespace: String,
    /// Animation parameters, in niri's vocabulary.
    pub animations: Animations,
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
            namespace: DEFAULT_NAMESPACE.to_owned(),
            animations: Animations::default(),
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
        config.animations = resolve_animations(&raw.animations)?;
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

/// The zoom must stay positive; the daemon clamps `scale × zoom` to ≥ 1.0 as a
/// second line of defence (a canvas smaller than the output would leave holes).
fn check_overview_zoom(zoom: f64) -> Result<f64, String> {
    if !zoom.is_finite() || zoom <= 0.0 {
        return Err(format!("zoom must be a positive number, got {zoom}"));
    }
    Ok(zoom)
}

fn resolve_animations(raw: &RawAnimations) -> Result<Animations, String> {
    let defaults = Animations::default();
    let global_off = raw.off.unwrap_or(false);
    let slowdown = raw.slowdown.unwrap_or(defaults.slowdown);
    if !slowdown.is_finite() || slowdown <= 0.0 {
        return Err(format!(
            "animations.slowdown must be a positive number, got {slowdown}"
        ));
    }

    let parallax = resolve_animation(
        "animations.parallax",
        raw.parallax.as_ref(),
        defaults.parallax,
        global_off,
    )?;
    let overview = raw.overview_open_close.as_ref();
    let zoom = match overview.and_then(|o| o.zoom) {
        Some(zoom) => check_overview_zoom(zoom)?,
        None => defaults.overview_open_close.zoom,
    };
    let overview_animation = resolve_animation(
        "animations.overview-open-close",
        overview.map(|o| &o.animation),
        defaults.overview_open_close.animation,
        global_off,
    )?;

    Ok(Animations {
        parallax,
        overview_open_close: OverviewAnimation {
            zoom,
            animation: overview_animation,
        },
        slowdown,
    })
}

/// Turn one niri-style animation block into an [`Animation`].
///
/// The three shapes are niri's: `off`, an easing (`duration_ms` + `curve`), or a
/// `spring`. Mixing them is an error rather than a silent precedence rule.
fn resolve_animation(
    key: &str,
    raw: Option<&RawAnimation>,
    default: Animation,
    global_off: bool,
) -> Result<Animation, String> {
    let Some(raw) = raw else {
        return Ok(if global_off { Animation::Off } else { default });
    };
    if raw.off == Some(true) {
        if raw.duration_ms.is_some() || raw.curve.is_some() || raw.spring.is_some() {
            return Err(format!(
                "{key}: `off` cannot be combined with other settings"
            ));
        }
        return Ok(Animation::Off);
    }
    if global_off {
        // The global switch wins, but say so rather than silently ignoring the
        // block the user wrote.
        return Ok(Animation::Off);
    }

    if let Some(spring) = &raw.spring {
        if raw.duration_ms.is_some() || raw.curve.is_some() || raw.cubic_bezier.is_some() {
            return Err(format!(
                "{key}: a spring and an easing are alternatives — remove duration_ms/curve"
            ));
        }
        return Ok(Animation::spring(
            check_damping_ratio(key, spring.damping_ratio)?,
            check_stiffness(key, spring.stiffness)?,
            check_epsilon(key, spring.epsilon)?,
        ));
    }

    match (raw.duration_ms, &raw.curve) {
        (None, None) => Ok(default),
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
            Ok(Animation::easing(
                curve,
                std::time::Duration::from_millis(duration_ms),
            ))
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
    span: Option<usize>,
    namespace: Option<String>,
    #[serde(default)]
    animations: RawAnimations,
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
            namespace = "custom"

            [animations.parallax]
            duration_ms = 250
            curve = "ease-out-expo"

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
        assert_eq!(
            config.animations.parallax,
            Animation::easing(Curve::EaseOutExpo, std::time::Duration::from_millis(250))
        );
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
        let duration = Config::parse("[animations.parallax]\nduration_ms = 0\ncurve = \"linear\"")
            .unwrap_err();
        assert!(duration.contains("duration_ms"), "{duration}");
        let zoom = Config::parse("[animations.overview-open-close]\nzoom = 0").unwrap_err();
        assert!(zoom.contains("zoom"), "{zoom}");
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
        let err = Config::parse("[animations.parallax]\nduration_ms = 250").unwrap_err();
        assert!(err.contains("curve"), "{err}");
        let err = Config::parse("[animations.parallax]\ncurve = \"linear\"").unwrap_err();
        assert!(err.contains("duration_ms"), "{err}");
    }

    #[test]
    fn spring_and_easing_are_alternatives() {
        let err = Config::parse(
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
            let err = Config::parse(&text).unwrap_err();
            assert!(err.contains(key), "{err}");
        }
    }

    #[test]
    fn off_can_be_global_or_per_animation() {
        let config = Config::parse("[animations]\noff = true").expect("parses");
        assert_eq!(parallax(&config), Animation::Off);
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::Off
        );

        let config = Config::parse("[animations.parallax]\noff = true").expect("parses");
        assert_eq!(parallax(&config), Animation::Off);
        // …while the other animation keeps its default.
        assert_ne!(
            config.animations.overview_open_close.animation,
            Animation::Off
        );

        let err =
            Config::parse("[animations.parallax]\noff = true\nduration_ms = 100").unwrap_err();
        assert!(err.contains("cannot be combined"), "{err}");
    }

    #[test]
    fn slowdown_must_be_positive() {
        assert_eq!(
            Config::parse("[animations]\nslowdown = 3.0")
                .unwrap()
                .animations
                .slowdown,
            3.0
        );
        let err = Config::parse("[animations]\nslowdown = 0").unwrap_err();
        assert!(err.contains("slowdown"), "{err}");
    }

    #[test]
    fn the_overview_block_takes_a_zoom_and_an_animation() {
        let config = Config::parse(
            "[animations.overview-open-close]\nzoom = 0.9\nspring = { damping_ratio = 0.5, stiffness = 400.0, epsilon = 0.001 }",
        )
        .expect("parses");
        assert_eq!(config.animations.overview_open_close.zoom, 0.9);
        assert_eq!(
            config.animations.overview_open_close.animation,
            Animation::spring(0.5, 400.0, 0.001)
        );

        // Easing works there too.
        let config = Config::parse(
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
        let config = Config::parse(
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
