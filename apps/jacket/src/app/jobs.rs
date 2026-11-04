use super::persistence;
use super::{App, Message, Panel};
use audio_engine::ProgressEvent;
use iced::{window, Task};
use jacket_core::{Review, SourceLink};
use jacket_desktop::downloader_updates;
use jacket_desktop::{Event, Job};
use std::time::Duration;

impl App {
    pub(super) fn tick(&mut self) -> Task<Message> {
        let mut events = Vec::new();
        if let Some(job) = &mut self.job {
            for _ in 0..64 {
                let Some(event) = job.poll() else {
                    break;
                };
                events.push(event);
            }
        }
        let mut auto_update = None;
        let mut save_inspection = false;
        for event in events {
            match event {
                Event::Track(update) => {
                    self.progress = Some(ProgressEvent {
                        stage: "inspecting".into(),
                        message: format!("Found {}", update.track.metadata.title),
                        completed: update.index + 1,
                        total: update.total.unwrap_or(update.index + 1),
                    })
                }
                Event::Progress(progress) => self.progress = Some(progress),
                Event::Inspected(inspection) => {
                    self.url = inspection.source_url.clone();
                    self.review = Some(Review::new(
                        inspection,
                        self.appearance.default_format.into(),
                    ));
                    self.example = false;
                    self.example_previous = None;
                    self.result = None;
                    self.cover = None;
                    self.source_cover = None;
                    self.source_cover_error = None;
                    self.preview_cover = None;
                    self.preview_cover_error = None;
                    self.crop_source = None;
                    self.track_scroll = 0.0;
                    self.preview = None;
                    self.pending_plan = None;
                    self.notice = None;
                    save_inspection = true;
                }
                Event::Exported(result) => {
                    self.finish_resumed_review(true);
                    self.recovery = None;
                    self.result = Some(result);
                    self.notice = None;
                }
                Event::Failed(error) => {
                    self.finish_resumed_review(false);
                    self.error = Some(error);
                    if let Some(desktop) = &self.desktop {
                        self.recovery = desktop.pending_export();
                    }
                }
                Event::AccessChecked(check) => self.notice = Some(check.message),
                Event::Downloader(status) => {
                    if status.auto_update
                        && status.update_available
                        && status.mode == downloader_updates::Mode::Managed
                    {
                        auto_update = Some(status.channel);
                    }
                    self.downloader_preferences = downloader_updates::Preferences {
                        mode: status.mode,
                        auto_update: status.auto_update,
                        local_path: status.local_path.clone(),
                    };
                    self.downloader = Some(status);
                }
                Event::Session(status) => self.notice = Some(status.message),
            }
        }
        let finished = self.job.as_ref().is_some_and(Job::is_finished);
        if finished {
            self.job = None;
            self.progress = None;
            if self.close_after_job {
                return self.close();
            }
            if let (Some(channel), Some(desktop)) = (auto_update, &self.desktop) {
                self.admit(desktop.update_downloader(channel));
            }
            if save_inspection {
                return Task::batch([self.autosave(), self.cover_preview()]);
            }
        }
        if self.close_after_job
            && self.job.is_none()
            && !self.dialog_open
            && !self.loading
            && self.save_pending == 0
            && !self.appearance_saving
        {
            return self.close();
        }
        if self.raster_refresh_pending {
            self.raster_refresh_pending = false;
            return self.display_scale_changed(self.display_scale);
        }
        if self.screenshot.is_some() && self.started.elapsed() > Duration::from_secs(2) {
            return window::oldest()
                .and_then(window::screenshot)
                .map(Message::Screenshot);
        }
        Task::none()
    }

    pub(super) fn position_crop(&mut self) {
        if let Some((asset, _)) = self.crop_asset() {
            let size = self
                .crop_size
                .parse::<u32>()
                .unwrap_or(asset.width.min(asset.height));
            let (x, y) =
                crate::crop_view::origin(self.crop_pointer, asset.width, asset.height, size);
            self.crop_x = x.to_string();
            self.crop_y = y.to_string();
        }
    }

    pub(super) fn restore_scroll(&self) -> Task<Message> {
        iced::widget::operation::scroll_to(
            "track-list",
            iced::widget::scrollable::AbsoluteOffset {
                x: 0.0,
                y: self.track_scroll,
            },
        )
    }

    pub(super) fn cover_url(&self) -> Option<&str> {
        if self.example || self.cover.is_some() {
            return None;
        }
        self.review.as_ref().and_then(crate::artwork::source_url)
    }

    pub(super) fn cover_preview(&mut self) -> Task<Message> {
        self.source_cover_error = None;
        if self.cover.is_some() {
            return self.load_owned_artwork();
        }
        if let (Some(desktop), Some(url)) = (self.desktop.clone(), self.cover_url()) {
            let url = url.to_owned();
            let source = url.clone();
            let scale = self.display_scale;
            return persistence::background(
                move || crate::artwork::preview(&desktop, &url, scale),
                move |result| Message::SourceCover(source, result),
            );
        }
        Task::none()
    }

    pub(super) fn inspect(&mut self) -> Task<Message> {
        let Some(desktop) = &self.desktop else {
            self.error = Some(
                "Jacket’s desktop services are unavailable. Retry startup or restart Jacket."
                    .into(),
            );
            return Task::none();
        };
        match SourceLink::parse(&self.url) {
            Ok(link) => {
                self.admit(desktop.inspect(link.target(self.intent).to_owned(), self.auth.clone()));
                self.panel = Panel::Workspace;
            }
            Err(error) => self.error = Some(error),
        }
        Task::none()
    }

    pub(super) fn admit(&mut self, result: Result<Job, String>) {
        match result {
            Ok(job) => {
                self.job = Some(job);
                self.error = None;
                self.progress = Some(ProgressEvent {
                    stage: "starting".into(),
                    message: "Preparing…".into(),
                    completed: 0,
                    total: 0,
                });
            }
            Err(error) => self.error = Some(error),
        }
    }

    pub(super) fn preview_artwork(&mut self) -> Task<Message> {
        self.preview_cover = None;
        self.preview_cover_error = None;
        let requested = self
            .pending_plan
            .as_ref()
            .and_then(crate::artwork::preview_url);
        let current = self.review.as_ref().and_then(crate::artwork::source_url);
        if self.source_cover.is_some() && requested.is_some() && requested == current {
            return Task::none();
        }
        if let (Some(desktop), Some(url)) = (
            self.desktop.clone(),
            self.pending_plan
                .as_ref()
                .and_then(crate::artwork::preview_url),
        ) {
            let url = url.to_owned();
            let source = url.clone();
            let scale = self.display_scale;
            return super::persistence::background(
                move || crate::artwork::preview(&desktop, &url, scale),
                move |result| Message::PreviewCover(source, result),
            );
        }
        self.load_owned_artwork()
    }
}
