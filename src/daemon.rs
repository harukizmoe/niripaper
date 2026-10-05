//! The wallpaper daemon: a layer surface per output, driven by niri's event
//! stream and paced by the compositor's frame callbacks.
//!
//! The whole loop is one `poll()` over two fds:
//!
//! ```text
//!   niri socket ──► event ──► motion::progress ──► anim::retarget ──┐
//!                                                                  ▼
//!   wayland fd ──► wl_callback::done ──► anim::sample ──► draw ──► commit
//!                        ▲                                          │
//!                        └──────────── request the next frame ──────┘
//! ```
//!
//! Two properties are load-bearing:
//!
//! * **Nothing is drawn unless the layout is moving.** At rest the animator is
//!   at its target, no frame callback is outstanding, and the process blocks in
//!   `poll()` — idle cost is zero (`HANDOFF.md` §6, M0b criterion ④).
//! * **The animation is paced by the compositor**, not by a timer: a frame
//!   callback arrives exactly when niri is ready to present, so a 180 Hz output
//!   gets 180 Hz and a busy compositor does not get a backlog.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::media::{self, Media};
use crate::motion::{DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN};
use crate::niri::Niri;
use crate::render::anim::{Animator, Curve};
use crate::render::egl::{Egl, EglVendor};
use crate::render::gbm;
use crate::render::gl::{self, Pattern};
use crate::render::layer::{self, Pool};
use crate::render::transition::Transition;
use crate::render::Fit;
use crate::scene::Scene;
use crate::{gpu, motion};

/// The layer-shell namespace, and therefore the name users match in
/// `~/.config/niri/rules.kdl` (§2).
pub const NAMESPACE: &str = "niripaper";

/// Pixel format we hand the compositor.
const FORMAT: u32 = gbm::FORMAT_XRGB8888;

/// How many buffers to keep. Two is enough to draw the next frame while the
/// previous one is on screen.
const POOL_DEPTH: usize = 2;

#[derive(Debug, Clone)]
pub struct Options {
    pub output: String,
    /// Layer-shell namespace. `niripaper` is the product name; the compositor
    /// only puts a layer in the backdrop if a rule matches this, so pointing it
    /// at an existing rule (`mpvpaper` on this machine) is how behaviour can be
    /// tested without touching the user's config.
    pub namespace: String,
    pub scale: f64,
    pub column_span: usize,
    pub workspace_span: usize,
    pub pattern: Pattern,
    /// A wallpaper to draw instead of the procedural pattern: a still image, or
    /// a video (routed by extension).
    pub wallpaper: Option<std::path::PathBuf>,
    /// Frame-rate cap for video wallpapers (`0` keeps the source's).
    pub video_fps: u32,
    /// The config file this daemon reads, for the reload watcher. `None` means
    /// the built-in defaults with no file to watch.
    pub config_path: Option<std::path::PathBuf>,
    /// Control socket to bind (`None` = `$XDG_RUNTIME_DIR/niripaper.sock`).
    /// Overridable so a second instance can be tested without fighting the
    /// session's daemon over one path.
    pub socket: Option<std::path::PathBuf>,
    /// Animation parameters, in niri's vocabulary (see `config.rs`).
    pub animations: crate::config::Animations,
    /// The transition when the wallpaper changes.
    pub transition: crate::render::transition::Settings,
    /// How a source whose aspect ratio does not match the canvas is placed.
    pub fit: Fit,
    /// Log every frame: the per-frame progress is how the "monotonic easing"
    /// acceptance is checked, and the cadence shows whether frames are being
    /// dropped.
    pub trace: bool,
}

impl Options {
    pub fn new(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            namespace: NAMESPACE.to_owned(),
            scale: motion::DEFAULT_SCALE,
            column_span: DEFAULT_COLUMN_SPAN,
            workspace_span: DEFAULT_WORKSPACE_SPAN,
            pattern: Pattern::Blocks,
            wallpaper: None,
            video_fps: 0,
            config_path: None,
            socket: None,
            animations: crate::config::Animations::default(),
            transition: crate::render::transition::Settings::default(),
            fit: Fit::Fill,
            trace: false,
        }
    }
}

/// One command for one output's worker.
enum Command {
    /// Swap this output's wallpaper (`set`).
    Set(PathBuf),
    /// What is on screen right now (`query`).
    Query(std::sync::mpsc::Sender<String>),
    /// The full snapshot for `state`.
    State(std::sync::mpsc::Sender<serde_json::Value>),
}

