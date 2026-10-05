//! The shape of the configuration, for clients that must not hardcode it
//! (`HANDOFF.md` §2).
//!
//! `niripaper schema` over the control socket returns this: one JSON line
//! listing every key a UI can offer — type, bounds, default, unit, and whether
//! editing the file while the daemon runs takes effect. A panel asks once,
//! builds its widgets from it, and afterwards only needs `state`.
//!
//! Two things it is deliberately *not*:
//!
//! - **Not the parser's contract.** Bounds here are what a UI should *offer*,
//!   which is sometimes narrower than what a file may contain: `zoom = 0.5`
//!   parses, but no panel should offer it. What the file accepts is the
//!   validators' business, and their messages name the key that is wrong.
//! - **Not versioned.** The point is that adding a key never needs a panel
//!   change, so there is nothing to bump. A `type` a panel does not recognise is
//!   the one thing it should skip rather than guess at.
//!
//! Per-output overrides (`[outputs."DP-1"]`) are not listed: a panel runs for one
//! output, and the values it needs are already resolved in `state`.

use serde::Serialize;
use serde_json::{json, Value};

use crate::config::{Animations, Config};
use crate::render::anim::{Animation, Curve};

/// What a key holds. A panel switches on this to pick a widget.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Kind {
    Bool,
    /// A whole number in `min..=max`; `max: null` means unbounded.
    Int {
        min: i64,
        max: Option<i64>,
    },
    Float {
        min: f64,
        max: Option<f64>,
    },
    /// A path on disk.
    Path,
    /// A free-form string.
    Text,
    /// One of `options`.
    Enum {
        options: Vec<String>,
    },
    /// An animation: `off = true`, or `duration_ms` + `curve`, or
    /// `spring { damping_ratio, stiffness, epsilon }`. The three forms are
    /// mutually exclusive, and `duration_ms` and `curve` go together.
    Animation {
        curves: Vec<String>,
    },
}

/// One configuration key.
#[derive(Debug, Clone, Serialize)]
pub struct Key {
    /// Dotted path, spelled exactly as in the config file.
    pub name: &'static str,
    pub kind: Kind,
    /// The built-in default, as the config file would spell it (`null` = unset).
    /// With `animations.follow_niri` on, niri's own value may win over this —
    /// `state` carries what is actually in effect.
    pub default: Value,
    /// Unit for display, when the number is not plain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<&'static str>,
    /// Whether editing the file while the daemon runs takes effect.
    pub hot: bool,
    /// One line, for a tooltip.
    pub about: &'static str,
}

/// Every key a UI can offer, in the order a panel should show them.
pub fn keys() -> Vec<Key> {
    // Defaults come from the same values the daemon starts from, so a changed
    // default cannot leave the schema behind.
    let config = Config::default();
    let animations = Animations::default();
    let curves = curve_names();

    let animation =
        |name: &'static str, curves: Vec<String>, default: Value, about: &'static str| Key {
            name,
            kind: Kind::Animation { curves },
            default,
            unit: None,
            hot: true,
            about,
        };

    vec![
        Key {
            name: "wallpaper",
            kind: Kind::Path,
            default: Value::Null,
            unit: None,
            hot: true,
            about: "Image or video to draw; a video is routed by extension. Unset draws the test pattern.",
        },
        Key {
            name: "video_fps",
            kind: Kind::Int {
                min: 0,
                max: Some(240),
            },
            default: json!(config.video_fps),
            unit: Some("fps"),
            hot: true,
            about: "Frame-rate cap for video wallpapers (0 = the source's). Applies at the next video load.",
        },
        Key {
            name: "scale",
            kind: Kind::Float {
                min: 1.0,
                max: Some(crate::motion::MAX_SCALE),
            },
            default: json!(config.scale),
            unit: None,
            hot: true,
            about: "Canvas = output × scale; the extra is the parallax travel.",
        },
        Key {
            name: "column_span",
            kind: Kind::Int { min: 2, max: None },
            default: json!(config.column_span),
            unit: None,
            hot: true,
            about: "Horizontal parallax spread, in columns.",
        },
        Key {
            name: "workspace_span",
            kind: Kind::Int { min: 2, max: None },
            default: json!(config.workspace_span),
            unit: None,
            hot: true,
            about: "Vertical parallax spread, in workspaces.",
        },
        Key {
            name: "namespace",
            kind: Kind::Text,
            default: json!(config.namespace),
            unit: None,
            hot: false,
            about: "layer-shell namespace, matched by niri's layer rules. Needs a restart.",
        },
        Key {
            name: "animations.follow_niri",
            kind: Kind::Bool,
            default: json!(animations.follow_niri),
            unit: None,
            hot: true,
            about: "Take the animations niri also has (overview-open-close, slowdown) from niri's own config.",
        },
        Key {
            name: "animations.off",
            kind: Kind::Bool,
            default: json!(animations.off),
            unit: None,
            hot: true,
            about: "Turn every animation off.",
        },
        Key {
            name: "animations.slowdown",
            kind: Kind::Float {
                min: 0.01,
                max: None,
            },
            default: json!(animations.slowdown),
            unit: None,
            hot: true,
            about: "Stretches every animation's timeline; greater than 1 is slower.",
        },
        animation(
            "animations.parallax",
            curves.clone(),
            animation_value(&animations.parallax),
            "How the parallax follows focus and workspace changes.",
        ),
        Key {
            name: "animations.overview-open-close.zoom",
            kind: Kind::Float {
                min: 0.9,
                max: Some(1.0),
            },
            default: json!(animations.overview_open_close.zoom),
            unit: None,
            hot: true,
            about: "How far the canvas recedes while the overview is open (1.0 turns it off).",
        },
        animation(
            "animations.overview-open-close",
            curves.clone(),
            animation_value(&animations.overview_open_close.animation),
            "The overview transition. Follows niri's own value unless follow_niri is off.",
        ),
        animation(
            "animations.wallpaper-change",
            curves,
            animation_value(&animations.wallpaper_change),
            "The cross-fade when the wallpaper changes.",
        ),
    ]
}

