//! niripaper — the daemon, and the commands that talk to it.
//!
//! ```text
//! niripaper watch [--output NAME]   follow niri's event stream and print the
//!                                   parallax target as it changes
//! ```
//!
//! `watch` is the observable half of M1: it proves the event stream is parsed
//! and that `motion.rs` tracks real window movement, without a GPU.

use std::process::ExitCode;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use niripaper::config::Config;
use niripaper::daemon::{self, Options};
use niripaper::motion::{
    offset_px, Progress, DEFAULT_COLUMN_SPAN, DEFAULT_SCALE, DEFAULT_WORKSPACE_SPAN,
};
use niripaper::niri::Niri;
use niripaper::render::gl::Pattern;

static EXIT: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_signal: libc::c_int) {
    EXIT.store(true, Ordering::SeqCst);
}

/// Install SIGINT/SIGTERM handlers that make blocking syscalls return `EINTR`.
///
/// `libc::signal` sets `SA_RESTART`, which makes `poll()` (and `read()`) resume
/// after the handler runs — so an idle daemon would ignore Ctrl-C until some
/// unrelated event happened to wake it. Clearing `sa_flags` is the whole point.
fn install_signal_handlers() {
    unsafe {
        let mut action: libc::sigaction = std::mem::zeroed();
        action.sa_sigaction = on_signal as extern "C" fn(libc::c_int) as libc::sighandler_t;
        action.sa_flags = 0;
        libc::sigemptyset(&mut action.sa_mask);
        libc::sigaction(libc::SIGINT, &action, std::ptr::null_mut());
        libc::sigaction(libc::SIGTERM, &action, std::ptr::null_mut());
    }
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let rest: Vec<String> = args.collect();
    let result = match command.as_str() {
        "daemon" => daemon_command(&rest),
        "watch" => watch(&rest),
        "set" => control("set", &rest),
        "query" => control("query", &rest),
        "schema" => control("schema", &rest),
        "state" => control("state", &rest),
        "kill" => control("kill", &rest),
        // The build says which build it is: an AGENTS.md entry exists precisely
        // because a stale `~/.cargo/bin/niripaper` once looked exactly like a
        // fresh one.
        "-V" | "--version" => {
            println!("niripaper {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        "-h" | "--help" | "help" => {
            usage();
            Ok(())
        }
        other => Err(format!("unknown command {other}")),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("niripaper: {err}");
            ExitCode::FAILURE
        }
    }
}

/// Talk to the running daemon over its control socket (`HANDOFF.md` §3).
fn control(command: &str, args: &[String]) -> Result<(), String> {
    let mut path = None;
    let mut output = None;
    let mut argument = None;
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--socket" => {
                path = Some(std::path::PathBuf::from(
                    iter.next().ok_or("--socket needs a value")?,
                ))
            }
            "--output" => output = Some(iter.next().ok_or("--output needs a value")?.clone()),
            other if other.starts_with("--") => return Err(format!("unknown argument {other}")),
            other => argument = Some(other.to_owned()),
        }
    }
    let path = match path {
        Some(path) => path,
        None => niripaper::ipc::find_path()?,
    };
    // The output belongs to the *request* now: one daemon draws all of them, and
    // `*` (or saying nothing) means every output.
    let output = output.unwrap_or_else(|| "*".to_owned());
    let request = match (command, argument) {
        ("set", Some(target)) => format!("set {output} {target}"),
        ("set", None) => return Err("set needs a path".to_owned()),
        ("schema" | "kill", None) => command.to_owned(),
        (other, None) => format!("{other} {output}"),
        (other, Some(_)) => return Err(format!("{other} takes no argument")),
    };
    let reply = niripaper::ipc::request(&path, &request)?;
    // `schema` and `state` answer in JSON. It is meant for a panel, but it is
    // readable enough to be worth indenting when a person asked for it.
    match (command, serde_json::from_str::<serde_json::Value>(&reply)) {
        ("schema" | "state", Ok(value)) => match serde_json::to_string_pretty(&value) {
            Ok(text) => println!("{text}"),
            Err(_) => println!("{reply}"),
        },
        _ => println!("{reply}"),
    }
    Ok(())
}

fn usage() {
    eprintln!(
        "usage: niripaper <command>\n\
         \n\
         commands:\n\
         \x20 daemon [--output NAME] [--config PATH] [--namespace NAME]\n\
         \x20        [--scale F] [--column-span N] [--workspace-span N]\n\
         \x20        [--wallpaper PATH] [--pattern blocks|bands] [--socket PATH]\n\
         \x20        [--trace]\n\
         \x20             draw the wallpaper layer and follow niri's layout\n\
         \x20 watch [--output NAME]   print the parallax target as niri's layout changes\n\
         \n\
         \x20 set PATH [--output NAME] [--socket PATH]   switch the running daemon's wallpaper\n\
         \x20 query    [--output NAME] [--socket PATH]   what is on screen right now\n\
         \x20 schema   [--output NAME] [--socket PATH]   every config key a UI can offer (JSON)\n\
         \x20 state    [--output NAME] [--socket PATH]   what the daemon is doing right now (JSON)\n\
         \x20 kill     [--output NAME] [--socket PATH]   ask the daemon to shut down\n\
         \n\
         \x20 --version  print the version\n"
    );
}

