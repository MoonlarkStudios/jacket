//! Bounded background artwork fetches for the active catalog page.
use super::{persistence, App, Message, Panel};
use iced::{widget::image::Handle, Task};
use jacket_desktop::catalog::SearchKind;
use std::collections::{BTreeMap, HashSet, VecDeque};

const CACHE_SIZE: usize = 96;
const CONCURRENT_REQUESTS: usize = 3;

#[derive(Default)]
pub struct CatalogMedia {
    loading: HashSet<String>,
    visible_releases: BTreeMap<String, String>,
    // Cache failed lookups as well so a missing cover doesn't trigger a fetch every tick.
    covers: VecDeque<(String, Option<Handle>)>,
}

impl CatalogMedia {
    pub fn get(&self, url: &str) -> Option<&Handle> {
        self.covers.iter().find(|(key, _)| key == url)?.1.as_ref()
    }

    pub(super) fn complete(&mut self, url: String, image: Option<Handle>) {
        self.loading.remove(&url);
        if self.covers.len() == CACHE_SIZE {
            self.covers.pop_front();
        }
        self.covers.push_back((url, image));
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

    fn needs_artwork(&mut self, url: &str) -> bool {
        if let Some(index) = self.covers.iter().position(|(key, _)| key == url) {
            let entry = self.covers.remove(index).expect("cached image index");
            self.covers.push_back(entry);
            return false;
        }
        !self.loading.contains(url)
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
                .collect()
        } else {
            // A full discography can exceed the artwork cache. Fetch its cards
            // near the viewport so distant rows do not evict visible artwork.
            let expanded = catalog.show_all_artist_releases();
            catalog
                .featured()
                .and_then(|hit| hit.item.artwork_url())
                .into_iter()
                .chain(
                    catalog
                        .artist()
                        .and_then(|page| page.artist.artwork_url.as_deref()),
                )
                .chain(
                    catalog
                        .hits()
                        .iter()
                        .filter(|hit| !expanded || hit.item.kind() != SearchKind::Albums)
                        .filter_map(|hit| hit.item.artwork_url()),
                )
                .chain(catalog.media.visible_releases.values().map(String::as_str))
                .map(str::to_owned)
                .collect()
        };
        let available = CONCURRENT_REQUESTS.saturating_sub(catalog.media.loading.len());
        let mut tasks = Vec::with_capacity(available);
        for url in urls {
            if tasks.len() == available {
                break;
            }
            if catalog.media.needs_artwork(&url) {
                catalog.media.loading.insert(url.clone());
                let desktop = desktop.clone();
                let source = url.clone();
                tasks.push(persistence::background(
                    move || crate::artwork::thumbnail(&desktop, &source),
                    move |image| Message::CatalogThumbnail(url, image.ok()),
                ));
            }
        }
        Task::batch(tasks)
    }
}
