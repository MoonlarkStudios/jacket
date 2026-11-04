//! Cover selection and bounded decoding for review and export previews.
use audio_engine::{ArtworkMode, ExportPlan};
mod raster;
use jacket_core::Review;
use jacket_desktop::Desktop;
pub use raster::{normalized_scale, Raster};
use std::sync::OnceLock;

/// The review's source image, independent of whether artwork will be exported.
/// An explicit imported asset never falls back to an unrelated source cover.
pub fn source_url(review: &Review) -> Option<&str> {
    if review.preferences.shared_artwork_asset.is_some() {
        return None;
    }
    review
        .preferences
        .shared_artwork
        .as_deref()
        .or(review.inspection.artwork_url.as_deref())
        .or_else(|| {
            review
                .inspection
                .tracks
                .iter()
                .find_map(|track| track.artwork_url.as_deref())
        })
}

/// The image actually selected by an export plan. Per-track output is represented
/// by the first selected track; callers label it as individual artwork.
pub fn preview_url(plan: &ExportPlan) -> Option<&str> {
    match plan.artwork_mode {
        ArtworkMode::None => None,
        ArtworkMode::Shared if plan.shared_artwork_asset.is_some() => None,
        ArtworkMode::Shared => plan.shared_artwork_url.as_deref(),
        ArtworkMode::PerTrack => plan
            .tracks
            .first()
            .and_then(|track| track.artwork_url.as_deref()),
    }
}

/// A compiled, original cover for the isolated album example. Cache its handle
/// so drawing the UI does not create a new image identity on each frame.
pub fn example_cover() -> Raster {
    static COVER: OnceLock<Raster> = OnceLock::new();
    COVER
        .get_or_init(|| {
            Raster::decode(
                include_bytes!("../../../assets/examples/room-to-breathe.png"),
                1.0,
            )
            .expect("compiled example cover")
        })
        .clone()
}

/// Fetch, decode and filter outside the event loop. A display-scale change can
/// reuse these retained source pixels without downloading or re-encoding them.
pub fn preview(desktop: &Desktop, url: &str, scale: f32) -> Result<Raster, String> {
    let bytes = desktop.artwork_preview(url)?;
    Raster::decode(&bytes, scale)
}

#[cfg(test)]
mod tests {
    use super::{example_cover, preview_url, source_url, Raster};
    use audio_engine::{ArtworkMode, AudioFormat, AuthOptions};
    use iced::widget::image::Handle;
    use jacket_core::Review;
    use std::io::Cursor;

    #[test]
    fn imported_artwork_never_falls_back_to_an_old_remote_cover() {
        let mut review = Review::new(crate::example::album(), AudioFormat::Compatible);
        review.preferences.shared_artwork = Some("https://i.ytimg.com/selected.jpg".into());
        review.inspection.artwork_url = Some("https://i.ytimg.com/original.jpg".into());
        assert_eq!(
            source_url(&review),
            Some("https://i.ytimg.com/selected.jpg")
        );
        review.preferences.shared_artwork_asset = Some("imported-cover".into());
        assert_eq!(source_url(&review), None);
    }

    #[test]
    fn preview_follows_artwork_mode_and_the_selected_track_order() {
        let mut review = Review::new(crate::example::album(), AudioFormat::Compatible);
        review.preferences.shared_artwork = Some("https://i.ytimg.com/album.jpg".into());
        for (index, track) in review.inspection.tracks.iter_mut().enumerate() {
            track.artwork_url = Some(format!("https://i.ytimg.com/track-{index}.jpg"));
        }
        review.selected.remove(&0);
        let mut plan = review.plan(AuthOptions::default()).expect("valid review");
        assert_eq!(preview_url(&plan), Some("https://i.ytimg.com/album.jpg"));
        plan.artwork_mode = ArtworkMode::PerTrack;
        assert_eq!(preview_url(&plan), Some("https://i.ytimg.com/track-1.jpg"));
        plan.tracks[0].artwork_url = None;
        assert_eq!(
            preview_url(&plan),
            None,
            "another track's cover must not represent the first track"
        );
        plan.artwork_mode = ArtworkMode::Shared;
        plan.shared_artwork_asset = Some("local-cover".into());
        assert_eq!(preview_url(&plan), None);
        plan.artwork_mode = ArtworkMode::None;
        assert_eq!(preview_url(&plan), None);
        // Turning off export artwork does not remove the source image from review.
        review.preferences.artwork_mode = ArtworkMode::None;
        assert_eq!(source_url(&review), Some("https://i.ytimg.com/album.jpg"));
    }

    #[test]
    fn cover_decoding_rejects_bad_data_and_bounds_the_ui_image() {
        assert!(Raster::decode(b"not an image", 1.0).is_err());
        for format in [image::ImageFormat::Png, image::ImageFormat::WebP] {
            let mut encoded = Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(1280, 640)
                .write_to(&mut encoded, format)
                .expect("synthetic source cover");
            let raster = Raster::decode(encoded.get_ref(), 1.0).expect("decodable cover");
            let handle = raster.source().clone();
            match handle {
                Handle::Rgba {
                    width,
                    height,
                    pixels,
                    ..
                } => {
                    assert_eq!((width, height), (1024, 512));
                    assert_eq!(pixels.len(), 1024 * 512 * 4);
                }
                _ => panic!("decoded covers must use bounded pixels"),
            }
        }
    }

    #[test]
    fn the_example_cover_has_a_stable_image_identity() {
        assert_eq!(example_cover(), example_cover());
    }
}