/// How long a request waits for a worker before giving up on it. A worker that
/// is busy decoding is not stuck, but a client must not be able to hang either.
const REPLY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// The workers a request applies to: the one it names, or all of them.
///
/// A name nobody serves is an error rather than an empty answer — "it silently
/// did nothing" is the failure mode this whole project keeps getting bitten by.
fn pick<'a>(
    channels: &'a [(String, std::sync::mpsc::Sender<Command>)],
    output: Option<&str>,
) -> Result<Vec<&'a (String, std::sync::mpsc::Sender<Command>)>, String> {
    let picked: Vec<&(String, std::sync::mpsc::Sender<Command>)> = channels
        .iter()
        .filter(|(name, _)| output.is_none_or(|wanted| wanted == name))
        .collect();
    if picked.is_empty() {
        let known: Vec<&str> = channels.iter().map(|(name, _)| name.as_str()).collect();
        return Err(format!(
            "no output {:?}; this daemon draws on {}",
            output.unwrap_or_default(),
            known.join(", ")
        ));
    }
    Ok(picked)
}

/// The output a request names, for [`pick`].
fn named_output(request: &crate::ipc::Request) -> Option<&str> {
    match request {
        crate::ipc::Request::Set { output, .. }
        | crate::ipc::Request::Query { output }
        | crate::ipc::Request::State { output } => output.as_deref(),
        crate::ipc::Request::Schema | crate::ipc::Request::Kill => None,
    }
}

