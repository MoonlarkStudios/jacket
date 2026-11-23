//! Native destination selection and the review-to-export save boundary.
use super::destination_dialog::{self, Selection};
use super::{persistence, App, Message, Panel};
use audio_engine::{CollectionMode, ExportPlan, Metadata, OrganizedOutput, OutputKind};
use iced::Task;
use jacket_core::PathRule;
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MusicFolder {
    pub root: PathBuf,
    pub rule: PathRule,
}

impl MusicFolder {
    fn resolve(&self, metadata: &Metadata) -> PathBuf {
        self.root.join(self.rule.resolve(metadata))
    }

    fn album(&self, plan: &ExportPlan) -> Result<OrganizedOutput, String> {
        let mut paths = plan
            .tracks
            .iter()
            .map(|track| self.resolve(&track.metadata));
        let first = paths.next().ok_or("Select at least one track.")?;
        let directory = first.parent().expect("library filenames have a parent");
        let stems = std::iter::once(first.clone())
            .chain(paths)
            .map(|path| {
                if path.parent() != Some(directory) {
                    return Err("This rule puts album tracks in different folders. Use a shared rule such as {{album_artist}}/{{album}}/{{song}}, or choose Save As….");
                }
                Ok(path.file_name().expect("resolved filename").to_string_lossy().into_owned())
            })
            .collect::<Result<Vec<_>, _>>()?;
        OrganizedOutput::album(self.root.clone(), directory.to_owned(), stems)
            .map_err(|error| error.to_string())
    }
}

pub struct MusicFolderEditor {
    pub root: Option<PathBuf>,
    pub rule: String,
    pub error: Option<String>,
    pub customize: bool,
    pub(super) direct: bool,
    pub(super) return_to: Panel,
}

#[derive(Clone)]
pub enum ExportDestination {
    Picked(PathBuf),
    Organized(OrganizedOutput),
}

impl App {
    pub fn uses_music_folder(&self) -> bool {
        self.is_album_folder()
            || self
                .review
                .as_ref()
                .is_some_and(|review| review.preferences.output_kind == OutputKind::File)
    }

    pub fn is_album_folder(&self) -> bool {
        self.review.as_ref().is_some_and(|review| {
            review.inspection.suggested_mode == CollectionMode::Album
                && review.preferences.mode == CollectionMode::Album
                && review.preferences.output_kind == OutputKind::Folder
        })
    }

    pub fn music_album_path(&self) -> Option<PathBuf> {
        if !self.is_album_folder() {
            return None;
        }
        let folder = self.music_folder.as_ref()?;
        let review = self.review.as_ref()?;
        let metadata = review.effective_metadata(*review.selected.iter().next()?)?;
        Some(folder.resolve(&metadata).parent()?.to_owned())
    }

    fn album_scan_is_current(&self) -> bool {
        self.is_album_folder()
            && self
                .album_scan_key
                .as_ref()
                .is_some_and(|(identity, folder)| {
                    self.music_folder.as_ref() == Some(folder)
                        && self
                            .review
                            .as_ref()
                            .is_some_and(|review| &review.identity() == identity)
                })
    }

    pub fn album_folder_preview(&self) -> Option<&Result<audio_engine::AlbumPreview, String>> {
        self.album_preview
            .as_ref()
            .filter(|_| self.album_scan_is_current())
    }

    pub(super) fn invalidate_album_scan(&mut self) {
        self.album_scan = self.album_scan.wrapping_add(1);
        self.album_scan_key = None;
        self.album_preview = None;
    }

