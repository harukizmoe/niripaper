//! Probe: can libmpv decode — and **hardware-accelerate** — a video on a given
//! GPU, and draw a frame into a GL texture of ours? (`HANDOFF.md` §2/§3, V2)
//!
//! ```sh
//! cargo build --release
//! ./target/release/vidpin --video ~/Pictures/Wallpapers/video/anime_rain.mp4 \
//!     --node /dev/dri/renderD128 --fps 25 --out /tmp/frame.png --seconds 5
//! ```
//!
//! Prints the GPU and GL vendor it landed on, what mpv is *actually* decoding
//! with (`hwdec-current` — `hwdec=auto-safe` is only a request), the video's own
//! size next to the render target's, and the frame timing. Exits non-zero when
//! nothing was drawn, so it can be used as a smoke test.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use niripaper::gpu;
use niripaper::render::egl::{Egl, EglVendor};
use niripaper::render::gbm::Device as GbmDevice;
use niripaper::render::gl;
use niripaper::render::video::Video;
use niripaper::render::Fit;

struct Args {
    video: PathBuf,
    node: Option<PathBuf>,
    fps: u32,
    width: u32,
    height: u32,
    out: Option<PathBuf>,
    seconds: f64,
    fit: Fit,
}

fn parse(args: &[String]) -> Result<Args, String> {
    let mut parsed = Args {
        video: PathBuf::new(),
        node: None,
        fps: 0,
        width: 2560,
        height: 1440,
        out: None,
        seconds: 5.0,
        fit: Fit::Fill,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = || iter.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--video" => parsed.video = PathBuf::from(value()?),
            "--node" => parsed.node = Some(PathBuf::from(value()?)),
            "--fps" => {
                parsed.fps = value()?.parse().map_err(|e| format!("--fps: {e}"))?;
            }
            "--width" => {
                parsed.width = value()?.parse().map_err(|e| format!("--width: {e}"))?;
            }
            "--height" => {
                parsed.height = value()?.parse().map_err(|e| format!("--height: {e}"))?;
            }
            "--fit" => parsed.fit = Fit::parse(value()?)?,
            "--out" => parsed.out = Some(PathBuf::from(value()?)),
            "--seconds" => {
                parsed.seconds = value()?.parse().map_err(|e| format!("--seconds: {e}"))?;
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    if parsed.video.as_os_str().is_empty() {
        return Err("--video PATH is required".to_owned());
    }
    Ok(parsed)
}

fn main() -> Result<(), String> {
    let args = parse(&std::env::args().skip(1).collect::<Vec<_>>())?;

    let node = match &args.node {
        Some(node) => node.clone(),
        None => gpu::render_nodes()
            .map_err(|e| e.to_string())?
            .into_iter()
            .next()
            .ok_or("no /dev/dri/renderD* nodes")?,
    };
    let gpu = gpu::gpu_for_node(&node)?;
    println!(
        "node {} ({} {}, vendor 0x{:04x})",
        node.display(),
        gpu.card,
        gpu.vendor_name(),
        gpu.vendor_id
    );

    let device = GbmDevice::open(&node)?;
    let mut egl = Egl::new(
        device.as_ptr(),
        device.fd(),
        EglVendor::for_pci_vendor(gpu.vendor_id),
    )?;
    egl.make_current()?;
    println!(
        "EGL {} / GL {} / {}",
        egl.version,
        gl::string(gl::GL_VERSION),
        gl::string(gl::GL_RENDERER)
    );

    // Same resolution the daemon does: a video cannot be tiled.
    let fit = args.fit.for_video();
    let mut video = Video::new(&args.video, args.width, args.height, args.fps, fit)?;
    match video.video_size() {
        Some((w, h)) => println!(
            "video {} — source {}×{}, render target {}×{}, fps cap {}",
            args.video.display(),
            w,
            h,
            args.width,
            args.height,
            if args.fps == 0 {
                "source".to_owned()
            } else {
                args.fps.to_string()
            }
        ),
        None => println!("video {} — source size unknown yet", args.video.display()),
    }

    let deadline = Instant::now() + Duration::from_secs_f64(args.seconds);
    let started = Instant::now();
    let mut saved = false;
    while Instant::now() < deadline {
        if video.render()? && !saved && video.frames >= 10 {
            // A few frames in, so the decoder is warm. Readback is synchronous,
            // and mpv manages GL state — bind our FBO again before reading.
            if let Some(out) = &args.out {
                gl::bind_framebuffer(gl::GL_FRAMEBUFFER, video.fbo());
                let (width, height) = video.size();
                let rgb = gl::read_pixels_rgb(width, height);
                let image = image::RgbImage::from_raw(width, height, rgb)
                    .ok_or("readback size mismatch")?;
                // No flip. `glReadPixels` hands back row 0 first, and mpv's
                // render target has the image's top row there — the same
                // orientation `Texture::from_rgb` gives the still-image path, so
                // the shader samples both identically. (Flipping here was why
                // the first frame came out upside down.)
                image.save(out).map_err(|e| e.to_string())?;
                println!("wrote {} ({}×{})", out.display(), width, height);
            }
            saved = true;
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let elapsed = started.elapsed().as_secs_f64();

    let hwdec = video.hwdec();
    println!(
        "frames {} in {:.2}s ({:.1} fps) — hwdec-current {:?}",
        video.frames,
        elapsed,
        video.frames as f64 / elapsed.max(f64::MIN_POSITIVE),
        hwdec
    );
    if video.frames == 0 {
        return Err("no frames were drawn".to_owned());
    }
    if hwdec.is_empty() || hwdec == "no" {
        return Err(format!(
            "hardware decoding is off (hwdec-current {hwdec:?}) — that is a requirement, not a preference"
        ));
    }
    Ok(())
}
