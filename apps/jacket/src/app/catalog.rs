use super::{persistence, App, Message, Panel};
use crate::artwork::Raster;
use iced::{widget::operation, Task};
use jacket_core::AlbumMatch;
use jacket_desktop::musicbrainz::{Album, Edition, Release};
use jacket_desktop::{CatalogEvent, Job};

#[derive(Clone, Debug)]
pub enum CatalogAction {
    Browse,
    PasteLink,
    ReturnToReview,
    Query(String),
    Search,
    More,
    Album(Album),
    Editions,
    Edition(String),
    Back,
    UsePlaylist,
    Playlist(String),
    CheckAudio(Option<String>),
    AcceptTrack(usize),
    OpenStore(String),
    Review,
    ConfirmReview,
}

pub struct Catalog {
    pub visible: bool,
    pub query: String,
    searched: String,
    pub results: Option<Vec<Album>>,
    pub next_offset: Option<usize>,
    pub group: Option<Album>,
    pub editions: Option<Vec<Edition>>,
    pub choosing_edition: bool,
    pub release: Option<Box<Release>>,
    pub cover: Option<Raster>,
    pub matched: Option<Box<AlbumMatch>>,
    pub playlist: String,
    pub use_playlist: bool,
    pub error: Option<String>,
    pub pending: Option<CatalogPending>,
    pub cancelling: bool,
    pub confirm_replace: bool,
}

#[derive(Clone, Copy)]
pub enum CatalogPending {
    Search { append: bool },
    Editions,
    Release,
    Match,
}

impl Catalog {
    pub(super) fn new(visible: bool) -> Self {
        Self {
            visible,
            query: String::new(),
            searched: String::new(),
            results: None,
            next_offset: None,
            group: None,
            editions: None,
            choosing_edition: false,
            release: None,
            cover: None,
            matched: None,
            playlist: String::new(),
            use_playlist: false,
            error: None,
            pending: None,
            cancelling: false,
            confirm_replace: false,
        }
    }

    pub fn can_search_more(&self) -> bool {
        self.next_offset.is_some() && self.query.trim() == self.searched
    }

    fn clear_album(&mut self) {
        self.group = None;
        self.editions = None;
        self.choosing_edition = false;
        self.release = None;
        self.cover = None;
        self.matched = None;
        self.playlist.clear();
        self.use_playlist = false;
        self.confirm_replace = false;
    }
}

