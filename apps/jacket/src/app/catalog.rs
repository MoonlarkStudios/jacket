use super::{persistence, App, Message, Panel};
use crate::artwork::Raster;
use audio_engine::Inspection;
use iced::{widget::operation, Task};
use jacket_core::AlbumMatch;
use jacket_desktop::musicbrainz::{
    Album, Artist, Edition, Release, SearchHit, SearchKind, SearchPage, SearchScope, Song,
    SongDetails,
};
use jacket_desktop::{CatalogEvent, Job};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub enum CatalogAction {
    Browse,
    PasteLink,
    ReturnToReview,
    Query(String),
    Search,
    Filter(Option<SearchKind>),
    LoadVisible,
    Retry(SearchKind),
    More(SearchKind),
    Artist(Artist),
    Song(Song),
    SongLink,
    SongUrl(String),
    CheckSong(Option<String>),
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
    pub searched: String,
    pub filter: Option<SearchKind>,
    pub results: SearchSections,
    queued_action: Option<CatalogAction>,
    pub artist: Option<ArtistPage>,
    pub song: Option<SongPage>,
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
    Search,
    Song,
    SongMatch,
    Editions,
    Release,
    Match,
}

#[derive(Default)]
pub struct SearchSections {
    pub pages: BTreeMap<SearchKind, SearchPage>,
    pub errors: BTreeMap<SearchKind, String>,
}

pub struct ArtistPage {
    pub artist: Artist,
    pub filter: SearchKind,
    pub results: SearchSections,
}

pub struct SongPage {
    pub selected: Song,
    pub details: Option<Box<SongDetails>>,
    pub matched: Option<Box<Inspection>>,
    pub url: String,
    pub show_link: bool,
}

