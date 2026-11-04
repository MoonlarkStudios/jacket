//! Prepare artwork for its physical display size on a blocking worker.
use iced::widget::image::Handle;
use image::imageops::FilterType;
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};
use std::io::{Cursor, Read};
use std::path::Path;

const MAX_ENCODED_BYTES: usize = 10 * 1024 * 1024;
const MAX_DECODED_BYTES: u64 = 64 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16_384;
const SOURCE_EDGE: u32 = 1024;
const COVER_EDGES: [u32; 6] = [86, 96, 104, 112, 128, 184];

/// Retains a bounded source and prepared display variants. Cloning only shares
/// iced's pixel handles; equality compares source identity and display scale.
#[derive(Clone, Debug)]
pub struct Raster {
    source: Handle,
    covers: [Handle; COVER_EDGES.len()],
    crop: Handle,
    scale: f32,
}

impl PartialEq for Raster {
    fn eq(&self, other: &Self) -> bool {
        self.source.id() == other.source.id() && self.scale == other.scale
    }
}

impl Raster {
    /// Decode once with explicit encoded, dimension and allocation bounds.
    pub fn decode(bytes: &[u8], scale: f32) -> Result<Self, String> {
        if bytes.len() > MAX_ENCODED_BYTES {
            return Err("Cover images must be 10 MiB or smaller.".into());
        }
        let mut reader = image::ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|_| "The cover image could not be read.")?;
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(MAX_DECODED_BYTES);
        limits.max_image_width = Some(MAX_DIMENSION);
        limits.max_image_height = Some(MAX_DIMENSION);
        reader.limits(limits);
        let decoded = reader
            .decode()
            .map_err(|_| "The cover image could not be decoded within its size limits.")?;
        let (width, height) = decoded.dimensions();
        // Grayscale inputs can meet decoder limits while exceeding the RGBA
        // budget. Check before allocating the retained four-channel image.
        if width == 0
            || height == 0
            || width > MAX_DIMENSION
            || height > MAX_DIMENSION
            || u64::from(width) * u64::from(height) > MAX_DECODED_BYTES / 4
        {
            return Err("The decoded cover image exceeds its size limits.".into());
        }
        let rgba = decoded.into_rgba8();
        let (width, height) = fit_dimensions(width, height, SOURCE_EDGE, SOURCE_EDGE);
        let source = if rgba.dimensions() == (width, height) {
            to_handle(rgba)
        } else {
            to_handle(resize(&rgba, width, height))
        };
        Ok(Self::prepare(source, normalized_scale(scale)))
    }

    /// Load a regular file without reading more than the encoded-image budget.
    pub fn load(path: &Path, scale: f32) -> Result<Self, String> {
        let metadata = std::fs::metadata(path)
            .map_err(|error| format!("The cover image could not be opened: {error}"))?;
        if !metadata.is_file() {
            return Err("Choose a regular cover image file.".into());
        }
        if metadata.len() > MAX_ENCODED_BYTES as u64 {
            return Err("Cover images must be 10 MiB or smaller.".into());
        }
        let file = std::fs::File::open(path)
            .map_err(|error| format!("The cover image could not be opened: {error}"))?;
        if !file
            .metadata()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err("Choose a regular cover image file.".into());
        }
        let mut bytes = Vec::new();
        file.take(MAX_ENCODED_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| format!("The cover image could not be read: {error}"))?;
        Self::decode(&bytes, scale)
    }

    /// Rebuild physical-size variants from decoded pixels, without file I/O or
    /// another image decode. Call on a blocking worker when display scale changes.
    pub fn for_scale(&self, scale: f32) -> Self {
        let scale = normalized_scale(scale);
        if scale == self.scale {
            self.clone()
        } else {
            Self::prepare(self.source.clone(), scale)
        }
    }

    /// Return a prepared variant for the six cover sizes used by the interface.
    /// Other requested sizes use the nearest prepared variant without doing CPU
    /// image work while constructing a view.
    pub fn handle(&self, logical_edge: u32) -> Handle {
        let index = COVER_EDGES
            .iter()
            .enumerate()
            .min_by_key(|(_, edge)| edge.abs_diff(logical_edge))
            .map(|(index, _)| index)
            .expect("the cover size list is nonempty");
        self.covers[index].clone()
    }

    /// An aspect-preserving image fitting the crop panel's 360 × 260 canvas.
    pub fn crop_handle(&self) -> Handle {
        self.crop.clone()
    }

    /// Stable across display-scale changes, for fencing background completions.
    pub fn source(&self) -> &Handle {
        &self.source
    }

    pub fn scale(&self) -> f32 {
        self.scale
    }

    fn prepare(source: Handle, scale: f32) -> Self {
        let Handle::Rgba {
            width,
            height,
            pixels,
            ..
        } = &source
        else {
            unreachable!("raster sources are always bounded decoded RGBA pixels");
        };
        let image = ImageBuffer::<Rgba<u8>, _>::from_raw(*width, *height, pixels.as_ref())
            .expect("the decoded source handle contains its complete pixel buffer");
        let prepare = |logical_width, logical_height| {
            let (width, height) = fit_dimensions(
                *width,
                *height,
                physical_size(logical_width, scale),
                physical_size(logical_height, scale),
            );
            if image.dimensions() == (width, height) {
                source.clone()
            } else {
                to_handle(resize(&image, width, height))
            }
        };
        let covers = COVER_EDGES.map(|edge| prepare(edge, edge));
        let crop = prepare(360, 260);
        Self {
            source,
            covers,
            crop,
            scale,
        }
    }
}

