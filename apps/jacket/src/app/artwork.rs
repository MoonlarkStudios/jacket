//! Background preparation of artwork at the window's physical pixel density.
use super::persistence;
use super::{App, Message};
use crate::artwork::{normalized_scale, Raster};
use iced::Task;

impl App {
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