    pub(super) fn sync_album_scan(&mut self) -> Task<Message> {
        if self.example
            || self.result.is_some()
            || !matches!(self.panel, Panel::Workspace | Panel::Preview)
            || !self.is_album_folder()
            || !self.can_preview()
            || self.album_scan_is_current()
            || (self.album_scan_key.is_some() && self.album_preview.is_none())
        {
            return Task::none();
        }
        let (Some(folder), Some(review)) = (self.music_folder.clone(), &self.review) else {
            return Task::none();
        };
        let identity = review.identity();
        let plan = review.plan(self.auth.clone());
        self.invalidate_album_scan();
        let album = plan.and_then(|plan| folder.album(&plan).map(|output| (plan, output)));
        self.album_scan_key = Some((identity, folder));
        let (plan, output) = match album {
            Ok(album) => album,
            Err(error) => {
                self.album_preview = Some(Err(error));
                return Task::none();
            }
        };
        let request = self.album_scan;
        persistence::background(
            move || {
                audio_engine::preview_album_output(&plan, &output)
                    .map_err(|error| error.to_string())
            },
            move |result| Message::AlbumScanned(request, result),
        )
    }

    pub fn music_file_path(&self) -> Option<PathBuf> {
        let review = self.review.as_ref()?;
        if review.preferences.output_kind != OutputKind::File || review.selected.len() != 1 {
            return None;
        }
        let folder = self.music_folder.as_ref()?;
        let metadata = review.effective_metadata(*review.selected.iter().next()?)?;
        Some(folder.resolve(&metadata))
    }

    pub fn music_folder_sample(&self) -> Metadata {
        self.review
            .as_ref()
            .and_then(|review| {
                review
                    .selected
                    .iter()
                    .next()
                    .and_then(|index| review.effective_metadata(*index))
            })
            .unwrap_or_else(|| crate::example::album().tracks.remove(0).metadata)
    }

    pub(super) fn open_music_folder(&mut self, direct: bool) -> Task<Message> {
        let editor = MusicFolderEditor {
            root: self.music_folder.as_ref().map(|folder| folder.root.clone()),
            rule: self.music_folder.as_ref().map_or_else(
                || PathRule::default().to_string(),
                |folder| folder.rule.to_string(),
            ),
            error: None,
            customize: false,
            direct,
            return_to: self.panel,
        };
        self.error = None;
        self.music_folder_editor = Some(editor);
        if direct {
            self.pick_music_root()
        } else {
            self.panel = Panel::MusicFolder;
            iced::widget::operation::focus("music-folder-root")
        }
    }

    pub(super) fn close_music_folder(&mut self) -> Task<Message> {
        let Some(editor) = self.music_folder_editor.take() else {
            return Task::none();
        };
        self.panel = editor.return_to;
        let focus = iced::widget::operation::focus(match self.panel {
            Panel::Settings => "settings-music-folder",
            Panel::Options => "export-music-folder",
            _ if editor.direct || self.music_folder.is_none() => "save-output",
            _ => "edit-music-folder",
        });
        Task::batch([self.restore_scroll(), focus])
    }

    pub(super) fn pick_music_root(&mut self) -> Task<Message> {
        let Some(editor) = &mut self.music_folder_editor else {
            return Task::none();
        };
        let current = editor.root.clone();
        editor.error = None;
        self.dialog_open = true;
        self.notice = Some("Opening the system folder chooser…".into());
        Task::perform(
            destination_dialog::pick("Choose your music folder", current, Selection::Folder),
            Message::MusicRootPicked,
        )
    }

    pub(super) fn apply_music_folder(&mut self) -> Task<Message> {
        let Some(editor) = &mut self.music_folder_editor else {
            return Task::none();
        };
        let Some(root) = editor.root.clone() else {
            editor.error = Some("Choose a music folder first.".into());
            return iced::widget::operation::focus("music-folder-root");
        };
        let rule = match editor.rule.parse() {
            Ok(rule) => rule,
            Err(error) => {
                editor.error = Some(error);
                editor.customize = true;
                return iced::widget::operation::focus("music-folder-rule");
            }
        };
        self.save_music_folder(Some(MusicFolder { root, rule }))
    }

