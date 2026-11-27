use super::destination::MusicFolder;
use super::state::Boot;
use super::Message;
use crate::settings::Appearance;
use iced::Task;
use jacket_core::ReviewDraft;
use jacket_desktop::Desktop;

pub(super) fn background<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
    done: impl FnOnce(Result<T, String>) -> Message + Send + 'static,
) -> Task<Message> {
    Task::perform(
        async move {
            tokio::task::spawn_blocking(work)
                .await
                .map_err(|_| {
                    "A background task stopped unexpectedly. Your saved files have been kept."
                        .to_owned()
                })
                .and_then(|r| r)
        },
        done,
    )
}

// The store rejects delayed revisions and retired sessions. An identical revision
// is already durable; a tombstone or a different snapshot must never count as saved.
pub(super) fn persist_review(desktop: &Desktop, draft: ReviewDraft) -> Result<bool, String> {
    if desktop.save_review(draft.clone())? {
        return Ok(true);
    }
    let stored=desktop.read_review()?.ok_or("This review has already been discarded or exported. Start a new review before saving edits.")?;
    if serde_json::to_vec(&stored).map_err(|error| error.to_string())?
        == serde_json::to_vec(&draft).map_err(|error| error.to_string())?
    {
        Ok(true)
    } else {
        Err("The saved review changed. Reopen Jacket before replacing it.".into())
    }
}

pub(super) fn load() -> Result<Boot, String> {
    let desktop = Desktop::new()?;
    let (downloads, download_warning) = match desktop.downloads() {
        Ok(items) => (items, None),
        Err(error) => (
            Vec::new(),
            Some(format!("Couldn’t read Downloads. {error}")),
        ),
    };
    for item in &downloads {
        if let Some(identity) = &item.recipe.review_identity {
            let _ = desktop.retire_queued_review(identity);
        }
    }
    let (draft, mut warning) = match desktop.load_review() {
        Ok(draft) => (draft, download_warning),
        Err(error) => (None, Some(error)),
    };
    let (appearance, appearance_valid) = match read_appearance(&desktop) {
        Ok(value) => (value, true),
        Err(error) => {
            warning = Some(warning.map_or(error.clone(), |warning| format!("{warning} {error}")));
            (Appearance::default(), false)
        }
    };
    let music_folder = match read_music_folder(&desktop) {
        Ok(folder) => folder,
        Err(error) => {
            warning = Some(warning.map_or(error.clone(), |warning| format!("{warning} {error}")));
            None
        }
    };
    Ok(Boot {
        recovery: desktop.pending_export(),
        downloads,
        downloader_preferences: desktop.downloader_preferences()?,
        desktop,
        draft,
        appearance,
        appearance_valid,
        music_folder,
        warning,
    })
}

pub(super) fn read_appearance(desktop: &Desktop) -> Result<Appearance, String> {
    let appearance: Appearance = read_preferences(
        desktop,
        "native-preferences.json",
        524288,
        "appearance preferences",
    )?;
    appearance.theme.resolve(0)?;
    Ok(appearance)
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct SavedMusicFolder {
    root: std::path::PathBuf,
    rule: jacket_core::PathRule,
    #[serde(default, rename = "schemaVersion")]
    schema_version: u8,
}

pub(super) fn read_music_folder(desktop: &Desktop) -> Result<Option<MusicFolder>, String> {
    let folder: Option<SavedMusicFolder> = read_preferences(
        desktop,
        "music-folder.json",
        8192,
        "music folder preferences",
    )
    .map_err(|_| {
        "Jacket’s music folder setting could not be read. Choose a music folder or use Queue As…"
    })?;
    if folder
        .as_ref()
        .is_some_and(|folder| !folder.root.is_absolute())
    {
        return Err(
            "Jacket’s saved music folder needs an absolute path. Choose a music folder again."
                .into(),
        );
    }
    Ok(folder.map(|folder| MusicFolder {
        root: folder.root,
        rule: if folder.schema_version == 0 {
            folder.rule.migrate_saved_default()
        } else {
            folder.rule
        },
    }))
}

fn read_preferences<T: serde::de::DeserializeOwned + Default>(
    desktop: &Desktop,
    name: &str,
    max_bytes: u64,
    label: &str,
) -> Result<T, String> {
    use std::io::Read;
    let path = desktop.data_root().join(name);
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(max_bytes + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > max_bytes {
        return Err(format!("Jacket’s {label} are too large."));
    }
    serde_json::from_slice(&bytes).map_err(|_| format!("Jacket’s {label} could not be read."))
}

pub(super) fn write_appearance(desktop: &Desktop, prefs: &Appearance) -> Result<(), String> {
    write_preferences(desktop, "native-preferences.json", prefs)
}

pub(super) fn write_music_folder(
    desktop: &Desktop,
    folder: &Option<MusicFolder>,
) -> Result<(), String> {
    let saved = folder.as_ref().map(|folder| SavedMusicFolder {
        root: folder.root.clone(),
        rule: folder.rule.clone(),
        schema_version: 1,
    });
    write_preferences(desktop, "music-folder.json", &saved)
}

fn write_preferences(
    desktop: &Desktop,
    name: &str,
    prefs: &impl serde::Serialize,
) -> Result<(), String> {
    use std::io::Write;
    let root = desktop.data_root();
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(prefs).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(root.join(name)).map_err(|e| e.to_string())?;
    Ok(())
}

/// Preserve a failed appearance restore until the user explicitly selects new settings.
pub(super) fn persist_before_close(
    desktop: &Desktop,
    draft: Option<ReviewDraft>,
    appearance: Option<Appearance>,
) -> Result<(), String> {
    if let Some(draft) = draft {
        persist_review(desktop, draft)?;
    }
    if let Some(appearance) = appearance {
        write_appearance(desktop, &appearance)?;
    }
    Ok(())
}
