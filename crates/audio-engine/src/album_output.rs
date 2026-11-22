//! Inspect an organized album without writing to the library or downloading audio.
use crate::{
    source::{validate_source_url, video_id},
    AudioFormat, CollectionMode, EngineError, ExportPlan, OrganizedOutput, OutputKind, Result,
};
use lofty::{config::ParseOptions, file::TaggedFileExt, probe::Probe, tag::ItemKey};
use std::{
    collections::{HashMap, HashSet},
    fs,
    io::ErrorKind,
    path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlbumTrackState {
    Keep,
    Add,
    AddCopy,
}

#[derive(Clone, Debug)]
pub struct AlbumTrackPreview {
    pub name: String,
    pub state: AlbumTrackState,
    pub extension_provisional: bool,
}

#[derive(Clone, Debug)]
pub struct AlbumPreview {
    pub tracks: Vec<AlbumTrackPreview>,
}

impl AlbumPreview {
    pub fn kept(&self) -> usize {
        self.tracks
            .iter()
            .filter(|track| track.state == AlbumTrackState::Keep)
            .count()
    }
}

pub fn preview_album_output(plan: &ExportPlan, output: &OrganizedOutput) -> Result<AlbumPreview> {
    let stems = output
        .album_stems()
        .filter(|stems| stems.len() == plan.tracks.len())
        .filter(|_| plan.mode == CollectionMode::Album && plan.output_kind == OutputKind::Folder)
        .ok_or_else(|| EngineError::Invalid("Choose an album folder export.".into()))?;
    output.check()?;
    let (extension, provisional) = match plan.format {
        AudioFormat::Mp3 => ("mp3", false),
        AudioFormat::Compatible => ("mp3-or-m4a", true),
        AudioFormat::Preserve => ("audio", true),
    };
    let mut occupied = HashSet::new();
    let mut existing = HashMap::<String, Vec<String>>::new();
    match fs::read_dir(output.path()) {
        Ok(entries) => {
            for entry in entries {
                let path = entry?.path();
                if path
                    .extension()
                    .and_then(|value| value.to_str())
                    .is_some_and(|ext| {
                        matches!(
                            ext.to_ascii_lowercase().as_str(),
                            "mp3"
                                | "m4a"
                                | "mp4"
                                | "aac"
                                | "opus"
                                | "ogg"
                                | "oga"
                                | "flac"
                                | "wav"
                                | "aif"
                                | "aiff"
                                | "webm"
                        )
                    })
                {
                    if let Some(stem) = path.file_stem().and_then(|value| value.to_str()) {
                        occupied.insert(stem.to_lowercase());
                        if let Some(name) = path.file_name().and_then(|value| value.to_str()) {
                            if !name.contains(['\r', '\n', '\\', ':']) {
                                if let Some(source) = saved_source(&path) {
                                    existing.entry(source).or_default().push(name.to_owned());
                                }
                            }
                        }
                    }
                }
            }
        }
        Err(error) if error.kind() == ErrorKind::NotFound => (),
        Err(error) => return Err(error.into()),
    }
    for names in existing.values_mut() {
        names.sort();
    }
    let mut tracks = Vec::with_capacity(stems.len());
    for (stem, track) in stems.iter().zip(&plan.tracks) {
        let saved = source_key(&track.source_url)
            .and_then(|source| existing.get_mut(&source))
            .filter(|names| !names.is_empty());
        if let Some(names) = saved {
            let preferred = names.iter().position(|name| {
                Path::new(name).file_stem().is_some_and(|name| {
                    name.to_string_lossy().to_lowercase() == stem.to_lowercase()
                })
            });
            tracks.push(AlbumTrackPreview {
                name: names.remove(preferred.unwrap_or(0)),
                state: AlbumTrackState::Keep,
                extension_provisional: false,
            });
            continue;
        }
        let mut candidate = stem.clone();
        let mut suffix = 2;
        while !occupied.insert(candidate.to_lowercase()) {
            candidate = format!("{stem} ({suffix})");
            suffix += 1;
        }
        tracks.push(AlbumTrackPreview {
            name: format!("{candidate}.{extension}"),
            state: if suffix == 2 {
                AlbumTrackState::Add
            } else {
                AlbumTrackState::AddCopy
            },
            extension_provisional: provisional,
        });
    }
    Ok(AlbumPreview { tracks })
}

fn source_key(input: &str) -> Option<String> {
    let valid = validate_source_url(input).ok()?;
    let url = url::Url::parse(&valid).ok()?;
    Some(video_id(&url).unwrap_or(valid))
}

pub(crate) fn open_existing_track(path: &Path) -> std::io::Result<fs::File> {
    let changed =
        || std::io::Error::new(ErrorKind::InvalidData, "An existing album track changed.");
    if !fs::symlink_metadata(path)?.is_file() {
        return Err(changed());
    }
    let mut options = fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(changed());
    }
    Ok(file)
}

fn saved_source(path: &Path) -> Option<String> {
    let file = open_existing_track(path).ok()?;
    let tagged = Probe::new(file)
        .options(
            ParseOptions::new()
                .read_properties(false)
                .read_cover_art(false),
        )
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    // WAV can expose the source comment via RIFF INFO instead of its primary tag.
    tagged
        .tags()
        .iter()
        .find_map(|tag| tag.get_string(ItemKey::Comment).and_then(source_key))
}
