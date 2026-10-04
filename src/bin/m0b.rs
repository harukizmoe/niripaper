//! M0b probe: a minimal layer-shell client with a self-managed dmabuf
//! swapchain, on a *chosen* GPU.
//!
//! It exists to prove four things (`HANDOFF.md` §6):
//!
//! 1. the layer reaches the compositor with the namespace we asked for;
//! 2. with a `place-within-backdrop` layer rule it lands in the backdrop, so
//!    blurred windows sample it (run with `--namespace mpvpaper` to hit the
//!    rule that already exists on this machine, without touching any config);
//! 3. the process opens only the target GPU's DRM node;
//! 4. idle costs nothing: one frame is drawn, no frame callbacks are requested,
//!    and then the process sits in `poll()`.
//!
//! The GPU is chosen from the compositor's dmabuf feedback, which is the only
//! correct source: niri advertises its *primary* render node as the main device
//! for every surface, and renders display-only outputs (like the NVIDIA-driven
//! DP-1 here) by copying from that GPU.
//!
//! ```text
//! m0b [--output DP-1] [--namespace niripaper] [--render-node PATH]
//!     [--format xrgb8888|argb8888] [--idle-seconds N]
//! ```

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use niripaper::gpu;
use niripaper::render::egl::EglVendor;
use niripaper::render::{dmabuf::Frame, egl::Egl, gbm, gl, layer};

static EXIT: AtomicBool = AtomicBool::new(false);

extern "C" fn on_signal(_signal: libc::c_int) {
    EXIT.store(true, Ordering::SeqCst);
}

struct Args {
    output: String,
    namespace: String,
    render_node: Option<PathBuf>,
    format: u32,
    idle_seconds: Option<f64>,
    frames: u32,
    /// `None` = follow the compositor's feedback (the production behaviour).
    modifier: Option<Vec<u64>>,
}

