use crate::{
    models::*,
    process::{run, Cancellation, Toolchain},
    source::{downloader, valid_artwork_url, validate_source_url},
    tagging::{embedded_metadata, finish_tags, metadata_warnings, supports_embedded_artwork},
    EngineError, Result,
};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

pub fn sanitize_component(input: &str) -> String {
    let replaced: String = input
        .chars()
        .map(|c| {
            if c.is_whitespace() {
                ' '
            } else if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    let compact = replaced.split_whitespace().collect::<Vec<_>>().join(" ");
    // Keep components comfortably below common filesystem byte limits.
    let mut shortened = String::new();
    for c in compact.chars() {
        if shortened.len() + c.len_utf8() > 160 {
            break;
        }
        shortened.push(c);
    }
    let result = shortened.trim().trim_end_matches(['.', ' ']);
    if result.is_empty() || result == "." || result == ".." {
        "Untitled".into()
    } else {
        result.to_owned()
    }
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn hash_file(path: &Path) -> Result<String> {
    let mut file = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn codec(tools: &Toolchain, path: &Path, cancel: &Cancellation) -> Result<String> {
    let bytes = run(
        Command::new(&tools.ffprobe)
            .args([
                "-v",
                "error",
                "-select_streams",
                "a:0",
                "-show_entries",
                "stream=codec_name",
                "-of",
                "csv=p=0",
            ])
            .arg(path),
        cancel,
        Duration::from_secs(30),
    )?;
    let codec = String::from_utf8_lossy(&bytes).trim().to_owned();
    if codec.is_empty() {
        Err(EngineError::Invalid(
            "Downloaded file has no audio stream.".into(),
        ))
    } else {
        Ok(codec)
    }
}

fn target(codec: &str, format: AudioFormat) -> Result<(&'static str, bool)> {
    match format {
        AudioFormat::Mp3 => Ok(("mp3", codec == "mp3")),
        AudioFormat::Compatible => match codec {
            "aac" | "alac" => Ok(("m4a", true)),
            _ => Ok(("mp3", codec == "mp3")),
        },
        AudioFormat::Preserve => match codec {
            "aac" | "alac" => Ok(("m4a", true)),
            "mp3" => Ok(("mp3", true)),
            "opus" => Ok(("opus", true)),
            "vorbis" => Ok(("ogg", true)),
            "flac" => Ok(("flac", true)),
            "pcm_s16le" | "pcm_s24le" => Ok(("wav", true)),
            other => Err(EngineError::Invalid(format!(
                "Preserving {other} is not supported yet. Choose the compatibility preset."
            ))),
        },
    }
}

// yt-dlp's media URLs can expire or fail on one server. Re-extract the original
// URL on a bounded retry, keeping its partial download and completed receipts.
fn transient_download_failure(error: &EngineError) -> bool {
    let detail = match error {
        EngineError::Tool(detail) => detail.to_lowercase(),
        EngineError::Invalid(detail) if detail == "Audio tool timed out; retry this item." => {
            return true;
        }
        _ => return false,
    };
    // These require a different source or access; a fresh extraction cannot
    // resolve them even if earlier warnings mention a network failure.
    if [
        "sign in",
        "sign-in",
        "login required",
        "authentication required",
        "http error 401",
        "confirm your age",
        "age-restricted",
        "private video",
        "video unavailable",
        "video is unavailable",
        "has been removed",
        "has been deleted",
        "members-only",
        "members only",
        "not available in your country",
        "not available in your region",
        "copyright",
        "unsupported url",
        "requested format is not available",
    ]
    .iter()
    .any(|pattern| detail.contains(pattern))
    {
        return false;
    }
    let temporary_status = [403, 408, 429].into_iter().chain(500..600).any(|status| {
        detail.contains(&format!("http error {status}"))
            || detail.contains(&format!("http status {status}"))
            || detail.contains(&format!("status code {status}"))
    });
    temporary_status
        || [
            "timed out",
            "timeout",
            "connection reset",
            "connection refused",
            "connection aborted",
            "remote end closed connection",
            "remote disconnected",
            "temporary failure in name resolution",
            "name or service not known",
            "nodename nor servname provided",
            "network is unreachable",
            "network unreachable",
            "incomplete read",
            "incompleteread",
        ]
        .iter()
        .any(|pattern| detail.contains(pattern))
}

fn wait_for_retry(cancel: &Cancellation, delay: Duration) -> Result<()> {
    let started = Instant::now();
    loop {
        cancel.check()?;
        let remaining = delay.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Ok(());
        }
        thread::sleep(remaining.min(Duration::from_millis(50)));
    }
}

fn download_with_recovery<T>(
    cancel: &Cancellation,
    mut attempt: impl FnMut() -> Result<T>,
    mut on_retry: impl FnMut(),
) -> Result<T> {
    let delays = [Duration::from_secs(1), Duration::from_secs(3)];
    let mut retries = 0;
    loop {
        cancel.check()?;
        match attempt() {
            Err(error) if retries < delays.len() && transient_download_failure(&error) => {
                on_retry();
                wait_for_retry(cancel, delays[retries])?;
                retries += 1;
            }
            outcome => return outcome,
        }
    }
}

fn acquire(
    tools: &Toolchain,
    track: &Track,
    auth: &AuthOptions,
    workdir: &Path,
    cancel: &Cancellation,
    on_retry: impl FnMut(),
) -> Result<PathBuf> {
    let source_url = validate_source_url(&track.source_url)?;
    let dir = workdir
        .join("sources")
        .join(hash_bytes(source_url.as_bytes()));
    fs::create_dir_all(&dir)?;
    let receipt = dir.join("receipt.json");
    if let Ok(bytes) = fs::read(&receipt) {
        if let Ok(path) = serde_json::from_slice::<PathBuf>(&bytes) {
            if path.starts_with(&dir) && path.is_file() && codec(tools, &path, cancel).is_ok() {
                return Ok(path);
            }
        }
    }
    let output = download_with_recovery(
        cancel,
        || {
            // Build a new command each time so yt-dlp refreshes extraction instead
            // of reusing the failed media URL. Its default partial-file resume is
            // retained; no completed source or receipt is removed on failure.
            let mut cmd = downloader(tools, auth)?;
            cmd.args([
                "--no-playlist",
                "--format",
                "bestaudio/best",
                "--extract-audio",
                "--audio-format",
                "best",
                "--fragment-retries",
                "1",
                "--retries",
                "1",
                "--extractor-retries",
                "1",
                "--socket-timeout",
                "20",
                "--print",
                "after_move:filepath",
            ])
            .arg("--ffmpeg-location")
            .arg(tools.ffmpeg.parent().unwrap_or(Path::new(".")))
            .arg("--output")
            .arg(dir.join("source.%(ext)s"))
            .args(["--", &source_url]);
            run(&mut cmd, cancel, Duration::from_secs(1800))
        },
        on_retry,
    )?;
    let path = String::from_utf8_lossy(&output)
        .lines()
        .rev()
        .map(PathBuf::from)
        .find(|path| path.starts_with(&dir) && path.is_file())
        .ok_or_else(|| {
            EngineError::Invalid("Downloader did not report a completed audio file.".into())
        })?;
    codec(tools, &path, cancel)?;
    fs::write(receipt, serde_json::to_vec(&path)?)?;
    Ok(path)
}

fn artwork(tools: &Toolchain, source: &str, dir: &Path, cancel: &Cancellation) -> Result<PathBuf> {
    if !valid_artwork_url(source) {
        return Err(EngineError::Invalid(
            "Artwork must be a YouTube source image.".into(),
        ));
    }
    cancel.check()?;
    fs::create_dir_all(dir)?;
    let output = dir.join(format!("{}.jpg", hash_bytes(source.as_bytes())));
    if output.is_file() {
        return Ok(output);
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !valid_artwork_url(attempt.url().as_str()) {
                attempt.stop()
            } else {
                attempt.follow()
            }
        }))
        .build()?;
    let response = client.get(source).send()?.error_for_status()?;
    let mut bytes = Vec::new();
    response
        .take(10 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(EngineError::Invalid(
            "Artwork exceeds the 10 MiB image limit.".into(),
        ));
    }
    let raw = dir.join("image-input");
    fs::write(&raw, bytes)?;
    run(
        Command::new(&tools.ffmpeg)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
            .arg(&raw)
            .args([
                "-frames:v",
                "1",
                "-vf",
                "scale=2000:2000:force_original_aspect_ratio=decrease",
                "-q:v",
                "2",
            ])
            .arg(&output),
        cancel,
        Duration::from_secs(30),
    )?;
    Ok(output)
}

pub struct AudioPreparation<'a> {
    pub source: &'a Path,
    pub output: &'a Path,
    pub metadata: &'a Metadata,
    pub source_url: &'a str,
    pub cover: Option<&'a Path>,
    pub format: AudioFormat,
}

