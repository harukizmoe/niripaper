//! What `state` reports, as JSON — the daemon's answer to the app's `state`
//! request.
//!
//! The daemon gathers the per-output snapshot (it is the only one that knows
//! the canvas size, the media, the animation position) and hands it to
//! [`state_json`], a pure function. Values are keyed by the same dotted names
//! `schema` lists, so a client can zip the two together without knowing
//! anything about either. `config.values` is what the configuration says;
//! `wallpaper.path` is what is *actually* on screen — `set` swaps the media
//! without touching the configuration, so the two can legitimately differ.

use serde_json::{json, Map, Value};

use crate::animation::Curve;
use crate::config::describe_source;

/// The per-output facts `state` reports. The daemon fills this from its
/// per-output worker; the app never builds one.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub output: String,
    pub namespace: String,
    /// What the configuration says (dotted key → JSON value).
    pub config_values: Map<String, Value>,
    /// Where the configuration came from (file path), if any.
    pub config_path: Option<std::path::PathBuf>,
    /// Whether the shared animations were read from niri's config.
    pub from_niri: bool,
    /// The files that make up the configuration, for "where does this value
    /// come from".
    pub config_files: Vec<std::path::PathBuf>,
    pub canvas: (u32, u32),
    /// What is actually on screen, if anything.
    pub wallpaper: Option<WallpaperState>,
    pub position: (f64, f64),
    pub zoom: f64,
}

/// One wallpaper the daemon currently draws.
#[derive(Debug, Clone)]
pub struct WallpaperState {
    pub path: std::path::PathBuf,
    /// `image` or `video`.
    pub kind: &'static str,
    /// Hardware decoder in use, for videos.
    pub hwdec: Option<String>,
    /// The *effective* fit: a video cannot be tiled, so `tile` shows up here as
    /// `fill` even when the configuration says otherwise.
    pub fit: &'static str,
}

/// The daemon's state, for `state` over the control socket.
pub fn state_json(state: &Snapshot) -> Value {
    let files: Vec<String> = state
        .config_files
        .iter()
        .map(|path| path.display().to_string())
        .collect();

    json!({
        "output": state.output,
        "namespace": state.namespace,
        "canvas": { "width": state.canvas.0, "height": state.canvas.1 },
        "config": {
            "source": describe_source(state.config_path.as_deref()),
            "files": files,
            "values": state.config_values,
            "from_niri": state.from_niri,
        },
        "wallpaper": match &state.wallpaper {
            Some(wallpaper) => json!({
                "path": wallpaper.path.display().to_string(),
                "kind": wallpaper.kind,
                "hwdec": wallpaper.hwdec,
                "fit": wallpaper.fit,
            }),
            None => Value::Null,
        },
        "position": {
            "horizontal": state.position.0,
            "vertical": state.position.1,
        },
        "zoom": state.zoom,
    })
}

