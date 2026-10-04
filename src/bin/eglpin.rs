//! Which GPUs can this process actually render on, and what can it produce?
//!
//! This started as the M0a probe ("can an EGL context be bound to one chosen
//! GPU?") with its own inline FFI. It now exercises the real renderer modules,
//! so the answer it prints is the answer the daemon will get.
//!
//! ```text
//! eglpin [RENDER_NODE...]      # default: every /dev/dri/renderD*
//! ```
//!
//! For each node it reports the DRM card and PCI vendor, whether a GBM device
//! and EGL display come up (using the GLVND vendor that owns that vendor id),
//! the GL strings, the dmabuf modifiers the driver can produce, and whether a
//! LINEAR buffer is usable as a render target.
//!
//! Note: GLVND reads `__EGL_VENDOR_LIBRARY_FILENAMES` once per process, so only
//! one EGL vendor can be exercised per run. Nodes belonging to another vendor
//! are reported from sysfs only.

use std::path::PathBuf;

use niripaper::gpu;
use niripaper::render::egl::{Egl, EglVendor};
use niripaper::render::{dmabuf::Frame, gbm, gl};

fn main() {
    let explicit: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let nodes = if explicit.is_empty() {
        match gpu::render_nodes() {
            Ok(nodes) => nodes,
            Err(err) => {
                eprintln!("eglpin: {err}");
                std::process::exit(1);
            }
        }
    } else {
        explicit
    };

    let mut pinned: Option<EglVendor> = None;
    for node in nodes {
        println!("== {} ==", node.display());
        let gpu = match gpu::gpu_for_node(&node) {
            Ok(gpu) => gpu,
            Err(err) => {
                println!("  FAILED: {err}");
                continue;
            }
        };
        println!(
            "  {} {}, vendor 0x{:04x}",
            gpu.card,
            gpu.vendor_name(),
            gpu.vendor_id
        );

        let vendor = EglVendor::for_pci_vendor(gpu.vendor_id);
        if pinned.is_some_and(|p| p != vendor) {
            println!("  skipped: GLVND already pinned to {pinned:?} in this process");
            continue;
        }

        match probe(&node, vendor) {
            Ok(report) => println!("{report}"),
            Err(err) => println!("  FAILED: {err}"),
        }
        pinned = Some(vendor);
    }
}

fn probe(node: &std::path::Path, vendor: EglVendor) -> Result<String, String> {
    let mut out = String::new();
    let device = gbm::Device::open(node)?;
    let egl = Egl::new(device.as_ptr(), device.fd(), vendor)?;
    out.push_str(&format!(
        "  EGL {} [{}] via {} (vendor {})\n",
        egl.version, egl.vendor, egl.platform, egl.vendor_pinned
    ));

    let renderer = gl::Renderer::new()?;
    out.push_str(&format!(
        "  GL  {} | {} | {}\n",
        gl::string(gl::GL_VERSION),
        gl::string(gl::GL_RENDERER),
        gl::string(gl::GL_VENDOR),
    ));
    drop(renderer);

    let format = gbm::FORMAT_XRGB8888;
    let modifiers = egl.dmabuf_modifiers(format);
    let renderable: Vec<u64> = modifiers
        .iter()
        .filter(|(_, external_only)| !external_only)
        .map(|(m, _)| *m)
        .collect();
    out.push_str(&format!(
        "  dmabuf {}: {} modifier(s), {} renderable\n",
        gbm::fourcc_name(format),
        modifiers.len(),
        renderable.len()
    ));

    // The decisive question: can this GPU hand us a buffer we can draw into?
    let linear = [gbm::MOD_LINEAR];
    match Frame::new(&egl, &device, 64, 64, format, &linear) {
        Ok(frame) => {
            out.push_str(&format!(
                "  LINEAR render target: ok ({})\n",
                frame.describe()
            ));
        }
        Err(err) => out.push_str(&format!("  LINEAR render target: {err}\n")),
    }
    if let Some(first) = renderable.first() {
        match Frame::new(&egl, &device, 64, 64, format, &[*first]) {
            Ok(frame) => out.push_str(&format!(
                "  {} render target: ok ({})\n",
                gbm::modifier_name(*first),
                frame.describe()
            )),
            Err(err) => out.push_str(&format!(
                "  {} render target: {err}\n",
                gbm::modifier_name(*first)
            )),
        }
    }
    Ok(out)
}
