//! Container-specific metadata policy. FFmpeg prepares audio and native covers;
//! Lofty writes canonical comment fields and Ogg picture blocks afterwards.
use crate::{Cancellation, EngineError, Metadata, Result};
use lofty::{
    config::WriteOptions,
    file::TaggedFileExt,
    picture::{Picture, PictureType},
    tag::{ItemKey, TagExt},
};
use std::{fs::File, path::Path};

fn metadata_error(error: impl std::error::Error) -> EngineError {
    let mut detail = error.to_string();
    let mut source = error.source();
    for _ in 0..3 {
        let Some(cause) = source else {
            break;
        };
        detail.push_str(&format!(": {cause}"));
        source = cause.source();
    }
    EngineError::Metadata(detail)
}

pub fn supports_embedded_artwork(ext: &str) -> bool {
    matches!(ext, "mp3" | "m4a" | "flac" | "opus" | "ogg")
}

pub fn embedded_metadata(metadata: &Metadata, ext: &str) -> Metadata {
    let mut embedded = metadata.clone();
    if ext == "wav" {
        embedded.album_artist.clear();
        embedded.disc_number = None;
    }
    embedded
}

pub fn metadata_warnings(metadata: &Metadata, ext: &str) -> Vec<String> {
    if ext != "wav" {
        return Vec::new();
    }
    let mut fields = Vec::new();
    if !metadata.album_artist.is_empty() {
        fields.push("album artist");
    }
    if metadata.disc_number.is_some() {
        fields.push("disc number");
    }
    if fields.is_empty() {
        return Vec::new();
    }
    vec![format!("WAV cannot embed {} for “{}” with the current RIFF tagging profile. These values remain in the manifest. Use M4A / MP3 to retain them in the audio file.", fields.join(" and "), metadata.title)]
}

pub fn finish_tags(
    path: &Path,
    ext: &str,
    source_url: &str,
    cover: Option<&Path>,
    cancel: &Cancellation,
) -> Result<()> {
    cancel.check()?;
    if ext == "wav" {
        return Ok(());
    }
    let mut file = lofty::read_from_path(path).map_err(metadata_error)?;
    let tag = file
        .primary_tag_mut()
        .ok_or_else(|| EngineError::Invalid(format!("No writable primary tag in {ext} output")))?;
    if !tag.insert_text(ItemKey::Comment, source_url.to_owned()) {
        return Err(EngineError::Invalid(format!(
            "Source comments are unsupported in {ext}"
        )));
    }
    if matches!(ext, "opus" | "ogg") {
        // Do not put image bytes on a command line: large covers can exceed
        // OS argument limits. Lofty writes METADATA_BLOCK_PICTURE in the file.
        if let Some(cover) = cover {
            let mut picture =
                Picture::from_reader(&mut File::open(cover)?).map_err(metadata_error)?;
            picture.set_pic_type(PictureType::CoverFront);
            picture.set_description(Some("Cover".into()));
            tag.remove_picture_type(PictureType::CoverFront);
            tag.push_picture(picture);
        }
    }
    cancel.check()?;
    tag.save_to_path(path, WriteOptions::default().use_id3v23(false))
        .map_err(metadata_error)?;
    cancel.check()
}
