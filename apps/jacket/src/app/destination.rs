//! Translate native picker paths into the engine's destination contract.
use audio_engine::OutputKind;
use std::path::PathBuf;

pub(super) fn resolve(kind: OutputKind, title: &str, picked: PathBuf) -> PathBuf {
    match kind {
        OutputKind::Folder => picked.join(audio_engine::export::sanitize_component(title)),
        OutputKind::File => {
            // The codec is resolved later. Accept a familiar suffix typed into
            // the picker without producing Song.mp3.m4a; preserve ordinary dots.
            let audio_suffix = picked
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| {
                    [
                        "mp3", "m4a", "aac", "mp4", "opus", "ogg", "oga", "flac", "wav", "aiff",
                        "aif", "webm",
                    ]
                    .iter()
                    .any(|extension| value.eq_ignore_ascii_case(extension))
                });
            if audio_suffix {
                picked.with_extension("")
            } else {
                picked
            }
        }
        OutputKind::Zip => {
            if picked
                .extension()
                .is_some_and(|value| value.eq_ignore_ascii_case("zip"))
            {
                picked
            } else if let Some(name) = picked.file_name() {
                let mut name = name.to_os_string();
                name.push(".zip");
                picked.with_file_name(name)
            } else {
                picked
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use audio_engine::OutputKind;
    use std::path::PathBuf;

    #[test]
    fn audio_picker_accepts_typed_extensions_and_preserves_dotted_stems() {
        for (picked, expected) in [
            ("/music/Song.MP3", "/music/Song"),
            ("/music/Dr. Song.m4a", "/music/Dr. Song"),
            ("/music/Dr. Song", "/music/Dr. Song"),
            ("/music/Live at 7.30", "/music/Live at 7.30"),
        ] {
            assert_eq!(
                resolve(OutputKind::File, "unused", picked.into()),
                PathBuf::from(expected)
            );
        }
    }

    #[test]
    fn archive_picker_adds_the_suffix_without_replacing_dots() {
        for (picked, expected) in [
            ("/music/Dr. Song", "/music/Dr. Song.zip"),
            ("/music/Song.ZIP", "/music/Song.ZIP"),
        ] {
            assert_eq!(
                resolve(OutputKind::Zip, "unused", picked.into()),
                PathBuf::from(expected)
            );
        }
        assert_eq!(
            resolve(OutputKind::Folder, "Album / one", "/music".into()),
            PathBuf::from("/music/Album _ one")
        );
    }
}