/// Keep scale finite and bounded before computing physical pixel dimensions.
pub fn normalized_scale(scale: f32) -> f32 {
    if scale.is_finite() && scale > 0.0 {
        scale.clamp(0.5, 4.0)
    } else {
        1.0
    }
}

fn physical_size(logical: u32, scale: f32) -> u32 {
    (logical as f32 * scale).round().max(1.0) as u32
}

fn fit_dimensions(width: u32, height: u32, bound_width: u32, bound_height: u32) -> (u32, u32) {
    let ratio = (f64::from(bound_width) / f64::from(width))
        .min(f64::from(bound_height) / f64::from(height))
        .min(1.0);
    (
        (f64::from(width) * ratio).round().max(1.0) as u32,
        (f64::from(height) * ratio).round().max(1.0) as u32,
    )
}

fn to_handle(image: RgbaImage) -> Handle {
    Handle::from_rgba(image.width(), image.height(), image.into_raw())
}

/// Supply premultiplied floating-point samples lazily, avoiding a second full
/// floating-point source allocation before the separable Lanczos filter.
struct Premultiplied<'a, I>(&'a I);

impl<I: GenericImageView<Pixel = Rgba<u8>>> GenericImageView for Premultiplied<'_, I> {
    type Pixel = Rgba<f32>;

    fn dimensions(&self) -> (u32, u32) {
        self.0.dimensions()
    }

    fn get_pixel(&self, x: u32, y: u32) -> Self::Pixel {
        let Rgba([red, green, blue, alpha]) = self.0.get_pixel(x, y);
        let alpha = f32::from(alpha) / 255.0;
        Rgba([
            f32::from(red) / 255.0 * alpha,
            f32::from(green) / 255.0 * alpha,
            f32::from(blue) / 255.0 * alpha,
            alpha,
        ])
    }
}