/// The whole schema, as one JSON value.
pub fn schema() -> Value {
    json!({ "keys": keys() })
}

/// niri's curve names, from the one place that parses them.
pub fn curve_names() -> Vec<String> {
    Curve::NAMES.iter().map(|name| (*name).to_owned()).collect()
}

/// An animation spelled the way the config file would: for the schema's defaults
/// and for `state`'s effective values, so a panel compares like with like.
pub fn animation_value(animation: &Animation) -> Value {
    match animation {
        Animation::Off => json!({ "off": true }),
        Animation::Easing { duration, curve } => json!({
            "duration_ms": duration.as_millis() as u64,
            "curve": curve.name(),
        }),
        Animation::Spring(spring) => json!({
            "spring": {
                "damping_ratio": spring.damping_ratio,
                "stiffness": spring.stiffness,
                "epsilon": spring.epsilon,
            }
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The schema is hand-written, so the way it drifts is a renamed or
    /// misspelled key — or a default the parser would reject. Writing each one
    /// out the way the config file would spell it, and parsing that, catches
    /// both: a wrong name fails with "unknown field".
    #[test]
    fn every_key_and_default_parses() {
        for key in keys() {
            if key.default.is_null() {
                continue; // `wallpaper` unset: nothing to write.
            }
            let text = match &key.kind {
                // An animation is a table in the file, not a dotted key.
                Kind::Animation { .. } => format!("[{}]\n{}\n", key.name, inline(&key.default)),
                _ => format!("{} = {}\n", key.name, toml_value(&key.default)),
            };
            Config::parse_without_niri(&text)
                .unwrap_or_else(|e| panic!("schema says {:?}: {e}", text.trim()));
        }
    }

    #[test]
    fn names_are_unique_and_have_a_tooltip() {
        let mut seen: Vec<&str> = Vec::new();
        for key in keys() {
            assert!(!key.name.is_empty(), "{key:?}");
            assert!(key.about.ends_with('.'), "{:?} has no tooltip", key.name);
            assert!(!seen.contains(&key.name), "{} listed twice", key.name);
            seen.push(key.name);
        }
    }

    /// Every key must be settable from a file, and the animation slots must be
    /// spelled the same way in `schema` and `state` — that spelling is what a
    /// panel zips the two together by.
    #[test]
    fn animation_defaults_spell_out_as_tables() {
        let animations: Vec<Key> = keys()
            .into_iter()
            .filter(|key| matches!(key.kind, Kind::Animation { .. }))
            .collect();
        assert_eq!(animations.len(), 3, "parallax, overview, wallpaper change");
        for key in animations {
            let text = format!("[{}]\n{}\n", key.name, inline(&key.default));
            Config::parse_without_niri(&text).unwrap_or_else(|e| panic!("{text:?}: {e}"));
        }
    }

    /// The config file's spelling of a JSON value: enough TOML to write a
    /// default back out, which is all these tests need.
    fn inline(value: &Value) -> String {
        match value {
            Value::Object(map) => map
                .iter()
                .map(|(key, value)| format!("{key} = {}", toml_value(value)))
                .collect::<Vec<_>>()
                .join("\n"),
            other => toml_value(other),
        }
    }

    fn toml_value(value: &Value) -> String {
        match value {
            Value::Bool(flag) => flag.to_string(),
            Value::Number(number) => number.to_string(),
            Value::String(text) => format!("{text:?}"),
            Value::Array(items) => format!(
                "[{}]",
                items.iter().map(toml_value).collect::<Vec<_>>().join(", ")
            ),
            Value::Object(map) => format!(
                "{{ {} }}",
                map.iter()
                    .map(|(key, value)| format!("{key} = {}", toml_value(value)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Value::Null => String::new(),
        }
    }
}
