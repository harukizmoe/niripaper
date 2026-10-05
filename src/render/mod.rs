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
/// The names are Wallpaper Engine's (and Windows' before it): `fill`, `fit`,
/// `stretch`, `center`, `tile`. Matching them is deliberate — anyone arriving
/// from Wallpaper Engine already knows what each one means, and a config that
/// spells them differently is a config they have to learn twice. `span` is the
/// one Windows mode left out: it spreads a picture across several monitors,
/// which cannot mean anything to a daemon that draws on exactly one output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fit {
    /// Fill the canvas, cropping the overflow. The default, and what most
    /// wallpapers are made for.
    Fill,
    /// Show the whole source, scaling it down if it is too big, and pad the rest
    /// with black. Nothing is lost; the bars are the price.
    Fit,
    /// Distort to the canvas: no cropping, no bars, wrong proportions.
    Stretch,
    /// No scaling at all: draw at the source's own pixel size, centred. Bigger
    /// than the canvas crops the edges, smaller leaves black all round.
    Center,
    /// Repeat the source at its own size across the canvas, from the top-left.
    /// Images only — a video cannot be tiled, so a video with `tile` falls back
    /// to `fill` and says so at startup.
    Tile,
}

impl Fit {
    pub const NAMES: [&'static str; 5] = ["fill", "fit", "stretch", "center", "tile"];

    pub fn name(self) -> &'static str {
        match self {
            Self::Fill => Self::NAMES[0],
            Self::Fit => Self::NAMES[1],
            Self::Stretch => Self::NAMES[2],
            Self::Center => Self::NAMES[3],
            Self::Tile => Self::NAMES[4],
        }
    }

    pub fn parse(name: &str) -> Result<Self, String> {
        match name {
            "fill" => Ok(Self::Fill),
            "fit" => Ok(Self::Fit),
            "stretch" => Ok(Self::Stretch),
            "center" => Ok(Self::Center),
            "tile" => Ok(Self::Tile),
            other => Err(format!(
                "unknown fit {other:?}; the choices are: {}",
                Self::NAMES.join(", ")
            )),
        }
    }

    /// Whether a video can honour this. `tile` cannot: mpv scales a video into
    /// the frame, and repeating it is not something it does.
    pub fn for_video(self) -> Self {
        if self == Self::Tile {
            Self::Fill
        } else {
            self
        }
    }
}
