//! Background preparation of artwork at the window's physical pixel density.
use super::persistence;
use super::{App, Message, Panel};
use crate::artwork::{normalized_scale, Raster};
use iced::Task;
use jacket_desktop::artwork_assets::Asset;
use std::path::PathBuf;

impl App {
    pub(super) fn edit_cover_crop(&mut self, asset: Asset, path: PathBuf) -> Task<Message> {
        let size = asset.width.min(asset.height);
        self.crop_x = ((asset.width - size) / 2).to_string();
        self.crop_y = ((asset.height - size) / 2).to_string();
        self.crop_size = size.to_string();
        self.crop_preview_size = size;
        self.crop_source = Some((asset, path));
        self.crop_dragging = false;
        self.panel = Panel::Crop;
        self.error = None;
        self.load_owned_artwork()
    }

    pub(super) fn resize_cover_crop(&mut self, value: String) {
        self.crop_size = value;
        self.error = None;
        let Some((asset, _)) = self.crop_asset() else {
            return;
        };
        let Ok(size) = self.crop_size.parse::<u32>() else {
            return;
        };
        if size == 0 || size > asset.width.min(asset.height) {
            return;
        }
        let shift = i64::from(self.crop_preview_size / 2) - i64::from(size / 2);
        let position = |coordinate: &str, edge: u32| {
            let value = coordinate.parse::<u32>().unwrap_or(0);
            (i64::from(value) + shift)
                .clamp(0, i64::from(edge - size))
                .to_string()
        };
        let x = position(&self.crop_x, asset.width);
        let y = position(&self.crop_y, asset.height);
        self.crop_x = x;
        self.crop_y = y;
        self.crop_preview_size = size;
    }

    pub(super) fn use_source_cover(&mut self) -> Task<Message> {
        let Some(review) = self.review.as_mut() else {
            return Task::none();
        };
        if review.preferences.shared_artwork_asset.is_none() {
            return Task::none();
        }
        if let Err(error) = review.changed() {
            self.error = Some(error);
            return Task::none();
        }
        review.preferences.shared_artwork_asset = None;
        self.cover = None;
        self.preview_cover = None;
        self.preview_cover_error = None;
        self.error = None;
        Task::batch([
            self.autosave()
                .chain(iced::widget::operation::focus("review-choose-cover")),
            self.cover_preview(),
        ])
    }

    pub fn owned_raster(&self, id: &str) -> Option<&Raster> {
        self.owned_rasters.get(id)
    }

    fn relevant_raster(&self, id: &str) -> bool {
        self.cover.as_ref().is_some_and(|(asset, _)| asset.id == id)
            || self
                .crop_source
                .as_ref()
                .is_some_and(|(asset, _)| asset.id == id)
            || self.owned_rasters.contains_key(id)
    }

    pub(super) fn load_owned_artwork(&mut self) -> Task<Message> {
        let mut sources = Vec::new();
        for source in [self.cover.as_ref(), self.crop_source.as_ref()]
            .into_iter()
            .flatten()
        {
            if !sources.iter().any(|(id, _)| *id == source.0.id) {
                sources.push((source.0.id.clone(), source.1.clone()));
            }
        }
        let mut tasks = Vec::new();
        let scale = self.display_scale;
        for (id, path) in sources {
            if self
                .raster_loading
                .iter()
                .any(|(loading, _)| loading == &id)
            {
                continue;
            }
            if self
                .owned_rasters
                .get(&id)
                .is_some_and(|raster| raster.scale() == scale)
            {
                continue;
            }
            self.raster_loading.insert((id.clone(), scale.to_bits()));
            let cached = self.owned_rasters.get(&id).cloned();
            tasks.push(persistence::background(
                move || {
                    cached.map_or_else(
                        || Raster::load(&path, scale),
                        |raster| Ok(raster.for_scale(scale)),
                    )
                },
                move |result| Message::OwnedCoverRaster(id, scale, result),
            ));
        }
        Task::batch(tasks)
    }

