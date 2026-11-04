//! Bounded source-cover reads for the native UI. No credentials are attached.
use crate::artwork_assets;
use std::io::Read;
use std::time::Duration;

const MAX_BYTES: u64 = 10 * 1024 * 1024;

pub(crate) fn fetch(url: &str) -> Result<Vec<u8>, String> {
    if !audio_engine::source::valid_artwork_url(url) {
        return Err("The source cover address is unsupported.".into());
    }
    let client = reqwest::blocking::Client::builder()
        .user_agent("Jacket-cover-preview")
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(15))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 3
                || !audio_engine::source::valid_artwork_url(attempt.url().as_str())
            {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "The source cover could not be loaded.")?;
    let response = client
        .get(url)
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|_| "The source cover could not be loaded.")?;
    if response
        .content_length()
        .is_some_and(|size| size > MAX_BYTES)
    {
        return Err("The source cover is too large to preview.".into());
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "The source cover could not be loaded.")?;
    validate(&bytes)?;
    Ok(bytes)
}

fn validate(bytes: &[u8]) -> Result<(), String> {
    if bytes.len() as u64 > MAX_BYTES {
        return Err("The source cover is too large to preview.".into());
    }
    artwork_assets::dimensions(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{fetch, validate};

    #[test]
    fn rejects_unsupported_addresses_without_opening_a_connection() {
        for url in [
            "file:///etc/passwd",
            "http://i.ytimg.com/cover.jpg",
            "https://ytimg.com.evil.example/cover.jpg",
            "https://user@i.ytimg.com/cover.jpg",
        ] {
            assert!(fetch(url).is_err());
        }
    }

    #[test]
    fn source_webp_images_pass_the_same_preview_bounds_as_jpeg_and_png() {
        for format in [
            image::ImageFormat::Png,
            image::ImageFormat::Jpeg,
            image::ImageFormat::WebP,
        ] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(544, 544)
                .write_to(&mut bytes, format)
                .expect("encoded cover");
            validate(bytes.get_ref()).expect("supported source cover");
        }
    }

    #[test]
    fn compressed_image_dimensions_are_checked_before_ui_decoding() {
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend_from_slice(&20_000_u32.to_be_bytes());
        png.extend_from_slice(&20_000_u32.to_be_bytes());
        assert!(validate(&png).is_err());
        assert!(validate(b"not an image").is_err());
    }
}