/// One process, every output.
///
/// A thread per output, each with its own Wayland connection, EGL context and
/// media — so the single-output loop below does not have to learn about several
/// surfaces at once (`HANDOFF.md` §2 has the decision and the reasons). What is
/// shared is the control socket and nothing else: a client names the output it
/// means, this routes the request, and `state` answers for every output in one
/// call. That is the shape a panel wants.
pub fn run(options: &Options, running: Arc<dyn Fn() -> bool + Send + Sync>) -> Result<(), String> {
    let socket_path = match &options.socket {
        Some(path) => path.clone(),
        None => crate::ipc::default_path()?,
    };
    crate::ipc::refuse_if_served(&socket_path)?;

    // Which outputs to draw on: the one named (`--output`, which is also how a
    // test instance stays off the user's screens), or every output niri knows.
    let names = if options.output != crate::ipc::EVERY_OUTPUT {
        vec![options.output.clone()]
    } else {
        let mut niri = Niri::connect(options.column_span, options.workspace_span)?;
        niri.wait_for_full_state()?;
        let mut names: Vec<String> = niri
            .motion
            .workspaces()
            .iter()
            .map(|workspace| workspace.output.clone())
            .filter(|name| !name.is_empty())
            .collect();
        names.sort();
        names.dedup();
        if names.is_empty() {
            return Err("niri reported no outputs; pass --output".to_owned());
        }
        names
    };

    // `kill` has to stop the workers too, and they only know the caller's
    // closure — so they get "the caller says keep going *and* nobody asked us to
    // stop".
    let quit = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let worker_running: Arc<dyn Fn() -> bool + Send + Sync> = {
        let quit = Arc::clone(&quit);
        let running = Arc::clone(&running);
        Arc::new(move || running() && !quit.load(std::sync::atomic::Ordering::Relaxed))
    };

    let mut channels: Vec<(String, std::sync::mpsc::Sender<Command>)> = Vec::new();
    let mut handles = Vec::new();
    for name in &names {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut worker_options = options.clone();
        worker_options.output = name.clone();
        let worker_running = Arc::clone(&worker_running);
        let worker_name = name.clone();
        handles.push(std::thread::spawn(move || {
            if let Err(err) = run_output(&worker_options, &worker_running, &rx) {
                log(&format!("{worker_name}: {err}"));
            }
        }));
        channels.push((name.clone(), tx));
    }

    // Bound after the workers exist, so "the socket exists" still means "there
    // is a daemon that works" — the property the one-process-per-output daemon
    // had, and the reason the bind is not the first thing here.
    let ipc = crate::ipc::Server::bind(&socket_path)?;
    log(&format!(
        "control socket {} (outputs: {})",
        socket_path.display(),
        names.join(", ")
    ));

    while worker_running() {
        let mut fd = libc::pollfd {
            fd: ipc.fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // A timeout so `running()` is checked even when nobody is talking to us.
        let ready = unsafe { libc::poll(&mut fd, 1, 250) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(format!("poll: {err}"));
        }
        if ready == 0 {
            continue;
        }
        let (request, stream) = match ipc.accept() {
            Ok(accepted) => accepted,
            Err(err) => {
                log(&format!("control socket: {err}"));
                continue;
            }
        };
        let targets = match pick(&channels, named_output(&request)) {
            Ok(targets) => targets,
            Err(err) => {
                crate::ipc::reply(&stream, &format!("error {err}"));
                continue;
            }
        };
        let reply = match request {
            crate::ipc::Request::Schema => Some(json_line(&crate::schema::schema())),
            crate::ipc::Request::Kill => {
                quit.store(true, std::sync::atomic::Ordering::Relaxed);
                Some("ok".to_owned())
            }
            crate::ipc::Request::Set { path, .. } => {
                let mut failed = Vec::new();
                for (name, tx) in targets {
                    if tx.send(Command::Set(path.clone())).is_err() {
                        failed.push(name.clone());
                    }
                }
                Some(if failed.is_empty() {
                    "ok".to_owned()
                } else {
                    format!("error no worker for {}", failed.join(", "))
                })
            }
            crate::ipc::Request::Query { .. } => {
                let mut lines = Vec::new();
                for (name, tx) in targets {
                    let (tx2, rx2) = std::sync::mpsc::channel();
                    if tx.send(Command::Query(tx2)).is_ok() {
                        match rx2.recv_timeout(REPLY_TIMEOUT) {
                            Ok(line) => lines.push(format!("{name}: {line}")),
                            Err(_) => lines.push(format!("{name}: no reply")),
                        }
                    }
                }
                Some(lines.join("; "))
            }
            crate::ipc::Request::State { output } => {
                let mut map = serde_json::Map::new();
                for (name, tx) in targets {
                    let (tx2, rx2) = std::sync::mpsc::channel();
                    if tx.send(Command::State(tx2)).is_ok() {
                        if let Ok(value) = rx2.recv_timeout(REPLY_TIMEOUT) {
                            map.insert(name.clone(), value);
                        }
                    }
                }
                // Naming one output hands back that output's own object, so a
                // client that asked for one does not have to unwrap a map.
                let value = if output.is_some() {
                    map.into_values().next().unwrap_or(serde_json::Value::Null)
                } else {
                    serde_json::json!({ "outputs": map })
                };
                Some(json_line(&value))
            }
        };
        if let Some(reply) = reply {
            crate::ipc::reply(&stream, &reply);
        }
    }

    for handle in handles {
        let _ = handle.join();
    }
    Ok(())
}

/// Draw the wallpaper layer for one output until asked to stop.
///
/// One thread runs this per output, so everything in here is that output's own:
/// its Wayland connection, its EGL context, its media, its animators. The only
/// thing it shares with its siblings is the `commands` channel the supervisor
/// feeds, which is how `set`/`query`/`state` reach it.
fn run_output(
    options: &Options,
    running: &Arc<dyn Fn() -> bool + Send + Sync>,
    commands: &std::sync::mpsc::Receiver<Command>,
) -> Result<(), String> {
    // The *effective* configuration, owned so a reload can change it: `set` over
    // the control socket and a config-file reload both write here. Command-line
    // flags are a startup-only override, exactly as they are in niri.
    let mut options = options.clone();

    // --- wayland -----------------------------------------------------------
    let mut client = layer::Client::connect()?;
    let output = client
        .state
        .output_named(&options.output)
        .ok_or_else(|| format!("no wl_output named {}", options.output))?;
    let surface = client.create_layer_surface(
        &output,
        &options.namespace,
        wayland_protocols_wlr::layer_shell::v1::client::zwlr_layer_shell_v1::Layer::Background,
    )?;
    client.use_surface_feedback(&surface.surface)?;

    // --- which GPU ---------------------------------------------------------
    // The compositor's main device is the one it composites on; anywhere else
    // means a cross-GPU copy per frame (§7.1).
    let node = match client.state.main_device {
        Some(device) => gpu::node_for_device_id(device)?,
        None => {
            log("no dmabuf main device advertised; using the output's GPU");
            gpu::gpu_for_connector(&options.output)?.render_node
        }
    };
    let gpu = gpu::gpu_for_node(&node)?;
    log(&format!(
        "{} on {} ({} {}), namespace {}, scale {:.3}, spans {}/{}, pattern {:?}\n\
         animations: parallax {}, overview-open-close {} (zoom {:.3}), slowdown {}{}",
        options.output,
        node.display(),
        gpu.card,
        gpu.vendor_name(),
        options.namespace,
        options.scale,
        options.column_span,
        options.workspace_span,
        options.pattern,
        options.animations.parallax.describe(),
        options.animations.overview_open_close.animation.describe(),
        options.animations.overview_open_close.zoom,
        options.animations.slowdown,
        match (
            options.animations.from_niri.is_empty(),
            &options.animations.niri_config,
        ) {
            (true, _) => String::new(),
            (false, Some(path)) => format!(
                "  (from {}: {})",
                path.display(),
                options.animations.from_niri.join(", ")
            ),
            (false, None) => format!(
                "  (from niri's config: {})",
                options.animations.from_niri.join(", ")
            ),
        },
    ));

    // --- render objects ----------------------------------------------------
    let device = gbm::Device::open(&node)?;
    let egl = Egl::new(
        device.as_ptr(),
        device.fd(),
        EglVendor::for_pci_vendor(gpu.vendor_id),
    )?;
    let renderer = gl::Renderer::new()?;
    let advertised = client.state.modifiers_for(FORMAT);
    let mut pool = Pool::new(
        &egl,
        &device,
        surface.width,
        surface.height,
        FORMAT,
        &advertised,
        POOL_DEPTH,
    )?;
    log(&format!(
        "{}x{} {} via {} [{}] ({})\n\
         vendor pinned: {}; vulkan ICDs: {}",
        surface.width,
        surface.height,
        gbm::fourcc_name(FORMAT),
        egl.vendor,
        egl.platform,
        layer::describe_modifiers(pool.chosen()),
        egl.vendor_pinned,
        egl.vulkan_pinned
    ));
    for (modifiers, err) in pool.failures() {
        log(&format!(
            "rejected {}: {err}",
            layer::describe_modifiers(modifiers)
        ));
    }

    // The canvas is the output enlarged by `scale` (§4.1); a wallpaper is
    // fitted to exactly that, so the shader samples it 1:1. `mut`, because a
    // reload can change `scale` — a canvas that no longer matches the media is
    // a wallpaper drawn at the wrong magnification.
    let screen = (surface.width as f64, surface.height as f64);
    let mut canvas = canvas_size(screen, options.scale);
    let mut media = match &options.wallpaper {
        Some(path) => {
            let loaded = Media::load(path, canvas, options.video_fps, options.fit)?;
            log(&format!(
                "{} {} → canvas {}×{}, fps cap {}",
                if media::is_video(path) {
                    "video"
                } else {
                    "wallpaper"
                },
                path.display(),
                canvas.0,
                canvas.1,
                if options.video_fps == 0 {
                    "source".to_owned()
                } else {
                    options.video_fps.to_string()
                }
            ));
            Some(loaded)
        }
        None => None,
    };

    // --- niri --------------------------------------------------------------
    let mut niri = Niri::connect(options.column_span, options.workspace_span)?;
    niri.wait_for_full_state()?;
    let mut animator = Animator::new(
        niri.motion.progress(&options.output),
        options.animations.parallax,
    )
    .with_slowdown(options.animations.slowdown);
    // The overview transition animates the canvas scale (§4.1): pulling back
    // shows more of the wallpaper, which reads as the workspace receding.
    let mut zoom = Animator::new(1.0f64, options.animations.overview_open_close.animation)
        .with_slowdown(options.animations.slowdown);
    let mut transition = Transition::new(options.transition.clone(), options.animations.slowdown);
    log(&format!(
        "initial progress h={:.3} v={:.3}",
        animator.target().horizontal,
        animator.target().vertical
    ));

    // --- the loop ----------------------------------------------------------
    let mut frame_pending = false;
    let mut seen_frames = 0u64;
    let mut drawn = 0u64;
    let mut skipped = 0u64;
    let mut last_frame_at = Instant::now();

    // Map the layer straight away. At rest no callback is requested, so the
    // process idles in `poll()` until the layout actually changes.
    // The slot matters: the cross-fade snapshots the buffer that is on screen.
    let mut last_slot: Option<usize> = Scene {
        view: Scene::view(
            animator.target(),
            1.0,
            screen,
            options.scale,
            options.pattern,
        ),
        content: match &media {
            Some(media) => media.content(),
            None => gl::Content::Pattern(options.pattern),
        },
        blend: None,
    }
    .draw(&mut client, &mut pool, &surface, &renderer, false)?;
    drawn += 1;

    // "Play one at startup" (§2). There is nothing on screen to fade from, so
    // the effect runs against an empty frame: the wallpaper arrives *through*
    // it rather than simply appearing. Logging in stops being a hard cut.
    if options.transition.on_start {
        transition.restart((screen.0 as u32, screen.1 as u32), Instant::now(), None);
    }

    // The video's wakeup fd is polled too — `-1` when there is no video, which
    // `poll()` ignores. That is what keeps the idle cost at zero for stills.
    let video_fd = media
        .as_ref()
        .map(|media| media.wakeup_fd())
        .unwrap_or(media::NO_WAKEUP);
    // Watch the configuration (§2). Directories, not files: tools write
    // atomically and may create `config.d` long after we started.
    let mut watcher = crate::watch::Watcher::new()?;
    let config_dirs: Vec<std::path::PathBuf> = match &options.config_path {
        Some(path) => {
            let dir = path.with_extension("d");
            let parent = path.parent().map(|p| p.to_owned());
            [parent, Some(dir)].into_iter().flatten().collect()
        }
        None => Vec::new(),
    };
    watcher.watch(&config_dirs)?;
    if !config_dirs.is_empty() {
        log(&format!(
            "watching {} for config changes",
            options
                .config_path
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default()
        ));
    }

    let mut fds = [
        libc::pollfd {
            fd: client.fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: niri.fd(),
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: video_fd,
            events: libc::POLLIN,
            revents: 0,
        },
        libc::pollfd {
            fd: watcher.fd(),
            events: libc::POLLIN,
            revents: 0,
        },
    ];

    log(&format!(
        "polling wayland fd {} and niri fd {}{}",
        fds[0].fd,
        fds[1].fd,
        if video_fd >= 0 {
            format!(" and video fd {video_fd}")
        } else {
            String::new()
        }
    ));
    let mut exit_note = None;
    // A decoded video frame that has not been presented yet. This is what turns
    // mpv's wakeup into a frame request.
    let mut video_present = false;
    while running() {
        fds[0].revents = 0;
        fds[1].revents = 0;
        fds[2].revents = 0;
        // Re-derive it every iteration: `set` can swap a still for a video, and
        // the new one has its own wakeup fd. Capturing it once at startup meant
        // a video switched in later was never pumped — the wallpaper stayed
        // black.
        fds[2].fd = media
            .as_ref()
            .map(|media| media.wakeup_fd())
            .unwrap_or(media::NO_WAKEUP);
        fds[3].revents = 0;
        // A timeout, not `-1`: `running()` has to be re-checked even when nothing
        // is happening, or `kill` could never stop this thread — it would sit in
        // `poll` forever and the supervisor's `join` would never return.
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 250) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue; // a signal: check `running()`
            }
            return Err(format!("poll: {err}"));
        }

        if fds[2].revents & libc::POLLIN != 0 {
            if let Some(media) = media.as_mut() {
                // Decode into our texture; the next frame callback presents it.
                match media.pump() {
                    Ok(true) => video_present = true,
                    Ok(false) => {}
                    Err(err) => log(&format!("media: {err}")),
                }
            }
        }

        if fds[1].revents & libc::POLLIN != 0 {
            if let Err(err) = niri.drain() {
                // The compositor going away is not an error worth shouting
                // about: the session is ending.
                exit_note = Some(err);
                break;
            }
            let now = Instant::now();
            let target = niri.motion.progress(&options.output);
            if animator.retarget(target, now) {
                log(&format!(
                    "target h={:.3} v={:.3} (moving)",
                    target.horizontal, target.vertical
                ));
            }
            // `retarget` ignores an unchanged target, so this is a no-op unless
            // the overview actually opened or closed.
            let wanted_zoom = if niri.overview_open {
                options.animations.overview_open_close.zoom
            } else {
                1.0
            };
            if zoom.retarget(wanted_zoom, now) {
                log(&format!(
                    "overview {} → zoom {wanted_zoom:.3}",
                    if niri.overview_open {
                        "opened"
                    } else {
                        "closed"
                    }
                ));
            }
        }
        if fds[3].revents & libc::POLLIN != 0 && watcher.drain() {
            let Some(path) = options.config_path.clone() else {
                continue;
            };
            // Watch *before* reloading. A `config.d` created just now is only
            // discovered by this reload, and inotify reports no past events — so
            // adding the watch afterwards would miss anything written into it in
            // the meantime (a tool's `mkdir -p config.d && write`).
            watcher.watch(&config_dirs)?;
            match crate::config::Config::load_from(&path) {
                // A failed reload keeps the running configuration: a tool
                // writing a file has to be able to get it wrong without the
                // screen going blank. (niri's `ConfigLoaded { failed: true }`.)
                Err(err) => log(&format!("config reload failed, keeping current: {err}")),
                Ok(reloaded) => {
                    // What the daemon runs with is resolved *per output*: a
                    // `[outputs."<name>"]` table overrides the global keys, and a
                    // reload has to resolve the same way startup does. Reading the
                    // globals here dropped a per-output wallpaper — and its scale and
                    // spans — the moment anything touched the configuration, which is
                    // exactly what a panel writes to (`config.d/noctalia.toml`).
                    let params = reloaded.output(&options.output);
                    // `fit` changes how the media is placed and `scale` changes
                    // the canvas it is built at, so both need the same reload the
                    // wallpaper path gets. Leaving `scale` out made the schema
                    // advertise it as hot while only half of it applied: the
                    // animators moved to the new scale, the texture did not.
                    let scale_changed = params.scale != options.scale;
                    let reload_media = params.wallpaper != options.wallpaper
                        || params.fit != options.fit
                        || scale_changed;
                    if reloaded.namespace != options.namespace {
                        log("namespace changed: needs a restart to take effect");
                    }
                    if reloaded.video_fps != options.video_fps {
                        log("video_fps changed: applies to the next video load");
                    }
                    if scale_changed {
                        // The media texture is built at the canvas size, so a new
                        // scale only takes effect once that is rebuilt — which is
                        // what the `reload_media` block below does.
                        canvas = canvas_size(screen, params.scale);
                        log(&format!(
                            "canvas → {}×{} (scale {:.3})",
                            canvas.0, canvas.1, params.scale
                        ));
                    }
                    options.scale = params.scale;
                    options.column_span = params.column_span;
                    options.workspace_span = params.workspace_span;
                    options.video_fps = reloaded.video_fps;
                    options.animations = reloaded.animations.clone();
                    options.transition = reloaded.transition.clone();
                    // The transition owns its own settings, so a reload has to
                    // rebuild it — the same way the animators above are rebuilt
                    // rather than poked. Without this, editing `[transition]`
                    // while the daemon runs silently does nothing, which is
                    // exactly what a panel writes to.
                    transition =
                        Transition::new(options.transition.clone(), options.animations.slowdown);
                    options.wallpaper = params.wallpaper;
                    options.namespace = reloaded.namespace.clone();
                    options.fit = params.fit;
                    niri.motion
                        .set_spans(options.column_span, options.workspace_span);
                    // Rebuild the animators at their current position so a
                    // changed curve or duration applies without a jump.
                    let now = Instant::now();
                    let position = animator.position(now);
                    let target = animator.target();
                    animator = Animator::new(position, options.animations.parallax)
                        .with_slowdown(options.animations.slowdown);
                    animator.retarget(target, now);
                    let zoom_position = zoom.position(now);
                    let zoom_target = zoom.target();
                    zoom = Animator::new(
                        zoom_position,
                        options.animations.overview_open_close.animation,
                    )
                    .with_slowdown(options.animations.slowdown);
                    zoom.retarget(zoom_target, now);
                    if reload_media {
                        if let Some(path) = options.wallpaper.clone() {
                            match switch_wallpaper(
                                &path,
                                canvas,
                                options.video_fps,
                                options.fit,
                                &mut media,
                                &mut transition,
                                &pool,
                                last_slot,
                                screen,
                            ) {
                                Ok(()) => {}
                                Err(err) => log(&format!("reloaded wallpaper: {err}")),
                            }
                        } else {
                            media = None;
                            log("wallpaper cleared");
                        }
                    }
                    log(&format!("config reloaded from {}", path.display()));
                }
            }
        }

        // Commands from the supervisor. This worker's output is already decided,
        // so a request never has to name it.
        for command in commands.try_iter() {
            match command {
                Command::Set(path) => {
                    // Load first, swap after: a bad path must leave the current
                    // wallpaper alone, not blank the screen.
                    match switch_wallpaper(
                        &path,
                        canvas,
                        options.video_fps,
                        options.fit,
                        &mut media,
                        &mut transition,
                        &pool,
                        last_slot,
                        screen,
                    ) {
                        Ok(()) => {}
                        Err(err) => log(&format!("set {}: {err}", path.display())),
                    }
                }
                Command::Query(reply) => {
                    let (progress, _) = animator.sample(Instant::now());
                    let kind = match &media {
                        Some(media) => media.describe(),
                        None => format!("pattern {:?}", options.pattern),
                    };
                    let _ = reply.send(format!(
                        "ok {kind} h={:.4} v={:.4}",
                        progress.horizontal, progress.vertical
                    ));
                }
                Command::State(reply) => {
                    let now = Instant::now();
                    let (progress, _) = animator.sample(now);
                    let _ = reply.send(state_json(&Snapshot {
                        options: &options,
                        media: media.as_ref(),
                        canvas,
                        position: (progress.horizontal, progress.vertical),
                        zoom: zoom.position(now),
                    }));
                }
            }
        }

        if fds[0].revents & libc::POLLIN != 0 {
            client.wait_events(Duration::ZERO)?;
        }

        let surface_frames = client
            .state
            .frames_done
            .get(&surface.id())
            .copied()
            .unwrap_or(0);
        if surface_frames > seen_frames {
            seen_frames = surface_frames;
            let now = Instant::now();
            let dt = now.saturating_duration_since(last_frame_at);
            last_frame_at = now;
            let (progress, moving) = animator.sample(now);
            let (zoom_now, zoom_moving) = zoom.sample(now);
            let blend = transition.sample(now);
            let moving = moving || zoom_moving || video_present || blend.is_some();
            let submitted_slot = Scene {
                view: Scene::view(progress, zoom_now, screen, options.scale, options.pattern),
                content: match &media {
                    Some(media) => media.content(),
                    None => gl::Content::Pattern(options.pattern),
                },
                blend,
            }
            .draw(&mut client, &mut pool, &surface, &renderer, moving)?;
            drawn += u64::from(submitted_slot.is_some());
            if submitted_slot.is_none() {
                skipped += 1;
            } else {
                last_slot = submitted_slot;
                // The decoded frame is on screen now; the next one has to wait
                // for mpv to say so.
                video_present = false;
            }
            if options.trace {
                log(&format!(
                    "frame {drawn:4} h={:.4} v={:.4} zoom={zoom_now:.4} effect={} t={:.3} dt={:>5.1}ms moving={moving} submitted={} in_flight={} released={} skipped={skipped}",
                    progress.horizontal,
                    progress.vertical,
                    transition.effect().name(),
                    blend.map(|blend| blend.progress).unwrap_or(1.0),
                    dt.as_secs_f64() * 1000.0,
                    submitted_slot.is_some(),
                    pool.in_flight(),
                    client.state.releases,
                ));
            }
            // Nothing submitted means the pool was empty: leave `frame_pending`
            // clear so the tail asks for another frame instead of stalling.
            frame_pending = submitted_slot.is_some() && moving;
        }

        // Anything still animating but no frame pending: kick it with a commit
        // that carries only the frame request. Both the parallax and the
        // overview zoom go through here, so neither can stall.
        if (animator.is_moving() || zoom.is_moving() || transition.is_moving() || video_present)
            && !frame_pending
        {
            surface.request_frame(&client.handle());
            surface.surface.commit();
            client.flush()?;
            frame_pending = true;
        }
    }

    if let Some(note) = exit_note {
        log(&note);
    }
    log(&format!(
        "stopping after {drawn} frame(s) ({skipped} skipped), {} release(s), {} ignored event(s), {} buffer(s) in flight",
        client.state.releases,
        niri.ignored,
        pool.in_flight()
    ));
    surface.destroy();
    client.flush()?;
    Ok(())
}

