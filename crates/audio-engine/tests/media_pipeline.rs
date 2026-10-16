//! Offline integration tests using real FFmpeg and generated media.
//! Run with: cargo test -p audio-engine --test media_pipeline -- --include-ignored
use audio_engine::{
    export::{hash_file, prepare_audio, AudioPreparation},
    *,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, io::Read, path::Path, process::Command};

fn tools() -> Toolchain {
    if let Some(resources) = std::env::var_os("AUDIO_DL_TEST_RESOURCES") {
        return Toolchain::discover_with_resources(Path::new(&resources))
            .expect("Provide a complete bundled test runtime");
    }
    Toolchain::discover().expect("Install yt-dlp and FFmpeg to run the media integration tests")
}

fn make_audio(tools: &Toolchain, path: &Path, codec: &str) {
    let output = Command::new(&tools.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.25",
            "-c:a",
            codec,
            "-ac",
            "2",
            "-strict",
            "experimental",
            "-threads",
            "1",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn physical_tags(path: &Path) -> Value {
    let python = std::env::var_os("AUDIO_DL_TAG_READER_PYTHON").unwrap_or_else(|| "python3".into());
    let output = Command::new(python)
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/inspect_tags.py"))
        .arg(path)
        .output()
        .expect("Python is required for independent physical-tag tests");
    assert!(
        output.status.success(),
        "Install tests/requirements.txt into a venv and set AUDIO_DL_TAG_READER_PYTHON. {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn audio_payloads(tools: &Toolchain, path: &Path) -> Vec<String> {
    let output = Command::new(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-select_streams",
            "a:0",
            "-show_packets",
            "-show_data_hash",
            "sha256",
            "-show_entries",
            "packet=data_hash",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).unwrap();
    result["packets"]
        .as_array()
        .unwrap()
        .iter()
        .map(|packet| packet["data_hash"].as_str().unwrap().to_owned())
        .collect()
}

fn decoded_audio_hash(tools: &Toolchain, path: &Path) -> String {
    let output = Command::new(&tools.ffmpeg)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args([
            "-map",
            "0:a:0",
            "-c:a",
            "pcm_s16le",
            "-f",
            "hash",
            "-hash",
            "sha256",
            "-",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn make_cover(tools: &Toolchain, path: &Path) {
    let output = Command::new(&tools.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "testsrc=size=96x64",
            "-frames:v",
            "1",
            "-threads",
            "1",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
#[ignore = "Requires FFmpeg, yt-dlp, and Python with Mutagen"]
fn physical_tags_and_cover_blocks_follow_each_container_and_keep_audio_payloads() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let source_url = "https://www.youtube.com/watch?v=fixture";
    let metadata = Metadata {
        title: "Björk — 東京".into(),
        ..tags()
    };
    for (cover_ext, mime) in [("jpg", "image/jpeg"), ("png", "image/png")] {
        let cover = dir.path().join(format!("cover.{cover_ext}"));
        make_cover(&tools, &cover);
        for (ext, encoder) in [
            ("mp3", "libmp3lame"),
            ("m4a", "aac"),
            ("flac", "flac"),
            ("opus", "libopus"),
            ("ogg", "vorbis"),
        ] {
            let source = dir.path().join(format!("source.{ext}"));
            let output = dir.path().join(format!("output.{ext}"));
            make_audio(&tools, &source, encoder);
            prepare_audio(
                &tools,
                AudioPreparation {
                    source: &source,
                    output: &output,
                    metadata: &metadata,
                    source_url,
                    cover: Some(&cover),
                    format: AudioFormat::Preserve,
                },
                &Cancellation::default(),
            )
            .unwrap();
            let result = physical_tags(&output);
            let tag = &result["tags"];
            match ext {
                "mp3" => {
                    assert_eq!(result["id3Version"], json!([2, 4, 0]));
                    for (key, value) in [
                        ("TIT2", metadata.title.as_str()),
                        ("TPE1", "Artist & Guest"),
                        ("TALB", "An album"),
                        ("TPE2", "Album artist"),
                        ("TRCK", "7"),
                        ("TPOS", "2"),
                        ("TDRC", "2024"),
                    ] {
                        assert_eq!(tag[key][0], value, "{ext}: {key}");
                    }
                    assert_eq!(tag["COMM"].as_array().unwrap().len(), 1);
                    assert_eq!(tag["COMM"][0]["text"][0], source_url);
                    assert!(tag.get("TXXX:comment").is_none());
                }
                "m4a" => {
                    for (key, value) in [
                        ("©nam", metadata.title.as_str()),
                        ("©ART", "Artist & Guest"),
                        ("©alb", "An album"),
                        ("aART", "Album artist"),
                        ("©day", "2024"),
                        ("©cmt", source_url),
                    ] {
                        assert_eq!(tag[key][0], value, "{ext}: {key}");
                    }
                    assert_eq!(tag["trkn"], json!([[7, 0]]));
                    assert_eq!(tag["disk"], json!([[2, 0]]));
                }
                _ => {
                    for (key, value) in [
                        ("TITLE", metadata.title.as_str()),
                        ("ARTIST", "Artist & Guest"),
                        ("ALBUM", "An album"),
                        ("ALBUMARTIST", "Album artist"),
                        ("TRACKNUMBER", "7"),
                        ("DISCNUMBER", "2"),
                        ("DATE", "2024"),
                        ("COMMENT", source_url),
                    ] {
                        assert_eq!(tag[key][0], value, "{ext}: {key}");
                    }
                    assert!(tag.get("DESCRIPTION").is_none());
                }
            }
            assert_eq!(result["pictures"].as_array().unwrap().len(), 1, "{ext}");
            assert_eq!(result["pictures"][0]["type"], 3, "{ext}: front cover");
            assert_eq!(result["pictures"][0]["mime"], mime, "{ext}: {cover_ext}");
            assert_eq!(
                result["pictures"][0]["bytes"],
                fs::metadata(&cover).unwrap().len(),
                "{ext}: {cover_ext} cover size"
            );
            assert_eq!(
                result["pictures"][0]["sha256"],
                hash_file(&cover).unwrap(),
                "{ext}: {cover_ext} cover bytes changed"
            );
            assert_eq!(
                audio_payloads(&tools, &source),
                audio_payloads(&tools, &output),
                "Tagging changed encoded audio in {ext}"
            );
            assert_eq!(
                decoded_audio_hash(&tools, &source),
                decoded_audio_hash(&tools, &output),
                "Tagging changed decoded samples in {ext}"
            );
        }
    }
}

#[test]
#[ignore = "Requires FFmpeg, yt-dlp, and Python with Mutagen"]
fn clearing_music_tags_and_artwork_does_not_resurrect_source_metadata() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let cover = dir.path().join("cover.jpg");
    make_cover(&tools, &cover);
    let metadata = Metadata {
        title: "Only title".into(),
        ..Metadata::default()
    };
    let source_url = "https://www.youtube.com/watch?v=new-source";
    for (ext, encoder) in [
        ("mp3", "libmp3lame"),
        ("m4a", "aac"),
        ("flac", "flac"),
        ("opus", "libopus"),
        ("ogg", "vorbis"),
    ] {
        let raw = dir.path().join(format!("raw.{ext}"));
        let source = dir.path().join(format!("source.{ext}"));
        let output = dir.path().join(format!("output.{ext}"));
        make_audio(&tools, &raw, encoder);
        prepare_audio(
            &tools,
            AudioPreparation {
                source: &raw,
                output: &source,
                metadata: &tags(),
                source_url: "https://www.youtube.com/watch?v=old-source",
                cover: Some(&cover),
                format: AudioFormat::Preserve,
            },
            &Cancellation::default(),
        )
        .unwrap();
        prepare_audio(
            &tools,
            AudioPreparation {
                source: &source,
                output: &output,
                metadata: &metadata,
                source_url,
                cover: None,
                format: AudioFormat::Preserve,
            },
            &Cancellation::default(),
        )
        .unwrap();
        let result = physical_tags(&output);
        let tag = &result["tags"];
        let absent: &[&str] = match ext {
            "mp3" => &[
                "TPE1",
                "TALB",
                "TPE2",
                "TRCK",
                "TPOS",
                "TDRC",
                "TXXX:comment",
            ],
            "m4a" => &["©ART", "©alb", "aART", "trkn", "disk", "©day"],
            _ => &[
                "ARTIST",
                "ALBUM",
                "ALBUMARTIST",
                "TRACKNUMBER",
                "DISCNUMBER",
                "DATE",
                "DESCRIPTION",
            ],
        };
        for key in absent {
            assert!(tag.get(*key).is_none(), "{ext} retained {key}: {tag}");
        }
        let comment = match ext {
            "mp3" => &tag["COMM"][0]["text"][0],
            "m4a" => &tag["©cmt"][0],
            _ => &tag["COMMENT"][0],
        };
        assert_eq!(comment, source_url);
        assert!(
            result["pictures"].as_array().unwrap().is_empty(),
            "{ext} retained old artwork"
        );
        assert_eq!(
            audio_payloads(&tools, &source),
            audio_payloads(&tools, &output)
        );
    }
}

#[test]
#[ignore = "Requires FFmpeg, yt-dlp, and Python with Mutagen"]
fn wav_export_warns_and_manifest_separates_requested_and_embedded_fields() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let url = "https://www.youtube.com/watch?v=fixture";
    let source_dir = dir
        .path()
        .join("sources")
        .join(format!("{:x}", Sha256::digest(url.as_bytes())));
    fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("source.wav");
    make_audio(&tools, &source, "pcm_s16le");
    fs::write(
        source_dir.join("receipt.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let metadata = tags();
    let track = Track {
        id: "fixture".into(),
        source_url: url.into(),
        uploader: "Channel".into(),
        duration: Some(0.25),
        artwork_url: None,
        source_metadata: metadata.clone(),
        metadata,
    };
    let plan = ExportPlan {
        title: "WAV review".into(),
        source_url: url.into(),
        mode: CollectionMode::Album,
        format: AudioFormat::Preserve,
        artwork_mode: ArtworkMode::None,
        shared_artwork_url: None,
        auth: AuthOptions::default(),
        tracks: vec![track],
    };
    let output = dir.path().join("output.zip");
    let result = export_archive(
        &tools,
        &plan,
        &output,
        dir.path(),
        &Cancellation::default(),
        |_| {},
    )
    .unwrap();
    assert_eq!(result.warnings.len(), 1);
    assert!(result.warnings[0].contains("album artist and disc number"));
    let tags = physical_tags(&dir.path().join("bundle/07 - Björk _ Live.wav"));
    for (key, value) in [
        ("INAM", "Björk / Live"),
        ("IART", "Artist & Guest"),
        ("IPRD", "An album"),
        ("IPRT", "7"),
        ("ICRD", "2024"),
        ("ICMT", url),
    ] {
        assert_eq!(tags["tags"][key][0], value);
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(dir.path().join("bundle/manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["tracks"][0]["metadata"]["albumArtist"],
        "Album artist"
    );
    assert_eq!(manifest["tracks"][0]["metadata"]["discNumber"], 2);
    assert_eq!(manifest["tracks"][0]["embeddedMetadata"]["albumArtist"], "");
    assert_eq!(
        manifest["tracks"][0]["embeddedMetadata"]["discNumber"],
        Value::Null
    );
    assert_eq!(manifest["tracks"][0]["embeddedArtwork"], false);
    assert_eq!(manifest["warnings"], json!(result.warnings));
}

fn probe(tools: &Toolchain, path: &Path) -> Value {
    let output = Command::new(&tools.ffprobe)
        .args([
            "-v",
            "error",
            "-show_format",
            "-show_streams",
            "-of",
            "json",
        ])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    serde_json::from_slice(&output.stdout).unwrap()
}

fn tags() -> Metadata {
    Metadata {
        title: "Björk / Live".into(),
        artist: "Artist & Guest".into(),
        album: "An album".into(),
        album_artist: "Album artist".into(),
        track_number: Some(7),
        disc_number: Some(2),
        release_year: Some(2024),
    }
}

#[test]
#[ignore = "Requires locally installed FFmpeg and yt-dlp"]
fn compatible_output_preserves_aac_and_embeds_cover_and_real_tags() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.m4a");
    make_audio(&tools, &source, "aac");
    let cover = dir.path().join("cover.jpg");
    let generated = Command::new(&tools.ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "lavfi",
            "-i",
            "color=c=green:s=32x32",
            "-frames:v",
            "1",
            "-threads",
            "1",
        ])
        .arg(&cover)
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    for (format, filename, expected) in [
        (AudioFormat::Compatible, "result.m4a", "aac"),
        (AudioFormat::Mp3, "result.mp3", "mp3"),
    ] {
        let output = dir.path().join(filename);
        let output_codec = prepare_audio(
            &tools,
            AudioPreparation {
                source: &source,
                output: &output,
                metadata: &tags(),
                source_url: "https://www.youtube.com/watch?v=fixture",
                cover: Some(&cover),
                format,
            },
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(output_codec, expected);
        let result = probe(&tools, &output);
        let metadata = &result["format"]["tags"];
        assert_eq!(metadata["title"], "Björk / Live");
        assert_eq!(metadata["artist"], "Artist & Guest");
        assert_eq!(metadata["album_artist"], "Album artist");
        assert_eq!(metadata["track"], "7");
        assert_eq!(metadata["disc"], "2");
        assert_eq!(metadata["date"], "2024");
        assert!(result["streams"]
            .as_array()
            .unwrap()
            .iter()
            .any(|stream| stream["disposition"]["attached_pic"] == 1));
    }
}

#[test]
#[ignore = "Requires locally installed FFmpeg and yt-dlp"]
fn preserve_output_keeps_opus_and_does_not_invent_music_tags() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source.opus");
    make_audio(&tools, &source, "libopus");
    let output = dir.path().join("output.opus");
    prepare_audio(
        &tools,
        AudioPreparation {
            source: &source,
            output: &output,
            metadata: &Metadata {
                title: "Live set".into(),
                ..Metadata::default()
            },
            source_url: "https://www.youtube.com/watch?v=fixture",
            cover: None,
            format: AudioFormat::Preserve,
        },
        &Cancellation::default(),
    )
    .unwrap();
    let result = probe(&tools, &output);
    assert_eq!(result["streams"][0]["codec_name"], "opus");
    let metadata = result["streams"][0]["tags"].as_object().unwrap();
    assert!(!metadata.keys().any(
        |key| ["ARTIST", "ALBUM", "TRACKNUMBER", "DATE"].contains(&key.to_uppercase().as_str())
    ));
}

#[test]
#[ignore = "Requires locally installed FFmpeg and yt-dlp"]
fn archive_reuses_sources_preserves_repeats_and_records_original_metadata() {
    let tools = tools();
    let dir = tempfile::tempdir().unwrap();
    let url = "https://www.youtube.com/watch?v=fixture";
    // Seed the same receipt used after a successful download. No network or
    // browser access occurs; this exercises the engine's actual retry path.
    let source_dir = dir
        .path()
        .join("sources")
        .join(format!("{:x}", Sha256::digest(url.as_bytes())));
    fs::create_dir_all(&source_dir).unwrap();
    let source = source_dir.join("source.m4a");
    make_audio(&tools, &source, "aac");
    fs::write(
        source_dir.join("receipt.json"),
        serde_json::to_vec(&source).unwrap(),
    )
    .unwrap();
    let track = Track {
        id: "fixture".into(),
        source_url: url.into(),
        uploader: "A channel".into(),
        duration: Some(0.25),
        artwork_url: None,
        source_metadata: Metadata {
            title: "Original title".into(),
            ..Metadata::default()
        },
        metadata: tags(),
    };
    let plan = ExportPlan {
        title: "../My album".into(),
        source_url: "https://www.youtube.com/playlist?list=fixture".into(),
        mode: CollectionMode::Album,
        format: AudioFormat::Compatible,
        artwork_mode: ArtworkMode::None,
        shared_artwork_url: None,
        auth: AuthOptions::default(),
        tracks: vec![track.clone(), track],
    };
    let output = dir.path().join("result.zip");
    let mut events = Vec::new();
    let result = export_archive(
        &tools,
        &plan,
        &output,
        dir.path(),
        &Cancellation::default(),
        |event| events.push(event),
    )
    .unwrap();
    assert_eq!(result.track_count, 2);
    assert_eq!(events.last().unwrap().stage, "packaging");
    let mut zip = zip::ZipArchive::new(fs::File::open(&output).unwrap()).unwrap();
    assert_eq!(zip.len(), 4);
    let mut contents = String::new();
    zip.by_name(".._My album/manifest.json")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    let manifest: Value = serde_json::from_str(&contents).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(
        manifest["tracks"][0]["relativePath"],
        "07 - Björk _ Live.m4a"
    );
    assert_eq!(
        manifest["tracks"][1]["relativePath"],
        "07 - Björk _ Live (2).m4a"
    );
    assert_eq!(
        manifest["tracks"][0]["sourceMetadata"]["title"],
        "Original title"
    );
    assert_eq!(manifest["tracks"][0]["metadata"]["trackNumber"], 7);
    assert_eq!(
        manifest["tracks"][0]["sha256"],
        json!(hash_file(&dir.path().join("bundle/07 - Björk _ Live.m4a")).unwrap())
    );
    contents.clear();
    zip.by_name(".._My album/playlist.m3u")
        .unwrap()
        .read_to_string(&mut contents)
        .unwrap();
    assert!(contents.contains("07 - Björk _ Live (2).m4a"));
}
