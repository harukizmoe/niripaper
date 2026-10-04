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

use niripaper::motion::{offset_px, Progress, DEFAULT_SCALE, DEFAULT_SPAN};
use niripaper::niri::Niri;

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(command) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let rest: Vec<String> = args.collect();
    let result = match command.as_str() {
        "watch" => watch(&rest),
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

fn usage() {
    eprintln!(
        "usage: niripaper <command>\n\
         \n\
         commands:\n\
         \x20 watch [--output NAME]   print the parallax target as niri's layout changes\n"
    );
}

/// Follow the event stream and print every change of the parallax target.
fn watch(args: &[String]) -> Result<(), String> {
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

    let mut niri = Niri::connect()?;
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
        "watching {} (span {}, scale {}), {} event(s) ignored so far",
        output, DEFAULT_SPAN, DEFAULT_SCALE, niri.ignored
    );
    let mut last = Progress::CENTER;
    report(&niri, &output, screen, &mut last, "initial");
    loop {
        let event = niri.next_event()?;
        let summary = summarize(&event);
        report(&niri, &output, screen, &mut last, &summary);
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
