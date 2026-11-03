use iced::widget::image::Handle;
use jacket_desktop::Desktop;
use std::io::Cursor;

/// Decode and resize off the event loop, with a second allocation limit at the decoder.
pub fn preview(desktop: &Desktop, url: &str) -> Result<Handle, String> {
    let bytes = desktop.artwork_preview(url)?;
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "The source cover could not be read.")?;
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(16_384);
    limits.max_image_height = Some(16_384);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| "The source cover could not be decoded.")?
        .thumbnail(640, 640)
        .into_rgba8();
    Ok(Handle::from_rgba(
        image.width(),
        image.height(),
        image.into_raw(),
    ))
}