    pub(super) fn owned_artwork_ready(
        &mut self,
        id: String,
        scale: f32,
        result: Result<Raster, String>,
    ) -> Task<Message> {
        self.raster_loading.remove(&(id.clone(), scale.to_bits()));
        if !self.relevant_raster(&id) {
            return Task::none();
        }
        match result {
            Ok(raster) => {
                if raster.scale() != self.display_scale {
                    let scale = self.display_scale;
                    if !self
                        .raster_loading
                        .iter()
                        .any(|(loading, _)| loading == &id)
                    {
                        self.raster_loading.insert((id.clone(), scale.to_bits()));
                        return persistence::background(
                            move || Ok(raster.for_scale(scale)),
                            move |result| Message::OwnedCoverRaster(id, scale, result),
                        );
                    }
                    return Task::none();
                }
                if self.owned_rasters.get(&id).is_some_and(|current| {
                    current.source() == raster.source() && current.scale() == raster.scale()
                }) {
                    return Task::none();
                }
                // Retain a small cache so returning from a sample/recovered job
                // can reuse pixels. Never evict the chosen cover or crop source.
                if !self.owned_rasters.contains_key(&id) && self.owned_rasters.len() >= 4 {
                    let obsolete = self
                        .owned_rasters
                        .keys()
                        .find(|key| {
                            self.cover
                                .as_ref()
                                .is_none_or(|(asset, _)| &asset.id != *key)
                                && self
                                    .crop_source
                                    .as_ref()
                                    .is_none_or(|(asset, _)| &asset.id != *key)
                        })
                        .cloned();
                    if let Some(obsolete) = obsolete {
                        self.owned_rasters.remove(&obsolete);
                    }
                }
                if self.cover.as_ref().is_some_and(|(asset, _)| asset.id == id) {
                    self.source_cover_error = None;
                }
                self.owned_rasters.insert(id, raster);
            }
            Err(error) => {
                if self
                    .crop_source
                    .as_ref()
                    .is_some_and(|(asset, _)| asset.id == id)
                {
                    self.error = Some(format!("The crop preview could not be prepared: {error}"));
                } else if self.cover.as_ref().is_some_and(|(asset, _)| asset.id == id) {
                    self.source_cover_error = Some(error);
                }
            }
        }
        Task::none()
    }

    pub(super) fn display_scale_changed(&mut self, scale: f32) -> Task<Message> {
        let scale = normalized_scale(scale);
        if self.display_scale == scale
            && self
                .source_cover
                .as_ref()
                .is_none_or(|raster| raster.scale() == scale)
            && self
                .preview_cover
                .as_ref()
                .is_none_or(|raster| raster.scale() == scale)
        {
            return self.load_owned_artwork();
        }
        self.display_scale = scale;
        let mut tasks = Vec::new();
        if let (Some(raster), Some(url)) = (
            self.source_cover.clone(),
            self.cover_url().map(str::to_owned),
        ) {
            tasks.push(persistence::background(
                move || Ok(raster.for_scale(scale)),
                move |result| Message::SourceCover(url, result),
            ));
        }
        if let (Some(raster), Some(url)) = (
            self.preview_cover.clone(),
            self.pending_plan
                .as_ref()
                .and_then(crate::artwork::preview_url)
                .map(str::to_owned),
        ) {
            tasks.push(persistence::background(
                move || Ok(raster.for_scale(scale)),
                move |result| Message::PreviewCover(url, result),
            ));
        }
        for (id, raster) in self.owned_rasters.clone() {
            if !self
                .raster_loading
                .iter()
                .any(|(loading, _)| loading == &id)
            {
                self.raster_loading.insert((id.clone(), scale.to_bits()));
                tasks.push(persistence::background(
                    move || Ok(raster.for_scale(scale)),
                    move |result| Message::OwnedCoverRaster(id, scale, result),
                ));
            }
        }
        tasks.push(self.load_owned_artwork());
        Task::batch(tasks)
    }
}
