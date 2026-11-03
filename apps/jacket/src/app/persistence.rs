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
    let stored=desktop.load_review()?.ok_or("This review has already been discarded or exported. Start a new review before saving edits.")?;
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
    let (draft, mut warning) = match desktop.load_review() {
        Ok(draft) => (draft, None),
        Err(error) => (None, Some(error)),
    };
    let (appearance, appearance_valid) = match read_appearance(&desktop) {
        Ok(value) => (value, true),
        Err(error) => {
            warning = Some(warning.map_or(error.clone(), |warning| format!("{warning} {error}")));
            (Appearance::default(), false)
        }
    };
    Ok(Boot {
        recovery: desktop.pending_export(),
        downloader_preferences: desktop.downloader_preferences()?,
        desktop,
        draft,
        appearance,
        appearance_valid,
        warning,
    })
}

pub(super) fn read_appearance(desktop: &Desktop) -> Result<Appearance, String> {
    use std::io::Read;
    let path = desktop.data_root().join("native-preferences.json");
    let file = match std::fs::File::open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Appearance::default())
        }
        Err(error) => return Err(error.to_string()),
    };
    let mut bytes = Vec::new();
    file.take(524289)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() > 524288 {
        return Err("Jacket’s appearance preferences are too large.".into());
    }
    let appearance: Appearance = serde_json::from_slice(&bytes)
        .map_err(|_| "Jacket’s appearance preferences could not be read.".to_string())?;
    appearance.theme.resolve(0)?;
    Ok(appearance)
}

pub(super) fn write_appearance(desktop: &Desktop, prefs: &Appearance) -> Result<(), String> {
    use std::io::Write;
    let root = desktop.data_root();
    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(root).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec(prefs).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.as_file().sync_all().map_err(|e| e.to_string())?;
    file.persist(root.join("native-preferences.json"))
        .map_err(|e| e.to_string())?;
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