/// Follow the event stream and print every change of the parallax target.
fn watch(args: &[String]) -> Result<(), String> {
    install_signal_handlers();
    let mut output = String::new();
    let mut screen = (0.0f64, 0.0f64);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--output" => output = iter.next().ok_or("--output needs a value")?.clone(),
            "--screen" => {
                let value = iter.next().ok_or("--screen needs WxH")?;
                let (w, h) = value.split_once(['x', 'X']).ok_or("--screen expects WxH")?;
                screen = (
                    w.parse().map_err(|e| format!("--screen width: {e}"))?,
                    h.parse().map_err(|e| format!("--screen height: {e}"))?,
                );
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }

    // `watch` is an observation tool: it takes no config, so it runs the
    // built-in defaults (as it already did for `scale`).
    let mut niri = Niri::connect(DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN)?;
    // The first events are the full state; wait for them so the first line we
    // print is the state we start from.
    niri.wait_for_full_state()?;
    if output.is_empty() {
        let names: Vec<String> = niri
            .motion
            .workspaces()
            .iter()
            .map(|w| w.output.clone())
            .collect();
        output = names
            .into_iter()
            .find(|name| !name.is_empty())
            .ok_or("niri reported no outputs; pass --output")?;
    }
    println!(
        "watching {} (built-in spans {}/{}, scale {}), {} event(s) ignored so far",
        output, DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN, DEFAULT_SCALE, niri.ignored
    );
    let mut last = Progress::CENTER;
    report(&niri, &output, screen, &mut last, "initial");
    loop {
        match niri.next_event() {
            Ok(event) => {
                let summary = summarize(&event);
                report(&niri, &output, screen, &mut last, &summary);
            }
            // A signal interrupts the blocking read; that is how Ctrl-C stops us.
            Err(_) if EXIT.load(Ordering::SeqCst) => return Ok(()),
            Err(err) => return Err(err),
        }
    }
}

fn report(niri: &Niri, output: &str, screen: (f64, f64), last: &mut Progress, why: &str) {
    let progress = niri.motion.progress(output);
    if progress == *last && why != "initial" {
        return;
    }
    *last = progress;
    let (x, y) = offset_px(progress, screen, DEFAULT_SCALE);
    if screen.0 > 0.0 {
        println!(
            "{why:<34} h={:.3} v={:.3}  offset=({x:+.1}, {y:+.1}) px",
            progress.horizontal, progress.vertical
        );
    } else {
        println!(
            "{why:<34} h={:.3} v={:.3}",
            progress.horizontal, progress.vertical
        );
    }
}

/// A short human description of an event, for the log line.
fn summarize(event: &niripaper::motion::Event) -> String {
    use niripaper::motion::Event;
    match event {
        Event::WorkspacesChanged { workspaces } => {
            format!("WorkspacesChanged({})", workspaces.len())
        }
        Event::WorkspaceActivated { id, focused } => {
            format!("WorkspaceActivated(id={id}, focused={focused})")
        }
        Event::WorkspaceActiveWindowChanged {
            workspace_id,
            active_window_id,
        } => {
            format!("WorkspaceActiveWindowChanged(ws={workspace_id}, win={active_window_id:?})")
        }
        Event::WindowsChanged { windows } => format!("WindowsChanged({})", windows.len()),
        Event::WindowOpenedOrChanged { window } => format!(
            "WindowOpenedOrChanged(id={}, ws={}, col={})",
            window.id, window.workspace_id, window.column
        ),
        Event::WindowClosed { id } => format!("WindowClosed(id={id})"),
        Event::WindowFocusChanged { id } => format!("WindowFocusChanged(id={id:?})"),
        Event::WindowLayoutsChanged { changes } => {
            format!("WindowLayoutsChanged({})", changes.len())
        }
    }
}

/// Flags given on the command line. Each one overrides the config file, which
/// in turn overrides the built-in defaults.
#[derive(Default)]
struct Overrides {
    output: Option<String>,
    namespace: Option<String>,
    scale: Option<f64>,
    column_span: Option<usize>,
    workspace_span: Option<usize>,
    pattern: Option<Pattern>,
    wallpaper: Option<PathBuf>,
    config: Option<PathBuf>,
    socket: Option<PathBuf>,
    trace: bool,
}