pub fn prepare_audio(
    tools: &Toolchain,
    preparation: AudioPreparation<'_>,
    cancel: &Cancellation,
) -> Result<String> {
    let AudioPreparation {
        source,
        output,
        metadata,
        source_url,
        cover,
        format,
    } = preparation;
    let source_codec = codec(tools, source, cancel)?;
    let (ext, copy) = target(&source_codec, format)?;
    let embed_cover = cover.is_some() && matches!(ext, "m4a" | "mp3" | "flac");
    let mut cmd = Command::new(&tools.ffmpeg);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(source);
    if embed_cover {
        cmd.arg("-i").arg(cover.unwrap());
    }
    cmd.args(["-map", "0:a:0", "-map_metadata", "-1"]);
    if copy {
        cmd.args(["-c:a", "copy"]);
    } else {
        cmd.args(["-c:a", "libmp3lame", "-q:a", "0"]);
    }
    if embed_cover {
        // Native picture containers accept both JPEG and PNG. Preserve the
        // normalized cover bytes so embedded art matches the archived image
        // without another lossy JPEG encode.
        cmd.args([
            "-map",
            "1:v:0",
            "-c:v",
            "copy",
            "-disposition:v:0",
            "attached_pic",
            "-metadata:s:v:0",
            "title=Cover",
            "-metadata:s:v:0",
            "comment=Cover (front)",
        ]);
    }
    let embedded = embedded_metadata(metadata, ext);
    for (key, value) in [
        ("title", metadata.title.as_str()),
        ("artist", &metadata.artist),
        ("album", &metadata.album),
        ("album_artist", &embedded.album_artist),
    ] {
        if !value.is_empty() {
            cmd.arg("-metadata").arg(format!("{key}={value}"));
        }
    }
    for (key, value) in [
        ("track", metadata.track_number),
        ("disc", embedded.disc_number),
        ("date", metadata.release_year),
    ] {
        if let Some(value) = value {
            cmd.arg("-metadata").arg(format!("{key}={value}"));
        }
    }
    if ext == "m4a" {
        cmd.args(["-f", "ipod"]);
    }
    if ext == "mp3" {
        cmd.args(["-id3v2_version", "4"]);
    }
    if ext == "wav" {
        cmd.arg("-metadata").arg(format!("comment={source_url}"));
    }
    cmd.arg(output);
    run(&mut cmd, cancel, Duration::from_secs(600))?;
    finish_tags(output, ext, source_url, cover, cancel)?;
    codec(tools, output, cancel)
}

