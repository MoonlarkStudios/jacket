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
                self.owned_rasters.insert(
                    asset.id.clone(),
                    crate::artwork::example_cover().for_scale(self.display_scale),
                );
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
        let catalog = super::catalog::Catalog::new(!isolated);
        let mut app = Self {
            desktop: None,
            review: None,
            catalog,
            job: None,
            url: String::new(),
            intent: SourceIntent::Song,
            intake: None,
            source_error: None,
            intake_cover_handoff: None,
            panel: Panel::Workspace,
            editor: None,
            preview: None,
            album_preview: None,
            album_scan: 0,
            album_scan_key: None,
            progress: None,
            result: None,
            export_recording: None,
            recovery: None,
            error: theme_error,
            notice: None,
            theme: appearance
                .theme
                .resolve(1)
                .expect("startup themes have already been validated"),
            appearance,
            music_folder: None,
            music_folder_editor: None,
            display_scale: 1.0,
            owned_rasters: std::collections::BTreeMap::new(),
            raster_loading: std::collections::BTreeSet::new(),
            raster_refresh_pending: false,
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
            crop_return_to_cover: false,
            track_scroll: 0.0,
            track_viewport_height: 0.0,
            collection_editor_anchor: None,
            arranged_track: None,
            track_position: None,
            crop_x: "0".into(),
            crop_y: "0".into(),
            crop_size: "1000".into(),
            crop_preview_size: 1000,
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
            if args.iter().any(|arg| arg == "--organized") {
                app.music_folder = Some(super::destination::MusicFolder {
                    root: if cfg!(windows) {
                        "C:\\Music".into()
                    } else {
                        "/Music".into()
                    },
                    rule: jacket_core::PathRule::default(),
                });
            }
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
                            app.crop_preview_size = 480;
                            app.panel = Panel::Crop;
                        }
                        Err(error) => app.error = Some(error.to_string()),
                    },
                    Some(
                        scenario @ ("catalog"
                        | "catalog-results"
                        | "catalog-results-loading"
                        | "catalog-artist"
                        | "catalog-album"
                        | "catalog-editions"
                        | "catalog-unavailable"),
                    ) => {
                        app.catalog_snapshot(scenario);
                    }
                    Some("settings") => app.panel = Panel::Settings,
                    Some("appearance") => app.panel = Panel::Appearance,
                    Some("options") => app.panel = Panel::Options,
                    Some("cover") => app.panel = Panel::Cover,
                    Some("music-folder") => {
                        let _ = app.open_music_folder(false);
                    }
                    Some("preview") => {
                        let _ = app.prepare_preview();
                    }
                    Some("opening" | "opening-tracks") => {
                        use super::intake::{Intake, SourceKind};
                        if let Some(review) = app.review.take() {
                            let (kind, url) = if review.is_song() {
                                (
                                    SourceKind::Song,
                                    "https://music.youtube.com/watch?v=fixture0001",
                                )
                            } else {
                                (
                                    SourceKind::Playlist,
                                    "https://www.youtube.com/playlist?list=PLSample123456",
                                )
                            };
                            app.url = url.to_owned();
                            let mut intake = Intake::new("example-opening".to_owned(), kind);
                            if args[index + 1] == "opening-tracks" {
                                let inspection = review.inspection;
                                let total = inspection.tracks.len();
                                for (index, track) in
                                    inspection.tracks.into_iter().take(3).enumerate()
                                {
                                    intake.observe(audio_engine::InspectionUpdate {
                                        title: inspection.title.clone(),
                                        source_url: url.to_owned(),
                                        suggested_mode: inspection.suggested_mode,
                                        artwork_url: None,
                                        index,
                                        total: Some(total),
                                        track,
                                    });
                                }
                                intake.cover = Some(crate::artwork::example_cover());
                            }
                            app.intake = Some(intake);
                        }
                    }
                    Some("saved") => {
                        if let Some(plan) = app
                            .review
                            .as_ref()
                            .and_then(|review| review.plan(app.auth.clone()).ok())
                        {
                            use audio_engine::{ExportResult, OutputKind};
                            let root = if cfg!(windows) { "C:\\Music" } else { "/Music" };
                            let path = PathBuf::from(root)
                                .join("The Paper Trails")
                                .join("Room to breathe");
                            let path = if plan.output_kind == OutputKind::File {
                                path.join("First light (2).m4a")
                            } else {
                                path
                            };
                            let playlist_path = (plan.output_kind == OutputKind::Folder)
                                .then(|| path.join("playlist.m3u").display().to_string());
                            app.export_recording =
                                Some(super::receipt::Recording::from_plan(&app, &plan));
                            let _ = app.finish_export(ExportResult {
                                output_path: path.display().to_string(),
                                output_kind: plan.output_kind,
                                playlist_path,
                                track_count: plan.tracks.len(),
                                album_save: None,
                                warnings: Vec::new(),
                            });
                        }
                    }
                    Some("editor") => {
                        let _ = app.update(super::Message::OpenEditor(editor::Kind::Track(0)));
                    }
                    Some("shared-tags") => {
                        let _ =
                            app.update(super::Message::Mode(crate::settings::ModeChoice::Album));
                        let _ = app.update(super::Message::OpenEditor(editor::Kind::Album));
                    }
                    Some("track-order") => {
                        let _ = app.update(super::Message::OpenPanel(Panel::TrackOrder));
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
