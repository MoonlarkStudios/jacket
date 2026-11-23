use super::{persistence, App, Message};
use crate::artwork::Raster;
use audio_engine::{CollectionMode, Inspection, InspectionUpdate, Metadata};
use iced::Task;
use jacket_core::{Review, SourceIntent, SourceLink};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    Song,
    Album,
    Playlist,
}

impl SourceKind {
    pub fn from_link(link: &SourceLink, intent: SourceIntent) -> Self {
        let target = link.target(intent);
        if link.song_url.as_deref() == Some(target) {
            Self::Song
        } else if audio_engine::source::collection_mode(target) == CollectionMode::Album {
            Self::Album
        } else {
            Self::Playlist
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Song => "song",
            Self::Album => "album",
            Self::Playlist => "playlist",
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct IntakeCoverRequest {
    job_id: String,
    url: String,
}

pub struct Intake {
    pub kind: SourceKind,
    pub title: String,
    pub tracks: Vec<Metadata>,
    pub count: usize,
    pub total: Option<usize>,
    pub cover: Option<Raster>,
    pub cancelling: bool,
    job_id: String,
    requested_cover: Option<IntakeCoverRequest>,
    cover_failed: bool,
}

impl Intake {
    pub(super) fn new(job_id: String, kind: SourceKind) -> Self {
        Self {
            kind,
            title: String::new(),
            tracks: Vec::new(),
            count: 0,
            total: None,
            cover: None,
            cancelling: false,
            job_id,
            requested_cover: None,
            cover_failed: false,
        }
    }

    /// Keep the opening screen bounded even for a large streamed playlist.
    pub(super) fn observe(&mut self, update: InspectionUpdate) -> Option<IntakeCoverRequest> {
        if self.cancelling {
            return None;
        }
        if !update.title.is_empty() {
            self.title = update.title;
        }
        self.count = update.index + 1;
        self.total = update.total.filter(|total| *total >= self.count);
        if self.tracks.len() < 3 {
            self.tracks.push(update.track.metadata);
        }
        if self.requested_cover.is_some() {
            return None;
        }
        let url = update
            .artwork_url
            .filter(|url| audio_engine::source::valid_artwork_url(url))?;
        let request = IntakeCoverRequest {
            job_id: self.job_id.clone(),
            url,
        };
        self.requested_cover = Some(request.clone());
        Some(request)
    }
}

impl App {
    pub(super) fn finish_inspection(&mut self, inspection: Inspection) -> Task<Message> {
        let intake = self.intake.take();
        if intake.as_ref().is_some_and(|intake| intake.cancelling) {
            self.notice = Some("Lookup cancelled.".into());
            return self.focus_after_lookup_stop();
        }
        self.catalog.visible = false;
        // Keep an explicit song/playlist choice available beside the link.
        if !SourceLink::parse(&self.url).is_ok_and(|link| link.has_playlist_context()) {
            self.url = inspection.source_url.clone();
        }
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
        self.track_viewport_height = 0.0;
        self.preview = None;
        self.pending_plan = None;
        self.notice = None;
        self.source_error = None;
        self.intake_cover_handoff = None;
        // The final inspection can replace a provisional thumbnail with exact
        // album artwork. Only reuse a request for the final selected URL.
        if let Some(intake) = intake {
            if let Some(request) = intake
                .requested_cover
                .filter(|request| self.cover_url() == Some(&request.url))
            {
                self.source_cover = intake.cover;
                if self.source_cover.is_none() && !intake.cover_failed {
                    self.intake_cover_handoff = Some(request);
                }
            }
        }
        let cover = if self.source_cover.is_some() {
            self.display_scale_changed(self.display_scale)
        } else if self.intake_cover_handoff.is_some() {
            Task::none()
        } else {
            self.cover_preview()
        };
        Task::batch([self.autosave(), cover])
    }

    pub(super) fn request_intake_cover(&self, request: IntakeCoverRequest) -> Task<Message> {
        if self
            .intake
            .as_ref()
            .and_then(|intake| intake.requested_cover.as_ref())
            != Some(&request)
            && self.intake_cover_handoff.as_ref() != Some(&request)
        {
            return Task::none();
        }
        let Some(desktop) = self.desktop.clone() else {
            return Task::none();
        };
        let url = request.url.clone();
        let scale = self.display_scale;
        persistence::background(
            move || crate::artwork::preview(&desktop, &url, scale),
            move |result| Message::IntakeCover(request, result),
        )
    }

    pub(super) fn focus_after_lookup_stop(&self) -> Task<Message> {
        if self.panel == super::Panel::Workspace {
            Task::batch([
                self.restore_scroll(),
                iced::widget::operation::focus("source-url"),
            ])
        } else {
            Task::none()
        }
    }

    pub(super) fn intake_cover_ready(
        &mut self,
        request: IntakeCoverRequest,
        result: Result<Raster, String>,
    ) -> Task<Message> {
        if self.intake_cover_handoff.as_ref() == Some(&request) {
            self.intake_cover_handoff = None;
            return self.update(Message::SourceCover(request.url, result));
        }
        if let Some(intake) = self.intake.as_mut().filter(|intake| {
            !intake.cancelling && intake.requested_cover.as_ref() == Some(&request)
        }) {
            match result {
                Ok(cover) => intake.cover = Some(cover),
                Err(_) => intake.cover_failed = true,
            }
        }
        Task::none()
    }
}

#[cfg(test)]
mod tests {
    use super::{App, Intake, SourceKind};
    use crate::app::{Message, Panel};
    use audio_engine::{AudioFormat, Inspection, InspectionUpdate};
    use jacket_core::{Review, SourceIntent};

    const FIRST_COVER: &str = "https://i.ytimg.com/vi/fixture0001/hqdefault.jpg";
    const FINAL_COVER: &str = "https://lh3.googleusercontent.com/album-cover";

    fn inspection(cover: &str) -> Inspection {
        let mut inspection = crate::example::album();
        inspection.artwork_url = Some(cover.to_owned());
        inspection.tracks[0].artwork_url = Some(cover.to_owned());
        inspection
    }

    fn update(index: usize) -> InspectionUpdate {
        let sample = inspection(FIRST_COVER);
        InspectionUpdate {
            title: sample.title,
            source_url: sample.source_url,
            suggested_mode: sample.suggested_mode,
            artwork_url: sample.artwork_url,
            index,
            total: None,
            track: sample.tracks[0].clone(),
        }
    }

    #[test]
    fn mixed_links_wait_for_scope_and_bad_links_keep_the_current_review() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.review = Some(Review::new(
            crate::example::album(),
            AudioFormat::Compatible,
        ));
        let review = app.review.as_ref().unwrap().identity();
        let link = "https://www.youtube.com/watch?v=fixture0001&list=PLSample123456";
        app.intent = SourceIntent::Playlist;
        let _ = app.update(Message::Pasted(link.into()));
        assert_eq!(app.panel, Panel::Workspace);
        assert_eq!(app.intent, SourceIntent::Song);
        assert!(app.intake.is_none());
        let _ = app.update(Message::Find);
        assert_eq!(app.panel, Panel::Replace);
        let _ = app.update(Message::ClosePanel);
        let _ = app.update(Message::Pasted("unrelated text".into()));
        assert!(app.source_error.is_some());
        assert_eq!(app.panel, Panel::Workspace);
        assert_eq!(app.review.as_ref().unwrap().identity(), review);
    }

    #[test]
    fn early_tracks_are_bounded_and_a_cancelled_result_cannot_replace_a_review() {
        let (mut app, _) = App::new();
        app.review = Some(Review::new(
            crate::example::album(),
            AudioFormat::Compatible,
        ));
        let review = app.review.as_ref().unwrap().identity();
        let mut intake = Intake::new("opening".into(), SourceKind::Playlist);
        assert!(intake.observe(update(0)).is_some());
        for index in 1..25 {
            assert!(intake.observe(update(index)).is_none());
        }
        assert_eq!(intake.tracks.len(), 3);
        assert_eq!(intake.count, 25);
        assert_eq!(intake.total, None);
        intake.cancelling = true;
        app.intake = Some(intake);
        let _ = app.finish_inspection(inspection(FINAL_COVER));
        assert_eq!(app.review.as_ref().unwrap().identity(), review);
        assert!(app.intake.is_none());
    }

    #[test]
    fn matching_artwork_keeps_its_in_flight_request_and_a_provisional_cover_cannot_replace_final_art(
    ) {
        let (mut app, _) = App::new();
        let mut intake = Intake::new("previous".into(), SourceKind::Album);
        let stale = intake.observe(update(0)).unwrap();
        let mut intake = Intake::new("matching".into(), SourceKind::Album);
        let matching = intake.observe(update(0)).unwrap();
        app.intake = Some(intake);
        let _ = app.finish_inspection(inspection(FIRST_COVER));
        let cover = crate::artwork::example_cover();
        let _ = app.update(Message::IntakeCover(stale, Ok(cover.clone())));
        assert!(app.source_cover.is_none());
        let _ = app.update(Message::IntakeCover(matching, Ok(cover.clone())));
        assert_eq!(app.source_cover, Some(cover.clone()));

        let mut intake = Intake::new("changed".into(), SourceKind::Album);
        let provisional = intake.observe(update(0)).unwrap();
        app.intake = Some(intake);
        let _ = app.finish_inspection(inspection(FINAL_COVER));
        let _ = app.update(Message::IntakeCover(provisional, Ok(cover)));
        assert!(app.source_cover.is_none());
        assert_eq!(app.cover_url(), Some(FINAL_COVER));
    }
}
