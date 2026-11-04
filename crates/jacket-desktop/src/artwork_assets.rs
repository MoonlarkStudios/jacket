//! App-owned JPEG assets. IPC callers receive opaque IDs and bounded previews,
//! never a persistent caller-controlled path that an export could later follow.
use audio_engine::{process::run, Cancellation, Toolchain};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

const MAX_IMAGE_BYTES: usize = 10 * 1024 * 1024;
const MAX_DIMENSION: u32 = 16_384;
const MAX_PIXELS: u64 = 40_000_000;
const MAX_OUTPUT_DIMENSION: u32 = 2_000;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Crop {
    pub x: u32,
    pub y: u32,
    pub size: u32,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Asset {
    pub id: String,
    pub preview_data_url: String,
    pub width: u32,
    pub height: u32,
}

pub fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn unavailable() -> String {
    "This imported cover is unavailable. Choose the image again.".into()
}

pub(crate) fn dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
    let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| "Choose a valid JPEG, PNG or WebP image.")?;
    if !matches!(
        reader.format(),
        Some(image::ImageFormat::Jpeg | image::ImageFormat::Png | image::ImageFormat::WebP)
    ) {
        return Err("Choose a JPEG, PNG or WebP image.".into());
    }
    let mut limits = image::Limits::default();
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    reader.limits(limits);
    // Read dimensions before decoding pixels or invoking FFmpeg. The supported
    // codec readers handle WebP's VP8, VP8L and extended headers consistently.
    let dimensions = reader
        .into_dimensions()
        .map_err(|_| "Choose a valid JPEG, PNG or WebP image.")?;
    let (width, height) = dimensions;
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
    {
        return Err(
            "Choose an image up to 40 megapixels, with neither side over 16,384 pixels.".into(),
        );
    }
    Ok(dimensions)
}

fn bounded_read(path: &Path) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|_| "The image could not be opened.".to_string())?;
    let metadata = file.metadata().map_err(|_| unavailable())?;
    if !metadata.is_file() || metadata.len() > MAX_IMAGE_BYTES as u64 {
        return Err("Choose an image smaller than 10 MiB.".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_IMAGE_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable())?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("Choose an image smaller than 10 MiB.".into());
    }
    Ok(bytes)
}

fn checked_crop(crop: Crop, width: u32, height: u32) -> Result<String, String> {
    if crop.size == 0
        || crop
            .x
            .checked_add(crop.size)
            .is_none_or(|right| right > width)
        || crop
            .y
            .checked_add(crop.size)
            .is_none_or(|bottom| bottom > height)
    {
        return Err("Keep the square crop inside the image.".into());
    }
    Ok(format!(
        "crop={}:{}:{}:{}:exact=1,",
        crop.size, crop.size, crop.x, crop.y
    ))
}

/// Snapshot and normalize a chosen image; the source path is never retained.
pub fn import(
    root: &Path,
    tools: &Toolchain,
    path: &Path,
    crop: Option<Crop>,
) -> Result<Asset, String> {
    let bytes = bounded_read(path)?;
    normalize(root, tools, &bytes, crop)
}

/// Snapshot a source cover using the same normalization and asset ownership as
/// an imported file. The caller has already bounded and validated the download.
pub(crate) fn import_bytes(root: &Path, tools: &Toolchain, bytes: &[u8]) -> Result<Asset, String> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("Choose an image smaller than 10 MiB.".into());
    }
    normalize(root, tools, bytes, None)
}

pub fn crop(root: &Path, tools: &Toolchain, id: &str, crop: Crop) -> Result<Asset, String> {
    let bytes = asset_bytes(root, id)?;
    normalize(root, tools, &bytes, Some(crop))
}