fn resize(image: &impl GenericImageView<Pixel = Rgba<u8>>, width: u32, height: u32) -> RgbaImage {
    let resized =
        image::imageops::resize(&Premultiplied(image), width, height, FilterType::Lanczos3);
    RgbaImage::from_fn(width, height, |x, y| {
        let Rgba([red, green, blue, alpha]) = *resized.get_pixel(x, y);
        let alpha = alpha.clamp(0.0, 1.0);
        let byte_alpha = (alpha * 255.0).round() as u8;
        if byte_alpha == 0 {
            Rgba([0, 0, 0, 0])
        } else {
            let unpremultiply =
                |channel: f32| (channel / alpha * 255.0).round().clamp(0.0, 255.0) as u8;
            Rgba([
                unpremultiply(red),
                unpremultiply(green),
                unpremultiply(blue),
                byte_alpha,
            ])
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{normalized_scale, Raster, COVER_EDGES, MAX_ENCODED_BYTES};
    use iced::widget::image::Handle;
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    use std::io::Cursor;

    fn encoded(image: RgbaImage) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, ImageFormat::Png)
            .expect("synthetic cover PNG");
        bytes.into_inner()
    }

    fn dimensions(handle: &Handle) -> (u32, u32) {
        match handle {
            Handle::Rgba { width, height, .. } => (*width, *height),
            _ => panic!("prepared images must be decoded pixels"),
        }
    }

    fn pixels(handle: &Handle) -> &[u8] {
        match handle {
            Handle::Rgba { pixels, .. } => pixels.as_ref(),
            _ => panic!("prepared images must be decoded pixels"),
        }
    }

    #[test]
    fn covers_use_physical_pixels_at_integer_and_fractional_display_scales() {
        let bytes = encoded(RgbaImage::from_pixel(512, 512, Rgba([51, 102, 153, 255])));
        let raster = Raster::decode(&bytes, 1.0).expect("cover");
        for scale in [1.0, 1.25, 2.0] {
            let scaled = raster.for_scale(scale);
            for edge in COVER_EDGES {
                let physical = (edge as f32 * scale).round() as u32;
                assert_eq!(dimensions(&scaled.handle(edge)), (physical, physical));
            }
            assert_eq!(scaled.scale(), scale);
        }
    }

    #[test]
    fn aspect_ratio_and_small_sources_are_preserved_without_enlargement() {
        let bytes = encoded(RgbaImage::from_fn(31, 19, |x, y| {
            Rgba([x as u8, y as u8, 100, 128])
        }));
        let raster = Raster::decode(&bytes, 1.0).expect("small transparent cover");
        let source = raster.source().clone();
        let original = pixels(&source).to_vec();
        for scale in [0.5, 1.25, 2.0, 4.0] {
            let scaled = raster.for_scale(scale);
            assert_eq!(scaled.source().id(), source.id());
            assert_eq!(pixels(scaled.source()), original);
            for edge in COVER_EDGES {
                assert_eq!(scaled.handle(edge).id(), source.id());
                assert_eq!(dimensions(&scaled.handle(edge)), (31, 19));
            }
            assert_eq!(scaled.crop_handle().id(), source.id());
        }
        let landscape = Raster::decode(
            &encoded(RgbaImage::from_pixel(512, 256, Rgba([10, 20, 30, 255]))),
            1.0,
        )
        .expect("landscape cover");
        assert_eq!(dimensions(&landscape.handle(104)), (104, 52));
        assert_eq!(dimensions(&landscape.crop_handle()), (360, 180));
        assert_eq!(
            dimensions(&landscape.for_scale(1.25).crop_handle()),
            (450, 225)
        );
        assert_eq!(
            dimensions(&landscape.for_scale(2.0).crop_handle()),
            (512, 256)
        );
    }

    #[test]
    fn high_frequency_artwork_is_filtered_across_the_entire_display_area() {
        let bytes = encoded(RgbaImage::from_fn(512, 512, |x, y| {
            let level = if (x + y) % 2 == 0 { 0 } else { 255 };
            Rgba([level, level, level, 255])
        }));
        let raster = Raster::decode(&bytes, 1.0).expect("checkerboard cover");
        for scale in [1.0, 1.25, 2.0] {
            let handle = raster.for_scale(scale).handle(86);
            let (width, height) = dimensions(&handle);
            let pixels = pixels(&handle);
            let mut checked = 0;
            for y in 4..height - 4 {
                for x in 4..width - 4 {
                    let offset = ((y * width + x) * 4) as usize;
                    for channel in &pixels[offset..offset + 3] {
                        assert!(
                            (126..=129).contains(channel),
                            "alias at ({x}, {y}): {channel}"
                        );
                    }
                    assert_eq!(pixels[offset + 3], 255);
                    checked += 1;
                }
            }
            assert!(
                checked > 5_000,
                "inspect a uniform area, not isolated sample pixels"
            );
        }
    }

    #[test]
    fn resizing_transparent_pixels_does_not_create_colored_fringes() {
        let bytes = encoded(RgbaImage::from_fn(256, 256, |x, y| {
            if (x + y) % 2 == 0 {
                Rgba([255, 0, 0, 255])
            } else {
                Rgba([0, 0, 255, 0])
            }
        }));
        let raster = Raster::decode(&bytes, 1.0).expect("transparent artwork");
        let handle = raster.handle(86);
        for pixel in pixels(&handle).chunks_exact(4) {
            assert_eq!(
                pixel[0], 255,
                "visible red must not darken during filtering"
            );
            assert_eq!(pixel[1], 0);
            assert_eq!(
                pixel[2], 0,
                "invisible blue must not bleed into visible pixels"
            );
            assert!((120..=135).contains(&pixel[3]));
        }
    }

    #[test]
    fn source_bounds_and_identity_survive_rescaling_without_another_decode() {
        let bytes = encoded(RgbaImage::from_pixel(1280, 640, Rgba([20, 40, 60, 255])));
        let raster = Raster::decode(&bytes, 1.0).expect("large cover");
        assert_eq!(dimensions(raster.source()), (1024, 512));
        let same = raster.for_scale(1.0);
        assert_eq!(same, raster);
        assert_eq!(same.handle(104).id(), raster.handle(104).id());
        let scaled = raster.for_scale(2.0);
        assert_ne!(scaled, raster);
        assert_eq!(scaled.source().id(), raster.source().id());
        assert_eq!(pixels(scaled.source()), pixels(raster.source()));
        assert_eq!(dimensions(&scaled.handle(104)), (208, 104));
    }

    #[test]
    fn invalid_scales_use_a_safe_default_and_finite_scales_are_clamped() {
        for scale in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 0.0, -2.0] {
            assert_eq!(normalized_scale(scale), 1.0);
        }
        assert_eq!(normalized_scale(0.25), 0.5);
        assert_eq!(normalized_scale(10.0), 4.0);
        assert_eq!(normalized_scale(1.25), 1.25);
    }

    #[test]
    fn file_and_decoder_limits_reject_invalid_and_oversized_input() {
        assert!(Raster::decode(b"not an image", 1.0).is_err());
        assert!(Raster::decode(&vec![0; MAX_ENCODED_BYTES + 1], 1.0)
            .expect_err("encoded byte limit")
            .contains("10 MiB"));
        let directory = tempfile::tempdir().expect("isolated image directory");
        assert!(Raster::load(directory.path(), 1.0).is_err());
        let oversized = directory.path().join("large.png");
        let file = std::fs::File::create(&oversized).expect("sparse fixture");
        file.set_len(MAX_ENCODED_BYTES as u64 + 1)
            .expect("oversized file");
        assert!(Raster::load(&oversized, 1.0)
            .expect_err("file byte limit")
            .contains("10 MiB"));
        let invalid = directory.path().join("invalid.png");
        std::fs::write(&invalid, b"not an image").expect("invalid fixture");
        assert!(Raster::load(&invalid, 1.0).is_err());
        let too_wide = encoded(RgbaImage::from_pixel(16_385, 1, Rgba([0, 0, 0, 255])));
        assert!(Raster::decode(&too_wide, 1.0).is_err());
        let mut grayscale = Cursor::new(Vec::new());
        DynamicImage::new_luma8(4097, 4097)
            .write_to(&mut grayscale, ImageFormat::Png)
            .expect("compressed grayscale fixture");
        assert!(Raster::decode(grayscale.get_ref(), 1.0)
            .expect_err("the grayscale decode must not grow beyond 64 MiB when converted to RGBA")
            .contains("size limits"));
        let valid = directory.path().join("valid.png");
        std::fs::write(
            &valid,
            encoded(RgbaImage::from_pixel(32, 16, Rgba([5, 10, 15, 255]))),
        )
        .expect("valid fixture");
        assert_eq!(
            dimensions(Raster::load(&valid, 1.25).expect("valid file").source()),
            (32, 16)
        );
    }
}