/// Build the `config.values` map the way the daemon reports it, keyed by the
/// dotted names `schema` lists. Lives here (not in the daemon) so the app can
/// render the same shape offline from a `Config`.
///
/// `fit` is the resolved per-output fit; the rest are the global keys.
pub fn config_values(
    config: &crate::config::Config,
    wallpaper: Option<&std::path::Path>,
    video_fps: u32,
    fit: crate::domain::Fit,
    namespace: &str,
) -> Map<String, Value> {
    let mut values = Map::new();
    let mut put = |name: &str, value: Value| values.insert(name.to_owned(), value);
    put(
        "wallpaper",
        json!(wallpaper.map(|path| path.display().to_string())),
    );
    put("video_fps", json!(video_fps));
    put("scale", json!(config.scale));
    put("parallax", json!(config.parallax));
    put("column_span", json!(config.column_span));
    put("workspace_span", json!(config.workspace_span));
    put("namespace", json!(namespace));
    put("fit", json!(fit.name()));
    let filter = &config.filter;
    put("filter.brightness", json!(filter.brightness));
    put("filter.contrast", json!(filter.contrast));
    put("filter.saturation", json!(filter.saturation));
    put("filter.vibrance", json!(filter.vibrance));
    put("filter.temperature", json!(filter.temperature));
    put("filter.hue", json!(filter.hue));
    put("filter.vignette", json!(filter.vignette));
    put("filter.grain", json!(filter.grain));
    put("filter.aberration", json!(filter.aberration));
    put("filter.grain_size", json!(filter.grain_size));
    put("filter.tone_shadow", json!(filter.tone_shadow));
    put("filter.tone_highlight", json!(filter.tone_highlight));
    put("filter.tone_amount", json!(filter.tone_amount));
    let playlist = &config.playlist;
    put("playlist.enabled", json!(playlist.enabled));
    put("playlist.interval_mins", json!(playlist.interval_mins));
    put("playlist.order", json!(playlist.order.name()));
    put(
        "playlist.items",
        json!(playlist
            .items
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()),
    );
    let animations = &config.animations;
    put("animations.follow_niri", json!(animations.follow_niri));
    put("animations.off", json!(animations.off));
    put("animations.slowdown", json!(animations.slowdown));
    put(
        "animations.parallax",
        crate::schema::animation_value(&animations.parallax),
    );
    put(
        "animations.overview-open-close.zoom",
        json!(animations.overview_open_close.zoom),
    );
    put(
        "animations.overview-open-close",
        crate::schema::animation_value(&animations.overview_open_close.animation),
    );
    let transition = &config.transition;
    put("transition.selection", json!(transition.selection.name()));
    put("transition.effect", json!(transition.effect.name()));
    put(
        "transition.effects",
        json!(transition
            .effects
            .iter()
            .map(|effect| effect.name())
            .collect::<Vec<_>>()),
    );
    put(
        "transition.duration_ms",
        json!(transition.duration.as_millis() as u64),
    );
    put("transition.curve", json!(transition.curve.name()));
    put(
        "transition.cubic_bezier",
        match transition.curve {
            Curve::CubicBezier(points) => json!(points),
            _ => Value::Null,
        },
    );
    put(
        "transition.allow_overshoot",
        json!(transition.allow_overshoot),
    );
    put("transition.softness", json!(transition.softness));
    put(
        "transition.center",
        json!([transition.center.0, transition.center.1]),
    );
    put("transition.direction", json!(transition.direction.name()));
    put("transition.stripes", json!(transition.stripes));
    put("transition.push", json!(transition.push));
    put("transition.start_radius", json!(transition.start_radius));
    put(
        "transition.hold_ms",
        json!(transition.hold.as_millis() as u64),
    );
    put("transition.on_start", json!(transition.on_start));
    values
}

/// A client cannot be allowed to stall anything, so this is only a helper for
/// formatting one value as a protocol line.
pub fn json_line(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("error serializing: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A panel zips `schema` and `state` together by dotted key name. That only
    /// works if the two agree, and they are written in two different files — so
    /// a key added to one and forgotten in the other is a silent hole in the
    /// panel's UI. This is the test that catches it.
    #[test]
    fn config_values_reports_every_key_the_schema_lists() {
        let config = crate::config::Config::default();
        let values = config_values(&config, None, 0, crate::domain::Fit::Fill, "niripaper");
        for key in crate::schema::keys() {
            assert!(
                values.contains_key(key.name),
                "schema lists {:?} but state does not report it",
                key.name
            );
        }
    }

    #[test]
    fn state_json_shape_matches_the_documented_contract() {
        let config = crate::config::Config::default();
        let values = config_values(&config, None, 0, crate::domain::Fit::Fill, "niripaper");
        let state = state_json(&Snapshot {
            output: "DP-1".to_owned(),
            namespace: "niripaper".to_owned(),
            config_values: values,
            config_path: None,
            from_niri: false,
            config_files: Vec::new(),
            canvas: (1920, 1080),
            wallpaper: None,
            position: (0.0, 0.0),
            zoom: 1.0,
        });
        assert_eq!(state["output"], "DP-1");
        assert_eq!(state["canvas"]["width"], 1920);
        assert!(state["wallpaper"].is_null());
        assert!(state["config"]["values"]["scale"].is_number());
        assert_eq!(
            state["config"]["source"],
            "built-in defaults (no config file)"
        );
    }
}