fn parse_modifier(raw: &str) -> Result<Option<Vec<u64>>, String> {
    match raw.to_ascii_lowercase().as_str() {
        // Search the compositor's list, then let the driver choose.
        "auto" => Ok(None),
        "linear" => Ok(Some(vec![gbm::MOD_LINEAR])),
        // No modifier list at all: the driver picks.
        "driver" => Ok(Some(Vec::new())),
        other => {
            let value = match other.strip_prefix("0x") {
                Some(hex) => u64::from_str_radix(hex, 16),
                None => other.parse(),
            }
            .map_err(|e| format!("--modifier {other}: {e}"))?;
            Ok(Some(vec![value]))
        }
    }
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        output: "DP-1".to_owned(),
        namespace: "niripaper".to_owned(),
        render_node: None,
        format: gbm::FORMAT_XRGB8888,
        idle_seconds: None,
        frames: 1,
        modifier: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        let mut value = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--output" => args.output = value()?,
            "--namespace" => args.namespace = value()?,
            "--render-node" => args.render_node = Some(PathBuf::from(value()?)),
            "--format" => {
                let raw = value()?;
                args.format = match raw.to_ascii_lowercase().as_str() {
                    "xrgb8888" => gbm::FORMAT_XRGB8888,
                    "argb8888" => gbm::FORMAT_ARGB8888,
                    other => return Err(format!("unknown --format {other}")),
                };
            }
            "--modifier" => args.modifier = parse_modifier(&value()?)?,
            "--frames" => {
                args.frames = value()?.parse().map_err(|e| format!("--frames: {e}"))?;
                if args.frames == 0 {
                    return Err("--frames must be at least 1".to_owned());
                }
            }
            "--idle-seconds" => {
                args.idle_seconds = Some(
                    value()?
                        .parse()
                        .map_err(|e| format!("--idle-seconds: {e}"))?,
                )
            }
            "-h" | "--help" => {
                println!(
                    "m0b [--output DP-1] [--namespace niripaper] [--render-node PATH]\n    \
                     [--format xrgb8888|argb8888] [--modifier auto|linear|driver|0x…]\n    \
                     [--frames N] [--idle-seconds N]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(args)
}

fn main() {
    install_signal_handlers();
    match run() {
        Ok(()) => {}
        Err(err) => {
            eprintln!("m0b: {err}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;

    // --- wayland -----------------------------------------------------------
    let mut client = layer::Client::connect()?;
    println!(
        "wayland     : compositor v{}, layer-shell v{}, dmabuf v{}, outputs bound at v{}",
        client.compositor_version,
        client.layer_shell_version,
        client.dmabuf_version,
        client.output_version
    );
    println!("outputs     : {:?}", client.state.output_names());
    println!(
        "dmabuf      : default feedback main device {} ({} pair(s))",
        describe_device(client.state.default_main_device),
        client.state.default_formats.len(),
    );
    let output = client
        .state
        .output_named(&args.output)
        .ok_or_else(|| format!("no wl_output named {}", args.output))?;

    // --- layer surface -----------------------------------------------------
    let surface = client.create_layer_surface(
        &output,
        &args.namespace,
        wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer::Background,
    )?;
    println!(
        "layer       : {} on {} configured {}x{} (serial {})",
        args.namespace, surface.output_name, surface.width, surface.height, surface.serial
    );

    // Per-surface feedback is the authoritative answer to "which GPU renders
    // this output", and it can differ from the default feedback.
    client.use_surface_feedback(&surface.surface)?;
    let candidates = client.state.modifiers_for(args.format);
    println!(
        "              surface feedback main device {}",
        describe_device(client.state.main_device)
    );
    println!(
        "              {} format/modifier pair(s), {} for {}:",
        client.state.formats.len(),
        candidates.len(),
        gbm::fourcc_name(args.format),
    );
    for modifier in candidates.iter().take(6) {
        println!("                {}", gbm::modifier_name(*modifier));
    }
    if candidates.len() > 6 {
        println!("                … {} more", candidates.len() - 6);
    }

    // --- which GPU ---------------------------------------------------------
    // Buffers the main device cannot import are rejected or copied every frame,
    // so allocate where the compositor renders. `--render-node` overrides that
    // for experiments.
    let node = match args.render_node.clone() {
        Some(explicit) => explicit,
        None => match client.state.main_device {
            Some(dev) => gpu::node_for_device_id(dev)?,
            None => {
                println!("              no main device advertised; using the output's GPU");
                gpu::gpu_for_connector(&args.output)?.render_node
            }
        },
    };
    let gpu = gpu::gpu_for_node(&node)?;
    let vendor = EglVendor::for_pci_vendor(gpu.vendor_id);
    println!(
        "gpu         : {} ({}, vendor 0x{:04x}) via {}",
        gpu.card,
        gpu.vendor_name(),
        gpu.vendor_id,
        node.display()
    );
    println!("egl vendor  : {vendor:?}");
    println!("namespace   : {}", args.namespace);
    println!("format      : {}", gbm::fourcc_name(args.format));
    println!();

    // --- GPU objects -------------------------------------------------------
    let device = gbm::Device::open(&node)?;
    println!(
        "gbm         : opened {} (supports {}: {})",
        node.display(),
        gbm::fourcc_name(args.format),
        device.supports(args.format, gbm::USE_RENDERING | gbm::USE_LINEAR),
    );
    let egl = Egl::new(device.as_ptr(), device.fd(), vendor)?;
    println!(
        "egl         : {} [{}] via {} config: {}",
        egl.version, egl.vendor, egl.platform, egl.config_label
    );
    println!("              vendor list: {}", egl.vendor_pinned);
    println!("              surfaceless: {}", egl.is_surfaceless());
    for wanted in [
        "EGL_EXT_image_dma_buf_import",
        "EGL_EXT_image_dma_buf_import_modifiers",
        "EGL_KHR_surfaceless_context",
        "EGL_MESA_image_dma_buf_export",
    ] {
        println!(
            "              {wanted}: {}",
            egl.has_display_extension(wanted)
        );
    }
    let gpu_modifiers = egl.dmabuf_modifiers(args.format);
    let renderable: Vec<u64> = gpu_modifiers
        .iter()
        .filter(|(_, external_only)| !external_only)
        .map(|(m, _)| *m)
        .collect();
    println!(
        "              driver can produce {} with {} modifier(s), {} renderable: {}",
        gbm::fourcc_name(args.format),
        gpu_modifiers.len(),
        renderable.len(),
        renderable
            .iter()
            .take(4)
            .map(|m| gbm::modifier_name(*m))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let renderer = gl::Renderer::new()?;
    println!(
        "gl          : {} | {} | {}",
        gl::string(gl::GL_VERSION),
        gl::string(gl::GL_RENDERER),
        gl::string(gl::GL_VENDOR),
    );
    println!();

    // --- one frame ---------------------------------------------------------
    // Search order: modifiers both sides accept first (compositor order), then
    // compositor-only ones (the driver may still refuse), then let the driver
    // pick — which the compositor may then reject, but it is worth reporting.
    let ordered: Vec<Vec<u64>> = match &args.modifier {
        Some(forced) => vec![forced.clone()],
        None => {
            let mut ordered: Vec<Vec<u64>> = candidates
                .iter()
                .filter(|m| renderable.contains(m))
                .map(|m| vec![*m])
                .collect();
            ordered.extend(
                candidates
                    .iter()
                    .filter(|m| !renderable.contains(m))
                    .map(|m| vec![*m]),
            );
            ordered.push(Vec::new());
            ordered
        }
    };
    if let Some(forced) = &args.modifier {
        println!("modifier    : forced to {}", modifier_label(forced));
    }

    // Find a modifier that is both advertised and renderable by actually
    // creating a buffer with it; the first success is reused for every frame.
    let mut failures: Vec<(Vec<u64>, String)> = Vec::new();
    let mut chosen_modifier: Option<Vec<u64>> = None;
    for modifier in ordered {
        match Frame::new(
            &egl,
            &device,
            surface.width,
            surface.height,
            args.format,
            &modifier,
        ) {
            Ok(_) => {
                chosen_modifier = Some(modifier);
                break;
            }
            Err(err) => failures.push((modifier, err)),
        }
    }
    let chosen_modifier = chosen_modifier.ok_or_else(|| {
        let mut report = String::from("no renderable buffer:");
        for (modifier, err) in &failures {
            report.push_str(&format!("\n  {}: {err}", modifier_label(modifier)));
        }
        report
    })?;
    for (modifier, err) in &failures {
        println!("              rejected {}: {err}", modifier_label(modifier));
    }

    // A frame is one buffer: allocate, render, hand over, then wait for the
    // compositor to give it back before allocating the next one. That is the
    // whole swapchain contract, minus the pool that M1 will want.
    let mut buffers = Vec::new();
    let mut frames_drawn = 0u32;
    for index in 0..args.frames {
        let frame = match Frame::new(
            &egl,
            &device,
            surface.width,
            surface.height,
            args.format,
            &chosen_modifier,
        ) {
            Ok(frame) => frame,
            Err(err) => return Err(format!("frame {index}: {err}")),
        };
        if index == 0 {
            println!(
                "buffer      : {} — modifier {}",
                frame.describe(),
                if candidates.contains(&frame.modifier) {
                    "advertised by the compositor for this surface"
                } else {
                    "chosen by the driver; NOT in the compositor's list"
                }
            );
        }
        frame.begin();
        renderer.draw_pattern();
        frame.finish();
        if let Some(err) = gl::last_error() {
            return Err(format!("GL error after drawing: 0x{err:x}"));
        }
        buffers.push(client.attach(&surface, &frame));
        client.flush()?;
        frames_drawn += 1;
        // The compositor releases a buffer only once it stops using it, i.e.
        // when a newer one replaces it. So after N commits, at most N-1
        // releases can be outstanding — that is the pipeline depth, and it is
        // what proves buffers really are recycled rather than leaked.
        if frames_drawn > 1 {
            client.wait_for_releases(frames_drawn - 1, Duration::from_secs(2))?;
        }
    }
    println!(
        "drawn       : {frames_drawn} frame(s), {frames_drawn} commit(s), {} buffer release(s), \
         0 frame callbacks requested",
        client.state.releases
    );
    println!();

    // --- criterion 3: which device nodes are open --------------------------
    println!("open drm nodes:");
    for node in open_drm_nodes()? {
        println!("  {node}");
    }
    println!();

    // --- criterion 4: idle -------------------------------------------------
    println!("idle        : waiting for events (Ctrl-C to stop)");
    let idle_budget = args
        .idle_seconds
        .map(Duration::from_secs_f64)
        .unwrap_or(Duration::from_secs(u64::MAX / 2));
    let start = Instant::now();
    let before = Proc::now()?;
    let mut events = 0usize;
    while !EXIT.load(Ordering::SeqCst) {
        let remaining = idle_budget.saturating_sub(start.elapsed());
        if remaining.is_zero() {
            break;
        }
        events += client.wait_events(remaining)?;
    }
    let after = Proc::now()?;
    let elapsed = start.elapsed();

    println!();
    println!("idle report (over {:.1} s)", elapsed.as_secs_f64());
    println!("  wayland events dispatched : {events}");
    println!(
        "  cpu                       : {:.1} ms user + {:.1} ms sys",
        before.ticks_to_ms(after.utime - before.utime),
        before.ticks_to_ms(after.stime - before.stime),
    );
    println!(
        "  bytes written (socket)    : {}",
        after.wchar - before.wchar
    );
    println!("  draws after the first frame: 0 (no frame callbacks requested)");

    surface.destroy();
    for buffer in &buffers {
        buffer.destroy();
    }
    client.flush()?;
    Ok(())
}

fn open_drm_nodes() -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir("/proc/self/fd").map_err(|e| format!("/proc/self/fd: {e}"))? {
        let entry = entry.map_err(|e| format!("/proc/self/fd: {e}"))?;
        let Ok(target) = std::fs::read_link(entry.path()) else {
            continue;
        };
        let target = target.to_string_lossy().into_owned();
        if target.starts_with("/dev/dri/") || target.contains("nvidia") {
            out.push(target);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

/// Name the DRM node behind a `dev_t` (as sent in dmabuf feedback).
fn describe_device(dev: Option<u64>) -> String {
    let Some(dev) = dev else {
        return "unknown".to_owned();
    };
    if let Ok(node) = gpu::node_for_device_id(dev) {
        return format!("{} (dev {dev})", node.display());
    }
    let major = ((dev >> 8) & 0xfff) | ((dev >> 32) & 0xffff_f000);
    let minor = (dev & 0xff) | ((dev >> 12) & 0xffff_ff00);
    format!("dev {dev} ({major}:{minor})")
}

fn modifier_label(modifiers: &[u64]) -> String {
    if modifiers.is_empty() {
        "driver-chosen modifier".to_owned()
    } else {
        modifiers
            .iter()
            .map(|m| gbm::modifier_name(*m))
            .collect::<Vec<_>>()
            .join("+")
    }
}

/// A `/proc/self` snapshot: CPU time and socket writes.
struct Proc {
    utime: u64,
    stime: u64,
    wchar: u64,
    ticks_per_second: u64,
}

impl Proc {
    fn now() -> Result<Self, String> {
        let stat = std::fs::read_to_string("/proc/self/stat")
            .map_err(|e| format!("/proc/self/stat: {e}"))?;
        // Field 2 (comm) can contain spaces and parentheses: split after the
        // last ')'. The remainder starts at field 3 (state).
        let rest = stat.rsplit_once(')').ok_or("unparsable /proc/self/stat")?.1;
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // utime is field 14, stime field 15 => indices 11 and 12 here.
        let utime: u64 = fields
            .get(11)
            .ok_or("missing utime")?
            .parse()
            .map_err(|e| format!("utime: {e}"))?;
        let stime: u64 = fields
            .get(12)
            .ok_or("missing stime")?
            .parse()
            .map_err(|e| format!("stime: {e}"))?;
        let io = std::fs::read_to_string("/proc/self/io").unwrap_or_default();
        let wchar = io
            .lines()
            .find_map(|l| l.strip_prefix("wchar: "))
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0);
        let ticks_per_second = unsafe { libc::sysconf(libc::_SC_CLK_TCK) }.max(1) as u64;
        Ok(Self {
            utime,
            stime,
            wchar,
            ticks_per_second,
        })
    }

    fn ticks_to_ms(&self, ticks: u64) -> f64 {
        ticks as f64 * 1000.0 / self.ticks_per_second as f64
    }
}

fn install_signal_handlers() {
    let handler = on_signal as extern "C" fn(libc::c_int) as *const () as libc::sighandler_t;
    unsafe {
        libc::signal(libc::SIGINT, handler);
        libc::signal(libc::SIGTERM, handler);
    }
}
