//! The cross-fade when the wallpaper changes (`HANDOFF.md` §3, M2's last piece).
//!
//! The snapshot is of the **rendered** frame, not of the source: that is what
//! makes any pair of sources blend identically (still ↔ video ↔ still), keeps
//! only one decoder alive, and means the parallax and overview zoom are already
//! baked into what fades out — no special cases for any of them.

use std::time::Instant;

use crate::render::anim::{Animation, Animator};
use crate::render::gl;

pub struct CrossFade {
    animator: Animator<f64>,
    animation: Animation,
    slowdown: f64,
    /// Created on the first change: a daemon that never switches wallpapers
    /// never pays for it.
    snapshot: Option<gl::Snapshot>,
}

impl CrossFade {
    /// Starts settled at 1.0 — "fully the new content". A cross-fade only exists
    /// while the wallpaper is changing.
    pub fn new(animation: Animation, slowdown: f64) -> Self {
        Self {
            animator: Animator::new(1.0, animation).with_slowdown(slowdown),
            animation,
            slowdown,
            snapshot: None,
        }
    }

    /// `off` means the wallpaper switches instantly; there is nothing to capture
    /// and nothing to animate.
    pub fn is_off(&self) -> bool {
        self.animation == Animation::Off
    }

    /// Snapshot what is on screen right now and start fading the new content in.
    ///
    /// `capture` is handed the snapshot to fill in. The caller binds the
    /// framebuffer: `Frame::begin` and `mpv_render_context_render` both manage
    /// bindings, so this module must not guess which one is current.
    pub fn restart(
        &mut self,
        screen: (u32, u32),
        now: Instant,
        capture: impl FnOnce(&gl::Snapshot),
    ) {
        let snapshot = self
            .snapshot
            .get_or_insert_with(|| gl::Snapshot::new(screen.0, screen.1));
        capture(snapshot);
        self.animator = Animator::new(0.0, self.animation).with_slowdown(self.slowdown);
        self.animator.retarget(1.0, now);
    }

    /// `(amount, moving)`. The amount is clamped: a spring may overshoot, but
    /// "more than the new wallpaper" is not something a fade can show.
    pub fn sample(&mut self, now: Instant) -> (f32, bool) {
        let (value, moving) = self.animator.sample(now);
        (value.clamp(0.0, 1.0) as f32, moving)
    }

    pub fn is_moving(&self) -> bool {
        self.animator.is_moving()
    }

    /// What to blend from, or `None` when there is nothing to fade.
    pub fn texture(&self) -> Option<u32> {
        self.snapshot.as_ref().map(|snapshot| snapshot.texture())
    }
}
