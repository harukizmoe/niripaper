//! Rendering: EGL/GBM on a chosen GPU, GL, dmabufs, and the Wayland layer.

pub mod anim;
pub mod dmabuf;
pub mod egl;
pub mod gbm;
pub mod gl;
pub mod image;
pub mod layer;
pub mod transition;
pub mod video;

/// How a source whose aspect ratio does not match the canvas is placed in it.
///
/// The same three choices every wallpaper tool offers, and the same names: the
/// still and video paths have to agree, or a wallpaper changes shape depending
/// on whether it happens to be a picture or a film.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Fill the canvas and crop the overflow. The default: nothing is ever
    /// letterboxed, at the price of losing whatever sticks out.
    Cover,
    /// Show the whole source and pad the rest with black. Nothing is lost; the
    /// bars are the price. Worth having for film-shaped sources, where cover can
    /// throw away a quarter of the picture.
    Contain,
    /// Distort to the canvas: no cropping, no bars, wrong proportions.
    Stretch,
}

impl Fit {
    pub const NAMES: [&'static str; 3] = ["cover", "contain", "stretch"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Cover => Self::NAMES[0],
            Self::Contain => Self::NAMES[1],
            Self::Stretch => Self::NAMES[2],
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "cover" => Ok(Self::Cover),
            "contain" => Ok(Self::Contain),
            "stretch" => Ok(Self::Stretch),
            other => Err(format!(
                "unknown fit {other:?}; the choices are: {}",
                Self::NAMES.join(", ")
            )),
        }
    }
}