impl Catalog {
    pub(super) fn new(visible: bool) -> Self {
        Self {
            visible,
            query: String::new(),
            searched: String::new(),
            filter: None,
            results: SearchSections::default(),
            queued_action: None,
            artist: None,
            song: None,
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

    pub fn search_home(&self) -> bool {
        self.artist.is_none() && self.song.is_none() && self.group.is_none()
    }

    pub fn sections(&self) -> &SearchSections {
        self.artist
            .as_ref()
            .map_or(&self.results, |artist| &artist.results)
    }

    fn sections_mut(&mut self) -> &mut SearchSections {
        self.artist
            .as_mut()
            .map_or(&mut self.results, |artist| &mut artist.results)
    }

    fn scope(&self) -> Option<SearchScope> {
        if let Some(artist) = &self.artist {
            Some(SearchScope::Artist(artist.artist.id.clone()))
        } else {
            (!self.searched.is_empty()).then(|| SearchScope::Text(self.searched.clone()))
        }
    }

    fn visible_kinds(&self) -> Vec<SearchKind> {
        if let Some(artist) = &self.artist {
            vec![artist.filter]
        } else {
            self.filter
                .map_or_else(|| SearchKind::ALL.to_vec(), |kind| vec![kind])
        }
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
            self.catalog.query = query;
            self.catalog.error = None;
            return Task::none();
        }
        if let CatalogAction::Filter(filter) = action {
            if let Some(artist) = &mut self.catalog.artist {
                if let Some(kind @ (SearchKind::Albums | SearchKind::Songs)) = filter {
                    artist.filter = kind;
                }
            } else {
                self.catalog.filter = filter;
            }
            if self.busy() {
                return reset_scroll();
            }
            return Task::batch([
                reset_scroll(),
                Task::done(Message::Catalog(CatalogAction::LoadVisible)),
            ]);
        }
        if self.busy() {
            if matches!(self.catalog.pending, Some(CatalogPending::Search)) {
                let replaces_query = matches!(action, CatalogAction::Search)
                    && self.catalog.search_home()
                    && !self.catalog.query.trim().is_empty()
                    && self.catalog.query.trim() != self.catalog.searched;
                let opens_result = matches!(
                    action,
                    CatalogAction::Artist(_) | CatalogAction::Album(_) | CatalogAction::Song(_)
                ) && !self.catalog.cancelling;
                if replaces_query || opens_result {
                    self.catalog.queued_action = Some(action);
                    return self.update(Message::Cancel);
                }
            }
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
            CatalogAction::Filter(_) => unreachable!("filters are handled before busy actions"),
            CatalogAction::Search => {
                let query = self.catalog.query.trim().to_owned();
                if query.is_empty() {
                    return operation::focus("album-search");
                }
                self.catalog.clear_album();
                self.catalog.artist = None;
                self.catalog.song = None;
                self.catalog.results = SearchSections::default();
                self.catalog.searched = query;
                return Task::batch([
                    reset_scroll(),
                    Task::done(Message::Catalog(CatalogAction::LoadVisible)),
                ]);
            }
            CatalogAction::LoadVisible => {
                if self.catalog.song.is_none() && self.catalog.group.is_none() {
                    let sections = self.catalog.sections();
                    let pages = self
                        .catalog
                        .visible_kinds()
                        .into_iter()
                        .filter(|kind| {
                            !sections.pages.contains_key(kind)
                                && !sections.errors.contains_key(kind)
                        })
                        .map(|kind| (kind, 0))
                        .collect();
                    self.search_pages(pages);
                }
            }
            CatalogAction::Retry(kind) => {
                let next = self
                    .catalog
                    .sections()
                    .pages
                    .get(&kind)
                    .and_then(|page| page.next_offset);
                self.catalog.sections_mut().errors.remove(&kind);
                self.search_pages(vec![(kind, next.unwrap_or_default())]);
            }
            CatalogAction::More(kind) => {
                let next = self
                    .catalog
                    .sections()
                    .pages
                    .get(&kind)
                    .and_then(|page| page.next_offset);
                if let Some(next) = next {
                    self.search_pages(vec![(kind, next)]);
                }
            }
            CatalogAction::Artist(artist) => {
                self.catalog.artist = Some(ArtistPage {
                    artist,
                    filter: SearchKind::Albums,
                    results: SearchSections::default(),
                });
                self.catalog.song = None;
                self.catalog.clear_album();
                self.search_pages(vec![(SearchKind::Albums, 0)]);
                return reset_scroll();
            }
            CatalogAction::Song(song) => {
                let Some(desktop) = desktop else {
                    return Task::none();
                };
                self.catalog.clear_album();
                self.catalog.song = Some(SongPage {
                    selected: song.clone(),
                    details: None,
                    matched: None,
                    url: String::new(),
                    show_link: false,
                });
                self.start_catalog(CatalogPending::Song, desktop.catalog_song(song));
                return reset_scroll();
            }
            CatalogAction::SongLink => {
                if let Some(song) = &mut self.catalog.song {
                    song.show_link = true;
                    return operation::focus("song-source-link");
                }
            }
            CatalogAction::SongUrl(url) => {
                if let Some(song) = &mut self.catalog.song {
                    song.url = url;
                }
            }
            CatalogAction::CheckSong(link) => {
                let (Some(desktop), Some(song)) = (desktop, &mut self.catalog.song) else {
                    return Task::none();
                };
                if let Some(details) = &song.details {
                    song.matched = None;
                    let url = link.unwrap_or_else(|| song.url.trim().to_owned());
                    let job = desktop.match_catalog_song(*details.clone(), url, self.auth.clone());
                    self.start_catalog(CatalogPending::SongMatch, job);
                }
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
                } else if self.catalog.song.take().is_none() && self.catalog.artist.take().is_none()
                {
                    self.catalog.visible = false;
                    return self.update(Message::FocusSource);
                }
                return Task::batch([
                    reset_scroll(),
                    Task::done(Message::Catalog(CatalogAction::LoadVisible)),
                ]);
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
                let inspection = if self.catalog.group.is_none() {
                    self.catalog
                        .song
                        .as_ref()
                        .and_then(|page| page.matched.as_deref())
                        .cloned()
                } else {
                    self.catalog
                        .matched
                        .as_ref()
                        .filter(|matched| !matched.inspection.tracks.is_empty())
                        .map(|matched| {
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
                            inspection
                        })
                };
                if let Some(inspection) = inspection {
                    self.catalog.confirm_replace = false;
                    self.url.clone_from(&inspection.source_url);
                    return self.finish_inspection(inspection);
                }
            }
        }
        Task::none()
    }

