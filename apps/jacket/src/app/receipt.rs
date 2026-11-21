use super::{App, Message, Panel};
use crate::artwork::{self, Raster};
use audio_engine::{ArtworkMode, ExportPlan, ExportResult, OutputKind};
use iced::Task;

pub struct Receipt {
    pub output: ExportResult,
    pub recording: Option<Recording>,
}

pub struct Recording {
    pub title: String,
    pub artist: Option<String>,
    pub cover: Option<Raster>,
}

impl Recording {
    pub(super) fn from_plan(app: &App, plan: &ExportPlan) -> Self {
        let (title, artist) = match (plan.output_kind, plan.tracks.as_slice()) {
            (OutputKind::File, [track]) => {
                let artist = track.metadata.artist.trim();
                (
                    track.metadata.title.clone(),
                    (!artist.is_empty()).then(|| artist.to_owned()),
                )
            }
            _ => (plan.title.clone(), None),
        };
        let cover = match plan.artwork_mode {
            ArtworkMode::None => None,
            ArtworkMode::PerTrack if plan.tracks.len() > 1 => None,
            ArtworkMode::Shared if plan.shared_artwork_asset.is_some() => plan
                .shared_artwork_asset
                .as_deref()
                .and_then(|id| app.owned_raster(id)),
            _ => artwork::preview_url(plan).and_then(|url| {
                if app.cover_url() == Some(url) {
                    app.source_cover.as_ref()
                } else if app.preview_plan().and_then(artwork::preview_url) == Some(url) {
                    app.preview_cover.as_ref()
                } else {
                    None
                }
            }),
        };
        Self {
            title,
            artist,
            cover: cover.cloned(),
        }
    }
}

impl App {
    pub(super) fn finish_export(&mut self, output: ExportResult) -> Task<Message> {
        let recording = self.export_recording.take();
        self.finish_resumed_review(true);
        self.recovery = None;
        self.notice = None;
        self.result = Some(Receipt { output, recording });
        let cover = self.refresh_receipt_cover();
        if self.panel == Panel::Workspace && !self.example {
            Task::batch([cover, iced::widget::operation::focus("receipt-reveal")])
        } else {
            cover
        }
    }

    pub(super) fn refresh_receipt_cover(&self) -> Task<Message> {
        let Some(cover) = self
            .result
            .as_ref()
            .and_then(|receipt| receipt.recording.as_ref())
            .and_then(|recording| recording.cover.clone())
        else {
            return Task::none();
        };
        let scale = self.display_scale;
        if cover.scale() == scale {
            return Task::none();
        }
        super::persistence::background(move || Ok(cover.for_scale(scale)), Message::ReceiptCover)
    }

    pub(super) fn update_receipt_cover(&mut self, raster: Raster) {
        if let Some(recording) = self
            .result
            .as_mut()
            .and_then(|receipt| receipt.recording.as_mut())
        {
            if raster.scale() == self.display_scale
                && recording
                    .cover
                    .as_ref()
                    .is_some_and(|cover| cover.source().id() == raster.source().id())
            {
                recording.cover = Some(raster);
            }
        }
    }
}
