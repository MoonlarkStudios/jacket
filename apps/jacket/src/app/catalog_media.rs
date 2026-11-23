//! Background thumbnail fetches for the catalog's current viewport.
use super::{persistence, App, Message, Panel};
use iced::{widget::image::Handle, Task};
use jacket_desktop::musicbrainz::{Album, PortraitCredit, SearchHit};
use std::collections::{HashMap, HashSet, VecDeque};

const CACHE_SIZE: usize = 96;
const CONCURRENT_REQUESTS: usize = 3;
const PREFETCH_ROWS: usize = 20;
const PORTRAIT_BATCH: usize = 3;

#[derive(Clone, Debug)]
pub struct Portrait {
    pub image: Handle,
    pub credit: PortraitCredit,
}

#[derive(Default)]
pub struct CatalogMedia {
    pub scroll_y: f32,
    loading: HashSet<String>,
    portraits_loading: bool,
    // Remember missing images alongside successful ones so scrolling, typing and
    // the next animation tick cannot keep asking the archive for the same cover.
    covers: ImageCache<Handle>,
    portraits: ImageCache<Portrait>,
}

impl CatalogMedia {
    pub fn get(&self, url: &str) -> Option<&Handle> {
        self.covers.get(url)
    }

    pub fn portrait(&self, id: &str) -> Option<&Portrait> {
        self.portraits.get(id)
    }

    pub(super) fn complete(&mut self, url: String, image: Option<Handle>) {
        self.loading.remove(&url);
        self.covers.insert(url, image);
    }

    pub(super) fn complete_portraits(
        &mut self,
        ids: Vec<String>,
        mut images: HashMap<String, Portrait>,
    ) {
        self.portraits_loading = false;
        for id in ids {
            let portrait = images.remove(&id);
            self.portraits.insert(id, portrait);
        }
    }
}

struct ImageCache<T>(VecDeque<(String, Option<T>)>);

impl<T> Default for ImageCache<T> {
    fn default() -> Self {
        Self(VecDeque::new())
    }
}

impl<T> ImageCache<T> {
    fn get(&self, key: &str) -> Option<&T> {
        self.0
            .iter()
            .find(|(url, _)| url == key)
            .and_then(|(_, image)| image.as_ref())
    }

    fn touch(&mut self, key: &str) -> bool {
        if let Some(index) = self.0.iter().position(|(url, _)| url == key) {
            let entry = self.0.remove(index).expect("cached image index");
            self.0.push_back(entry);
            true
        } else {
            false
        }
    }

    fn insert(&mut self, key: String, image: Option<T>) {
        if self.0.len() == CACHE_SIZE {
            self.0.pop_front();
        }
        self.0.push_back((key, image));
    }
}

impl App {
    pub(super) fn tick_catalog_media(&mut self) -> Task<Message> {
        if self.panel != Panel::Workspace || !self.catalog.visible || self.catalog.group.is_some() {
            return Task::none();
        }
        let Some(desktop) = self.desktop.clone() else {
            return Task::none();
        };
        let catalog = &mut self.catalog;
        let available = CONCURRENT_REQUESTS.saturating_sub(catalog.media.loading.len());
        if available == 0 && catalog.media.portraits_loading {
            return Task::none();
        }
        if catalog.search_home() && catalog.query.trim() != catalog.searched {
            return Task::none();
        }
        // The overscan keeps art ready around the viewport, including rows
        // with wrapped metadata and the header on artist and song pages.
        let first = ((catalog.media.scroll_y / 64.0) as usize).saturating_sub(5);
        let hits: Vec<_> = if catalog.youtube_music_active || catalog.song.is_some() {
            Vec::new()
        } else if let Some(kind) = catalog
            .artist
            .as_ref()
            .map(|page| page.filter)
            .or(catalog.filter)
        {
            catalog
                .sections()
                .pages
                .get(&kind)
                .iter()
                .flat_map(|page| &page.hits)
                .skip(first)
                .take(PREFETCH_ROWS)
                .collect()
        } else {
            // A large window shows at most four rows in each All section.
            // The featured result has its own slot and may be excluded below.
            let top = catalog.top_result();
            top.into_iter()
                .chain(catalog.result_order().into_iter().flat_map(|kind| {
                    catalog
                        .results
                        .pages
                        .get(&kind)
                        .into_iter()
                        .flat_map(|page| &page.hits)
                        .filter(|hit| top.is_none_or(|selected| selected.id() != hit.id()))
                        .take(4)
                }))
                .collect()
        };
        let urls: Vec<_> = if catalog.youtube_music_active {
            catalog
                .youtube_music_artist
                .as_ref()
                .and_then(|page| page.artist.artwork_url.as_deref())
                .into_iter()
                .chain(
                    catalog
                        .youtube_music_hits()
                        .iter()
                        .skip(first)
                        .take(PREFETCH_ROWS)
                        .filter_map(|hit| hit.item.artwork_url()),
                )
                .map(str::to_owned)
                .collect()
        } else if let Some(song) = &catalog.song {
            song.details
                .iter()
                .flat_map(|details| &details.albums)
                .skip(first)
                .take(PREFETCH_ROWS)
                .filter_map(Album::cover_thumbnail)
                .collect()
        } else {
            hits.iter()
                .filter_map(|hit| hit.cover_thumbnail())
                .collect()
        };
        let artist_ids: Vec<_> = catalog
            .artist
            .as_ref()
            .map(|page| page.artist.id.as_str())
            .into_iter()
            .chain(hits.into_iter().filter_map(|hit| match hit {
                SearchHit::Artist(artist) => Some(artist.id.as_str()),
                _ => None,
            }))
            .map(str::to_owned)
            .collect();
        let mut tasks: Vec<_> = urls
            .into_iter()
            .filter(|url| {
                !catalog.media.covers.touch(url) && catalog.media.loading.insert(url.clone())
            })
            .take(available)
            .map(|url| {
                let desktop = desktop.clone();
                let source = url.clone();
                persistence::background(
                    move || crate::artwork::thumbnail(&desktop, &source),
                    move |image| Message::CatalogThumbnail(url, image.ok()),
                )
            })
            .collect();
        if !catalog.media.portraits_loading {
            let ids: Vec<_> = artist_ids
                .into_iter()
                .filter(|id| !catalog.media.portraits.touch(id))
                .take(PORTRAIT_BATCH)
                .collect();
            if !ids.is_empty() {
                catalog.media.portraits_loading = true;
                let requested = ids.clone();
                tasks.push(persistence::background(
                    move || {
                        Ok(desktop
                            .artist_portraits(&requested)?
                            .into_iter()
                            .filter_map(|(id, portrait)| {
                                let image =
                                    crate::artwork::decode_thumbnail(&portrait.bytes).ok()?;
                                Some((
                                    id,
                                    Portrait {
                                        image,
                                        credit: portrait.credit,
                                    },
                                ))
                            })
                            .collect())
                    },
                    move |portraits| Message::CatalogPortraits(ids, portraits.unwrap_or_default()),
                ));
            }
        }
        Task::batch(tasks)
    }
}
