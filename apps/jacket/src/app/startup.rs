use super::persistence;
use super::{App, Message, Panel};
use crate::editor::{self, Editor};
use crate::example;
use crate::settings::{Appearance, BrowserChoice};
use audio_engine::{AudioFormat, AuthOptions};
use iced::Task;
use jacket_core::{Review, SourceIntent};
use jacket_desktop::downloader_updates;
use std::path::PathBuf;
use std::time::Instant;

impl App {
    /// Keep the isolated sample's selected cover and file preview consistent.
    /// The temporary asset never enters the ordinary artwork or review stores.
    pub(super) fn initialize_example_cover(&mut self) {
        if !self.example || self.review.is_none() {
            return;
        }
        match example::cover() {
            Ok((asset, file)) => {
                if let Some(review) = &mut self.review {
                    review.preferences.shared_artwork_asset = Some(asset.id.clone());
                    review.preferences.artwork_mode = audio_engine::ArtworkMode::Shared;
                }
                self.cover = Some((asset, file.path().to_owned()));
                self.source_cover = None;
                self.preview_cover = None;
                self._example_cover_file = Some(file);
            }
            Err(error) => {
                self.error = Some(format!("Sample artwork could not be prepared: {error}"))
            }
        }
    }

    pub fn new() -> (Self, Task<Message>) {
        let args: Vec<String> = std::env::args().collect();
        let screenshot = args
            .iter()
            .position(|arg| arg == "--screenshot")
            .and_then(|index| args.get(index + 1))
            .map(PathBuf::from);
        let isolated = args.iter().any(|arg| arg == "--example") || screenshot.is_some();
        let mut appearance = Appearance {
            theme: crate::theme::Selection::Preset {
                preset: if args.iter().any(|a| a == "--light") {
                    crate::theme::Preset::Light
                } else if args.iter().any(|a| a == "--mocha") {
                    crate::theme::Preset::CatppuccinMocha
                } else {
                    crate::theme::Preset::Dark
                },
            },
            ..Appearance::default()
        };
        let mut theme_error = None;
        if isolated {
            if let Some(path) = args
                .iter()
                .position(|arg| arg == "--theme-file")
                .and_then(|index| args.get(index + 1))
            {
                match crate::theme::Selection::import(std::path::Path::new(path)) {
                    Ok(selection) => appearance.theme = selection,
                    Err(error) => theme_error = Some(error),
                }
            }
        }
        let mut app = Self {
            desktop: None,
            review: None,
            job: None,
            url: String::new(),
            intent: SourceIntent::Song,
            panel: Panel::Workspace,
            editor: None,
            preview: None,
            progress: None,
            result: None,
            recovery: None,
            error: theme_error,
            notice: None,
            theme: appearance
                .theme
                .resolve(1)
                .expect("startup themes have already been validated"),
            appearance,
            example: isolated,
            loading: !isolated,
            dialog_open: false,
            save_pending: 0,
            save_failed: false,
            browser: BrowserChoice::Public,
            profiles: Vec::new(),
            profile: None,
            profile_notice: None,
            auth: AuthOptions::default(),
            downloader: None,
            downloader_preferences: downloader_updates::Preferences::default(),
            cover: None,
            source_cover: None,
            preview_cover: None,
            source_cover_error: None,
            preview_cover_error: None,
            crop_source: None,
            track_scroll: 0.0,
            crop_x: "0".into(),
            crop_y: "0".into(),
            crop_size: "1000".into(),
            pending_plan: None,
            close_window: None,
            close_after_job: false,
            screenshot,
            started: Instant::now(),
            example_previous: None,
            resumed_previous: None,
            crop_pointer: iced::Point::new(180.0, 130.0),
            crop_dragging: false,
            appearance_saving: false,
            appearance_dirty: false,
            appearance_valid: true,
            _example_cover_file: None,
        };
        if isolated {
            let mut inspection = example::album();
            if args.iter().any(|arg| arg == "--song") {
                inspection.tracks.truncate(1);
                inspection.suggested_mode = audio_engine::CollectionMode::Collection;
                inspection.title = inspection.tracks[0].metadata.title.clone();
            }
            if args.iter().any(|arg| arg == "--playlist") {
                inspection.suggested_mode = audio_engine::CollectionMode::Collection;
                inspection.title = "Evening rotation".into();
            }
            if args.iter().any(|arg| arg == "--long") {
                if let Some(track) = inspection.tracks.first_mut() {
                    track.metadata.title = "A very long song title — live at the old theatre, remastered and expanded edition".into();
                    track.metadata.artist =
                        "The Paper Trails featuring the Northern Chamber Orchestra".into();
                    track.metadata.album = "Room to breathe: the complete anniversary recordings and unreleased sessions".into();
                }
            }
            if args.iter().any(|arg| arg == "--stress") {
                let originals = inspection.tracks.clone();
                inspection.tracks = (0..10_000)
                    .map(|index| {
                        let mut track = originals[index % originals.len()].clone();
                        track.metadata.track_number = Some(index as u32 + 1);
                        track
                    })
                    .collect();
            }
            if !args.iter().any(|arg| arg == "--empty") {
                app.review = Some(Review::new(inspection, AudioFormat::Compatible));
                app.initialize_example_cover();
            }
            if let Some(index) = args.iter().position(|arg| arg == "--panel") {
                match args.get(index + 1).map(String::as_str) {
                    Some("crop") => match example::cover() {
                        Ok((asset, file)) => {
                            app.cover = Some((asset, file.path().to_owned()));
                            app._example_cover_file = Some(file);
                            app.crop_x = "80".into();
                            app.crop_y = "80".into();
                            app.crop_size = "480".into();
                            app.panel = Panel::Crop;
                        }
                        Err(error) => app.error = Some(error.to_string()),
                    },
                    Some("settings") => app.panel = Panel::Settings,
                    Some("appearance") => app.panel = Panel::Appearance,
                    Some("options") => app.panel = Panel::Options,
                    Some("preview") => {
                        let _ = app.prepare_preview();
                    }
                    Some("editor") => {
                        app.editor = app
                            .review
                            .as_ref()
                            .map(|r| Editor::new(editor::Kind::Track(0), r));
                        app.panel = Panel::Editor;
                    }
                    Some("bulk") => {
                        app.editor = app
                            .review
                            .as_ref()
                            .map(|r| Editor::new(editor::Kind::Bulk, r));
                        app.panel = Panel::Editor;
                    }
                    _ => {}
                }
            }
            (app, Task::none())
        } else {
            (
                app,
                persistence::background(|| persistence::load().map(Box::new), Message::Booted),
            )
        }
    }
}