    pub(super) fn save_music_folder(&mut self, folder: Option<MusicFolder>) -> Task<Message> {
        if let Some(editor) = &mut self.music_folder_editor {
            editor.error = None;
        }
        if self.example {
            self.music_folder = folder;
            return self.close_music_folder();
        }
        let Some(desktop) = self.desktop.clone() else {
            return Task::none();
        };
        self.dialog_open = true;
        self.notice = Some("Saving music folder settings…".into());
        persistence::background(
            move || {
                let folder = folder
                    .map(|folder| {
                        let canonical = folder
                            .root
                            .canonicalize()
                            .ok()
                            .filter(|path| path.is_dir())
                            .ok_or(
                                "Jacket couldn’t open that folder. Choose an available folder.",
                            )?;
                        if canonical.to_str().is_none() {
                            return Err("The music folder needs a Unicode path.");
                        }
                        Ok(MusicFolder {
                            root: canonical,
                            rule: folder.rule,
                        })
                    })
                    .transpose()?;
                persistence::write_music_folder(&desktop, &folder)
                    .map_err(|error| format!("Couldn’t save the music folder setting. {error}"))?;
                Ok(folder)
            },
            Message::MusicFolderSaved,
        )
    }

    pub(super) fn save_export(&mut self, save_as: bool) -> Task<Message> {
        if !save_as && self.uses_music_folder() && self.music_folder.is_none() {
            return self.open_music_folder(true);
        }
        if !save_as && self.is_album_folder() && !matches!(self.album_folder_preview(), Some(Ok(_)))
        {
            return self.sync_album_scan();
        }
        let Some(review) = &self.review else {
            return Task::none();
        };
        let plan = if self.panel == Panel::Preview {
            let Some(plan) = self.pending_plan.clone() else {
                return Task::none();
            };
            plan
        } else {
            match review.plan(self.auth.clone()) {
                Ok(plan) => plan,
                Err(error) => {
                    self.error = Some(error);
                    return Task::none();
                }
            }
        };
        self.error = None;
        let library = self
            .music_folder
            .as_ref()
            .filter(|_| self.uses_music_folder());
        if let Some(folder) = library.filter(|_| !save_as) {
            let output = if self.is_album_folder() {
                folder.album(&plan)
            } else {
                let stem = folder.resolve(&plan.tracks[0].metadata);
                OrganizedOutput::new(folder.root.clone(), stem).map_err(|error| error.to_string())
            };
            return match output {
                Ok(output) => self.begin_export(plan, ExportDestination::Organized(output)),
                Err(error) => {
                    self.error = Some(error);
                    Task::none()
                }
            };
        }

        let kind = plan.output_kind;
        let mut title = audio_engine::export::sanitize_component(&plan.title);
        let start_in = library.map(|folder| folder.root.clone());
        if kind == OutputKind::Zip {
            title.push_str(".zip");
        }
        self.pending_plan = Some(plan);
        self.dialog_open = true;
        let (title, selection, notice) = match kind {
            OutputKind::Folder => (
                "Choose a parent music folder",
                Selection::Folder,
                "Opening the system folder chooser…",
            ),
            OutputKind::Zip => (
                "Save ZIP archive",
                Selection::File(title),
                "Opening the system save dialog…",
            ),
            OutputKind::File => (
                "Save audio file",
                Selection::File(title),
                "Opening the system save dialog…",
            ),
        };
        self.notice = Some(notice.into());
        Task::perform(
            destination_dialog::pick(title, start_in, selection),
            Message::Destination,
        )
    }

    pub(super) fn begin_export(
        &mut self,
        plan: ExportPlan,
        destination: ExportDestination,
    ) -> Task<Message> {
        let (Some(desktop), Some(review)) = (self.desktop.clone(), &self.review) else {
            return Task::none();
        };
        let draft = review.draft();
        let identity = review.identity();
        self.invalidate_album_scan();
        self.dialog_open = true;
        self.notice = Some("Preparing your export…".into());
        persistence::background(
            move || persistence::persist_review(&desktop, draft),
            move |result| Message::ExportSaved(Box::new(plan), destination, identity, result),
        )
    }
}

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
