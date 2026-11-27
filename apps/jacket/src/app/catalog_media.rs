//! Bounded background artwork fetches for the active catalog page.
use super::{persistence, App, Message, Panel};
use crate::artwork::ThumbnailShape;
use iced::{widget::image::Handle, Task};
use jacket_desktop::catalog::SearchKind;
use jacket_desktop::youtube_music::SearchItem;
use std::collections::{BTreeMap, HashSet, VecDeque};

const CACHE_SIZE: usize = 96;
const CONCURRENT_REQUESTS: usize = 3;

type ThumbnailKey = (String, ThumbnailShape);

pub enum Thumbnail<'a> {
    Loading,
    Missing,
    Ready(&'a Handle),
}

#[derive(Default)]
pub struct CatalogMedia {
    loading: HashSet<ThumbnailKey>,
    visible_releases: BTreeMap<String, String>,
    // Cache failed lookups as well so a missing cover doesn't trigger a fetch every tick.
    covers: VecDeque<(ThumbnailKey, Option<Handle>)>,
}

impl CatalogMedia {
    pub fn cover(&self, url: Option<&str>) -> Thumbnail<'_> {
        self.get_shaped(url, ThumbnailShape::Square)
    }

    pub fn portrait(&self, url: Option<&str>) -> Thumbnail<'_> {
        self.get_shaped(url, ThumbnailShape::Circle)
    }

    fn get_shaped(&self, url: Option<&str>, shape: ThumbnailShape) -> Thumbnail<'_> {
        let Some(url) = url else {
            return Thumbnail::Missing;
        };
        match self
            .covers
            .iter()
            .find(|((candidate, variant), _)| candidate == url && *variant == shape)
        {
            Some((_, Some(image))) => Thumbnail::Ready(image),
            Some((_, None)) => Thumbnail::Missing,
            None => Thumbnail::Loading,
        }
    }

    pub(super) fn complete(&mut self, url: String, shape: ThumbnailShape, image: Option<Handle>) {
        let key = (url, shape);
        self.loading.remove(&key);
        if self.covers.len() == CACHE_SIZE {
            self.covers.pop_front();
        }
        self.covers.push_back((key, image));
    }

    pub(super) fn set_release_artwork(&mut self, id: String, url: Option<String>) {
        if let Some(url) = url {
            self.visible_releases.insert(id, url);
        } else {
            self.visible_releases.remove(&id);
        }
    }

    pub(super) fn clear_release_artwork(&mut self) {
        self.visible_releases.clear();
    }

    fn needs_artwork(&mut self, request: &ThumbnailKey) -> bool {
        if let Some(index) = self.covers.iter().position(|(key, _)| key == request) {
            let entry = self.covers.remove(index).expect("cached image index");
            self.covers.push_back(entry);
            return false;
        }
        !self.loading.contains(request)
    }
}

impl App {
    pub(super) fn tick_catalog_media(&mut self) -> Task<Message> {
        let catalog = &mut self.catalog;
        if self.panel != Panel::Workspace
            || !catalog.visible
            || (catalog.search_home() && catalog.query.trim() != catalog.searched)
        {
            return Task::none();
        }
        let Some(desktop) = self.desktop.clone() else {
            return Task::none();
        };
        let urls: Vec<_> = if let Some(album) = catalog.album() {
            album
                .inspection
                .artwork_url
                .iter()
                .cloned()
                .chain(
                    catalog
                        .edition_shelf()
                        .filter(|shelf| shelf.open)
                        .and_then(|shelf| shelf.rows.as_ref())
                        .into_iter()
                        .flatten()
                        .filter_map(|edition| edition.artwork_url.clone()),
                )
                .map(|url| (url, ThumbnailShape::Square))
                .collect()
        } else {
            // A full discography can exceed the artwork cache. Fetch its cards
            // near the viewport so distant rows do not evict visible artwork.
            let expanded = catalog.show_all_artist_releases();
            catalog
                .featured()
                .and_then(|hit| thumbnail_request(&hit.item))
                .into_iter()
                .chain(
                    catalog
                        .artist()
                        .and_then(|page| page.artist.artwork_url.as_deref())
                        .map(|url| (url, ThumbnailShape::Circle)),
                )
                .chain(
                    catalog
                        .hits()
                        .iter()
                        .filter(|hit| !expanded || hit.item.kind() != SearchKind::Albums)
                        .filter_map(|hit| thumbnail_request(&hit.item)),
                )
                .chain(
                    catalog
                        .media
                        .visible_releases
                        .values()
                        .map(|url| (url.as_str(), ThumbnailShape::Square)),
                )
                .map(|(url, shape)| (url.to_owned(), shape))
                .collect()
        };
        let available = CONCURRENT_REQUESTS.saturating_sub(catalog.media.loading.len());
        let mut tasks = Vec::with_capacity(available);
        for request in urls {
            if tasks.len() == available {
                break;
            }
            if catalog.media.needs_artwork(&request) {
                catalog.media.loading.insert(request.clone());
                let (url, shape) = request;
                let desktop = desktop.clone();
                let source = url.clone();
                tasks.push(persistence::background(
                    move || crate::artwork::thumbnail(&desktop, &source, shape),
                    move |image| Message::CatalogThumbnail {
                        url,
                        shape,
                        image: image.ok(),
                    },
                ));
            }
        }
        Task::batch(tasks)
    }
}

fn thumbnail_request(item: &SearchItem) -> Option<(&str, ThumbnailShape)> {
    let shape = match item {
        SearchItem::Artist(_) => ThumbnailShape::Circle,
        _ => ThumbnailShape::Square,
    };
    item.artwork_url().map(|url| (url, shape))
}

#[cfg(test)]
mod tests {
    use super::{CatalogMedia, Thumbnail};
    use crate::artwork::ThumbnailShape;
    use iced::widget::image::Handle;

    #[test]
    fn thumbnail_states_distinguish_pending_urls_from_missing_artwork() {
        let mut media = CatalogMedia::default();
        let url = "https://i.ytimg.com/example.jpg";
        assert!(matches!(media.cover(None), Thumbnail::Missing));
        assert!(matches!(media.cover(Some(url)), Thumbnail::Loading));

        media.complete(url.into(), ThumbnailShape::Square, None);
        assert!(matches!(media.cover(Some(url)), Thumbnail::Missing));
        assert!(matches!(media.portrait(Some(url)), Thumbnail::Loading));

        let image = Handle::from_rgba(1, 1, vec![10, 20, 30, 255]);
        media.complete(url.into(), ThumbnailShape::Circle, Some(image.clone()));
        assert!(matches!(media.portrait(Some(url)), Thumbnail::Ready(handle) if handle == &image));
    }
}