pub fn available_archive_path(output: &Path) -> PathBuf {
    let stem = output.file_stem().unwrap_or_default().to_string_lossy();
    let extension = output.extension().unwrap_or_default().to_string_lossy();
    let mut candidate = output.to_owned();
    let mut number = 2;
    while candidate.symlink_metadata().is_ok() {
        candidate = output.with_file_name(format!("{stem} ({number}).{extension}"));
        number += 1;
    }
    candidate
}

pub fn write_archive(
    root_name: &str,
    files: &[(String, PathBuf)],
    output: &Path,
    cancel: &Cancellation,
) -> Result<PathBuf> {
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    {
        let mut writer = ZipWriter::new(temporary.as_file_mut());
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .unix_permissions(0o644);
        for (relative, path) in files {
            cancel.check()?;
            // Every name is generated internally; preserve that invariant if
            // other engine callers start providing file lists later.
            if relative.starts_with('/')
                || relative.split('/').any(|part| part == "..")
                || relative.contains('\\')
            {
                return Err(EngineError::Invalid("Unsafe archive entry".into()));
            }
            writer.start_file(
                format!("{}/{relative}", sanitize_component(root_name)),
                options,
            )?;
            let mut input = fs::File::open(path)?;
            let mut buffer = [0; 65536];
            loop {
                cancel.check()?;
                let count = input.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                writer.write_all(&buffer[..count])?;
            }
        }
        writer.finish()?;
    }
    temporary.as_file_mut().sync_all()?;
    cancel.check()?;
    // Select a free filename and publish without replacing a file that may
    // have appeared during the download. Retry collisions atomically.
    loop {
        cancel.check()?;
        let destination = available_archive_path(output);
        match temporary.persist_noclobber(&destination) {
            Ok(_) => return Ok(destination),
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                temporary = error.file;
            }
            Err(error) => return Err(error.error.into()),
        }
    }
}