fn normalize(
    root: &Path,
    tools: &Toolchain,
    bytes: &[u8],
    crop: Option<Crop>,
) -> Result<Asset, String> {
    let (width, height) = dimensions(bytes)?;
    let crop_filter = crop
        .map(|crop| checked_crop(crop, width, height))
        .transpose()?
        .unwrap_or_default();
    crate::reviews::private_directory(root).map_err(|_| unavailable())?;
    let working = tempfile::Builder::new()
        .prefix(".jacket-artwork-")
        .tempdir_in(root)
        .map_err(|_| unavailable())?;
    let source = working.path().join("input-image");
    let output = working.path().join("normalized.jpg");
    let mut source_file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&source)
        .map_err(|_| unavailable())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        source_file
            .set_permissions(fs::Permissions::from_mode(0o600))
            .map_err(|_| unavailable())?;
    }
    source_file.write_all(bytes).map_err(|_| unavailable())?;
    let filter = format!("{crop_filter}scale=w='min({MAX_OUTPUT_DIMENSION},iw)':h='min({MAX_OUTPUT_DIMENSION},ih)':force_original_aspect_ratio=decrease:force_divisible_by=1,format=yuvj444p");
    run(
        Command::new(&tools.ffmpeg)
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-protocol_whitelist",
                "file,pipe",
                "-max_alloc",
                "268435456",
                "-threads",
                "1",
                "-i",
            ])
            .arg(&source)
            .args([
                "-map_metadata",
                "-1",
                "-frames:v",
                "1",
                "-vf",
                &filter,
                "-q:v",
                "2",
                "-threads",
                "1",
                "-filter_threads",
                "1",
            ])
            .arg(&output),
        &Cancellation::default(),
        Duration::from_secs(30),
    )
    .map_err(|_| {
        "This image could not be read. Choose a valid JPEG, PNG or WebP image.".to_string()
    })?;
    let bytes = bounded_read(&output)?;
    if !bytes.starts_with(b"\xff\xd8") {
        return Err(unavailable());
    }
    let (width, height) = dimensions(&bytes)?;
    if width > MAX_OUTPUT_DIMENSION || height > MAX_OUTPUT_DIMENSION {
        return Err(unavailable());
    }
    let id = hex::encode(Sha256::digest(&bytes));
    let path = root.join(format!("{id}.jpg"));
    let mut temporary = tempfile::NamedTempFile::new_in(root).map_err(|_| unavailable())?;
    temporary.write_all(&bytes).map_err(|_| unavailable())?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| unavailable())?;
    if let Err(error) = temporary.persist_noclobber(&path) {
        if error.error.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(unavailable());
        }
        // Never trust a pre-existing path merely because its name matches.
        asset_bytes(root, &id)?;
    }
    crate::reviews::sync_directory(root).map_err(|_| unavailable())?;
    Ok(asset(id, bytes, width, height))
}

fn asset_bytes(root: &Path, id: &str) -> Result<Vec<u8>, String> {
    if !valid_id(id)
        || root
            .symlink_metadata()
            .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(unavailable());
    }
    let path = root.join(format!("{id}.jpg"));
    let metadata = path.symlink_metadata().map_err(|_| unavailable())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(unavailable());
    }
    let bytes = bounded_read(&path)?;
    if !bytes.starts_with(b"\xff\xd8") || hex::encode(Sha256::digest(&bytes)) != id {
        return Err(unavailable());
    }
    let (width, height) = dimensions(&bytes)?;
    if width > MAX_OUTPUT_DIMENSION || height > MAX_OUTPUT_DIMENSION {
        return Err(unavailable());
    }
    Ok(bytes)
}

pub fn load(root: &Path, id: &str) -> Result<Asset, String> {
    let bytes = asset_bytes(root, id)?;
    let (width, height) = dimensions(&bytes)?;
    Ok(asset(id.into(), bytes, width, height))
}

/// Resolve only content-addressed app assets. The engine must not deserialize
/// an artwork filesystem path from an export plan supplied over IPC.
pub fn resolve(root: &Path, id: &str) -> Result<PathBuf, String> {
    asset_bytes(root, id)?;
    Ok(root.join(format!("{id}.jpg")))
}