impl App {
    pub(super) fn update_catalog(&mut self, action: CatalogAction) -> Task<Message> {
        if self.panel != Panel::Workspace
            || (!self.catalog.visible && !matches!(action, CatalogAction::Browse))
        {
            return Task::none();
        }
        if let CatalogAction::Query(query) = action {
            if self.catalog.pending.is_none() {
                self.catalog.query = query;
                self.catalog.error = None;
            }
            return Task::none();
        }
        if self.busy() {
            return Task::none();
        }
        let desktop = self.desktop.clone();
        match action {
            CatalogAction::Browse => {
                self.catalog.visible = true;
                return operation::focus("album-search");
            }
            CatalogAction::PasteLink => {
                self.catalog.visible = false;
                self.catalog.confirm_replace = false;
                return self.update(Message::PasteLink);
            }
            CatalogAction::ReturnToReview => {
                self.catalog.visible = false;
                self.catalog.confirm_replace = false;
            }
            CatalogAction::Query(_) => unreachable!("search entry is handled before busy actions"),
            CatalogAction::Search | CatalogAction::More => {
                let Some(desktop) = desktop else {
                    return Task::none();
                };
                let append = matches!(action, CatalogAction::More);
                if append && !self.catalog.can_search_more() {
                    return Task::none();
                }
                let query = self.catalog.query.trim().to_owned();
                if query.is_empty() {
                    return operation::focus("album-search");
                }
                let offset = if append {
                    self.catalog.next_offset.unwrap_or_default()
                } else {
                    self.catalog.clear_album();
                    self.catalog.results = None;
                    self.catalog.next_offset = None;
                    self.catalog.searched = query.clone();
                    0
                };
                self.start_catalog(
                    CatalogPending::Search { append },
                    desktop.search_albums(query, offset),
                );
            }
            CatalogAction::Album(group) => {
                let Some(desktop) = desktop else {
                    return Task::none();
                };
                self.catalog.clear_album();
                self.catalog.group = Some(group.clone());
                self.start_catalog(CatalogPending::Editions, desktop.album_editions(group));
            }
            CatalogAction::Editions => {
                self.catalog.choosing_edition = true;
                self.catalog.error = None;
                return reset_scroll();
            }
            CatalogAction::Edition(id) => {
                let Some(desktop) = desktop else {
                    return Task::none();
                };
                if !self
                    .catalog
                    .editions
                    .as_ref()
                    .is_some_and(|editions| editions.iter().any(|edition| edition.id == id))
                {
                    return Task::none();
                }
                self.start_catalog(CatalogPending::Release, desktop.album_release(id));
            }
            CatalogAction::Back => {
                self.catalog.error = None;
                if self.catalog.confirm_replace {
                    self.catalog.confirm_replace = false;
                } else if self.catalog.choosing_edition && self.catalog.release.is_some() {
                    self.catalog.choosing_edition = false;
                } else if self.catalog.group.is_some() {
                    self.catalog.clear_album();
                } else {
                    self.catalog.visible = false;
                    return self.update(Message::FocusSource);
                }
                return reset_scroll();
            }
            CatalogAction::UsePlaylist => {
                self.catalog.use_playlist = true;
                return operation::focus("album-playlist");
            }
            CatalogAction::Playlist(link) => self.catalog.playlist = link,
            CatalogAction::CheckAudio(source) => {
                let (Some(desktop), Some(release)) = (desktop, self.catalog.release.as_ref())
                else {
                    return Task::none();
                };
                self.catalog.matched = None;
                let url = source.unwrap_or_else(|| self.catalog.playlist.trim().to_owned());
                self.start_catalog(
                    CatalogPending::Match,
                    desktop.match_album(release, url, self.auth.clone()),
                );
            }
            CatalogAction::AcceptTrack(index) => {
                if let Some(matched) = &mut self.catalog.matched {
                    matched.accept_suggestion(index);
                    return operation::focus("review-matched-album");
                }
            }
            CatalogAction::OpenStore(url) => {
                if let Some(desktop) = desktop {
                    return persistence::background(
                        move || desktop.open_album_store(&url),
                        Message::Opened,
                    );
                }
            }
            CatalogAction::Review if self.review.is_some() => {
                self.catalog.confirm_replace = true;
                return operation::focus("keep-current-review");
            }
            CatalogAction::Review | CatalogAction::ConfirmReview => {
                if matches!(action, CatalogAction::ConfirmReview) && !self.catalog.confirm_replace {
                    return Task::none();
                }
                if let Some(matched) = self
                    .catalog
                    .matched
                    .as_ref()
                    .filter(|matched| !matched.inspection.tracks.is_empty())
                {
                    let mut inspection = matched.review_inspection();
                    if let Some(cover) = self
                        .catalog
                        .release
                        .as_ref()
                        .and_then(|release| release.cover.as_ref())
                    {
                        inspection.artwork_url = Some(cover.clone());
                        if let [track] = inspection.tracks.as_mut_slice() {
                            track.artwork_url = Some(cover.clone());
                        }
                    }
                    self.catalog.confirm_replace = false;
                    self.url.clone_from(&inspection.source_url);
                    return self.finish_inspection(inspection);
                }
            }
        }
        Task::none()
    }

    fn start_catalog(&mut self, pending: CatalogPending, job: Result<Job, String>) {
        self.catalog.error = None;
        match job {
            Ok(job) => {
                self.catalog.pending = Some(pending);
                self.catalog.cancelling = false;
                self.job = Some(job);
            }
            Err(error) => self.catalog.error = Some(error),
        }
    }

    pub(super) fn catalog_finished(
        &mut self,
        event: Result<CatalogEvent, String>,
    ) -> Task<Message> {
        let pending = self.catalog.pending.take();
        if std::mem::take(&mut self.catalog.cancelling) {
            return Task::none();
        }
        match event {
            Ok(CatalogEvent::Searched(result)) => {
                self.catalog.next_offset = result.next_offset;
                if matches!(pending, Some(CatalogPending::Search { append: true })) {
                    self.catalog
                        .results
                        .get_or_insert_default()
                        .extend(result.albums);
                    return Task::none();
                }
                self.catalog.results = Some(result.albums);
            }
            Ok(CatalogEvent::Editions(editions)) => {
                let first = editions.first().map(|edition| edition.id.clone());
                self.catalog.editions = Some(editions);
                if let Some(first) = first {
                    return Task::done(Message::Catalog(CatalogAction::Edition(first)));
                }
            }
            Ok(CatalogEvent::Released(release)) => {
                self.catalog.choosing_edition = false;
                self.catalog.matched = None;
                self.catalog.cover = None;
                self.catalog.playlist.clear();
                self.catalog.use_playlist = false;
                let cover = release.cover.clone();
                self.catalog.release = Some(release);
                if let (Some(url), Some(desktop)) = (cover, self.desktop.clone()) {
                    let scale = self.display_scale;
                    return Task::batch([
                        reset_scroll(),
                        persistence::background(
                            {
                                let url = url.clone();
                                move || crate::artwork::preview(&desktop, &url, scale)
                            },
                            move |result| Message::CatalogCover(url, result),
                        ),
                    ]);
                }
            }
            Ok(CatalogEvent::Matched(matched)) => {
                self.catalog.matched = Some(matched);
                return Task::none();
            }
            Err(error) => {
                self.catalog.error = Some(error);
                return Task::none();
            }
        }
        reset_scroll()
    }
}

