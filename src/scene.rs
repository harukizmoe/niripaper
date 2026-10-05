//! Putting the current state on screen: build a [`Scene`], draw it into the next
//! free buffer, hand that buffer to the compositor.
//!
//! Everything the renderer needs for one frame is bundled into [`Scene`] rather
//! than passed as arguments. That is deliberate: the fade was added late and
//! every call site had to change for it. A struct means the next addition does
//! not ripple.

use crate::motion::{offset_px, Progress};
use crate::render::gl::{self, Content, Pattern, Renderer, View};
use crate::render::layer::{Client, LayerSurface, Pool};

/// One frame's worth of decisions.
pub struct Scene<'a> {
    pub view: View,
    pub content: Content<'a>,
    /// The new content's weight; `1.0` means no cross-fade.
    pub fade: f32,
    /// What to blend from while `fade < 1.0` ([`crate::crossfade`]).
    pub previous: Option<u32>,
}

impl<'a> Scene<'a> {
    /// Build the view for a given parallax position and overview zoom.
    ///
    /// The zoom is just an animated multiplier on `scale`: the canvas, the
    /// overflow and therefore the parallax travel all follow from it, and the
    /// wallpaper texture (already canvas-sized) is sampled as a sub-region —
    /// nothing to re-upload.
    pub fn view(
        progress: Progress,
        zoom: f64,
        screen: (f64, f64),
        scale: f64,
        pattern: Pattern,
    ) -> View {
        let scale = (scale * zoom).max(1.0);
        let offset = offset_px(progress, screen, scale);
        View {
            screen: (screen.0 as f32, screen.1 as f32),
            scale: scale as f32,
            offset: (offset.0 as f32, offset.1 as f32),
            pattern,
        }
    }

    /// Draw and submit. Returns the slot that was submitted, or `None` when
    /// every buffer was still on screen.
    pub fn draw(
        &self,
        client: &mut Client,
        pool: &mut Pool<'_>,
        surface: &LayerSurface,
        renderer: &Renderer,
        want_next_frame: bool,
    ) -> Result<Option<usize>, String> {
        let released = std::mem::take(&mut client.state.released);
        // Every buffer still on screen: the compositor is behind us. Dropping a
        // frame is the right answer — queueing them up would just add latency,
        // and the caller keeps the animation alive with a bare frame request.
        let Some(slot) = pool.acquire(&released) else {
            return Ok(None);
        };

        let frame = pool.frame(slot);
        frame.begin();
        renderer.draw(self.view, self.content, self.fade, self.previous);
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
        Ok(Some(slot))
    }
}