pub fn export_archive(
    tools: &Toolchain,
    plan: &ExportPlan,
    output: &Path,
    workdir: &Path,
    cancel: &Cancellation,
    mut emit: impl FnMut(ProgressEvent),
) -> Result<ExportResult> {
    if plan.tracks.is_empty() {
        return Err(EngineError::Invalid("Select at least one track.".into()));
    }
    validate_source_url(&plan.source_url)?;
    let total = plan.tracks.len();
    let bundle = workdir.join("bundle");
    fs::create_dir_all(&bundle)?;
    let mut files = Vec::new();
    let mut names = std::collections::HashSet::new();
    let mut tracks = Vec::new();
    let mut warnings = Vec::new();
    let mut playlist = String::from("#EXTM3U\n");
    for (index, track) in plan.tracks.iter().enumerate() {
        cancel.check()?;
        emit(ProgressEvent {
            stage: "downloading".into(),
            message: format!("Downloading {}", track.metadata.title),
            completed: index,
            total,
        });
        let acquired = acquire(tools, track, &plan.auth, workdir, cancel, || {
            emit(ProgressEvent {
                stage: "downloading".into(),
                message: format!(
                    "Retrying {} after a connection problem",
                    track.metadata.title
                ),
                completed: index,
                total,
            });
        })?;
        let source_codec = codec(tools, &acquired, cancel)?;
        let (ext, _) = target(&source_codec, plan.format)?;
        let prefix = if plan.mode == CollectionMode::Album {
            track.metadata.track_number.unwrap_or((index + 1) as u32)
        } else {
            (index + 1) as u32
        };
        // Include a unique ordinal in artwork paths; repeated playlist entries
        // and duplicate track titles remain distinct and cannot overwrite.
        let stem = format!(
            "{:02} - {}",
            prefix,
            sanitize_component(&track.metadata.title)
        );
        let mut filename = format!("{stem}.{ext}");
        let mut duplicate = 2;
        while !names.insert(filename.to_lowercase()) {
            filename = format!("{stem} ({duplicate}).{ext}");
            duplicate += 1;
        }
        let chosen_artwork = match plan.artwork_mode {
            ArtworkMode::None => None,
            ArtworkMode::Shared => plan.shared_artwork_url.as_deref(),
            ArtworkMode::PerTrack => track.artwork_url.as_deref(),
        };
        let cover = if let Some(url) = chosen_artwork {
            match artwork(tools, url, &workdir.join("images"), cancel) {
                Ok(path) => Some(path),
                Err(EngineError::Cancelled) => return Err(EngineError::Cancelled),
                Err(_) => {
                    warnings.push(format!("Artwork unavailable for {}.", track.metadata.title));
                    None
                }
            }
        } else {
            None
        };
        emit(ProgressEvent {
            stage: "preparing".into(),
            message: format!("Tagging {}", track.metadata.title),
            completed: index,
            total,
        });
        let prepared = bundle.join(&filename);
        let output_codec = prepare_audio(
            tools,
            AudioPreparation {
                source: &acquired,
                output: &prepared,
                metadata: &track.metadata,
                source_url: &track.source_url,
                cover: cover.as_deref(),
                format: plan.format,
            },
            cancel,
        )?;
        let artwork_path = if let Some(cover) = &cover {
            let relative = if matches!(plan.artwork_mode, ArtworkMode::Shared) {
                "cover.jpg".into()
            } else {
                format!("artwork/{:02}.jpg", index + 1)
            };
            if !files.iter().any(|(name, _)| name == &relative) {
                files.push((relative.clone(), cover.clone()));
            }
            if !supports_embedded_artwork(ext) {
                warnings.push(format!("Artwork for {} is included as a separate image; embedding in {ext} is not implemented.", track.metadata.title));
            }
            Some(relative)
        } else {
            None
        };
        warnings.extend(metadata_warnings(&track.metadata, ext));
        playlist.push_str(&format!(
            "#EXTINF:{},{}\n{}\n",
            track.duration.unwrap_or(-1.0).round(),
            track.metadata.title.replace(['\n', '\r'], " "),
            filename
        ));
        tracks.push(ManifestTrack {
            source_id: track.id.clone(),
            source_url: track.source_url.clone(),
            source_metadata: track.source_metadata.clone(),
            metadata: track.metadata.clone(),
            embedded_metadata: Some(embedded_metadata(&track.metadata, ext)),
            embedded_artwork: Some(cover.is_some() && supports_embedded_artwork(ext)),
            relative_path: filename.clone(),
            artwork_path,
            source_codec,
            output_codec,
            sha256: hash_file(&prepared)?,
        });
        files.push((filename, prepared));
        emit(ProgressEvent {
            stage: "preparing".into(),
            message: format!("Prepared {}", track.metadata.title),
            completed: index + 1,
            total,
        });
    }
    let manifest_path = bundle.join("manifest.json");
    fs::write(
        &manifest_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1, "title": plan.title, "sourceUrl": plan.source_url, "mode": plan.mode, "tracks": tracks, "warnings": warnings
        }))?,
    )?;
    let playlist_path = bundle.join("playlist.m3u");
    fs::write(&playlist_path, playlist)?;
    files.extend([
        ("manifest.json".into(), manifest_path),
        ("playlist.m3u".into(), playlist_path),
    ]);
    emit(ProgressEvent {
        stage: "packaging".into(),
        message: "Building your audio archive".into(),
        completed: total,
        total,
    });
    let output = write_archive(&plan.title, &files, output, cancel)?;
    Ok(ExportResult {
        output_path: output.to_string_lossy().into_owned(),
        track_count: total,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_distinguishes_network_failures_from_access_and_source_failures() {
        for detail in [
            "ERROR: unable to download video data: HTTP Error 403: Forbidden",
            "HTTP Error 503: Service Unavailable",
            "HTTP Error 507: Insufficient Storage",
            "The read operation timed out",
            "Connection reset by peer",
            "Temporary failure in name resolution",
            "IncompleteRead(200 bytes read, 500 more expected)",
        ] {
            assert!(
                transient_download_failure(&EngineError::Tool(detail.into())),
                "{detail}"
            );
        }
        for detail in [
            "HTTP Error 403: Sign in to confirm you're not a bot",
            "HTTP Error 503. This is a private video. Sign in if you have access.",
            "Video unavailable",
            "This video has been removed by the uploader",
            "This video is age-restricted",
            "Join this channel for members-only content",
            "This video is not available in your country",
            "HTTP Error 404: Not Found",
            "HTTP Error 401: Unauthorized",
            "Requested format is not available",
            "ffmpeg exited with code 1",
        ] {
            assert!(
                !transient_download_failure(&EngineError::Tool(detail.into())),
                "{detail}"
            );
        }
        assert!(transient_download_failure(&EngineError::Invalid(
            "Audio tool timed out; retry this item.".into()
        )));
        assert!(!transient_download_failure(&EngineError::Cancelled));
        assert!(!transient_download_failure(&EngineError::Io(
            std::io::Error::new(std::io::ErrorKind::NotFound, "Missing yt-dlp")
        )));
    }

    #[test]
    fn cancellation_interrupts_retry_delay_before_another_attempt() {
        let cancel = Cancellation::default();
        let worker_cancel = cancel.clone();
        let started = Instant::now();
        let mut attempts = 0;
        let mut worker = None;
        let outcome: Result<()> = download_with_recovery(
            &cancel,
            || {
                attempts += 1;
                Err(EngineError::Tool("HTTP Error 503".into()))
            },
            || {
                let worker_cancel = worker_cancel.clone();
                worker = Some(thread::spawn(move || {
                    thread::sleep(Duration::from_millis(75));
                    worker_cancel.cancel();
                }));
            },
        );
        worker.unwrap().join().unwrap();
        assert!(matches!(outcome, Err(EngineError::Cancelled)));
        assert_eq!(attempts, 1);
        assert!(started.elapsed() < Duration::from_millis(500));
    }

    #[cfg(unix)]
    fn recovery_fixture(script: &str) -> (tempfile::TempDir, Toolchain) {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let downloader = dir.path().join("yt-dlp");
        let probe = dir.path().join("ffprobe");
        fs::write(&downloader, format!("#!/bin/sh\nset -eu\n{script}")).unwrap();
        fs::write(&probe, "#!/bin/sh\nprintf 'aac\\n'\n").unwrap();
        for path in [&downloader, &probe] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
        }
        let tools = Toolchain {
            yt_dlp: downloader,
            ffmpeg: dir.path().join("ffmpeg"),
            ffprobe: probe,
            node: None,
        };
        (dir, tools)
    }

    #[cfg(unix)]
    fn fixture_track(id: &str) -> Track {
        Track {
            id: id.into(),
            source_url: format!("https://www.youtube.com/watch?v={id}"),
            uploader: String::new(),
            duration: None,
            artwork_url: None,
            source_metadata: Metadata::default(),
            metadata: Metadata::default(),
        }
    }

    #[test]
    #[cfg(unix)]
    fn fresh_extraction_recovers_403_and_preserves_completed_and_partial_downloads() {
        let (dir, tools) = recovery_fixture(
            r#"
output=''
source_url=''
previous=''
retry_limits=0
socket_timeout=''
for argument do
    if [ "$previous" = '--output' ]; then output="$argument"; fi
    case "$previous" in
        --retries|--fragment-retries|--extractor-retries)
            [ "$argument" = '1' ]
            retry_limits=$((retry_limits + 1))
            ;;
        --socket-timeout) socket_timeout="$argument" ;;
    esac
    previous="$argument"
    source_url="$argument"
done
[ "$retry_limits" = 3 ]
[ "$socket_timeout" = 20 ]
case "$source_url" in
    https://www.youtube.com/watch?v=first|https://www.youtube.com/watch?v=second) ;;
    *) exit 2 ;;
