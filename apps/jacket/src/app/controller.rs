use super::persistence;
use super::{App, Message, Panel, ProfileChoice};
use crate::editor::{self, Editor};
use crate::settings::BrowserChoice;
use crate::{example, view};
use audio_engine::AuthOptions;
use iced::{keyboard, window, Element, Subscription, Task, Theme};
use jacket_core::{Review, SourceIntent, SourceLink};
use jacket_desktop::artwork_assets::Crop;
use jacket_desktop::downloader_updates::{self, Channel};
use std::time::Duration;

impl App {
    pub fn theme(&self) -> Theme {
        self.theme.iced.clone()
    }
    pub fn presentation(&self) -> crate::style::Presentation {
        crate::style::Presentation {
            colors: self.theme.colors,
            derived: self.theme.derived,
        }
    }

    pub fn preview_plan(&self) -> Option<&audio_engine::ExportPlan> {
        self.pending_plan.as_ref()
    }

    pub fn crop_asset(
        &self,
    ) -> Option<&(jacket_desktop::artwork_assets::Asset, std::path::PathBuf)> {
        self.crop_source.as_ref().or(self.cover.as_ref())
    }

    pub fn can_crop_cover(&self) -> bool {
        self.cover.is_some()
            || self
                .review
                .as_ref()
                .and_then(crate::artwork::source_url)
                .is_some()
    }

    pub fn can_preview(&self) -> bool {
        !self.busy()
            && !self.catalog.visible
            && !self.panel.review_tool()
            && self.review.as_ref().is_some_and(|review| {
                let uses_shared_cover = matches!(
                    review.preferences.artwork_mode,
                    audio_engine::ArtworkMode::Shared
                ) || (review.is_song()
                    && matches!(
                        review.preferences.artwork_mode,
                        audio_engine::ArtworkMode::PerTrack
                    ));
                let cover_available = !uses_shared_cover
                    || review
                        .preferences
                        .shared_artwork_asset
                        .as_ref()
                        .is_none_or(|id| {
                            self.cover
                                .as_ref()
                                .is_some_and(|(asset, _)| asset.id == *id)
                        });
                !review.selected.is_empty()
                    && cover_available
                    && (review.preferences.output_kind != audio_engine::OutputKind::File
                        || review.selected.len() == 1)
            })
    }

