use audio_engine::{CollectionMode, Inspection, Metadata, Track};

/// A disposable, embedded brand asset for isolated crop-layout qualification.
pub fn cover() -> std::io::Result<(
    jacket_desktop::artwork_assets::Asset,
    tempfile::NamedTempFile,
)> {
    use std::io::Write;
    let mut file = tempfile::Builder::new().suffix(".png").tempfile()?;
    file.write_all(include_bytes!(
        "../../../branding/jacket/png/jacket-icon-256.png"
    ))?;
    let asset = jacket_desktop::artwork_assets::Asset {
        id: "example-cover".into(),
        preview_data_url: String::new(),
        width: 256,
        height: 256,
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
        warnings: vec![
            "Fictional example. Downloads are disabled; your saved review is kept.".into(),
        ],
    }
}
