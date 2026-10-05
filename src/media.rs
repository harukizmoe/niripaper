//! What is being drawn: a still image, or a video (`HANDOFF.md` §3).
//!
//! Keeping this in one place is what lets the rest of the daemon stay ignorant
//! of the difference: it loads a path, asks for a [`gl::Content`] to draw, and
//! pumps a wakeup fd. Nothing outside this module matches on the variants, so
//! adding a third kind of media (an animated image, say) touches one file.

use std::os::fd::RawFd;
use std::path::Path;

use crate::render::gl;
use crate::render::image::Wallpaper;
use crate::render::video::Video;

/// A video's wakeup fd is polled; a still's is `-1`, which `poll()` ignores —
/// that is what keeps the idle cost at zero for stills (§6, M0b criterion ④).
pub const NO_WAKEUP: RawFd = -1;

pub enum Media {
    Image(Wallpaper),
    Video(Video),
}

impl Media {
    /// Load a wallpaper: a still image, or a video routed by extension. Shared
    /// by startup and by `set` over the control socket.
    pub fn load(path: &Path, canvas: (u32, u32), video_fps: u32) -> Result<Self, String> {
        if is_video(path) {
            let video = Video::new(path, canvas.0, canvas.1, video_fps)?;
            return Ok(Self::Video(video));
        }
        Ok(Self::Image(Wallpaper::load(path, canvas)?))
    }

    /// How to draw this, in the renderer's terms. Both end up canvas-sized, so
    /// the shader samples them identically.
    pub fn content(&self) -> gl::Content<'_> {
        match self {
            Self::Image(wallpaper) => gl::Content::Wallpaper(&wallpaper.texture),
            Self::Video(video) => gl::Content::Video(video.texture()),
        }
    }

    /// One line describing what is on screen, for `query`.
    pub fn describe(&self) -> String {
        match self {
            Self::Image(_) => "image".to_owned(),
            Self::Video(video) => {
                let hwdec = video.hwdec();
                if hwdec.is_empty() {
                    "video".to_owned()
                } else {
                    // Say what is *actually* decoding: `hwdec=auto-safe` is only
                    // a request, and a silent fall back to software would be
                    // invisible otherwise.
                    format!("video hwdec={hwdec}")
                }
            }
        }
    }

    /// The fd to watch for "mpv has a frame ready".
    pub fn wakeup_fd(&self) -> RawFd {
        match self {
            Self::Image(_) => NO_WAKEUP,
            Self::Video(video) => video.fd(),
        }
    }

    /// Decode the next video frame if mpv says one is ready. Returns whether a
    /// new frame is now waiting to be presented. A still never has anything to
    /// pump.
    pub fn pump(&mut self) -> Result<bool, String> {
        match self {
            Self::Image(_) => Ok(false),
            Self::Video(video) => {
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