fn reset_scroll() -> Task<Message> {
    operation::scroll_to(
        "album-browser",
        iced::widget::scrollable::AbsoluteOffset::<f32>::default(),
    )
}

impl App {
    pub(super) fn catalog_snapshot(&mut self, scenario: &str) {
        use jacket_core::{AlbumRelease, ReleaseTrack};
        use serde_json::{json, Value};

        let group = |id: &str, title: &str, artist: &str, date: &str| {
            serde_json::from_value::<Album>(json!({
                "id": id, "title": title, "artist-credit": [{"name": artist}],
                "first-release-date": date, "primary-type": "Album"
            }))
            .expect("fictional album group")
        };
        self.review = None;
        self.catalog = Catalog::new(true);
        if scenario == "catalog" {
            return;
        }
        self.catalog.query = "Paper Trails".into();
        self.catalog.searched = self.catalog.query.clone();
        let album = group(
            "50de827a-6631-4099-bd85-23dd6cb69d66",
            "Room to breathe",
            "The Paper Trails",
            "2024-04-12",
        );
        self.catalog.results = Some(vec![
            album.clone(),
            group(
                "3edfa2fd-fef9-4134-b936-83bc689adcac",
                "Evenings at Holloway",
                "The Paper Trails",
                "2021-10-22",
            ),
            group(
                "6c9ecc1f-ce5a-40ba-ac4c-afef3ab7a2d9",
                "Read the Signs",
                "The Paper Trails",
                "2019-07-09",
            ),
        ]);
        if scenario == "catalog-results" {
            return;
        }
        self.catalog.group = Some(album);
        let edition = |id: &str, title: &str, date: &str, media: Vec<Value>| {
            serde_json::from_value::<Edition>(json!({
                "id":id, "title":title, "date":date, "country":"XW", "media": media,
            }))
            .expect("fictional album edition")
        };
        let medium = |count: u32| json!({"format":"Digital Media","track-count":count});
        let standard = edition(
            "cf7a9185-90a9-4824-8f31-3b01b640e1df",
            "Room to breathe",
            "2024-04-12",
            vec![medium(5)],
        );
        let expanded = edition(
            "15d5c595-9285-4f4b-b73a-60d2334e6004",
            "Room to breathe (Expanded edition)",
            "2026-04-10",
            vec![medium(5), medium(2)],
        );
        self.catalog.editions = Some(vec![standard, expanded.clone()]);
        let source = "https://music.youtube.com/playlist?list=OLAK5uy_fixture123";
        let mut inspection = crate::example::album();
        inspection.source_url = source.into();
        let mut tracks = Vec::new();
        for (index, track) in inspection.tracks.iter().enumerate() {
            tracks.push(ReleaseTrack {
                title: track.metadata.title.clone(),
                artist: track.metadata.artist.clone(),
                disc: 1,
                number: index as u32 + 1,
                duration_ms: track.duration.map(|seconds| (seconds * 1000.0) as u64),
                source_urls: vec![track.source_url.clone()],
            });
        }
        tracks.push(ReleaseTrack {
            title: "Holloway rehearsal".into(),
            artist: "The Paper Trails".into(),
            disc: 2,
            number: 1,
            duration_ms: Some(202_000),
            source_urls: Vec::new(),
        });
        tracks.push(ReleaseTrack {
            title: "Blue Window (acoustic)".into(),
            artist: "The Paper Trails feat. Avery Hale".into(),
            disc: 2,
            number: 2,
            duration_ms: Some(186_000),
            source_urls: Vec::new(),
        });
        let mut album = AlbumRelease {
            title: expanded.title.clone(),
            artist: "The Paper Trails".into(),
            year: Some(2026),
            tracks,
        };
        self.catalog.cover = Some(crate::artwork::example_cover());
        self.catalog.choosing_edition = scenario == "catalog-editions";
        if scenario == "catalog-variant" {
            let track = &mut inspection.tracks[2];
            track.source_metadata.title.push_str(" (Japan CD)");
            album.tracks[2].source_urls.clear();
        }
        if matches!(scenario, "catalog-match" | "catalog-variant") {
            self.catalog.matched = album.match_source(inspection).ok().map(Box::new);
        }
        self.catalog.release = Some(Box::new(Release {
            edition: expanded,
            album,
            cover: None,
            sources: vec![source.into()],
            stores: Vec::new(),
        }));
    }
}