/// Reclaim abandoned imports only at startup, before any review can hold an
/// unsaved asset. The caller must supply the complete set of IDs referenced by
/// the saved draft and every retained export record, and skip pruning if that
/// inventory cannot be read. Interrupted import scratch directories are also
/// removed; unknown files, directories, and symlinks are left untouched.
pub fn prune(root: &Path, retained: &HashSet<String>) -> Result<usize, String> {
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(unavailable());
    }
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(_) => return Err(unavailable()),
    };
    let mut removed = 0;
    for entry in entries {
        let entry = entry.map_err(|_| unavailable())?;
        let file_type = entry.file_type().map_err(|_| unavailable())?;
        let filename = entry.file_name();
        let Some(filename) = filename.to_str() else {
            continue;
        };
        if file_type.is_dir() && filename.starts_with(".jacket-artwork-") {
            fs::remove_dir_all(entry.path()).map_err(|_| unavailable())?;
            removed += 1;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let Some(id) = filename.strip_suffix(".jpg") else {
            continue;
        };
        if valid_id(id) && !retained.contains(id) {
            fs::remove_file(entry.path()).map_err(|_| unavailable())?;
            removed += 1;
        }
    }
    if removed > 0 {
        crate::reviews::sync_directory(root).map_err(|_| unavailable())?;
    }
    Ok(removed)
}

fn asset(id: String, bytes: Vec<u8>, width: u32, height: u32) -> Asset {
    Asset {
        id,
        preview_data_url: format!("data:image/jpeg;base64,{}", base64(&bytes)),
        width,
        height,
    }
}

fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut output = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let packed = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        output.push(ALPHABET[((packed >> 18) & 63) as usize] as char);
        output.push(ALPHABET[((packed >> 12) & 63) as usize] as char);
        output.push(if chunk.len() > 1 {
            ALPHABET[((packed >> 6) & 63) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            ALPHABET[(packed & 63) as usize] as char
        } else {
            '='
        });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Option<Toolchain> {
        Some(Toolchain {
            ffmpeg: audio_engine::process::find_tool("ffmpeg")?,
            ffprobe: audio_engine::process::find_tool("ffprobe")?,
            yt_dlp: "unused".into(),
            node: None,
        })
    }

    #[test]
    fn bounds_image_headers_crop_and_asset_identifiers() {
        assert!(dimensions(b"not an image").is_err());
        let mut png = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        png.extend(40_000_u32.to_be_bytes());
        png.extend(1_u32.to_be_bytes());
        assert!(dimensions(&png).is_err());
        assert!(checked_crop(
            Crop {
                x: 90,
                y: 0,
                size: 20
            },
            100,
            100
        )
        .is_err());
        assert!(checked_crop(
            Crop {
                x: u32::MAX,
                y: 0,
                size: 20
            },
            100,
            100
        )
        .is_err());
        assert!(!valid_id("../image"));
        assert!(!valid_id(&"G".repeat(64)));
        assert!(valid_id(&"a".repeat(64)));
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
    }

    #[test]
    fn remote_webp_bytes_normalize_to_an_owned_crop_ready_jpeg() {
        let Some(tools) = tools() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(544, 544)
            .write_to(&mut bytes, image::ImageFormat::WebP)
            .expect("WebP source cover");
        let imported =
            import_bytes(root.path(), &tools, bytes.get_ref()).expect("own remote cover");
        let path = resolve(root.path(), &imported.id).expect("owned JPEG path");
        assert!(fs::read(path).unwrap().starts_with(b"\xff\xd8"));
        assert_eq!((imported.width, imported.height), (544, 544));
        let cropped = crop(
            root.path(),
            &tools,
            &imported.id,
            Crop {
                x: 44,
                y: 44,
                size: 456,
            },
        )
        .expect("crop normalized source cover");
        assert_eq!((cropped.width, cropped.height), (456, 456));
        assert!(
            load(root.path(), &imported.id).is_ok(),
            "crop keeps the source asset"
        );
    }

    #[test]
    fn normalization_keeps_small_sources_and_caps_large_covers_without_distortion() {
        let Some(tools) = tools() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        for ((width, height), expected) in [
            ((641, 401), (641, 401)),
            ((3200, 1600), (2000, 1000)),
            ((1600, 3200), (1000, 2000)),
        ] {
            let mut bytes = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(width, height)
                .write_to(&mut bytes, image::ImageFormat::Png)
                .expect("source cover");
            let asset =
                import_bytes(root.path(), &tools, bytes.get_ref()).expect("normalized cover");
            assert_eq!((asset.width, asset.height), expected);
        }
    }

    #[test]
    fn imported_and_cropped_jpegs_survive_original_deletion() {
        let Some(tools) = tools() else {
            return;
        };
        let root = tempfile::tempdir().unwrap();
        let original = root.path().join("original.png");
        run(
            Command::new(&tools.ffmpeg)
                .args([
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "lavfi",
                    "-i",
                    "color=c=red:s=100x60",
                    "-frames:v",
                    "1",
                    "-threads",
                    "1",
                ])
                .arg(&original),
            &Cancellation::default(),
            Duration::from_secs(10),
        )
        .unwrap();
        let assets = root.path().join("assets");
        let imported = import(&assets, &tools, &original, None).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&assets).unwrap().permissions().mode() & 0o777,
                0o700
            );
            assert_eq!(
                fs::metadata(resolve(&assets, &imported.id).unwrap())
                    .unwrap()
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
        fs::remove_file(&original).unwrap();
        let restored = load(&assets, &imported.id).unwrap();
        assert_eq!(restored.width, 100);
        assert_eq!(restored.height, 60);
        assert_eq!(restored.preview_data_url, imported.preview_data_url);
        assert!(restored
            .preview_data_url
            .starts_with("data:image/jpeg;base64,/9j/"));
        assert!(fs::read(resolve(&assets, &imported.id).unwrap())
            .unwrap()
            .starts_with(b"\xff\xd8"));
        let cropped = crop(
            &assets,
            &tools,
            &imported.id,
            Crop {
                x: 20,
                y: 0,
                size: 60,
            },
        )
        .unwrap();
        assert_eq!((cropped.width, cropped.height), (60, 60));
        assert_ne!(cropped.id, imported.id);
        assert!(load(&assets, &imported.id).is_ok());
        assert!(crop(
            &assets,
            &tools,
            &imported.id,
            Crop {
                x: 4000,
                y: 0,
                size: 60
            }
        )
        .is_err());
        fs::write(assets.join(format!("{}.jpg", imported.id)), b"tampered").unwrap();
        assert!(resolve(&assets, &imported.id).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn resolution_never_follows_a_caller_path_or_symlink() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().unwrap();
        let id = "a".repeat(64);
        let outside = root.path().join("outside");
        fs::write(&outside, b"private file").unwrap();
        let assets = root.path().join("assets");
        fs::create_dir(&assets).unwrap();
        symlink(&outside, assets.join(format!("{id}.jpg"))).unwrap();
        assert!(resolve(&assets, &id).is_err());
        assert!(resolve(&assets, "../outside").is_err());
    }

    #[test]
    fn startup_pruning_preserves_every_referenced_asset_and_unknown_file() {
        let root = tempfile::tempdir().unwrap();
        let retained = "a".repeat(64);
        let abandoned = "b".repeat(64);
        fs::write(root.path().join(format!("{retained}.jpg")), b"retained").unwrap();
        fs::write(root.path().join(format!("{abandoned}.jpg")), b"abandoned").unwrap();
        fs::write(root.path().join("unrelated.jpg"), b"unknown").unwrap();
        fs::create_dir(root.path().join("unknown-directory")).unwrap();
        fs::create_dir(root.path().join(".jacket-artwork-interrupted")).unwrap();
        fs::write(
            root.path().join(".jacket-artwork-interrupted/input-image"),
            b"raw source",
        )
        .unwrap();
        assert_eq!(
            prune(root.path(), &HashSet::from([retained.clone()])).unwrap(),
            2
        );
        assert!(root.path().join(format!("{retained}.jpg")).exists());
        assert!(!root.path().join(format!("{abandoned}.jpg")).exists());
        assert!(root.path().join("unrelated.jpg").exists());
        assert!(root.path().join("unknown-directory").exists());
        assert!(!root.path().join(".jacket-artwork-interrupted").exists());
    }
}