    fn search_pages(&mut self, pages: Vec<(SearchKind, usize)>) {
        if pages.is_empty() {
            return;
        }
        if let (Some(desktop), Some(scope)) = (&self.desktop, self.catalog.scope()) {
            self.start_catalog(CatalogPending::Search, desktop.search_catalog(scope, pages));
        }
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
        if let Ok(CatalogEvent::SearchPage {
            kind,
            append,
            result,
        }) = event
        {
            if matches!(self.catalog.pending, Some(CatalogPending::Search))
                && !self.catalog.cancelling
            {
                let sections = self.catalog.sections_mut();
                match result {
                    Ok(page) => {
                        sections.errors.remove(&kind);
                        if let Some(existing) = sections.pages.get_mut(&kind).filter(|_| append) {
                            existing.append(page);
                        } else {
                            sections.pages.insert(kind, page);
                        }
                    }
                    Err(error) => {
                        sections.errors.insert(kind, error);
                    }
                }
            }
            return Task::none();
        }
        self.catalog.pending = None;
        if std::mem::take(&mut self.catalog.cancelling) {
            if let Some(action) = self.catalog.queued_action.take() {
                return Task::done(Message::Catalog(action));
            }
            return Task::none();
        }
        match event {
            Ok(CatalogEvent::SearchPage { .. }) => unreachable!("search pages do not finish a job"),
            Ok(CatalogEvent::SearchFinished) => {
                return Task::done(Message::Catalog(CatalogAction::LoadVisible));
            }
            Ok(CatalogEvent::SongDetails(details)) => {
                if let Some(song) = &mut self.catalog.song {
                    if song.selected.id == details.song.id {
                        song.details = Some(details);
                    }
                }
            }
            Ok(CatalogEvent::SongMatched(inspection)) => {
                if let Some(song) = &mut self.catalog.song {
                    song.matched = Some(inspection);
                    return operation::focus("review-matched-song");
                }
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
        let albums = vec![
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
        ];
        let albums = SearchPage {
            hits: albums.into_iter().map(SearchHit::Album).collect(),
            next_offset: None,
        };
        let artist: Artist = serde_json::from_value(json!({
            "id": "e2326bd9-f0bf-42c2-aa82-2c72d7b31bf7", "name": "The Paper Trails",
            "type": "Group", "area": {"name": "United Kingdom"}, "disambiguation": "Indie folk band",
        })).expect("fictional artist");
        let song: Song = serde_json::from_value(json!({
            "id": "8d1f1ae3-ee5b-4b75-955a-447a66445c18", "title": "First light",
            "artist-credit": [{"name": "The Paper Trails"}], "length": 185000,
            "first-release-date":"2024-04-12",
            "releases": [{"title": "Room to breathe", "release-group": {"id": album.id, "title": album.title}}],
        })).expect("fictional recording");
        let version: Song = serde_json::from_value(json!({
            "id": "3929a862-cf11-4ce9-a889-2af536192337", "title": "First light",
            "artist-credit": [{"name": "The Paper Trails"}], "length": 231000,
            "disambiguation":"Live at Holloway",
        }))
        .expect("fictional live recording");
        let songs = SearchPage {
            hits: vec![SearchHit::Song(song.clone()), SearchHit::Song(version)],
            next_offset: Some(24),
        };
        self.catalog.results.pages.insert(
            SearchKind::Artists,
            SearchPage {
                hits: vec![SearchHit::Artist(artist.clone())],
                next_offset: None,
            },
        );
        self.catalog
            .results
            .pages
            .insert(SearchKind::Albums, albums.clone());
        self.catalog
            .results
            .pages
            .insert(SearchKind::Songs, songs.clone());
        match scenario {
            "catalog-results" => return,
            "catalog-artist" => {
                self.catalog.artist = Some(ArtistPage {
                    artist,
                    filter: SearchKind::Albums,
                    results: SearchSections {
                        pages: BTreeMap::from([
                            (SearchKind::Albums, albums),
                            (SearchKind::Songs, songs),
                        ]),
                        errors: BTreeMap::new(),
                    },
                });
                return;
            }
            "catalog-song" | "catalog-song-matched" => {
                let mut inspection = crate::example::album();
                inspection.tracks.truncate(1);
                inspection.title = song.title.clone();
                inspection
                    .source_url
                    .clone_from(&inspection.tracks[0].source_url);
                inspection.suggested_mode = audio_engine::CollectionMode::Collection;
                let source = inspection.source_url.clone();
                self.catalog.song = Some(SongPage {
                    selected: song.clone(),
                    details: Some(Box::new(SongDetails {
                        song,
                        albums: vec![album],
                        sources: vec![source],
                    })),
                    matched: (scenario == "catalog-song-matched").then(|| Box::new(inspection)),
                    url: String::new(),
                    show_link: false,
                });
                return;
            }
            _ => (),
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

#[cfg(test)]
mod tests {
    use super::{CatalogAction, CatalogPending};
    use crate::app::{App, Message};
    use jacket_desktop::musicbrainz::{SearchHit, SearchKind, SearchPage};
    use jacket_desktop::CatalogEvent;
    use serde_json::json;

    #[test]
    fn progressive_pages_preserve_other_sections_and_discard_cancelled_results() {
        let (mut app, _) = App::new();
        app.catalog.visible = true;
        app.catalog.pending = Some(CatalogPending::Search);
        let songs = SearchPage {
            hits: vec![SearchHit::Song(
                serde_json::from_value(json!({
                    "id":"one","title":"A song",
                }))
                .unwrap(),
            )],
            next_offset: Some(24),
        };
        let _ = app.catalog_finished(Ok(CatalogEvent::SearchPage {
            kind: SearchKind::Songs,
            append: false,
            result: Ok(songs),
        }));
        let _ = app.catalog_finished(Ok(CatalogEvent::SearchPage {
            kind: SearchKind::Artists,
            append: false,
            result: Err("Rate limited".into()),
        }));
        assert!(app.catalog.pending.is_some());
        assert_eq!(app.catalog.results.pages[&SearchKind::Songs].hits.len(), 1);
        assert!(app
            .catalog
            .results
            .errors
            .contains_key(&SearchKind::Artists));

        app.catalog.cancelling = true;
        app.catalog.queued_action = Some(CatalogAction::Search);
        let _ = app.catalog_finished(Ok(CatalogEvent::SearchPage {
            kind: SearchKind::Albums,
            append: false,
            result: Ok(SearchPage {
                hits: vec![],
                next_offset: None,
            }),
        }));
        assert!(!app.catalog.results.pages.contains_key(&SearchKind::Albums));
        assert!(
            app.catalog_finished(Ok(CatalogEvent::SearchFinished))
                .units()
                > 0
        );
        assert!(app.catalog.pending.is_none());
        assert!(app.catalog.queued_action.is_none());
    }

    #[test]
    fn matched_recording_enters_single_song_review() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-song-matched");
        let _ = app.update(Message::Catalog(CatalogAction::Review));
        assert!(!app.catalog.visible);
        let review = app.review.as_ref().unwrap();
        assert!(review.is_song());
        assert_eq!(review.inspection.tracks[0].metadata.title, "First light");
        assert_eq!(
            review.preferences.output_kind,
            audio_engine::OutputKind::File
        );
    }
}
