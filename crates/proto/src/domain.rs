//! Shared value types: the vocabulary the config file, the schema, and the
//! state report all speak. Pure data — the motion state machines and the
//! renderers stay in the daemon crate.

/// The parallax span for columns, fixed by spec (§4.2.1): *not* the workspace's
/// real column count, otherwise opening or closing any window would move the
/// wallpaper.
pub const DEFAULT_COLUMN_SPAN: usize = 6;
/// The parallax span for workspaces, fixed for the same reason: dividing by the
/// live workspace count moves the wallpaper whenever one is created or closed.
///
/// Separate from the column span because the two count different things —
/// columns within a workspace versus workspaces on an output — and a setup with
/// many workspaces should not have to shrink its column steps to say so.
pub const DEFAULT_WORKSPACE_SPAN: usize = 6;
/// Parallax scale: the canvas is enlarged by this factor and cropped.
pub const DEFAULT_SCALE: f64 = 1.1;
/// Upper bound on the scale, taken from the §4.3 `panFor(_, 9)` row.
pub const MAX_SCALE: f64 = 1.35;

/// `(horizontal, vertical)`, both 0..=1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Progress {
    pub horizontal: f64,
    pub vertical: f64,
}

impl Progress {
    pub const CENTER: Self = Self {
        horizontal: 0.5,
        vertical: 0.5,
    };

    pub fn new(horizontal: f64, vertical: f64) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }

    /// Linear interpolation towards `to`, used by the easing in [`crate::animation`].
    pub fn lerp(self, to: Self, t: f64) -> Self {
        Self {
            horizontal: self.horizontal + (to.horizontal - self.horizontal) * t,
            vertical: self.vertical + (to.vertical - self.vertical) * t,
        }
    }
}

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

    /// Whether a video can honour this. `tile` cannot: a video is scaled into
    /// the frame by the daemon's decoder, and repeating it is not something it
    /// does.
    pub fn for_video(self) -> Self {
        if self == Self::Tile {
            Self::Fill
        } else {
            self
        }
    }
}
