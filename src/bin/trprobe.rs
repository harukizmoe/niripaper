//! Probe: render wallpaper-transition effects offscreen and write PNGs, so a
//! shader change can be looked at without taking over a screen.
//!
//! Not a user tool. Run it with a render node and an output directory:
//! `trprobe /dev/dri/renderD128 /tmp/tr`.
//!
//! It has caught three real bugs that unit tests cannot see (a radial effect
//! whose reach used the whole diagonal, a hexagonal tiling built from a sheared
//! rounding that produced parallelograms, and a texture missing
//! `GL_TEXTURE_MIN_FILTER`, which made every snapshot sample as black). Run it
//! after touching the shader.

use std::ffi::c_void;

use niripaper::gpu;
use niripaper::render::dmabuf::Frame;
use niripaper::render::egl::{self, Egl, EglVendor};
use niripaper::render::gbm;
use niripaper::render::gl::{Blend, Content, Pattern, Renderer, Snapshot, Texture, View};
use niripaper::render::transition::{Direction, Effect, Settings};

const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;

fn main() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let node = std::path::PathBuf::from(
        args.next()
            .unwrap_or_else(|| "/dev/dri/renderD128".to_owned()),
    );
    let out = args.next().unwrap_or_else(|| "/tmp/tr".to_owned());
    let (width, height) = (1280u32, 720u32);

    let gpu = gpu::gpu_for_node(&node)?;
    let device = gbm::Device::open(&node)?;
    let mut context = Egl::new(
        device.as_ptr(),
        device.fd(),
        EglVendor::for_pci_vendor(gpu.vendor_id),
    )?;
    context.make_current()?;
    println!("gl: {} [{}]", context.version, context.vendor);

    let renderer = Renderer::new()?;
    // LINEAR is not a render target on every driver (NVIDIA): ask for what the
    // driver can actually render into.
    let modifiers: Vec<u64> = context
        .dmabuf_modifiers(gbm::FORMAT_XRGB8888)
        .into_iter()
        .filter(|(_, external_only)| !external_only)
        .map(|(m, _)| m)
        .collect();
    let frame = Frame::new(
        &context,
        &device,
        width,
        height,
        gbm::FORMAT_XRGB8888,
        &modifiers,
    )?;
    let snapshot = Snapshot::new(width, height);

    // The "before" frame: one pattern, so the transition's mask is obvious.
    let view = View::flat((width as f32, height as f32), Pattern::Bands);
    let filter = niripaper::config::Filter::default();
    frame.begin();
    renderer.draw(view, Content::Pattern(Pattern::Bands), &filter, None);
    snapshot.capture();

    let read_pixels: extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void) =
        unsafe { std::mem::transmute(egl::get_proc_address("glReadPixels")) };

    // GL hands back bottom-up rows; write them top-down as a PNG.
    let save = |path: &str| -> Result<(), String> {
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        read_pixels(
            0,
            0,
            width as i32,
            height as i32,
            GL_RGBA,
            GL_UNSIGNED_BYTE,
            pixels.as_mut_ptr().cast(),
        );
        let row = width as usize * 4;
        let mut flipped = vec![0u8; pixels.len()];
        for y in 0..height as usize {
            flipped[y * row..(y + 1) * row].copy_from_slice(
                &pixels[(height as usize - 1 - y) * row..(height as usize - y) * row],
            );
        }
        image::save_buffer(
            path,
            &flipped,
            width,
            height,
            image::ExtendedColorType::Rgba8,
        )
        .map_err(|e| format!("{path}: {e}"))
    };

    std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;

    // `trprobe NODE OUT WALLPAPER`: render the wallpaper once per filter preset,
    // so a look can be judged on a real picture instead of on a test pattern.
    if let Some(wallpaper) = std::env::args().nth(3).filter(|a| a != "--sequence") {
        let img = image::open(&wallpaper)
            .map_err(|e| format!("{wallpaper}: {e}"))?
            .resize_to_fill(width, height, image::imageops::FilterType::Triangle)
            .to_rgb8();
        let texture = Texture::from_rgb(width, height, img.as_raw());
        for (id, _, preset) in niripaper::config::Filter::presets() {
            frame.begin();
            renderer.draw(view, Content::Wallpaper(&texture), &preset, None);
            let path = format!("{out}/filter-{id}.png");
            save(&path)?;
            println!("{path}");
        }
        return Ok(());
    }

    // `trprobe NODE OUT --sequence FROM TO [FRAMES]`: every effect carrying a real
    // picture FROM over to TO, as numbered frames (`<effect>-000.png` …). The page
    // in `site/` turns these into its gallery clips (scripts/gen-site-transitions.sh).
    // Progress follows the default ease-out-cubic, so the clips move like the daemon.
    if std::env::args().nth(3).as_deref() == Some("--sequence") {
        let load = |n: usize| -> Result<Texture, String> {
            let path = std::env::args()
                .nth(n)
                .ok_or("--sequence needs FROM and TO")?;
            let img = image::open(&path)
                .map_err(|e| format!("{path}: {e}"))?
                .resize_to_fill(width, height, image::imageops::FilterType::Triangle)
                .to_rgb8();
            // `save` flips the readback, which the daemon's own draw path does not
            // expect of a still: flip the input once so the clips come out upright.
            let img = image::imageops::flip_vertical(&img);
            Ok(Texture::from_rgb(width, height, img.as_raw()))
        };
        let (from, to) = (load(4)?, load(5)?);
        let count: u32 = match std::env::args().nth(6) {
            Some(n) => n.parse().map_err(|e| format!("FRAMES: {e}"))?,
            None => 24,
        };
        frame.begin();
        renderer.draw(view, Content::Wallpaper(&from), &filter, None);
        snapshot.capture();
        let settings = Settings {
            direction: Direction::Right,
            ..Settings::default()
        };
        for name in Effect::NAMES.iter().filter(|n| **n != "none") {
            let effect = Effect::parse(name)?;
            for i in 0..count {
                let t = i as f32 / (count - 1) as f32;
                let blend = Blend {
                    previous: snapshot.texture(),
                    effect: effect.index(),
                    progress: 1.0 - (1.0 - t).powi(3),
                    softness: settings.softness as f32,
                    center: (settings.center.0 as f32, settings.center.1 as f32),
                    direction: settings.direction.vector(),
                    stripes: settings.stripes as f32,
                    push: settings.push as f32,
                    start_radius: settings.start_radius as f32,
                };
                frame.begin();
                renderer.draw(view, Content::Wallpaper(&to), &filter, Some(&blend));
                save(&format!("{out}/{name}-{i:03}.png"))?;
            }
            println!("{out}/{name}-*.png");
        }
        return Ok(());
    }
    let shoot =
        |settings: &Settings, effect: Effect, progress: f32, label: &str| -> Result<(), String> {
            let blend = Blend {
                previous: snapshot.texture(),
                effect: effect.index(),
                progress,
                softness: settings.softness as f32,
                center: (settings.center.0 as f32, settings.center.1 as f32),
                direction: settings.direction.vector(),
                stripes: settings.stripes as f32,
                push: settings.push as f32,
                start_radius: settings.start_radius as f32,
            };
            frame.begin();
            renderer.draw(
                view,
                Content::Pattern(Pattern::Blocks),
                &filter,
                Some(&blend),
            );
            let path = format!("{out}/{label}-{}.png", (progress * 100.0) as u32);
            save(&path)?;
            println!("{path}");
            Ok(())
        };

    // Every effect at a few progress points. The parameters are the defaults,
    // except the direction, which is set explicitly so the shot shows the sweep
    // going the documented way.
    let settings = Settings {
        direction: Direction::Right,
        ..Settings::default()
    };
    for name in Effect::NAMES {
        let effect = Effect::parse(name)?;
        for progress in [0.35f32, 0.6, 1.0] {
            shoot(&settings, effect, progress, name)?;
        }
    }

    // A circle that does not start in the middle and starts as a complete circle
    // rather than a point: the pair a wallpaper is tuned with when it wants "the
    // new one appears over there". At t = 1 it still has to cover the far corner.
    let positioned = Settings {
        center: (0.2, 0.22),
        start_radius: 0.25,
        ..settings
    };
    for progress in [0.0f32, 0.3, 0.7, 1.0] {
        shoot(&positioned, Effect::Iris, progress, "positioned")?;
    }

    Ok(())
}