esac
folder=${output%/*}
counter="$folder/attempts"
count=0
if [ -f "$counter" ]; then read -r count < "$counter"; fi
count=$((count + 1))
printf '%s\n' "$count" > "$counter"
if [ "$source_url" = 'https://www.youtube.com/watch?v=second' ] && [ "$count" = 1 ]; then
    printf 'keep partial data' > "$folder/source.aac.part"
    printf 'ERROR: HTTP Error 403: Forbidden\n' >&2
    exit 1
fi
if [ "$source_url" = 'https://www.youtube.com/watch?v=second' ]; then
    [ "$(cat "$folder/source.aac.part")" = 'keep partial data' ]
fi
printf 'audio fixture' > "$folder/source.aac"
printf '%s\n' "$folder/source.aac"
"#,
        );
        let cancel = Cancellation::default();
        let auth = AuthOptions::default();
        let mut retry_messages = 0;
        let first = acquire(
            &tools,
            &fixture_track("first"),
            &auth,
            dir.path(),
            &cancel,
            || panic!("Cached first track must not retry"),
        )
        .unwrap();
        let second = acquire(
            &tools,
            &fixture_track("second"),
            &auth,
            dir.path(),
            &cancel,
            || retry_messages += 1,
        )
        .unwrap();
        assert_eq!(retry_messages, 1);
        assert_eq!(
            fs::read_to_string(first.parent().unwrap().join("attempts"))
                .unwrap()
                .trim(),
            "1"
        );
        assert_eq!(
            fs::read_to_string(second.parent().unwrap().join("attempts"))
                .unwrap()
                .trim(),
            "2"
        );
        assert_eq!(fs::read(&first).unwrap(), b"audio fixture");
        for (id, expected, expected_attempts) in [("first", first, "1"), ("second", second, "2")] {
            let cached = acquire(
                &tools,
                &fixture_track(id),
                &auth,
                dir.path(),
                &cancel,
                || panic!("Completed download must be reused"),
            )
            .unwrap();
            assert_eq!(cached, expected);
            assert_eq!(
                fs::read_to_string(cached.parent().unwrap().join("attempts"))
                    .unwrap()
                    .trim(),
                expected_attempts
            );
        }
        assert_eq!(retry_messages, 1);
    }

    #[test]
    #[cfg(unix)]
    fn persistent_transient_failure_stops_after_three_extractions() {
        let (dir, tools) = recovery_fixture(
            r#"
counter=''
previous=''
for argument do
    if [ "$previous" = '--output' ]; then counter="${argument%/*}/attempts"; fi
    previous="$argument"
done
count=0
if [ -f "$counter" ]; then read -r count < "$counter"; fi
count=$((count + 1))
printf '%s\n' "$count" > "$counter"
printf 'ERROR: HTTP Error 502: Bad Gateway\n' >&2
exit 1
"#,
        );
        let track = fixture_track("failure");
        let mut retries = 0;
        let outcome = acquire(
            &tools,
            &track,
            &AuthOptions::default(),
            dir.path(),
            &Cancellation::default(),
            || retries += 1,
        );
        assert!(matches!(outcome, Err(EngineError::Tool(detail)) if detail.contains("502")));
        assert_eq!(retries, 2);
        let source = dir
            .path()
            .join("sources")
            .join(hash_bytes(track.source_url.as_bytes()));
        assert_eq!(
            fs::read_to_string(source.join("attempts")).unwrap().trim(),
            "3"
        );
        assert!(!source.join("receipt.json").exists());
    }

    #[test]
    #[cfg(unix)]
    fn access_failure_does_not_retry_or_write_a_completed_receipt() {
        let (dir, tools) = recovery_fixture(
            "printf 'ERROR: HTTP Error 403: Sign in to confirm your age\\n' >&2\nexit 1\n",
        );
        let outcome = acquire(
            &tools,
            &fixture_track("restricted"),
            &AuthOptions::default(),
            dir.path(),
            &Cancellation::default(),
            || panic!("Access errors cannot recover automatically"),
        );
        assert!(matches!(outcome, Err(EngineError::Tool(detail)) if detail.contains("Sign in")));
        let source = dir.path().join("sources").join(hash_bytes(
            fixture_track("restricted").source_url.as_bytes(),
        ));
        assert!(!source.join("receipt.json").exists());
    }

    #[test]
    fn components_cannot_escape_folders_and_preserve_unicode() {
        assert_eq!(sanitize_component(" ../Björk: Live\n "), ".._Björk_ Live");
        assert_eq!(sanitize_component(".."), "Untitled");
        assert!(sanitize_component(&"é".repeat(200)).len() <= 160);
    }

    #[test]
    fn archive_contains_portable_paths_and_rejects_traversal() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("song.mp3");
        fs::write(&file, b"fixture").unwrap();
        let zip = dir.path().join("result.zip");
        write_archive(
            "My mix",
            &[("01 - Song.mp3".into(), file.clone())],
            &zip,
            &Cancellation::default(),
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(fs::File::open(zip).unwrap()).unwrap();
        assert_eq!(archive.by_index(0).unwrap().name(), "My mix/01 - Song.mp3");
        assert!(write_archive(
            "My mix",
            &[("../escape".into(), file)],
            &dir.path().join("unsafe.zip"),
            &Cancellation::default()
        )
        .is_err());
        assert!(!dir.path().join("unsafe.zip").exists());
    }

    #[test]
    fn repeated_exports_keep_existing_archives_and_report_the_saved_filename() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("Album.zip");
        fs::write(&output, b"original").unwrap();
        fs::write(dir.path().join("Album (2).zip"), b"second").unwrap();
        let saved = write_archive("Album", &[], &output, &Cancellation::default()).unwrap();
        assert_eq!(saved, dir.path().join("Album (3).zip"));
        assert_eq!(fs::read(&output).unwrap(), b"original");
        assert_eq!(
            fs::read(dir.path().join("Album (2).zip")).unwrap(),
            b"second"
        );
        assert!(zip::ZipArchive::new(fs::File::open(saved).unwrap()).is_ok());
    }

    #[test]
    #[cfg(unix)]
    fn archive_names_skip_dangling_links() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("Album.zip");
        std::os::unix::fs::symlink(dir.path().join("missing"), &output).unwrap();
        assert_eq!(
            available_archive_path(&output),
            dir.path().join("Album (2).zip")
        );
    }

    #[test]
    fn cancellation_does_not_replace_an_existing_archive() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join("existing.zip");
        fs::write(&output, b"original").unwrap();
        let cancel = Cancellation::default();
        cancel.cancel();
        assert!(write_archive("mix", &[], &output, &cancel).is_err());
        assert_eq!(fs::read(output).unwrap(), b"original");
    }
}
