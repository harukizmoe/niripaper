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
use niripaper::render::gl::{Blend, Content, Pattern, Renderer, Snapshot, View};
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
    let frame = Frame::new(
        &context,
        &device,
        width,
        height,
        gbm::FORMAT_XRGB8888,
        &[gbm::MOD_LINEAR],
    )?;
    let snapshot = Snapshot::new(width, height);

    // The "before" frame: one pattern, so the transition's mask is obvious.
    let view = View::flat((width as f32, height as f32), Pattern::Bands);
    frame.begin();
    renderer.draw(view, Content::Pattern(Pattern::Bands), None);
    snapshot.capture();

    let read_pixels: extern "C" fn(i32, i32, i32, i32, u32, u32, *mut c_void) =
        unsafe { std::mem::transmute(egl::get_proc_address("glReadPixels")) };

    std::fs::create_dir_all(&out).map_err(|e| format!("{out}: {e}"))?;
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
            renderer.draw(view, Content::Pattern(Pattern::Blocks), Some(&blend));
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
            // GL hands back bottom-up rows.
            let row = width as usize * 4;
            let mut flipped = vec![0u8; pixels.len()];
            for y in 0..height as usize {
                flipped[y * row..(y + 1) * row].copy_from_slice(
                    &pixels[(height as usize - 1 - y) * row..(height as usize - y) * row],
                );
            }
            let path = format!("{out}/{label}-{}.png", (progress * 100.0) as u32);
            image::save_buffer(
                &path,
                &flipped,
                width,
                height,
                image::ExtendedColorType::Rgba8,
            )
            .map_err(|e| format!("{path}: {e}"))?;
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
