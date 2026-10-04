//! A dmabuf that we render into and then hand to the compositor.
//!
//! This is the "self-managed swapchain" of `HANDOFF.md` §6: we allocate on the
//! GPU that drives the output, import the buffer into GL as a texture, render
//! into it through an FBO, and advertise it to the compositor with
//! `zwp_linux_dmabuf_v1`. No `wl_egl_window`, no `eglSwapBuffers`, and therefore
//! no way for the compositor's default GPU to sneak back in.

use std::os::fd::AsRawFd;

use super::egl::{DmaBufDesc, DmaBufPlane, Egl, EglImage};
use super::gbm::{self, Bo, Device, Plane};
use super::gl;

/// Search order for `(format, modifier)`: what both the compositor and the
/// driver accept first, then what only the compositor advertises (the driver
/// may still refuse), then "let the driver choose" — which the compositor may
/// then reject, but it is worth trying last.
pub fn modifier_candidates(advertised: &[u64], renderable: &[u64]) -> Vec<Vec<u64>> {
    let mut order: Vec<Vec<u64>> = advertised
        .iter()
        .filter(|m| renderable.contains(m))
        .map(|m| vec![*m])
        .collect();
    order.extend(
        advertised
            .iter()
            .filter(|m| !renderable.contains(m))
            .map(|m| vec![*m]),
    );
    order.push(Vec::new());
    order
}

pub struct Frame<'a> {
    egl: &'a Egl,
    pub bo: Bo,
    pub planes: Vec<Plane>,
    pub width: u32,
    pub height: u32,
    pub format: u32,
    pub modifier: u64,
    image: EglImage,
    pub texture: u32,
    pub fbo: u32,
}

impl<'a> Frame<'a> {
    /// Allocate a buffer and set it up as an FBO on `egl`'s context.
    ///
    /// `modifiers` comes from the compositor's dmabuf feedback; an empty slice
    /// means "no constraint" and lets the driver choose.
    pub fn new(
        egl: &'a Egl,
        device: &Device,
        width: u32,
        height: u32,
        format: u32,
        modifiers: &[u64],
    ) -> Result<Self, String> {
        let bo = device.allocate(width, height, format, modifiers)?;
        let planes = bo.planes()?;
        let desc = DmaBufDesc {
            width: bo.width(),
            height: bo.height(),
            fourcc: bo.format(),
            modifier: bo.modifier(),
            planes: planes
                .iter()
                .map(|p| DmaBufPlane {
                    fd: p.fd.as_raw_fd(),
                    offset: p.offset,
                    stride: p.stride,
                })
                .collect(),
        };
        let image = egl.import_dmabuf(&desc)?;

        let texture = gl::gen_texture();
        gl::bind_texture(gl::GL_TEXTURE_2D, texture);
        gl::egl_image_target_texture(gl::GL_TEXTURE_2D, image);
        let fbo = gl::gen_framebuffer();
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, fbo);
        gl::framebuffer_texture_2d(
            gl::GL_FRAMEBUFFER,
            gl::GL_COLOR_ATTACHMENT0,
            gl::GL_TEXTURE_2D,
            texture,
        );
        let status = gl::framebuffer_status();
        if status != gl::GL_FRAMEBUFFER_COMPLETE {
            gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);
            gl::delete_framebuffer(fbo);
            gl::delete_texture(texture);
            egl.destroy_image(image);
            return Err(format!(
                "framebuffer incomplete (0x{status:x}); {} is not a render target here",
                gbm::modifier_name(bo.modifier())
            ));
        }
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);

        Ok(Self {
            egl,
            width: desc.width,
            height: desc.height,
            format: desc.fourcc,
            modifier: desc.modifier,
            bo,
            planes,
            image,
            texture,
            fbo,
        })
    }

    /// Bind the framebuffer and set the viewport to the whole buffer.
    pub fn begin(&self) {
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, self.fbo);
        gl::viewport(self.width, self.height);
    }

    /// Wait until the GPU is done writing, so a compositor without an explicit
    /// fence attached sees finished pixels.
    pub fn finish(&self) {
        gl::finish();
    }

    pub fn describe(&self) -> String {
        let planes = self
            .planes
            .iter()
            .map(|p| format!("off={} stride={}", p.offset, p.stride))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "{}x{} {} {} ({} plane(s): {planes})",
            self.width,
            self.height,
            gbm::fourcc_name(self.format),
            gbm::modifier_name(self.modifier),
            self.planes.len(),
        )
    }
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        // Requires the owning context to be current; callers keep the context
        // alive until after the frame is dropped.
        gl::bind_framebuffer(gl::GL_FRAMEBUFFER, 0);
        gl::delete_framebuffer(self.fbo);
        gl::delete_texture(self.texture);
        if !self.image.is_none() {
            self.egl.destroy_image(self.image);
        }
    }
}