/// One line of JSON. The protocol is one line in, one line out, so the compact
/// form is the only one that fits.
fn json_line(value: &serde_json::Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|e| format!("error serializing: {e}"))
}

/// What `state` reports, gathered by the caller because only it has these.
struct Snapshot<'a> {
    options: &'a Options,
    media: Option<&'a Media>,
    canvas: (u32, u32),
    position: (f64, f64),
    zoom: f64,
}

/// The daemon's state, for `state` over the control socket.
///
/// Values are keyed by the same dotted names `schema` lists, so a client can zip
/// the two together without knowing anything about either. `config.values` is
/// what the configuration says; `wallpaper.path` is what is *actually* on
/// screen — `set` swaps the media without touching the configuration, so the two
/// can legitimately differ.
fn state_json(state: &Snapshot<'_>) -> serde_json::Value {
    use serde_json::{json, Map, Value};

    let options = state.options;
    let mut values = Map::new();
    let mut put = |name: &str, value: Value| values.insert(name.to_owned(), value);
    put(
        "wallpaper",
        json!(options
            .wallpaper
            .as_ref()
            .map(|path| path.display().to_string())),
    );
    put("video_fps", json!(options.video_fps));
    put("scale", json!(options.scale));
    put("column_span", json!(options.column_span));
    put("workspace_span", json!(options.workspace_span));
    put("namespace", json!(options.namespace));
    put("fit", json!(options.fit.name()));
    let animations = &options.animations;
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
    let transition = &options.transition;
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

    // Which files this configuration is made of: the main one plus every
    // `config.d/*.toml` merged over it. A panel showing "where does this value
    // come from" needs exactly this list.
    let files: Vec<String> = options
        .config_path
        .as_deref()
        .and_then(|path| crate::config::Config::config_files(path).ok())
        .unwrap_or_default()
        .iter()
        .map(|path| path.display().to_string())
        .collect();

    json!({
        "output": options.output,
        "namespace": options.namespace,
        "canvas": { "width": state.canvas.0, "height": state.canvas.1 },
        "config": {
            "source": crate::config::describe_source(options.config_path.as_deref()),
            "files": files,
            "values": values,
            "from_niri": animations.from_niri,
        },
        "wallpaper": match state.media {
            Some(media) => json!({
                "path": media.path().display().to_string(),
                "kind": media.kind(),
                "hwdec": media.hwdec(),
                // The *effective* fit: a video cannot be tiled, so `tile` shows
                // up here as `fill` even when the configuration says otherwise.
                "fit": media.fit().name(),
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

/// The canvas: the output enlarged by `scale` (§4.1). A wallpaper is fitted to
/// exactly this, so the shader samples it 1:1.
///
/// `round`, not `ceil`: 1440 × 1.1 is 1584 exactly in decimal but
/// 1584.0000000000002 in binary, and `ceil` would hand the shader a canvas one
/// pixel too tall — which shifts the whole vertical travel by a pixel.
fn canvas_size(screen: (f64, f64), scale: f64) -> (u32, u32) {
    (
        (screen.0 * scale).round() as u32,
        (screen.1 * scale).round() as u32,
    )
}

/// Swap the wallpaper, cross-fading from whatever is on screen.
///
/// Shared by `set` over the control socket and by a config reload — both mean
/// "this is the wallpaper now", and both must behave the same way: load first,
/// swap after, so a bad path leaves the current one alone.
#[allow(clippy::too_many_arguments)]
fn switch_wallpaper(
    path: &std::path::Path,
    canvas: (u32, u32),
    video_fps: u32,
    fit: Fit,
    media: &mut Option<Media>,
    transition: &mut Transition,
    pool: &Pool<'_>,
    last_slot: Option<usize>,
    screen: (f64, f64),
) -> Result<(), String> {
    let loaded = Media::load(path, canvas, video_fps, fit)?;
    *media = Some(loaded);
    // Snapshot *before* the swap: that is what is on screen right now.
    // `Frame::begin` only binds the framebuffer and sets the viewport, so it
    // does not disturb the pixels being copied.
    if let Some(slot) = last_slot {
        transition.restart(
            (screen.0 as u32, screen.1 as u32),
            Instant::now(),
            Some(&|snapshot: &gl::Snapshot| {
                pool.frame(slot).begin();
                snapshot.capture();
            }),
        );
    }
    // Say which effect ran: "the transition did something odd" is otherwise
    // impossible to pin on one of ten.
    log(&format!(
        "wallpaper → {} ({}, {} transition)",
        path.display(),
        media
            .as_ref()
            .map(|media| media.fit().name())
            .unwrap_or("?"),
        transition.effect().name()
    ));

    // Dropping the old media frees a lot — an mpv context plus its decoder — but
    // glibc keeps freed memory in its per-thread arenas rather than returning it
    // to the kernel, and mpv runs enough threads that each new context tends to
    // land in fresh arenas. Measured before this line existed: **~90 MB per
    // wallpaper change, monotonically** (five changes took a 4K daemon from
    // 233 MB to 621 MB of anonymous memory). `malloc_trim` hands the free arenas
    // back. It is a hint, and it costs a few milliseconds, on an event that
    // happens when a person changes their wallpaper.
    #[cfg(target_env = "gnu")]
    unsafe {
        libc::malloc_trim(0);
    }
    Ok(())
}

/// `log`, for modules that are not the daemon (the watcher reports what it picks
/// up, and that report is how "why did my edit do nothing" gets answered).
pub(crate) fn log_public(message: &str) {
    log(message);
}

fn log(message: &str) {
    // Flush every line. When stdout is a file — which is exactly the autostart
    // case — Rust block-buffers it, so a crash or a SIGTERM would take the whole
    // log with it. A startup that fails silently is worse than a slow log.
    use std::io::Write;
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "niripaper: {message}");
    let _ = out.flush();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canvas_rounds_instead_of_ceiling() {
        // 1440 × 1.1 is 1584 exactly in decimal but 1584.0000000000002 in
        // binary. `ceil` would make the canvas a pixel too tall, which shifts
        // the whole vertical travel by a pixel — and the canvas is exactly what
        // the wallpaper is fitted to, so the error would be in the picture.
        assert_eq!(canvas_size((2560.0, 1440.0), 1.1), (2816, 1584));
        assert_eq!(canvas_size((2560.0, 1440.0), 1.0), (2560, 1440));
    }

    /// A panel zips `schema` and `state` together by dotted key name. That only
    /// works if the two agree, and they are written in two different files — so
    /// a key added to one and forgotten in the other is a silent hole in the
    /// panel's UI. This is the test that catches it.
    #[test]
    fn state_reports_every_key_the_schema_lists() {
        let options = Options::new("test");
        let state = state_json(&Snapshot {
            options: &options,
            media: None,
            canvas: (1920, 1080),
            position: (0.0, 0.0),
            zoom: 1.0,
        });
        let values = state["config"]["values"]
            .as_object()
            .expect("values is an object");
        for key in crate::schema::keys() {
            assert!(
                values.contains_key(key.name),
                "schema lists {:?} but state does not report it",
                key.name
            );
        }
    }
}