    pub fn view(&self) -> Element<'_, Message> {
        view::view(self)
    }
    pub fn busy(&self) -> bool {
        self.job.is_some()
            || self.dialog_open
            || self.loading
            || self.close_after_job
            || self.save_pending > 0
    }
    pub fn can_export(&self) -> bool {
        self.can_preview() && !self.example
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch([
            iced::time::every(Duration::from_millis(100)).map(|_| Message::Tick),
            window::close_requests().map(Message::CloseRequested),
            iced::event::listen_with(|event, status, _window| {
                if matches!(event, iced::Event::Window(window::Event::Opened { .. })) {
                    return Some(Message::NativeReady);
                }
                if let iced::Event::Window(window::Event::Rescaled(scale)) = &event {
                    return Some(Message::DisplayScale(*scale));
                }
                if let iced::Event::Window(window::Event::FileDropped(path)) = &event {
                    return Some(Message::Dropped(path.clone()));
                }
                let iced::Event::Keyboard(keyboard::Event::KeyPressed { key, modifiers, .. }) =
                    event
                else {
                    return None;
                };
                if modifiers.command()
                    && matches!(&key, keyboard::Key::Character(value) if value.eq_ignore_ascii_case("k"))
                {
                    return Some(Message::Catalog(super::CatalogAction::FocusSearch));
                }
                if !modifiers.command()
                    && matches!(&key, keyboard::Key::Named(keyboard::key::Named::ArrowDown))
                {
                    return Some(Message::Catalog(super::CatalogAction::FocusResult));
                }
                if status == iced::event::Status::Captured {
                    return matches!(key, keyboard::Key::Named(keyboard::key::Named::Escape))
                        .then_some(Message::CapturedEscape);
                }
                match key {
                    keyboard::Key::Character(ref value)
                        if modifiers.command()
                            && (value.eq_ignore_ascii_case("q")
                                || value.eq_ignore_ascii_case("w")) =>
                    {
                        Some(Message::Quit)
                    }
                    keyboard::Key::Named(keyboard::key::Named::Enter) => {
                        Some(Message::PrimaryAction)
                    }
                    keyboard::Key::Character(ref value) if !modifiers.command() && value == "1" => {
                        Some(Message::ThemeShortcut(crate::theme::Preset::Dark))
                    }
                    keyboard::Key::Character(ref value) if !modifiers.command() && value == "2" => {
                        Some(Message::ThemeShortcut(crate::theme::Preset::Light))
                    }
                    keyboard::Key::Character(ref value) if !modifiers.command() && value == "3" => {
                        Some(Message::ThemeShortcut(
                            crate::theme::Preset::CatppuccinMocha,
                        ))
                    }
                    keyboard::Key::Named(keyboard::key::Named::Tab) => Some(if modifiers.shift() {
                        Message::FocusPrevious
                    } else {
                        Message::FocusNext
                    }),
                    keyboard::Key::Named(keyboard::key::Named::Escape) => Some(Message::Escape),
                    keyboard::Key::Character(ref value)
                        if modifiers.command() && value.eq_ignore_ascii_case("l") =>
                    {
                        Some(Message::FocusSource)
                    }
                    keyboard::Key::Character(ref value)
                        if modifiers.command()
                            && modifiers.shift()
                            && value.eq_ignore_ascii_case("s") =>
                    {
                        Some(Message::SaveAs)
                    }
                    keyboard::Key::Character(ref value)
                        if modifiers.command() && value.eq_ignore_ascii_case("s") =>
                    {
                        Some(Message::Save)
                    }
                    keyboard::Key::Character(ref value)
                        if modifiers.command()
                            && (value == "," || (modifiers.shift() && value == "<")) =>
                    {
                        Some(Message::OpenPanel(if modifiers.shift() {
                            Panel::Appearance
                        } else {
                            Panel::Settings
                        }))
                    }
                    _ => None,
                }
            }),
        ])
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Booted(result) => {
                self.loading = false;
                match result {
                    Ok(boot) => {
                        self.recovery = boot.recovery;
                        self.appearance = boot.appearance;
                        self.appearance_valid = boot.appearance_valid;
                        self.music_folder = boot.music_folder;
                        match self.appearance.theme.resolve(self.theme.revision + 1) {
                            Ok(theme) => self.theme = theme,
                            Err(error) => {
                                self.appearance_valid = false;
                                self.error = Some(format!("Could not restore the theme: {error}"));
                                self.theme = crate::theme::ResolvedTheme::preset(
                                    crate::theme::Preset::Dark,
                                    self.theme.revision + 1,
                                );
                            }
                        }
                        self.downloader_preferences = boot.downloader_preferences;
                        self.notice = boot.warning;
                        if let Some(draft) = boot.draft {
                            match Review::from_draft(draft) {
                                Ok(review) => {
                                    self.url = review.inspection.source_url.clone();
                                    self.review = Some(review);
                                    self.catalog.visible = false;
                                }
                                Err(error) => self.error = Some(error),
                            }
                        }
                        if let Some(id) = self
                            .review
                            .as_ref()
                            .and_then(|r| r.preferences.shared_artwork_asset.as_deref())
                        {
                            if let (Ok(asset), Ok(path)) = (
                                boot.desktop.artwork_asset(id),
                                boot.desktop.artwork_path(id),
                            ) {
                                self.cover = Some((asset, path));
                            }
                        }
                        self.desktop = Some(boot.desktop);
                        // Update checks retain the existing explicit preference and run outside the UI thread.
                        if !self.close_after_job {
                            if let Some(desktop) = &self.desktop {
                                self.admit(desktop.check_downloader(false, None));
                            }
                        }
                        return self.cover_preview();
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::Tick => return self.tick(),
            Message::Catalog(action) => return self.update_catalog(action),
            Message::CatalogThumbnail(url, image) => self.catalog.media.complete(url, image),
            Message::CatalogPortraits(ids, images) => {
                self.catalog.media.complete_portraits(ids, images)
            }
            Message::CatalogCover(url, Ok(raster)) => {
                if self
                    .catalog
                    .release
                    .as_ref()
                    .and_then(|release| release.cover.as_ref())
                    == Some(&url)
                {
                    self.catalog.cover = Some(raster);
                }
            }
            Message::ThemeShortcut(preset) if self.panel == Panel::Appearance => {
                return self.update(Message::ThemePreset(preset))
            }
            Message::PrimaryAction => {
                return match self.panel {
                    Panel::Workspace if self.catalog.visible => Task::none(),
                    Panel::Workspace if self.result.is_some() => self.update(Message::OpenOutput),
                    Panel::Workspace | Panel::Preview if self.can_export() => {
                        self.update(Message::Save)
                    }
                    Panel::Workspace | Panel::Options if self.can_preview() => {
                        self.update(Message::Preview)
                    }
                    Panel::MusicFolder if !self.busy() => self.update(Message::ApplyMusicFolder),
                    Panel::Editor | Panel::SongEditor | Panel::CollectionEditor if !self.busy() => {
                        self.update(Message::Editor(editor::Action::Apply))
                    }
                    Panel::Crop if !self.busy() && !self.example => self.update(Message::ApplyCrop),
                    Panel::Settings | Panel::Appearance => self.update(Message::ClosePanel),
                    _ => Task::none(),
                }
            }
            Message::NativeReady => {
                crate::platform::window_ready();
                return Task::batch([
                    self.load_owned_artwork(),
                    if self.catalog.visible && self.catalog.search_home() {
                        iced::widget::operation::focus("album-search")
                    } else {
                        Task::none()
                    },
                    window::oldest()
                        .and_then(window::scale_factor)
                        .map(Message::DisplayScale),
                ]);
            }
            Message::DisplayScale(scale) => {
                let task = self.display_scale_changed(scale);
                return Task::batch([task, self.refresh_receipt_cover()]);
            }
            Message::ReceiptCover(Ok(raster)) => self.update_receipt_cover(raster),
            Message::OwnedCoverRaster(id, scale, result) => {
                return self.owned_artwork_ready(id, scale, result)
            }
            Message::Quit => {
                return window::oldest()
                    .map(|window| window.map_or(Message::Tick, Message::CloseRequested))
            }
            Message::RetryStartup if !self.busy() && self.desktop.is_none() && !self.example => {
                self.loading = true;
                self.error = None;
                return persistence::background(
                    || persistence::load().map(Box::new),
                    Message::Booted,
                );
            }
            Message::Url(url) if !self.busy() && !self.panel.review_tool() => {
                self.url = url;
                self.intent = SourceIntent::Song;
                self.source_error = None;
            }
            Message::Pasted(url) if !self.busy() && !self.panel.review_tool() => {
                self.catalog.visible = false;
                self.url = url.trim().to_owned();
                self.intent = SourceIntent::Song;
                self.source_error = None;
                match SourceLink::parse(&self.url) {
                    Ok(link) if link.has_playlist_context() => {
                        return self.update(Message::FocusSource)
                    }
                    Ok(_) => return self.update(Message::Find),
                    Err(error) => {
                        self.source_error = Some(error);
                        return self.update(Message::FocusSource);
                    }
                }
            }
            Message::Intent(intent) if !self.busy() && !self.panel.review_tool() => {
                self.intent = intent;
                self.source_error = None;
            }
            Message::Find if !self.busy() && !self.panel.review_tool() => {
                if let Err(error) = SourceLink::parse(&self.url) {
                    self.source_error = Some(error);
                    return self.update(Message::FocusSource);
                }
                self.source_error = None;
                if (!self.example && self.review.is_some())
                    || self
                        .example_previous
                        .as_ref()
                        .is_some_and(|previous| previous.review.is_some())
                {
                    self.panel = Panel::Replace;
                } else {
                    return self.inspect();
                }
            }
            Message::ConfirmFind if !self.busy() && self.panel == Panel::Replace => {
                return self.inspect()
            }
            Message::Example if !self.busy() && !self.panel.review_tool() => {
                if !self.example {
                    self.example_previous = Some(self.suspend_review());
                }
                self.example = true;
                self.catalog.visible = false;
                self.review = Some(Review::new(
                    example::album(),
                    self.appearance.default_format.into(),
                ));
                self.cover = None;
                self.source_cover = None;
                self.result = None;
                self.preview = None;
                self.pending_plan = None;
                self.panel = Panel::Workspace;
                self.initialize_example_cover();
            }
            Message::TrackScrolled(offset) if offset.is_finite() => {
                self.track_scroll = offset.max(0.0);
            }
            Message::TrackListSized(height) if height.is_finite() => {
                self.track_viewport_height = height;
                if self.panel == Panel::CollectionEditor {
                    if let Some(editor::Kind::Track(index)) =
                        self.editor.as_ref().map(|editor| editor.kind)
                    {
                        let before = self.track_scroll;
                        self.reveal_track(index);
                        if self.track_scroll != before {
                            return self.restore_scroll();
                        }
                    }
                }
            }
            Message::Toggle(index, selected) if !self.busy() => {
                return self.change_review(|review| review.select(index, selected))
            }
            Message::SelectAll(selected) if !self.busy() => {
                return self.change_review(|review| review.select_all(selected))
            }
            Message::OpenEditor(kind) if !self.busy() && !self.panel.review_tool() => {
                if let Some(review) = &self.review {
                    self.editor = Some(Editor::new(kind, review));
                    if review.is_song() && matches!(kind, editor::Kind::Track(0)) {
                        self.panel = Panel::SongEditor;
                        return Task::batch([
                            iced::widget::operation::scroll_to(
                                "song-review",
                                iced::widget::scrollable::AbsoluteOffset::<f32>::default(),
                            ),
                            iced::widget::operation::focus("metadata-title"),
                        ]);
                    }
                    if !review.is_song()
                        && matches!(
                            kind,
                            editor::Kind::Track(_)
                                | editor::Kind::Album
                                | editor::Kind::PlaylistName
                        )
                    {
                        self.panel = Panel::CollectionEditor;
                        self.collection_editor_anchor =
                            matches!(kind, editor::Kind::Track(_)).then_some(self.track_scroll);
                        return iced::widget::operation::focus(
                            if matches!(kind, editor::Kind::Album) {
                                "metadata-album"
                            } else {
                                "metadata-title"
                            },
                        );
                    }
                    self.panel = Panel::Editor;
                    return iced::widget::operation::focus(if matches!(kind, editor::Kind::Bulk) {
                        "metadata-prefix"
                    } else {
                        "metadata-title"
                    });
                }
            }
            Message::Editor(editor::Action::Apply | editor::Action::ApplyNext) if !self.busy() => {
                let focus = self.editor_return_focus();
                if let (Some(editor), Some(review)) = (&mut self.editor, &mut self.review) {
                    match editor.apply(review) {
                        Ok(()) => {
                            if matches!(message, Message::Editor(editor::Action::ApplyNext))
                                && self.panel == Panel::CollectionEditor
                            {
                                if let editor::Kind::Track(index) = editor.kind {
                                    let next = index + 1;
                                    if next < review.inspection.tracks.len() {
                                        self.editor =
                                            Some(Editor::new(editor::Kind::Track(next), review));
                                        self.collection_editor_anchor = None;
                                        self.reveal_track(next);
                                        return self.autosave().chain(
                                            iced::widget::operation::focus("metadata-title"),
                                        );
                                    }
                                }
                            }
                            self.panel = Panel::Workspace;
                            self.restore_collection_anchor();
                            self.editor = None;
                            let saved = self.autosave();
                            return match focus {
                                Some(id) => saved.chain(iced::widget::operation::focus(id)),
                                None => saved,
                            };
                        }
                        Err(error) => editor.error = Some(error),
                    }
                }
            }
            Message::Editor(action) if !self.busy() => {
                if let Some(editor) = &mut self.editor {
                    editor.input(action);
                }
            }
            Message::RestoreSourceTags if !self.busy() && self.panel.inline_editor() => {
                if let (Some(editor), Some(review)) = (&mut self.editor, &self.review) {
                    editor.restore_source_tags(review);
                    return iced::widget::operation::focus("metadata-title");
                }
            }
            Message::Format(format) if !self.busy() => {
                return self.change_review(|review| {
                    review.preferences.format = format.into();
                    review.changed()
                })
            }
            Message::Artwork(artwork) if !self.busy() => {
                return self.change_review(|review| {
                    review.preferences.artwork_mode = artwork.into();
                    review.changed()
                })
            }
            Message::Output(output) if !self.busy() => {
                return self.change_review(|review| {
                    review.preferences.output_kind = output.into();
                    review.changed()
                })
            }
            Message::Mode(mode) if !self.busy() => {
                return self.change_review(|review| review.set_mode(mode.into()))
            }
            Message::UndoBulk if !self.busy() => {
                return self.change_review(|review| review.undo_bulk().map(|_| ()))
            }
            Message::NumberTracks if !self.busy() => {
                return self.change_review(Review::number_album_tracks)
            }
            Message::UndoNumbering if !self.busy() => {
                return self.change_review(|review| review.undo_numbering().map(|_| ()))
            }
            Message::MoveTrack { from, to }
                if !self.busy()
                    && self.panel == Panel::TrackOrder
                    && self.track_position.is_none() =>
            {
                return self.move_review_track(from, to);
            }
            Message::UndoTrackMove
                if !self.busy()
                    && self.panel == Panel::TrackOrder
                    && self.track_position.is_none() =>
            {
                return self.undo_review_move();
            }
            Message::ChooseTrackPosition(index)
                if !self.busy() && self.panel == Panel::TrackOrder =>
            {
                return self.choose_track_position(index);
            }
            Message::TrackPosition(value) if self.panel == Panel::TrackOrder => {
                if let Some(edit) = &mut self.track_position {
                    edit.position = value;
                }
            }
            Message::ApplyTrackPosition if !self.busy() && self.panel == Panel::TrackOrder => {
                if let Some(to) = self.track_position_target() {
                    if let Some(edit) = &self.track_position {
                        if edit.index == to {
                            return self.cancel_track_position();
                        }
                    }
                    if let Some(edit) = self.track_position.take() {
                        return self.move_review_track(edit.index, to);
                    }
                }
            }
            Message::CancelTrackPosition if self.panel == Panel::TrackOrder => {
                return self.cancel_track_position();
            }
            Message::OpenPanel(Panel::TrackOrder) => {
                if !self.busy()
                    && self.panel == Panel::Workspace
                    && self
                        .review
                        .as_ref()
                        .is_some_and(|review| review.inspection.tracks.len() > 1)
                {
                    self.panel = Panel::TrackOrder;
                    self.arranged_track = None;
                    return iced::widget::operation::focus("finish-track-order");
                }
            }
            Message::OpenPanel(panel)
                if !self.dialog_open
                    && ((!self.panel.review_tool()
                        && !matches!(
                            self.panel,
                            Panel::Editor | Panel::Cover | Panel::Crop | Panel::MusicFolder
                        ))
                        || panel == self.panel) =>
            {
                self.panel = panel
            }
            Message::ClosePanel => {
                if self.dialog_open {
                    return Task::none();
                }
                if self.panel == Panel::MusicFolder {
                    return self.close_music_folder();
                }
                if self.panel == Panel::Crop && self.crop_return_to_cover {
                    self.crop_return_to_cover = false;
                    self.crop_source = None;
                    self.crop_dragging = false;
                    self.panel = Panel::Cover;
                    self.error = None;
                    return iced::widget::operation::focus("review-crop-cover");
                }
                let focus = self.editor_return_focus().or_else(|| match self.panel {
                    Panel::Cover | Panel::Crop => Some("edit-cover".into()),
                    Panel::TrackOrder => Some("open-track-order".into()),
                    _ => None,
                });
                self.editor = None;
                self.restore_collection_anchor();
                self.arranged_track = None;
                self.track_position = None;
                self.crop_source = None;
                self.crop_dragging = false;
                if self.panel == Panel::Cover {
                    self.error = None;
                }
                self.panel = Panel::Workspace;
                self.pending_plan = None;
                return Task::batch([
                    self.restore_scroll(),
                    focus.map_or_else(Task::none, iced::widget::operation::focus),
                ]);
            }
            Message::Save
                if !self.busy() && (self.panel == Panel::Editor || self.panel.inline_editor()) =>
            {
                return self.update(Message::Editor(editor::Action::Apply));
            }
            Message::Save if self.panel == Panel::Options => {
                return self.update(Message::Preview);
            }
            Message::Preview
                if !self.busy()
                    && self.result.is_none()
                    && matches!(self.panel, Panel::Workspace | Panel::Options) =>
            {
                if let Err(error) = self.prepare_preview() {
                    self.error = Some(error);
                } else {
                    return Task::batch([self.preview_artwork(), self.sync_album_scan()]);
                }
            }
            Message::AlbumScanned(request, result) if request == self.album_scan => {
                self.album_preview = Some(result);
            }
            Message::ScanAlbumFolder
                if matches!(self.panel, Panel::Workspace | Panel::Preview) && !self.busy() =>
            {
                self.invalidate_album_scan();
                return self.sync_album_scan();
            }
            Message::Save | Message::SaveAs
                if self.can_export()
                    && self.result.is_none()
                    && matches!(self.panel, Panel::Workspace | Panel::Preview) =>
            {
                return self.save_export(matches!(message, Message::SaveAs));
            }
            Message::Destination(result) => {
                self.dialog_open = false;
                self.notice = None;
                if self.close_after_job {
                    self.pending_plan = None;
                    return self.close();
                }
                let destination = result.unwrap_or_else(|error| {
                    self.error = Some(error);
                    None
                });
                let Some(destination) = destination else {
                    if self.panel == Panel::Workspace {
                        self.pending_plan = None;
                    }
                    return iced::widget::operation::focus(if self.uses_music_folder() {
                        "save-as"
                    } else {
                        "save-output"
                    });
                };
                if let Some(plan) = self.pending_plan.clone() {
                    let destination =
                        super::destination::resolve(plan.output_kind, &plan.title, destination);
                    return self.begin_export(
                        plan,
                        super::destination::ExportDestination::Picked(destination),
                    );
                }
            }
            Message::ExportSaved(plan, destination, identity, result) => {
                self.dialog_open = false;
                self.notice = None;
                if self.close_after_job {
                    return self.close();
                }
                match result {
                    Ok(true)
                        if self
                            .review
                            .as_ref()
                            .is_some_and(|review| review.identity() == identity) =>
                    {
                        self.save_failed = false;
                        if let Some(desktop) = &self.desktop {
                            use super::destination::ExportDestination;
                            let recording = super::receipt::Recording::from_plan(self, &plan);
                            let job = match destination {
                                ExportDestination::Picked(path) => {
                                    desktop.export(*plan, path, Some(identity))
                                }
                                ExportDestination::Organized(output) => {
                                    desktop.export_organized(*plan, &output, Some(identity))
                                }
                            };
                            if job.is_ok() {
                                self.export_recording = Some(recording);
                                self.pending_plan = None;
                                self.panel = Panel::Workspace;
                            }
                            self.admit(job);
                        }
                    }
                    Ok(_) => {
                        self.error = Some(
                            "The saved review changed. Preview your files again before exporting."
                                .into(),
                        )
                    }
                    Err(error) => {
                        self.save_failed = true;
                        self.pending_plan = Some(*plan);
                        self.error = Some(format!(
                            "Your review could not be saved before export: {error}"
                        ));
                    }
                }
            }
            Message::OpenMusicFolder | Message::PickMusicFolder
                if !self.busy()
                    && matches!(
                        self.panel,
                        Panel::Workspace | Panel::Preview | Panel::Options | Panel::Settings
                    ) =>
            {
                return self.open_music_folder(matches!(message, Message::PickMusicFolder));
            }
            Message::ChooseMusicRoot if !self.busy() && self.panel == Panel::MusicFolder => {
                return self.pick_music_root();
            }
            Message::MusicRootPicked(result) if self.music_folder_editor.is_some() => {
                self.dialog_open = false;
                self.notice = None;
                if self.close_after_job {
                    return self.close();
                }
                if let Some(editor) = &mut self.music_folder_editor {
                    match result {
                        Ok(Some(path)) => {
                            editor.root = Some(path);
                            if editor.direct {
                                return self.apply_music_folder();
                            }
                            return iced::widget::operation::focus("save-music-folder");
                        }
                        Ok(None) if editor.direct => return self.close_music_folder(),
                        Ok(None) => {}
                        Err(error) => {
                            editor.error = Some(error);
                            self.panel = Panel::MusicFolder;
                        }
                    }
                }
                return iced::widget::operation::focus("music-folder-root");
            }
            Message::TogglePathRule if !self.busy() && self.panel == Panel::MusicFolder => {
                if let Some(editor) = &mut self.music_folder_editor {
                    editor.customize = !editor.customize;
                    if editor.customize {
                        return iced::widget::operation::focus("music-folder-rule");
                    }
                }
            }
            Message::MusicRule(rule) if !self.busy() && self.panel == Panel::MusicFolder => {
                if let Some(editor) = &mut self.music_folder_editor {
                    editor.rule = rule;
                    editor.error = None;
                }
            }
            Message::ApplyMusicFolder if !self.busy() && self.panel == Panel::MusicFolder => {
                return self.apply_music_folder();
            }
            Message::ClearMusicFolder
                if !self.busy()
                    && self.panel == Panel::MusicFolder
                    && self.music_folder.is_some() =>
            {
                return self.save_music_folder(None);
            }
            Message::MusicFolderSaved(result) if self.music_folder_editor.is_some() => {
                self.dialog_open = false;
                self.notice = None;
                match result {
                    Ok(folder) => {
                        self.music_folder = folder;
                        self.invalidate_album_scan();
                        if self.close_after_job {
                            return self.close();
                        }
                        return self.close_music_folder();
                    }
                    Err(error) => {
                        self.close_after_job = false;
                        self.panel = Panel::MusicFolder;
                        if let Some(editor) = &mut self.music_folder_editor {
                            editor.error = Some(error);
                        }
                        return iced::widget::operation::focus("music-folder-root");
                    }
                }
            }
            Message::Cancel => {
                if let Some(job) = &self.job {
                    job.cancel();
                    if self.catalog.pending.is_some() {
                        self.catalog.cancelling = true;
                    } else if let Some(intake) = &mut self.intake {
                        intake.cancelling = true;
                    } else {
                        self.notice = Some(
                            "Cancelling… completed source downloads are kept for retry.".into(),
                        );
                    }
                }
            }
            Message::Saved(identity, result) => {
                self.save_pending = self.save_pending.saturating_sub(1);
                if self
                    .review
                    .as_ref()
                    .is_some_and(|review| review.identity() == identity)
                {
                    match result {
                        Ok(true) => self.save_failed = false,
                        Ok(false) => {
                            self.save_failed = true;
                            self.error=Some("Your review could not be saved because the saved snapshot changed.".into());
                        }
                        Err(error) => {
                            self.save_failed = true;
                            self.error = Some(format!("Your review could not be saved: {error}"));
                        }
                    }
                }
            }
            Message::DismissError => {
                self.error = None;
                self.notice = None;
            }
            Message::Reset if !self.busy() && !self.panel.review_tool() => {
                self.panel = Panel::Discard
            }
            Message::ConfirmReset if !self.busy() && !self.panel.review_tool() => {
                self.panel = Panel::Workspace;
                if self.example {
                    self.example = false;
                    if let Some(previous) = self.example_previous.take() {
                        self.restore_review(previous);
                    } else {
                        self.clear_review();
                    }
                    return Task::none();
                }
                if let (Some(desktop), Some(review)) = (self.desktop.clone(), &self.review) {
                    let identity = review.identity();
                    self.dialog_open = true;
                    return persistence::background(
                        move || desktop.discard_review(&identity.session_id, identity.revision + 1),
                        Message::ReviewDiscarded,
                    );
                }
                self.clear_review();
            }
            Message::ReviewDiscarded(result) => {
                self.dialog_open = false;
                match result {
                    Ok(true) => self.clear_review(),
                    Ok(false) => {
                        self.error = Some(
                            "The saved review changed. Reopen Jacket before discarding it.".into(),
                        )
                    }
                    Err(error) => self.error = Some(error),
                }
                if self.close_after_job {
                    return self.close();
                }
            }
            Message::Resume if !self.busy() && !self.panel.review_tool() => {
                if let (Some(desktop), Some(record)) = (&self.desktop, self.recovery.clone()) {
                    match desktop.resume(record.clone()) {
                        Ok(job) => {
                            self.export_recording =
                                Some(super::receipt::Recording::from_plan(self, &record.plan));
                            let previous = if self.example {
                                self.example_previous
                                    .take()
                                    .unwrap_or_else(|| self.suspend_review())
                            } else {
                                self.suspend_review()
                            };
                            self.resumed_previous =
                                Some((previous, record.review_identity.clone()));
                            self.example = false;
                            self.review = Some(Review::from_plan(&record.plan));
                            self.catalog.visible = false;
                            self.url = record.plan.source_url.clone();
                            self.auth = record.plan.auth.clone();
                            self.cover = None;
                            self.admit(Ok(job));
                            self.panel = Panel::Workspace;
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            Message::DismissRecovery if !self.busy() && !self.panel.review_tool() => {
                if let (Some(desktop), Some(record)) = (self.desktop.clone(), self.recovery.clone())
                {
                    return persistence::background(
                        move || desktop.dismiss_recovery(&record.job_id),
                        Message::RecoveryDismissed,
                    );
                }
            }
            Message::RecoveryDismissed(result) => match result {
                Ok(()) => self.recovery = None,
                Err(error) => self.error = Some(error),
            },
            Message::CloseRequested(id) => {
                self.close_window = Some(id);
                if self.job.is_some() {
                    self.panel = Panel::Close;
                } else {
                    return self.close();
                }
            }
            Message::ConfirmClose => {
                self.close_after_job = true;
                self.panel = Panel::Workspace;
                if let Some(job) = &self.job {
                    job.cancel();
                    if self.catalog.pending.is_some() {
                        self.catalog.cancelling = true;
                    } else if let Some(intake) = &mut self.intake {
                        intake.cancelling = true;
                    }
                } else {
                    return self.close();
                }
            }
            Message::CloseReady(result) => match result {
                Ok(()) => {
                    if let Some(id) = self.close_window {
                        return window::close(id);
                    }
                }
                Err(error) => {
                    self.dialog_open = false;
                    self.close_after_job = false;
                    self.error = Some(format!(
                        "Jacket could not save your review before closing: {error}"
                    ));
                }
            },
            Message::Browser(browser) if !self.busy() => {
                if browser == BrowserChoice::Public {
                    return self.update(Message::PublicAccess);
                }
                self.browser = browser;
                self.profile = None;
                self.profiles.clear();
                self.profile_notice = None;
                if let (Some(browser_kind), Some(desktop)) =
                    (browser.browser(), self.desktop.clone())
                {
                    return persistence::background(
                        move || Ok(desktop.browser_profiles(browser_kind)),
                        move |result| match result {
                            Ok(catalog) => Message::Profiles(browser, catalog),
                            Err(error) => Message::Opened(Err(error)),
                        },
                    );
                }
            }
            Message::Profiles(browser, catalog) if browser == self.browser => {
                self.profiles = catalog
                    .profiles
                    .into_iter()
                    .map(|profile| ProfileChoice {
                        id: profile.id,
                        label: format!("{} · {}", profile.label, profile.detail),
                    })
                    .collect();
                self.profile_notice = catalog.notice;
            }
            Message::Profile(profile) if !self.busy() => self.profile = Some(profile),
            Message::UseSession if !self.busy() => {
                if let (Some(browser), Some(profile)) = (self.browser.browser(), &self.profile) {
                    self.auth = AuthOptions {
                        browser: Some(browser),
                        browser_profile: Some(profile.id.clone()),
                        ..AuthOptions::default()
                    };
                    self.notice = Some("Browser session connected.".into());
                }
            }
            Message::PublicAccess if !self.busy() => {
                self.auth = AuthOptions::default();
                self.browser = BrowserChoice::Public;
                self.profile = None;
                self.notice = Some("Public access enabled.".into());
            }
            Message::TestAccess if !self.busy() => {
                if let Some(desktop) = &self.desktop {
                    match SourceLink::parse(&self.url) {
                        Ok(link) => {
                            self.admit(desktop.test_access(
                                link.target(self.intent).to_owned(),
                                self.auth.clone(),
                            ))
                        }
                        Err(error) => self.error = Some(error),
                    }
                }
            }
            Message::ThemePreset(preset) if !self.close_after_job => {
                let selection = crate::theme::Selection::Preset { preset };
                self.theme = crate::theme::ResolvedTheme::preset(preset, self.theme.revision + 1);
                self.appearance.theme = selection;
                return self.save_appearance();
            }
            Message::ImportTheme if !self.dialog_open && !self.close_after_job => {
                self.dialog_open = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Load theme")
                            .add_filter("TOML theme", &["toml"])
                            .pick_file()
                            .await
                            .map(|file| file.path().to_owned())
                    },
                    Message::ThemeFile,
                );
            }
            Message::ThemeFile(path) => {
                self.dialog_open = false;
                if let Some(path) = path {
                    self.dialog_open = true;
                    return persistence::background(
                        move || crate::theme::Selection::import(&path),
                        Message::ThemeImported,
                    );
                }
            }
            Message::ThemeImported(result) => {
                self.dialog_open = false;
                match result.and_then(|selection| {
                    selection
                        .resolve(self.theme.revision + 1)
                        .map(|theme| (selection, theme))
                }) {
                    Ok((selection, theme)) => {
                        self.theme = theme;
                        self.appearance.theme = selection;
                        return self.save_appearance();
                    }
                    Err(error) => self.error = Some(format!("Theme could not be loaded: {error}")),
                }
            }
            Message::PasteLink if !self.busy() && !self.panel.review_tool() => {
                return iced::clipboard::read()
                    .map(|value| Message::Pasted(value.unwrap_or_default()))
            }
            Message::FocusNext => return iced::widget::operation::focus_next(),
            Message::FocusPrevious => return iced::widget::operation::focus_previous(),
            Message::DefaultFormat(format) if !self.close_after_job => {
                self.appearance.default_format = format.into();
                return self.save_appearance();
            }
            Message::AppearanceSaved(result) => {
                self.appearance_saving = false;
                if let Err(error) = result {
                    self.appearance_dirty = true;
                    self.error = Some(error);
                } else if self.appearance_dirty && !self.close_after_job {
                    return self.flush_appearance();
                }
            }
            Message::CheckDownloader if !self.busy() => {
                if let Some(desktop) = &self.desktop {
                    self.admit(desktop.check_downloader(true, None));
                }
            }
            Message::InstallDownloader if !self.busy() => {
                if let Some(desktop) = &self.desktop {
                    self.admit(
                        desktop.update_downloader(
                            self.downloader
                                .as_ref()
                                .map_or(Channel::Nightly, |s| s.channel),
                        ),
                    );
                }
            }
            Message::RestoreDownloader if !self.busy() => {
                if let Some(desktop) = &self.desktop {
                    self.admit(desktop.restore_downloader());
                }
            }
            Message::AutoUpdate(enabled) if !self.busy() => {
                if let Some(desktop) = &self.desktop {
                    let mut prefs = self.downloader_preferences.clone();
                    prefs.auto_update = enabled;
                    self.admit(desktop.configure_downloader(prefs));
                }
            }
            Message::PickDownloader if !self.busy() => {
                self.dialog_open = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose a local yt-dlp executable")
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    Message::DownloaderPath,
                );
            }
            Message::DownloaderPath(path) => {
                self.dialog_open = false;
                if self.close_after_job {
                    return self.close();
                }
                if let (Some(path), Some(desktop)) = (path, &self.desktop) {
                    let mut prefs = self.downloader_preferences.clone();
                    prefs.mode = downloader_updates::Mode::Local;
                    prefs.local_path = Some(path.to_string_lossy().into_owned());
                    self.admit(desktop.configure_downloader(prefs));
                }
            }
            Message::PreviewCover(source, result) => {
                if self
                    .pending_plan
                    .as_ref()
                    .and_then(crate::artwork::preview_url)
                    == Some(source.as_str())
                {
                    match result {
                        Ok(handle) => {
                            if handle.scale() != self.display_scale {
                                let scale = self.display_scale;
                                return persistence::background(
                                    move || Ok(handle.for_scale(scale)),
                                    move |result| Message::PreviewCover(source, result),
                                );
                            }
                            self.preview_cover = Some(handle);
                            self.preview_cover_error = None;
                        }
                        Err(error) => self.preview_cover_error = Some(error),
                    }
                }
            }
            Message::SourceCover(source, result) => {
                if self.cover_url() == Some(source.as_str()) {
                    match result {
                        Ok(handle) => {
                            if handle.scale() != self.display_scale {
                                let scale = self.display_scale;
                                return persistence::background(
                                    move || Ok(handle.for_scale(scale)),
                                    move |result| Message::SourceCover(source, result),
                                );
                            }
                            self.source_cover = Some(handle);
                            self.source_cover_error = None;
                        }
                        Err(error) => self.source_cover_error = Some(error),
                    }
                }
            }
            Message::IntakeCover(request, result) => {
                return self.intake_cover_ready(request, result)
            }
            Message::RetryArtwork if !self.busy() => {
                return if self.panel == Panel::Preview {
                    self.preview_artwork()
                } else {
                    self.cover_preview()
                };
            }
            Message::Dropped(path)
                if !self.busy()
                    && !self.example
                    && self.review.is_some()
                    && matches!(self.panel, Panel::Workspace | Panel::Cover | Panel::Options) =>
            {
                if path.extension().is_some_and(|extension| {
                    ["jpg", "jpeg", "png", "webp"]
                        .iter()
                        .any(|kind| extension.eq_ignore_ascii_case(kind))
                }) {
                    return self.update(Message::CoverFile(Some(path)));
                }
            }
            Message::UseSourceCover
                if !self.busy()
                    && !self.example
                    && matches!(self.panel, Panel::Workspace | Panel::Cover) =>
            {
                return self.use_source_cover();
            }
            Message::ImportCover
                if !self.busy()
                    && !self.example
                    && self.review.is_some()
                    && matches!(self.panel, Panel::Workspace | Panel::Cover | Panel::Options) =>
            {
                self.dialog_open = true;
                return Task::perform(
                    async {
                        rfd::AsyncFileDialog::new()
                            .set_title("Choose cover artwork")
                            .add_filter("Cover images", &["jpg", "jpeg", "png", "webp"])
                            .pick_file()
                            .await
                            .map(|f| f.path().to_path_buf())
                    },
                    Message::CoverFile,
                );
            }
            Message::CoverFile(path) => {
                self.dialog_open = false;
                if self.close_after_job {
                    return self.close();
                }
                if path.is_none()
                    && matches!(self.panel, Panel::Workspace | Panel::Cover)
                    && self.review.is_some()
                {
                    return iced::widget::operation::focus(if self.panel == Panel::Cover {
                        "review-choose-cover"
                    } else {
                        "edit-cover"
                    });
                }
                if let (Some(path), Some(desktop)) = (path, self.desktop.clone()) {
                    self.dialog_open = true;
                    return persistence::background(
                        move || {
                            let asset = desktop.import_artwork(&path, None)?;
                            let path = desktop.artwork_path(&asset.id)?;
                            Ok((asset, path))
                        },
                        Message::CoverImported,
                    );
                }
            }
            Message::CoverImported(result) => {
                self.dialog_open = false;
                if self.close_after_job {
                    return self.close();
                }
                match result {
                    Ok((asset, path)) => {
                        let Some(review) = &mut self.review else {
                            return Task::none();
                        };
                        if let Err(error) = review.changed() {
                            self.error = Some(error);
                            return Task::none();
                        }
                        review.preferences.shared_artwork_asset = Some(asset.id.clone());
                        review.preferences.artwork_mode = audio_engine::ArtworkMode::Shared;
                        self.cover = Some((asset, path));
                        self.crop_source = None;
                        self.source_cover_error = None;
                        self.preview_cover_error = None;
                        if self.panel == Panel::Crop && self.crop_return_to_cover {
                            self.panel = Panel::Cover;
                        } else if self.panel != Panel::Cover {
                            self.panel = Panel::Workspace;
                        }
                        self.crop_return_to_cover = false;
                        let focus = if self.panel == Panel::Cover {
                            "review-choose-cover"
                        } else {
                            "edit-cover"
                        };
                        return Task::batch([
                            self.autosave().chain(iced::widget::operation::focus(focus)),
                            self.load_owned_artwork(),
                        ]);
                    }
                    Err(error) => self.error = Some(error),
                }
            }
            Message::CropPointer(point) if self.panel == Panel::Crop => {
                self.crop_pointer = point;
                if self.crop_dragging {
                    self.position_crop();
                }
            }
            Message::CropDown if self.panel == Panel::Crop => {
                self.crop_dragging = true;
                self.position_crop();
            }
            Message::CropUp => self.crop_dragging = false,
            Message::CropCover
                if !self.busy()
                    && !self.example
                    && matches!(self.panel, Panel::Workspace | Panel::Cover | Panel::Options) =>
            {
                if let Some((asset, path)) = &self.cover {
                    return self.edit_cover_crop(asset.clone(), path.clone());
                } else if let (Some(desktop), Some(review), Some(url)) = (
                    self.desktop.clone(),
                    self.review.as_ref(),
                    self.cover_url().map(str::to_owned),
                ) {
                    let identity = review.identity();
                    self.dialog_open = true;
                    self.notice = Some("Preparing cover for cropping…".into());
                    return persistence::background(
                        move || {
                            let asset = desktop.import_remote_artwork(&url)?;
                            let path = desktop.artwork_path(&asset.id)?;
                            Ok((asset, path))
                        },
                        move |result| Message::RemoteCoverImported(identity, result),
                    );
                }
            }
            Message::RemoteCoverImported(identity, result) => {
                self.dialog_open = false;
                self.notice = None;
                if self.close_after_job {
                    return self.close();
                }
                if self
                    .review
                    .as_ref()
                    .is_some_and(|review| review.identity() == identity)
                {
                    match result {
                        Ok((asset, path)) => {
                            return self.edit_cover_crop(asset, path);
                        }
                        Err(error) => {
                            self.error = Some(format!(
                                "The cover could not be prepared for cropping: {error}"
                            ))
                        }
                    }
                }
            }
            Message::CropX(value) if self.panel == Panel::Crop && !self.busy() => {
                self.crop_x = value;
                self.error = None;
            }
            Message::CropY(value) if self.panel == Panel::Crop && !self.busy() => {
                self.crop_y = value;
                self.error = None;
            }
            Message::CropSize(value) if self.panel == Panel::Crop && !self.busy() => {
                self.resize_cover_crop(value)
            }
            Message::ApplyCrop if !self.busy() && !self.example && self.panel == Panel::Crop => {
                if let (Some((asset, _)), Some(desktop)) = (self.crop_asset(), self.desktop.clone())
                {
                    match self
                        .crop_x
                        .parse::<u32>()
                        .ok()
                        .zip(self.crop_y.parse::<u32>().ok())
                        .zip(self.crop_size.parse::<u32>().ok())
                    {
                        Some(((x, y), size))
                            if size > 0
                                && x.checked_add(size).is_some_and(|n| n <= asset.width)
                                && y.checked_add(size).is_some_and(|n| n <= asset.height) =>
                        {
                            let id = asset.id.clone();
                            self.dialog_open = true;
                            return persistence::background(
                                move || {
                                    let asset = desktop.crop_artwork(&id, Crop { x, y, size })?;
                                    let path = desktop.artwork_path(&asset.id)?;
                                    Ok((asset, path))
                                },
                                Message::CoverImported,
                            );
                        }
                        _ => {
                            self.error = Some("Enter a square crop inside the image bounds.".into())
                        }
                    }
                }
            }
            Message::OpenOutput => {
                if let (Some(desktop), Some(receipt)) = (self.desktop.clone(), &self.result) {
                    let result = receipt.output.clone();
                    return persistence::background(
                        move || desktop.reveal_export(&result),
                        Message::Opened,
                    );
                }
            }
            Message::OpenPlaylist => {
                if let (Some(desktop), Some(receipt)) = (self.desktop.clone(), &self.result) {
                    let result = receipt.output.clone();
                    return persistence::background(
                        move || desktop.open_playlist(&result),
                        Message::Opened,
                    );
                }
            }
            Message::DismissReceipt if !self.busy() && self.result.is_some() => {
                self.result = None;
                self.error = None;
                return if self.review.is_some() {
                    self.restore_scroll()
                } else {
                    self.update(Message::FocusSource)
                };
            }
            Message::Opened(Err(error)) => self.error = Some(error),
            Message::FocusSource if !self.busy() && self.panel == Panel::Workspace => {
                self.catalog.visible = false;
                return iced::widget::operation::focus("source-url");
            }
            Message::Escape | Message::CapturedEscape
                if matches!(message, Message::Escape)
                    || self.panel.review_tool()
                    || self.panel == Panel::Crop
                    || self.catalog.visible =>
            {
                if self.panel == Panel::TrackOrder && self.track_position.is_some() {
                    return self.cancel_track_position();
                }
                if self.panel != Panel::Workspace {
                    return self.update(Message::ClosePanel);
                } else if self.catalog.pending.is_some() {
                    return self.update_catalog(super::CatalogAction::Cancel);
                } else if self.job.is_some() {
                    return self.update(Message::Cancel);
                } else if self.catalog.visible {
                    return self.update_catalog(super::CatalogAction::Back);
                }
            }
            Message::Screenshot(screenshot) => {
                if let Some(path) = self.screenshot.take() {
                    return persistence::background(
                        move || {
                            image::save_buffer(
                                &path,
                                &screenshot.rgba,
                                screenshot.size.width,
                                screenshot.size.height,
                                image::ColorType::Rgba8,
                            )
                            .map_err(|e| e.to_string())
                        },
                        Message::ScreenshotSaved,
                    );
                }
            }
            Message::ScreenshotSaved(result) => {
                if let Err(error) = result {
                    eprintln!("Screenshot failed: {error}");
                }
                return window::oldest().and_then(window::close);
            }
            _ => {}
        }
        Task::none()
    }
}
