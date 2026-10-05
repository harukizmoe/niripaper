//! What is being drawn: a still image, or a video (`HANDOFF.md` §3).
//!
//! Keeping this in one place is what lets the rest of the daemon stay ignorant
//! of the difference: it loads a path, asks for a [`gl::Content`] to draw, and
//! pumps a wakeup fd. Nothing outside this module matches on the variants, so
//! adding a third kind of media (an animated image, say) touches one file.

use std::os::fd::RawFd;
use std::path::{Path, PathBuf};

use crate::render::gl;
use crate::render::image::Wallpaper;
use crate::render::video::Video;

/// A video's wakeup fd is polled; a still's is `-1`, which `poll()` ignores —
/// that is what keeps the idle cost at zero for stills (§6, M0b criterion ④).
pub const NO_WAKEUP: RawFd = -1;

/// A wallpaper: what to draw, and where it came from.
///
/// The path is kept so a client asking `state` sees what is *actually* on
/// screen. `set` over the control socket swaps the media without touching the
/// configuration, so the two could otherwise disagree.
pub struct Media {
    path: PathBuf,
    inner: Inner,
}

enum Inner {
    Image(Wallpaper),
    Video(Video),
}

impl Media {
    /// Load a wallpaper: a still image, or a video routed by extension. Shared
    /// by startup and by `set` over the control socket.
    pub fn load(path: &Path, canvas: (u32, u32), video_fps: u32) -> Result<Self, String> {
        let inner = if is_video(path) {
            Inner::Video(Video::new(path, canvas.0, canvas.1, video_fps)?)
        } else {
            Inner::Image(Wallpaper::load(path, canvas)?)
        };
        Ok(Self {
            path: path.to_owned(),
            inner,
        })
    }

    /// Where this came from, for `state`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// `"image"` or `"video"`, for `state`.
    pub fn kind(&self) -> &'static str {
        match self.inner {
            Inner::Image(_) => "image",
            Inner::Video(_) => "video",
        }
    }

    /// What is *actually* decoding a video — `hwdec=auto-safe` is only a
    /// request, and a silent fall back to software would be invisible
    /// otherwise. `None` for a still.
    pub fn hwdec(&self) -> Option<String> {
        match &self.inner {
            Inner::Image(_) => None,
            Inner::Video(video) => {
                let hwdec = video.hwdec();
                (!hwdec.is_empty()).then_some(hwdec)
            }
        }
    }

    /// How to draw this, in the renderer's terms. Both end up canvas-sized, so
    /// the shader samples them identically.
    pub fn content(&self) -> gl::Content<'_> {
        match &self.inner {
            Inner::Image(wallpaper) => gl::Content::Wallpaper(&wallpaper.texture),
            Inner::Video(video) => gl::Content::Video(video.texture()),
        }
    }

    /// One line describing what is on screen, for `query`.
    pub fn describe(&self) -> String {
        match self.hwdec() {
            Some(hwdec) => format!("{} hwdec={hwdec}", self.kind()),
            None => self.kind().to_owned(),
        }
    }

    /// The fd to watch for "mpv has a frame ready".
    pub fn wakeup_fd(&self) -> RawFd {
        match &self.inner {
            Inner::Image(_) => NO_WAKEUP,
            Inner::Video(video) => video.fd(),
        }
    }

    /// Decode the next video frame if mpv says one is ready. Returns whether a
    /// new frame is now waiting to be presented. A still never has anything to
    /// pump.
    pub fn pump(&mut self) -> Result<bool, String> {
        match &mut self.inner {
            Inner::Image(_) => Ok(false),
            Inner::Video(video) => {
                video.drain();
                video.render()
            }
        }
    }
}

/// Video containers mpv handles and `image` does not. Routed by extension
/// because guessing wrong is worse than a clear failure: a still handed to mpv
/// plays as a one-frame video, while a video handed to `image` fails to decode.
pub fn is_video(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("mp4" | "webm" | "mkv" | "mov" | "m4v" | "avi")
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn routes_by_extension() {
        for video in ["a.mp4", "A.WEBM", "b.mkv", "c.mov", "d.m4v", "e.avi"] {
            assert!(is_video(&PathBuf::from(video)), "{video} is a video");
        }
        for still in ["a.png", "b.jpg", "c.webp", "no-extension", "d.mp4.txt"] {
            assert!(!is_video(&PathBuf::from(still)), "{still} is not a video");
        }
    }
}
