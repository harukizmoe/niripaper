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

use crate::motion::{offset_px, DEFAULT_SPAN};
use crate::niri::Niri;
use crate::render::anim::Animator;
use crate::render::egl::{Egl, EglVendor};
use crate::render::gbm;
use crate::render::gl::{self, Pattern};
use crate::render::layer::{self, LayerSurface, Pool};
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
    pub span: usize,
    pub pattern: Pattern,
    /// A static wallpaper to draw instead of the procedural pattern.
    pub wallpaper: Option<std::path::PathBuf>,
    /// Animation parameters, in niri's vocabulary (see `config.rs`).
    pub animations: crate::config::Animations,
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
            span: DEFAULT_SPAN,
            pattern: Pattern::Blocks,
            wallpaper: None,
            animations: crate::config::Animations::default(),
            trace: false,
        }
    }
}

pub fn run(options: &Options, running: &dyn Fn() -> bool) -> Result<(), String> {
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
        "{} on {} ({} {}), namespace {}, scale {:.3}, span {}, pattern {:?}\n\
         animations: parallax {}, overview-open-close {} (zoom {:.3}), slowdown {}{}",
        options.output,
        node.display(),
        gpu.card,
        gpu.vendor_name(),
        options.namespace,
        options.scale,
        options.span,
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
    let wallpaper = match &options.wallpaper {
        Some(path) => {
            let loaded = crate::render::image::Wallpaper::load(path, canvas)?;
            log(&format!("wallpaper {}", loaded.describe()));
            Some(loaded)
        }
        None => None,
    };

    // --- niri --------------------------------------------------------------
    let mut niri = Niri::connect()?;
    niri.wait_for_full_state()?;
    let screen = (surface.width as f64, surface.height as f64);
    let animations = &options.animations;
    let mut animator = Animator::new(niri.motion.progress(&options.output), animations.parallax)
        .with_slowdown(animations.slowdown);
    // The overview transition animates the canvas scale (§4.1): pulling back
    // shows more of the wallpaper, which reads as the workspace receding.
    let mut zoom = Animator::new(1.0f64, animations.overview_open_close.animation)
        .with_slowdown(animations.slowdown);
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
    draw(
        &mut client,
        &mut pool,
        &surface,
        &renderer,
        animator.target(),
        1.0,
        screen,
        options,
        wallpaper.as_ref(),
        false,
    )?;
    drawn += 1;

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
    ];

    log(&format!(
        "polling wayland fd {} and niri fd {}",
        fds[0].fd, fds[1].fd
    ));
    let mut exit_note = None;
    while running() {
        fds[0].revents = 0;
        fds[1].revents = 0;
        let ready = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, -1) };
        if ready < 0 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue; // a signal: check `running()`
            }
            return Err(format!("poll: {err}"));
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
                animations.overview_open_close.zoom
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
            let moving = moving || zoom_moving;
            let submitted = draw(
                &mut client,
                &mut pool,
                &surface,
                &renderer,
                progress,
                zoom_now,
                screen,
                options,
                wallpaper.as_ref(),
                moving,
            )?;
            drawn += u64::from(submitted);
            if !submitted {
                skipped += 1;
            }
            if options.trace {
                log(&format!(
                    "frame {drawn:4} h={:.4} v={:.4} zoom={zoom_now:.4} dt={:>5.1}ms moving={moving} submitted={submitted} in_flight={} released={} skipped={skipped}",
                    progress.horizontal,
                    progress.vertical,
                    dt.as_secs_f64() * 1000.0,
                    pool.in_flight(),
                    client.state.releases,
                ));
            }
            // Nothing submitted means the pool was empty: leave `frame_pending`
            // clear so the tail asks for another frame instead of stalling.
            frame_pending = submitted && moving;
        }

        // Anything still animating but no frame pending: kick it with a commit
        // that carries only the frame request. Both the parallax and the
        // overview zoom go through here, so neither can stall.
        if (animator.is_moving() || zoom.is_moving()) && !frame_pending {
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

/// Draw one frame of the parallax pattern.
#[allow(clippy::too_many_arguments)]
fn draw(
    client: &mut layer::Client,
    pool: &mut Pool<'_>,
    surface: &LayerSurface,
    renderer: &gl::Renderer,
    progress: motion::Progress,
    zoom: f64,
    screen: (f64, f64),
    options: &Options,
    wallpaper: Option<&crate::render::image::Wallpaper>,
    want_next_frame: bool,
) -> Result<bool, String> {
    let released = std::mem::take(&mut client.state.released);
    // Every buffer still on screen: the compositor is behind us. Dropping a
    // frame is the right answer — queueing them up would just add latency, and
    // the caller will keep the animation alive with a bare frame request.
    let Some(slot) = pool.acquire(&released) else {
        return Ok(false);
    };

    // The zoom is just an animated multiplier on `scale`: the canvas, the
    // overflow and therefore the parallax travel all follow from it, and the
    // wallpaper texture (already canvas-sized) is sampled as a sub-region —
    // nothing to re-upload.
    let scale = (options.scale * zoom).max(1.0);
    let offset = offset_px(progress, screen, scale);
    let frame = pool.frame(slot);
    frame.begin();
    let content = match wallpaper {
        Some(wallpaper) => gl::Content::Wallpaper(&wallpaper.texture),
        None => gl::Content::Pattern(options.pattern),
    };
    renderer.draw(
        gl::View {
            screen: (screen.0 as f32, screen.1 as f32),
            scale: scale as f32,
            offset: (offset.0 as f32, offset.1 as f32),
            pattern: options.pattern,
        },
        content,
    );
    frame.finish();
    if let Some(err) = gl::last_error() {
        return Err(format!("GL error after drawing: 0x{err:x}"));
    }

    // The frame request travels with this commit; on its own it would be
    // dropped.
    let _callback = want_next_frame.then(|| surface.request_frame(&client.handle()));
    let buffer = client.attach(surface, frame);
    pool.mark_submitted(slot, &buffer);
    client.flush()?;
    Ok(true)
}

fn log(message: &str) {
    println!("niripaper: {message}");
}
