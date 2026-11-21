use audio_engine::{CollectionMode, Inspection, Metadata, Track};

/// A disposable album cover for isolated artwork-layout qualification.
pub fn cover() -> std::io::Result<(
    jacket_desktop::artwork_assets::Asset,
    tempfile::NamedTempFile,
)> {
    use std::io::Write;
    let mut file = tempfile::Builder::new().suffix(".png").tempfile()?;
    file.write_all(include_bytes!(
        "../../../assets/examples/room-to-breathe.png"
    ))?;
    let asset = jacket_desktop::artwork_assets::Asset {
        id: audio_engine::export::hash_file(file.path())
            .map_err(|error| std::io::Error::other(error.to_string()))?,
        preview_data_url: String::new(),
        width: 640,
        height: 640,
    };
    Ok((asset, file))
}

pub fn album() -> Inspection {
    let titles = [
        "First light",
        "A little further",
        "Small hours",
        "Room to breathe",
        "Home again",
    ];
    Inspection {
        title: "Room to breathe".into(),
        source_url: "https://music.youtube.com/playlist?list=JACKET_EXAMPLE".into(),
        suggested_mode: CollectionMode::Album,
        artwork_url: None,
        album_release_year: Some(2026),
        tracks: titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let metadata = Metadata {
                    title: (*title).into(),
                    artist: "The Paper Trails".into(),
                    album: "Room to breathe".into(),
                    album_artist: "The Paper Trails".into(),
                    track_number: Some(index as u32 + 1),
                    disc_number: Some(1),
                    release_year: Some(2026),
                };
                Track {
                    id: format!("example-{index}"),
                    source_url: "https://www.youtube.com/watch?v=EXAMPLEONLY".into(),
                    uploader: "The Paper Trails".into(),
                    duration: Some(185.0 + index as f64 * 13.0),
                    artwork_url: None,
                    source_metadata: metadata.clone(),
                    metadata,
                }
            })
            .collect(),
        warnings: vec!["Sample album. Downloads are disabled.".into()],
    }
}
