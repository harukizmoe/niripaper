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
use crate::render::Fit;

/// A decoded, canvas-sized wallpaper living in GPU memory.
pub struct Wallpaper {
    pub texture: gl::Texture,
    pub path: PathBuf,
    /// Size of the file, before the fit.
    pub source: (u32, u32),
    /// How it was placed in the canvas.
    pub fit: Fit,
}

impl Wallpaper {
    /// Decode `path` and fit it to a `canvas` of `canvas_w`×`canvas_h`.
    pub fn load(path: &Path, canvas: (u32, u32), fit: Fit) -> Result<Self, String> {
        let image = image::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let source = (image.width(), image.height());
        if source.0 == 0 || source.1 == 0 {
            return Err(format!("{}: image is empty", path.display()));
        }
        let canvas = (canvas.0.max(1), canvas.1.max(1));

        let placed = place(&image.to_rgb8(), canvas, fit);

        Ok(Self {
            texture: gl::Texture::from_rgb(canvas.0, canvas.1, placed.as_raw()),
            path: path.to_owned(),
            source,
            fit,
        })
    }

    pub fn describe(&self) -> String {
        format!(
            "{} ({}×{} → {}×{}, {})",
            self.path.display(),
            self.source.0,
            self.source.1,
            self.texture.width,
            self.texture.height,
            self.fit.name()
        )
    }
}

/// Lay `image` out on a `canvas`-sized RGB buffer according to `fit`.
///
/// Doing it here, once at load time, is what keeps the shader's sampling a
/// straight 1:1 lookup (§3): the canvas is filled by the CPU instead of being
/// filtered every frame.
pub fn place(image: &image::RgbImage, canvas: (u32, u32), fit: Fit) -> image::RgbImage {
    let canvas = (canvas.0.max(1), canvas.1.max(1));
    let source = (image.width().max(1), image.height().max(1));
    let filter = image::imageops::FilterType::Lanczos3;
    match fit {
        Fit::Cover => {
            let (x, y, w, h) = cover_rect(source, canvas);
            let cropped = image::imageops::crop_imm(image, x, y, w, h).to_image();
            image::imageops::resize(&cropped, canvas.0, canvas.1, filter)
        }
        Fit::Contain => {
            let (w, h) = fit_inside(source, canvas);
            let scaled = image::imageops::resize(image, w, h, filter);
            // `ImageBuffer::new` zeroes, so the padding is black without asking.
            let mut out = image::RgbImage::new(canvas.0, canvas.1);
            image::imageops::overlay(
                &mut out,
                &scaled,
                ((canvas.0 - w) / 2) as i64,
                ((canvas.1 - h) / 2) as i64,
            );
            out
        }
        Fit::Stretch => image::imageops::resize(image, canvas.0, canvas.1, filter),
    }
}

/// The largest `source`-aspect rectangle that fits inside `canvas`.
///
/// Cross-multiplied rather than divided, like `cover_rect`: no floating point,
/// so no rounding surprise at the boundary.
fn fit_inside(source: (u32, u32), canvas: (u32, u32)) -> (u32, u32) {
    let wider = source.0 as u128 * canvas.1 as u128 >= canvas.0 as u128 * source.1 as u128;
    if wider {
        let h = (canvas.0 as u128 * source.1 as u128 / source.0 as u128).max(1) as u32;
        (canvas.0, h.min(canvas.1))
    } else {
        let w = (canvas.1 as u128 * source.0 as u128 / source.1 as u128).max(1) as u32;
        (w.min(canvas.0), canvas.1)
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

    /// A 2×4 source (top half red, bottom half blue) into a 2×2 canvas. All three
    /// modes give a different answer, so one test covers the geometry of each.
    fn two_by_four() -> image::RgbImage {
        let mut image = image::RgbImage::new(2, 4);
        for (_, y, pixel) in image.enumerate_pixels_mut() {
            *pixel = if y < 2 {
                image::Rgb([255, 0, 0])
            } else {
                image::Rgb([0, 0, 255])
            };
        }
        image
    }

    fn pixel(image: &image::RgbImage, x: u32, y: u32) -> (u8, u8, u8) {
        let p = image.get_pixel(x, y);
        (p[0], p[1], p[2])
    }

    /// Resampling is not a copy: Lanczos rings, so "this pixel is the red half"
    /// has to mean "mostly red" rather than an exact byte. The padding, on the
    /// other hand, is never resampled — that one is exact.
    fn mostly(got: (u8, u8, u8), want: (u8, u8, u8)) -> bool {
        let near = |a: u8, b: u8| (a as i16 - b as i16).abs() < 40;
        near(got.0, want.0) && near(got.1, want.1) && near(got.2, want.2)
    }

    const RED: (u8, u8, u8) = (255, 0, 0);
    const BLUE: (u8, u8, u8) = (0, 0, 255);

    #[test]
    fn cover_crops_and_fills() {
        // 1:2 into 1:1: keep the width, take the middle two rows.
        let placed = place(&two_by_four(), (2, 2), Fit::Cover);
        assert_eq!(placed.dimensions(), (2, 2));
        assert!(
            mostly(pixel(&placed, 0, 0), RED),
            "{:?}",
            pixel(&placed, 0, 0)
        );
        assert!(
            mostly(pixel(&placed, 0, 1), BLUE),
            "{:?}",
            pixel(&placed, 0, 1)
        );
    }

    #[test]
    fn contain_pads_with_black() {
        // 1:2 into 1:1: fit by height, so the picture is one column wide and the
        // other is padding.
        let placed = place(&two_by_four(), (2, 2), Fit::Contain);
        assert_eq!(placed.dimensions(), (2, 2));
        assert!(
            mostly(pixel(&placed, 0, 0), RED),
            "{:?}",
            pixel(&placed, 0, 0)
        );
        assert!(
            mostly(pixel(&placed, 0, 1), BLUE),
            "{:?}",
            pixel(&placed, 0, 1)
        );
        // Not resampled, so this one is exact.
        assert_eq!(pixel(&placed, 1, 0), (0, 0, 0), "the padding is black");
        assert_eq!(pixel(&placed, 1, 1), (0, 0, 0));
    }

    #[test]
    fn stretch_ignores_the_aspect() {
        // 1:2 into 1:1: every output row covers two source rows, so the red and
        // the blue each fill a whole row across the width.
        let placed = place(&two_by_four(), (2, 2), Fit::Stretch);
        assert_eq!(placed.dimensions(), (2, 2));
        for x in 0..2 {
            assert!(
                mostly(pixel(&placed, x, 0), RED),
                "{:?}",
                pixel(&placed, x, 0)
            );
            assert!(
                mostly(pixel(&placed, x, 1), BLUE),
                "{:?}",
                pixel(&placed, x, 1)
            );
        }
    }

    #[test]
    fn a_matching_aspect_is_the_same_in_all_three() {
        let source = two_by_four();
        // 1:2 source into a 1:2 canvas: nothing to crop, pad or distort.
        let canvas = (2, 4);
        for fit in [Fit::Cover, Fit::Contain, Fit::Stretch] {
            let placed = place(&source, canvas, fit);
            assert_eq!(placed.dimensions(), canvas);
            assert!(mostly(pixel(&placed, 0, 0), RED), "{fit:?}");
            assert!(mostly(pixel(&placed, 0, 3), BLUE), "{fit:?}");
        }
    }

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
