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

use std::time::{Duration, Instant};

use crate::media::{self, Media};
use crate::motion::{DEFAULT_COLUMN_SPAN, DEFAULT_WORKSPACE_SPAN};
use crate::niri::Niri;
use crate::render::anim::Animator;
use crate::render::egl::{Egl, EglVendor};
use crate::render::gbm;
use crate::render::gl::{self, Pattern};
use crate::render::layer::{self, Pool};
use crate::render::transition::Transition;
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
            trace: false,
        }
    }
}

/// What is being drawn: a decoded still, or a playing video. Both end up
/// canvas-sized and are sampled identically — the only difference is that the
/// video's texture is redrawn by libmpv as it plays.
pub fn run(options: &Options, running: &dyn Fn() -> bool) -> Result<(), String> {
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
        "{}x{} {} via {} [{}] ({})",
        surface.width,
        surface.height,
        gbm::fourcc_name(FORMAT),
        egl.vendor,
        egl.platform,
        layer::describe_modifiers(pool.chosen())
    ));
    for (modifiers, err) in pool.failures() {
        log(&format!(
            "rejected {}: {err}",
            layer::describe_modifiers(modifiers)
        ));
    }

    // The canvas is the output enlarged by `scale` (§4.1); a wallpaper is
    // fitted to exactly that, so the shader samples it 1:1.
    // `round`, not `ceil`: 1440 × 1.1 is 1584 exactly in decimal but
    // 1584.0000000000002 in binary, and `ceil` would hand the shader a canvas
    // one pixel too tall — which shifts the whole vertical travel by a pixel.
    let canvas = (
        (surface.width as f64 * options.scale).round() as u32,
        (surface.height as f64 * options.scale).round() as u32,
    );
    let mut media = match &options.wallpaper {
        Some(path) => {
            let loaded = Media::load(path, canvas, options.video_fps)?;
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
    let screen = (surface.width as f64, surface.height as f64);
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
    let mut seen_frames = client.state.frames_done;
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
    // The control socket (§3). Bound here, after everything that can fail at
    // startup has already failed: a socket that exists means a daemon that works.
    let socket_path = match &options.socket {
        Some(path) => path.clone(),
        None => crate::ipc::default_path(&options.output)?,
    };
    let ipc = crate::ipc::Server::bind(&socket_path)?;
    log(&format!("control socket {}", socket_path.display()));
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
            fd: ipc.fd(),
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
    let mut quit = false;
    while running() && !quit {
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
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
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
        if fds[4].revents & libc::POLLIN != 0 && watcher.drain() {
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
                    let changed_wallpaper = reloaded.wallpaper != options.wallpaper;
                    if reloaded.namespace != options.namespace {
                        log("namespace changed: needs a restart to take effect");
                    }
                    if reloaded.video_fps != options.video_fps {
                        log("video_fps changed: applies to the next video load");
                    }
                    options.scale = reloaded.scale;
                    options.column_span = reloaded.column_span;
                    options.workspace_span = reloaded.workspace_span;
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
                    options.wallpaper = reloaded.wallpaper.clone();
                    options.namespace = reloaded.namespace.clone();
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
                    if changed_wallpaper {
                        if let Some(path) = options.wallpaper.clone() {
                            match switch_wallpaper(
                                &path,
                                canvas,
                                options.video_fps,
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

        if fds[3].revents & libc::POLLIN != 0 {
            match ipc.accept() {
                Ok((request, stream)) => match request {
                    crate::ipc::Request::Set(path) => {
                        // Load first, swap after: a bad path must leave the
                        // current wallpaper alone, not blank the screen.
                        match switch_wallpaper(
                            &path,
                            canvas,
                            options.video_fps,
                            &mut media,
                            &mut transition,
                            &pool,
                            last_slot,
                            screen,
                        ) {
                            Ok(()) => crate::ipc::reply(&stream, "ok"),
                            Err(err) => {
                                crate::ipc::reply(&stream, &format!("error {err}"));
                                log(&format!("set {}: {err}", path.display()));
                            }
                        }
                    }
                    crate::ipc::Request::Query => {
                        let (progress, _) = animator.sample(Instant::now());
                        let kind = match &media {
                            Some(media) => media.describe(),
                            None => format!("pattern {:?}", options.pattern),
                        };
                        crate::ipc::reply(
                            &stream,
                            &format!(
                                "ok {kind} h={:.4} v={:.4}",
                                progress.horizontal, progress.vertical
                            ),
                        );
                    }
                    crate::ipc::Request::Schema => {
                        crate::ipc::reply(&stream, &json_line(&crate::schema::schema()));
                    }
                    crate::ipc::Request::State => {
                        let now = Instant::now();
                        let (progress, _) = animator.sample(now);
                        crate::ipc::reply(
                            &stream,
                            &json_line(&state_json(&Snapshot {
                                options: &options,
                                media: media.as_ref(),
                                canvas,
                                position: (progress.horizontal, progress.vertical),
                                zoom: zoom.position(now),
                            })),
                        );
                    }
                    crate::ipc::Request::Kill => {
                        crate::ipc::reply(&stream, "ok");
                        log("kill requested");
                        quit = true;
                    }
                },
                Err(err) => log(&format!("control socket: {err}")),
            }
        }

        if fds[0].revents & libc::POLLIN != 0 {
            client.wait_events(Duration::ZERO)?;
        }

        if client.state.frames_done > seen_frames {
            seen_frames = client.state.frames_done;
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
    put("transition.cell", json!(transition.cell));
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
    media: &mut Option<Media>,
    transition: &mut Transition,
    pool: &Pool<'_>,
    last_slot: Option<usize>,
    screen: (f64, f64),
) -> Result<(), String> {
    let loaded = Media::load(path, canvas, video_fps)?;
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
        "wallpaper → {} ({} transition)",
        path.display(),
        transition.effect().name()
    ));
    Ok(())
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
