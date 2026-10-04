//! Read niri's own animation parameters out of its KDL config.
//!
//! niri does not expose them: `niri msg` has no config dump, `niri validate`
//! only reports validity, and the event stream carries nothing but
//! `ConfigLoaded { failed }`. The config file is the only source — so if you
//! retune `overview-open-close` in niri, the wallpaper can follow instead of you
//! having to duplicate the values here.
//!
//! Only what the two programs actually share is read:
//!
//! * `animations.overview-open-close` — the transition this daemon mirrors;
//! * `animations.slowdown` — a global time scale, applied to every animation;
//! * a global `off`, or `off` inside `overview-open-close` — disables *that*
//!   transition only. The parallax is not a niri animation, so niri's `off`
//!   does not switch it off.
//!
//! Anything unreadable is reported, never silently ignored — a wallpaper that
//! quietly stops matching niri would be exactly the kind of failure this
//! project keeps getting bitten by.

use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};

use crate::render::anim::{Animation, Curve};

/// The parameters shared with niri. `None` means "niri's config says nothing
/// about this", which is different from "niri says off".
#[derive(Debug, Clone, PartialEq, Default)]
pub struct NiriAnimations {
    /// The file these came from, for the log.
    pub path: Option<PathBuf>,
    /// A global `animations { off }`.
    pub off: Option<bool>,
    /// `animations { slowdown <factor> }`.
    pub slowdown: Option<f64>,
    /// `animations { overview-open-close { … } }`.
    pub overview_open_close: Option<Animation>,
}

/// Where niri's config lives, in niri's own order of preference.
///
/// 1. `$NIRI_CONFIG` (niri accepts the same variable);
/// 2. the `--config` the *running* niri was started with — niri deletes
///    `NIRI_CONFIG` from its environment at startup, so its command line is the
///    only way to see what it actually loaded;
/// 3. `$XDG_CONFIG_HOME/niri/config.kdl`, else `~/.config/niri/config.kdl`;
/// 4. `/etc/niri/config.kdl`, niri's system fallback.
///
/// Getting this wrong is silent divergence, so the caller logs which file it
/// used.
pub fn config_path() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("NIRI_CONFIG") {
        if !explicit.is_empty() {
            return Some(PathBuf::from(explicit));
        }
    }
    if let Some(running) = running_niri_config() {
        return Some(running);
    }
    let user = user_config_path();
    if user.as_ref().is_some_and(|path| path.exists()) {
        return user;
    }
    let system = PathBuf::from("/etc/niri/config.kdl");
    if system.exists() {
        return Some(system);
    }
    user
}

/// `$XDG_CONFIG_HOME/niri/config.kdl`, else `~/.config/niri/config.kdl`.
pub fn user_config_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        if !dir.is_empty() {
            return Some(PathBuf::from(dir).join("niri/config.kdl"));
        }
    }
    let home = std::env::var_os("HOME")?;
    Some(PathBuf::from(home).join(".config/niri/config.kdl"))
}

/// The `--config`/`-c` argument of the running niri, read from `/proc`.
fn running_niri_config() -> Option<PathBuf> {
    for entry in std::fs::read_dir("/proc").ok()?.flatten() {
        let pid = entry.file_name();
        let pid = pid.to_str()?;
        if !pid.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let comm = std::fs::read_to_string(entry.path().join("comm")).unwrap_or_default();
        if comm.trim() != "niri" {
            continue;
        }
        let cmdline = std::fs::read(entry.path().join("cmdline")).unwrap_or_default();
        let args: Vec<String> = cmdline
            .split(|byte| *byte == 0)
            .filter(|arg| !arg.is_empty())
            .map(|arg| String::from_utf8_lossy(arg).into_owned())
            .collect();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "-c" | "--config" => return iter.next().map(PathBuf::from),
                other => {
                    if let Some(path) = other.strip_prefix("--config=") {
                        return Some(PathBuf::from(path));
                    }
                }
            }
        }
    }
    None
}

/// niri expands a leading `~` in include paths; so do we.
fn expand_tilde(name: &str, home: Option<&Path>) -> Option<PathBuf> {
    if let Some(rest) = name.strip_prefix('~') {
        let home = home?;
        return Some(home.join(rest.trim_start_matches('/')));
    }
    Some(PathBuf::from(name))
}

/// Read niri's shared animation parameters.
///
/// `Ok(None)` means there is no niri config to read; that is not an error.
pub fn animations() -> Result<Option<NiriAnimations>, String> {
    let Some(path) = config_path() else {
        return Ok(None);
    };
    if !path.exists() {
        return Ok(None);
    }
    let mut out = NiriAnimations {
        path: Some(path.clone()),
        ..Default::default()
    };
    let mut visited = Vec::new();
    read_file(&path, &mut out, &mut visited)?;
    Ok(Some(out))
}

