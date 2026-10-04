//! Static wallpapers: decode once, fit the canvas, upload once.
//!
//! The canvas is `output × scale` (§4.1) and the image *covers* it: whatever
//! does not fit is cropped from the centre, the way every other wallpaper tool
//! behaves. Resizing on the CPU at load time rather than sampling a
//! full-resolution texture every frame is the §3 decision — a 4K source drawn
//! into a 2816×1584 canvas costs one resize instead of 60 Hz of filtering, and
//! it keeps the sampling in the shader a straight 1:1 lookup.

use std::path::{Path, PathBuf};

use crate::render::gl;

/// A decoded, canvas-sized wallpaper living in GPU memory.
pub struct Wallpaper {
    pub texture: gl::Texture,
    pub path: PathBuf,
    /// Size of the file, before the cover fit.
    pub source: (u32, u32),
}

impl Wallpaper {
    /// Decode `path` and fit it to a `canvas` of `canvas_w`×`canvas_h`.
    pub fn load(path: &Path, canvas: (u32, u32)) -> Result<Self, String> {
        let image = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let source = (image.width(), image.height());
        if source.0 == 0 || source.1 == 0 {
            return Err(format!("{}: image is empty", path.display()));
        }
        let canvas = (canvas.0.max(1), canvas.1.max(1));

        let (x, y, w, h) = cover_rect(source, canvas);
        let cropped = image.crop_imm(x, y, w, h).to_rgb8();
        let scaled = image::imageops::resize(
            &cropped,
            canvas.0,
            canvas.1,
            image::imageops::FilterType::Lanczos3,
        );

        Ok(Self {
            texture: gl::Texture::from_rgb(canvas.0, canvas.1, scaled.as_raw()),
            path: path.to_owned(),
            source,
        })
    }

    pub fn describe(&self) -> String {
        format!(
            "{} ({}×{} → {}×{}, cover)",
            self.path.display(),
            self.source.0,
            self.source.1,
            self.texture.width,
            self.texture.height
        )
    }
}

/// The centre-cropped rectangle of `image` that has the aspect ratio of
/// `canvas`, i.e. "cover": fill the canvas, crop the overflow.
///
/// Returns `(x, y, width, height)` in image coordinates.
pub fn cover_rect(image: (u32, u32), canvas: (u32, u32)) -> (u32, u32, u32, u32) {
    if image.0 == 0 || image.1 == 0 || canvas.0 == 0 || canvas.1 == 0 {
        return (0, 0, image.0, image.1);
    }
    // Compare the aspect ratios without floating point: image.w/image.h vs
    // canvas.w/canvas.h, cross-multiplied.
    let image_ratio = image.0 as u128 * canvas.1 as u128;
    let canvas_ratio = canvas.0 as u128 * image.1 as u128;
    if image_ratio == canvas_ratio {
        return (0, 0, image.0, image.1);
    }
    if image_ratio > canvas_ratio {
        // Image is wider: keep the full height, crop the sides.
        let w = ((image.1 as u128 * canvas.0 as u128) / canvas.1 as u128) as u32;
        let w = w.clamp(1, image.0);
        ((image.0 - w) / 2, 0, w, image.1)
    } else {
        // Image is taller: keep the full width, crop top and bottom.
        let h = ((image.0 as u128 * canvas.1 as u128) / canvas.0 as u128) as u32;
        let h = h.clamp(1, image.1);
        (0, (image.1 - h) / 2, image.0, h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_aspect_keeps_the_whole_image() {
        assert_eq!(cover_rect((1920, 1080), (2560, 1440)), (0, 0, 1920, 1080));
        assert_eq!(cover_rect((100, 100), (500, 500)), (0, 0, 100, 100));
    }

    #[test]
    fn wider_image_crops_the_sides() {
        // 2560×1080 (21:9-ish) into a 2560×1440 canvas: keep the height, the
        // width shrinks to 2560 * 1080/1440 = 1920, cropped from the centre.
        let (x, y, w, h) = cover_rect((2560, 1080), (2560, 1440));
        assert_eq!((y, h), (0, 1080));
        assert_eq!(w, 1920);
        assert_eq!(x, (2560 - 1920) / 2);
    }

    #[test]
    fn taller_image_crops_top_and_bottom() {
        // A 1000×3000 strip into a square canvas: keep the full width, and the
        // height becomes 1000 * 1000/1000 = 1000, centred.
        let (x, y, w, h) = cover_rect((1000, 3000), (1000, 1000));
        assert_eq!((x, w), (0, 1000));
        assert_eq!(h, 1000);
        assert_eq!(y, (3000 - 1000) / 2);
    }

    #[test]
    fn degenerate_sizes_do_not_panic() {
        assert_eq!(cover_rect((0, 0), (10, 10)), (0, 0, 0, 0));
        assert_eq!(cover_rect((100, 100), (0, 0)), (0, 0, 100, 100));
        // A 1-pixel image into a big canvas still yields a 1-pixel crop.
        let (_, _, w, h) = cover_rect((1, 1), (2560, 1440));
        assert!(w >= 1 && h >= 1);
    }

    #[test]
    fn the_crop_rect_has_the_canvas_aspect_ratio() {
        // Whatever the input, the crop must share the canvas' aspect ratio,
        // otherwise the resize would stretch the image. The integer division
        // may be off by one pixel, which is invisible.
        for image in [
            (2560u32, 1080u32),
            (1080, 1920),
            (4000, 3000),
            (1920, 1080),
            (1000, 1000),
        ] {
            let canvas = (2560u32, 1440u32);
            let (x, y, w, h) = cover_rect(image, canvas);
            let expected_h = (w as f64 * canvas.1 as f64 / canvas.0 as f64).round() as i64;
            assert!(
                (h as i64 - expected_h).abs() <= 1,
                "{image:?} → {w}×{h}, expected height {expected_h}"
            );
            assert!(
                w <= image.0 && h <= image.1,
                "{image:?} → {w}×{h} exceeds the image"
            );
            assert!(
                x + w <= image.0 && y + h <= image.1,
                "{image:?} → crop out of bounds"
            );
        }
    }
}