fn parse_overrides(args: &[String]) -> Result<Overrides, String> {
    let mut over = Overrides::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--output" => over.output = Some(value()?.clone()),
            "--namespace" => over.namespace = Some(value()?.clone()),
            "--config" => over.config = Some(PathBuf::from(value()?)),
            "--socket" => over.socket = Some(PathBuf::from(value()?)),
            "--wallpaper" => over.wallpaper = Some(PathBuf::from(value()?)),
            "--scale" => over.scale = Some(value()?.parse().map_err(|e| format!("--scale: {e}"))?),
            "--column-span" => {
                over.column_span = Some(
                    value()?
                        .parse()
                        .map_err(|e| format!("--column-span: {e}"))?,
                )
            }
            "--workspace-span" => {
                over.workspace_span = Some(
                    value()?
                        .parse()
                        .map_err(|e| format!("--workspace-span: {e}"))?,
                )
            }
            "--pattern" => {
                over.pattern = Some(match value()?.as_str() {
                    "blocks" => Pattern::Blocks,
                    "bands" => Pattern::Bands,
                    other => return Err(format!("unknown --pattern {other}")),
                })
            }
            "--trace" => over.trace = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(over)
}

/// Draw the wallpaper layer for one output until asked to stop.
fn daemon_command(args: &[String]) -> Result<(), String> {
    install_signal_handlers();
    let over = parse_overrides(args)?;

    // The config file to watch for reloads: the explicit one, or the default one
    // when there is one. Built-in defaults mean nothing to watch.
    let (config, config_path) = match &over.config {
        Some(path) => (Config::load_from(path)?, Some(path.clone())),
        None => {
            // A `config.d` with no main file beside it is a valid setup — that is
            // exactly where a tool writes — so it counts as "there is one".
            let path = niripaper::config::default_path()
                .filter(|path| path.exists() || path.with_extension("d").is_dir());
            match path {
                Some(path) => (Config::load_from(&path)?, Some(path)),
                None => (Config::load()?, None),
            }
        }
    };
    let source = niripaper::config::describe_source(config_path.as_deref());

    // `--output` limits the daemon to one output. Without it the daemon draws on
    // every output niri reports, and each worker resolves its own per-output
    // overrides — so there is nothing to resolve here.
    let output = over
        .output
        .clone()
        .unwrap_or_else(|| niripaper::ipc::EVERY_OUTPUT.to_owned());

    let mut options = Options::new(output.clone());
    // The per-output keys are resolved by `Options::resolve_output` — here for a
    // single output, and again by each worker when the daemon draws on several.
    // One code path, so the two cannot drift apart.
    options.cli = daemon::CliOverrides {
        scale: over.scale,
        column_span: over.column_span,
        workspace_span: over.workspace_span,
        wallpaper: over.wallpaper.clone(),
        fit: None,
    };
    options.config = Some(config.clone());
    options.resolve_output(&output);
    options.video_fps = config.video_fps;
    options.config_path = config_path;
    options.socket = over.socket;
    options.namespace = over.namespace.unwrap_or_else(|| config.namespace.clone());
    // Animations and the transition come from the config only: they are tuned by
    // feel, and a flag per parameter would be noise.
    apply_config_only_settings(&mut options, &config);
    if let Some(pattern) = over.pattern {
        options.pattern = pattern;
    }
    options.trace = over.trace;
    println!("niripaper: config {source}");
    daemon::run(
        &options,
        std::sync::Arc::new(|| !EXIT.load(Ordering::SeqCst)),
    )
}

/// The settings no flag can override, applied in one place — and tested in one
/// place, because a config key that never reaches `Options` is invisible until
/// someone sets a non-default value and wonders why nothing changed. `[transition]`
/// did exactly that: every one of its keys was ignored at startup, and the
/// built-in defaults happened to match the documented values, so `state` looked
/// right the whole time.
fn apply_config_only_settings(options: &mut Options, config: &Config) {
    options.animations = config.animations.clone();
    options.transition = config.transition.clone();
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every key of `[transition]` has to reach the daemon. This is the test that
    /// would have caught the missing line.
    #[test]
    fn the_transition_settings_reach_the_daemon() {
        let config = Config::parse_without_niri(
            "[transition]\nselection = \"fixed\"\neffect = \"zoom\"\n\
             duration_ms = 2400\ncurve = \"linear\"\nsoftness = 0.1\n\
             center = [0.2, 0.8]\nstart_radius = 0.4\npush = 1.2\nstripes = 7\n\
             hold_ms = 250\non_start = false\nallow_overshoot = true",
        )
        .expect("parses");
        let mut options = Options::new("test");
        apply_config_only_settings(&mut options, &config);

        let transition = &options.transition;
        assert_eq!(transition.selection.name(), "fixed");
        assert_eq!(transition.effect.name(), "zoom");
        assert_eq!(transition.duration.as_millis(), 2400);
        assert_eq!(transition.curve.name(), "linear");
        assert_eq!(transition.softness, 0.1);
        assert_eq!(transition.center, (0.2, 0.8));
        assert_eq!(transition.start_radius, 0.4);
        assert_eq!(transition.push, 1.2);
        assert_eq!(transition.stripes, 7);
        assert_eq!(transition.hold.as_millis(), 250);
        assert!(!transition.on_start);
        assert!(transition.allow_overshoot);
        // And the animations beside it, which had the same shape of bug once.
        assert_eq!(options.animations, config.animations);
    }
}