/// Read one file and everything it includes, in order (later wins).
fn read_file(
    path: &Path,
    out: &mut NiriAnimations,
    visited: &mut Vec<PathBuf>,
) -> Result<(), String> {
    // `include` cycles would otherwise recurse forever.
    if visited.iter().any(|seen| seen == path) {
        return Ok(());
    }
    visited.push(path.to_owned());

    let text =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let document = KdlDocument::parse(&text).map_err(|e| kdl_error(path, &e))?;
    apply(&document, path, out, visited)
}

fn apply(
    document: &KdlDocument,
    from: &Path,
    out: &mut NiriAnimations,
    visited: &mut Vec<PathBuf>,
) -> Result<(), String> {
    for node in document.nodes() {
        match node.name().value() {
            "include" => include(node, from, out, visited)?,
            "animations" => {
                if let Some(children) = node.children() {
                    for child in children.nodes() {
                        apply_animation(child, out);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

/// niri's config is KDL **v1** (it parses with `knuffel`), so the `kdl` crate is
/// built with `v1-fallback`: v2 first, v1 after. A parse failure has to say
/// *what* went wrong — `KdlError`'s `Display` is just "Failed to parse KDL
/// document", which is useless in a log.
fn kdl_error(path: &Path, error: &kdl::KdlError) -> String {
    let mut message = format!("{}: KDL parse error", path.display());
    for diagnostic in error.diagnostics.iter().take(3) {
        message.push_str(&format!("\n  {diagnostic}"));
    }
    message
}

/// `include "file.kdl"` / `include optional=true "file.kdl"`, relative to the
/// file that contains it.
fn include(
    node: &KdlNode,
    from: &Path,
    out: &mut NiriAnimations,
    visited: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let Some(arg) = node.entries().iter().find(|e| e.name().is_none()) else {
        return Ok(());
    };
    let Some(name) = arg.value().as_string() else {
        return Ok(());
    };
    let optional = node
        .get("optional")
        .and_then(|value| value.as_bool())
        .unwrap_or(false);
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let Some(name) = expand_tilde(name, home.as_deref()) else {
        return Err(format!(
            "{}: cannot expand ~ in include path {name:?}",
            from.display()
        ));
    };
    let path = if name.is_absolute() {
        name
    } else {
        match from.parent() {
            Some(dir) => dir.join(name),
            None => name,
        }
    };
    if !path.exists() {
        return if optional {
            Ok(())
        } else {
            Err(format!(
                "{}: included file {} is missing",
                from.display(),
                path.display()
            ))
        };
    }
    read_file(&path, out, visited)
}

fn apply_animation(node: &KdlNode, out: &mut NiriAnimations) {
    match node.name().value() {
        "off" => out.off = Some(true),
        "slowdown" => {
            if let Some(value) = first_argument(node).and_then(as_number) {
                out.slowdown = Some(value);
            }
        }
        "overview-open-close" => {
            out.overview_open_close = animation_of(node);
        }
        _ => {}
    }
}

/// Parse one animation block: `off`, a `spring`, or `duration-ms` + `curve`.
///
/// Returns `None` only when the block says nothing we understand, in which case
/// the caller keeps its own default.
pub fn animation_of(node: &KdlNode) -> Option<Animation> {
    let children = node.children()?;
    let mut duration_ms = None;
    let mut curve = None;
    let mut control_points = None;
    for child in children.nodes() {
        match child.name().value() {
            "off" => return Some(Animation::Off),
            "spring" => {
                let property = |name: &str| child.get(name).and_then(as_number);
                let (damping_ratio, stiffness, epsilon) = (
                    property("damping-ratio")?,
                    property("stiffness")?,
                    property("epsilon")?,
                );
                return Some(Animation::spring(damping_ratio, stiffness, epsilon));
            }
            "duration-ms" => duration_ms = first_argument(child).and_then(|v| v.as_integer()),
            // `curve "name"` — and for cubic-bezier the four control points are
            // further arguments of the *same* node, niri's syntax:
            // `curve "cubic-bezier" 0.05 0.7 0.1 1`.
            "curve" => {
                let mut arguments = child
                    .entries()
                    .iter()
                    .filter(|entry| entry.name().is_none());
                curve = arguments
                    .next()
                    .and_then(|e| e.value().as_string())
                    .map(str::to_owned);
                let points: Vec<f64> = arguments.filter_map(|e| as_number(e.value())).collect();
                if !points.is_empty() {
                    control_points = <[f64; 4]>::try_from(points).ok();
                }
            }
            _ => {}
        }
    }
    let (duration_ms, curve) = (duration_ms?, curve?);
    if duration_ms < 0 {
        return None;
    }
    let curve = Curve::parse(&curve, control_points).ok()?;
    Some(Animation::easing(
        curve,
        std::time::Duration::from_millis(duration_ms as u64),
    ))
}

/// KDL distinguishes `400` (integer) from `0.5` (float); niri's config uses
/// both forms interchangeably, so numbers have to be read as either.
fn as_number(value: &kdl::KdlValue) -> Option<f64> {
    value
        .as_float()
        .or_else(|| value.as_integer().map(|integer| integer as f64))
}

fn first_argument(node: &KdlNode) -> Option<&kdl::KdlValue> {
    node.entries()
        .iter()
        .find(|entry| entry.name().is_none())
        .map(|entry| entry.value())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn parse(text: &str) -> NiriAnimations {
        let document = KdlDocument::parse(text).expect("valid KDL");
        let mut out = NiriAnimations::default();
        apply(
            &document,
            Path::new("/tmp/config.kdl"),
            &mut out,
            &mut Vec::new(),
        )
        .expect("applies");
        out
    }

    #[test]
    fn reads_the_animation_this_daemon_mirrors() {
        // Exactly the block the user has in __custom__.kdl.
        let out = parse(
            r#"
            animations {
                overview-open-close {
                    spring damping-ratio=0.5 stiffness=400 epsilon=0.001
                }
            }
            "#,
        );
        assert_eq!(
            out.overview_open_close,
            Some(Animation::spring(0.5, 400.0, 0.001))
        );
    }

    #[test]
    fn reads_easing_blocks_too() {
        let out = parse(
            r#"
            animations {
                overview-open-close {
                    duration-ms 200
                    curve "ease-out-quad"
                }
            }
            "#,
        );
        assert_eq!(
            out.overview_open_close,
            Some(Animation::easing(
                Curve::EaseOutQuad,
                Duration::from_millis(200)
            ))
        );

        let out = parse(
            r#"
            animations {
                overview-open-close {
                    duration-ms 150
                    curve "cubic-bezier" 0.05 0.7 0.1 1
                }
            }
            "#,
        );
        assert_eq!(
            out.overview_open_close,
            Some(Animation::easing(
                Curve::CubicBezier([0.05, 0.7, 0.1, 1.0]),
                Duration::from_millis(150)
            ))
        );
    }

    #[test]
    fn reads_the_global_switches() {
        let out = parse("animations { slowdown 3.0 }");
        assert_eq!(out.slowdown, Some(3.0));
        let out = parse("animations { off }");
        assert_eq!(out.off, Some(true));
        // An `off` inside the transition only disables that transition.
        let out = parse("animations { overview-open-close { off } }");
        assert_eq!(out.overview_open_close, Some(Animation::Off));
        assert_eq!(out.off, None);
    }

    #[test]
    fn ignores_everything_it_does_not_share() {
        // niri has many other animations; none of them may leak into ours.
        let out = parse(
            r#"
            input { keyboard { xkb { layout "us" } } }
            animations {
                workspace-switch { spring damping-ratio=1.0 stiffness=1000 epsilon=0.0001 }
                window-open { duration-ms 150; curve "ease-out-expo" }
                overview-open-close { spring damping-ratio=1.0 stiffness=800 epsilon=0.0001 }
            }
            layout { gaps 12 }
            "#,
        );
        assert_eq!(
            out.overview_open_close,
            Some(Animation::spring(1.0, 800.0, 0.0001))
        );
        assert_eq!(out.off, None);
        assert_eq!(out.slowdown, None);
    }

    #[test]
    fn a_missing_block_leaves_the_value_unset() {
        let out = parse("animations { window-open { duration-ms 100; curve \"linear\" } }");
        assert_eq!(out, NiriAnimations::default());
    }

    #[test]
    fn incomplete_blocks_are_left_unset_rather_than_guessed() {
        // A spring missing epsilon, or an easing missing its curve: niri would
        // reject these, and so do we — by not inventing a value.
        let out =
            parse("animations { overview-open-close { spring damping-ratio=1.0 stiffness=800 } }");
        assert_eq!(out.overview_open_close, None);
        let out = parse("animations { overview-open-close { duration-ms 200 } }");
        assert_eq!(out.overview_open_close, None);
    }

    #[test]
    fn config_path_follows_niris_order() {
        let path = config_path().expect("a config path is always found in tests");
        assert!(path.ends_with("niri/config.kdl"), "{path:?}");
    }

    #[test]
    fn tilde_in_include_paths_expands_like_niri() {
        let home = Path::new("/home/someone");
        assert_eq!(
            expand_tilde("~/wall/anim.kdl", Some(home)),
            Some(PathBuf::from("/home/someone/wall/anim.kdl"))
        );
        assert_eq!(
            expand_tilde("~/anim.kdl", Some(home)),
            Some(PathBuf::from("/home/someone/anim.kdl"))
        );
        // A plain relative name stays relative to the including file.
        assert_eq!(
            expand_tilde("anim.kdl", Some(home)),
            Some(PathBuf::from("anim.kdl"))
        );
        assert_eq!(
            expand_tilde("/etc/x.kdl", Some(home)),
            Some(PathBuf::from("/etc/x.kdl"))
        );
        // No home to expand into is an error, not a silent miss.
        assert_eq!(expand_tilde("~/x.kdl", None), None);
    }
}
