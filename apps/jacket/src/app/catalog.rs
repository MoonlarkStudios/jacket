use super::{catalog_media::CatalogMedia, persistence, App, Message, Panel};
use audio_engine::Inspection;
use iced::{widget::operation, Task};
use jacket_desktop::catalog::{SearchKind, SearchRequest};
use jacket_desktop::youtube_music::{ArtistPage, SearchHit, SearchItem};
use jacket_desktop::{CatalogEvent, Job};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const SEARCH_DELAY: Duration = Duration::from_millis(450);

#[derive(Clone, Debug)]
pub enum CatalogAction {
    Browse,
    FocusSearch,
    FocusResult,
    ClearSearch,
    Cancel,
    PasteLink,
    ReturnToReview,
    Query(String),
    Search,
    Filter(Option<SearchKind>),
    ToggleArtistReleases,
    OpenResult(SearchKind, String),
    OpenInBrowser,
    Scrolled(f32),
    Back,
    Review,
    ConfirmReview,
}

#[derive(Clone, Copy)]
pub enum CatalogPending {
    Search,
    Artist,
    Album,
    Song,
}

pub struct Catalog {
    pub visible: bool,
    pub query: String,
    pub searched: String,
    pub filter: Option<SearchKind>,
    pub artist: Option<ArtistPage>,
    pub all_artist_releases: bool,
    pub album: Option<Box<Inspection>>,
    pub media: CatalogMedia,
    pub error: Option<String>,
    pub pending: Option<CatalogPending>,
    pub cancelling: bool,
    pub confirm_replace: bool,
    search_due: Option<Instant>,
    results: BTreeMap<Option<SearchKind>, Vec<SearchHit>>,
    song: Option<Box<Inspection>>,
    queued_action: Option<CatalogAction>,
    scroll_y: f32,
    search_scroll_y: f32,
    album_origin: Option<(String, f32)>,
}

impl Catalog {
    pub(super) fn new(visible: bool) -> Self {
        Self {
            visible,
            query: String::new(),
            searched: String::new(),
            filter: None,
            artist: None,
            all_artist_releases: false,
            album: None,
            media: CatalogMedia::default(),
            error: None,
            pending: None,
            cancelling: false,
            confirm_replace: false,
            search_due: None,
            results: BTreeMap::new(),
            song: None,
            queued_action: None,
            scroll_y: 0.0,
            search_scroll_y: 0.0,
            album_origin: None,
        }
    }

    pub fn search_home(&self) -> bool {
        self.artist.is_none() && self.album.is_none()
    }

    pub fn hits(&self) -> &[SearchHit] {
        if let Some(artist) = &self.artist {
            return &artist.hits;
        }
        self.results.get(&self.filter).map_or(&[], Vec::as_slice)
    }

    pub fn featured(&self) -> Option<&SearchHit> {
        if self.filter.is_some() || !self.search_home() {
            return None;
        }
        self.hits().iter().find(|hit| hit.featured)
    }

    pub fn result_order(&self) -> [SearchKind; 3] {
        let mut kinds = SearchKind::ALL;
        let featured = self.featured();
        kinds.sort_by_key(|kind| {
            self.hits()
                .iter()
                .position(|hit| {
                    hit.item.kind() == *kind
                        && featured.is_none_or(|first| first.item.id() != hit.item.id())
                })
                .unwrap_or(usize::MAX)
        });
        kinds
    }

    fn edit_query(&mut self, query: String, now: Instant) {
        self.query = query;
        self.error = None;
        let trimmed = self.query.trim();
        if trimmed.is_empty() {
            self.searched.clear();
            self.results.clear();
            self.queued_action = None;
        }
        self.search_due = (trimmed != self.searched
            && trimmed.chars().take(201).count() <= 200
            && trimmed
                .chars()
                .filter(|character| character.is_alphanumeric())
                .take(2)
                .count()
                == 2)
            .then_some(now + SEARCH_DELAY);
    }
}

impl App {
    pub(super) fn update_catalog(&mut self, action: CatalogAction) -> Task<Message> {
        if self.panel != Panel::Workspace
            || (!self.catalog.visible
                && !matches!(action, CatalogAction::Browse | CatalogAction::FocusSearch))
        {
            return Task::none();
        }
        match action {
            CatalogAction::Cancel => {
                self.catalog.search_due = None;
                self.catalog.queued_action = None;
                return self.update(Message::Cancel);
            }
            CatalogAction::Scrolled(offset) => {
                self.catalog.scroll_y = offset;
                return Task::none();
            }
            CatalogAction::OpenInBrowser => {
                let query = self.catalog.query.trim().to_owned();
                if let Some(desktop) = self.desktop.clone() {
                    if !query.is_empty() && query.chars().count() <= 200 {
                        return persistence::background(
                            move || desktop.open_youtube_music_search(&query),
                            Message::Opened,
                        );
                    }
                }
                return Task::none();
            }
            CatalogAction::ClearSearch => {
                return Task::batch([
                    self.update_catalog(CatalogAction::Query(String::new())),
                    operation::focus("album-search"),
                ]);
            }
            CatalogAction::Query(query) => {
                if self.catalog.search_home() && !self.catalog.confirm_replace {
                    self.catalog.edit_query(query, Instant::now());
                    if self.catalog.searched.is_empty()
                        && matches!(self.catalog.pending, Some(CatalogPending::Search))
                    {
                        return self.update(Message::Cancel);
                    }
                }
                return Task::none();
            }
            CatalogAction::FocusResult => {
                let catalog = &self.catalog;
                if self.busy()
                    || catalog.confirm_replace
                    || !catalog.search_home()
                    || catalog.query.trim() != catalog.searched
                {
                    return Task::none();
                }
                return catalog
                    .featured()
                    .or_else(|| catalog.hits().first())
                    .map_or_else(Task::none, |hit| {
                        operation::focus(result_id(hit.item.kind(), hit.item.id()))
                    });
            }
            CatalogAction::FocusSearch if self.catalog.visible && self.catalog.search_home() => {
                return operation::focus("album-search");
            }
            _ => {}
        }
        if self.busy() {
            if matches!(action, CatalogAction::Search)
                && matches!(self.catalog.pending, Some(CatalogPending::Search))
                && self.catalog.search_home()
                && !self.catalog.query.trim().is_empty()
                && self.catalog.query.trim() != self.catalog.searched
            {
                self.catalog.search_due = None;
                self.catalog.queued_action = Some(action);
                return self.update(Message::Cancel);
            }
            return Task::none();
        }
        let desktop = self.desktop.clone();
        match action {
            CatalogAction::Browse => {
                self.catalog.visible = true;
                return operation::focus(if self.catalog.album.is_some() {
                    "review-album"
                } else if self.catalog.artist.is_some() {
                    "artist-back"
                } else {
                    "album-search"
                });
            }
            CatalogAction::FocusSearch => {
                self.catalog.visible = true;
                self.catalog.album = None;
                self.catalog.artist = None;
                self.catalog.song = None;
                self.catalog.album_origin = None;
                self.catalog.confirm_replace = false;
                self.catalog.error = None;
                return Task::batch([self.catalog_scroll(0.0), operation::focus("album-search")]);
            }
            CatalogAction::PasteLink => {
                self.catalog.search_due = None;
                self.catalog.visible = false;
                self.catalog.song = None;
                self.catalog.confirm_replace = false;
                return self.update(Message::PasteLink);
            }
            CatalogAction::ReturnToReview => {
                self.catalog.search_due = None;
                self.catalog.visible = false;
                self.catalog.song = None;
                self.catalog.confirm_replace = false;
            }
            CatalogAction::Filter(filter) => {
                if self.catalog.search_home()
                    && !self.catalog.confirm_replace
                    && self.catalog.filter != filter
                {
                    self.catalog.filter = filter;
                    self.catalog.error = None;
                    let query = self.catalog.query.trim();
                    if !query.is_empty()
                        && (query != self.catalog.searched
                            || !self.catalog.results.contains_key(&filter))
                    {
                        return self.update_catalog(CatalogAction::Search);
                    }
                    return self.catalog_scroll(0.0);
                }
            }
            CatalogAction::ToggleArtistReleases => {
                if self.catalog.artist.is_some()
                    && self.catalog.album.is_none()
                    && !self.catalog.confirm_replace
                {
                    self.catalog.all_artist_releases = !self.catalog.all_artist_releases;
                }
            }
            CatalogAction::OpenResult(kind, id) => {
                let catalog = &mut self.catalog;
                if catalog.album.is_some()
                    || catalog.confirm_replace
                    || (catalog.search_home() && catalog.query.trim() != catalog.searched)
                {
                    return Task::none();
                }
                let Some(item) = catalog
                    .hits()
                    .iter()
                    .find(|hit| hit.item.kind() == kind && hit.item.id() == id)
                    .map(|hit| hit.item.clone())
                else {
                    return Task::none();
                };
                catalog.song = None;
                catalog.error = None;
                match item {
                    SearchItem::Artist(_) => {
                        if let Some(desktop) = desktop {
                            catalog.search_scroll_y = catalog.scroll_y;
                            self.start_catalog(
                                CatalogPending::Artist,
                                desktop.open_youtube_music_artist(id),
                            );
                        }
                    }
                    SearchItem::Album(_) => {
                        if let Some(desktop) = desktop {
                            catalog.album_origin = Some((id.clone(), catalog.scroll_y));
                            self.start_catalog(
                                CatalogPending::Album,
                                desktop.open_youtube_music_album(id),
                            );
                        }
                    }
                    SearchItem::Song(song) => {
                        if let Some(desktop) = desktop.filter(|_| song.needs_details()) {
                            self.start_catalog(
                                CatalogPending::Song,
                                desktop.open_youtube_music_song(song),
                            );
                            return Task::none();
                        }
                        match song.inspection() {
                            Ok(inspection) => {
                                self.catalog.song = Some(Box::new(inspection));
                                return self.update_catalog(CatalogAction::Review);
                            }
                            Err(error) => self.catalog.error = Some(error),
                        }
                    }
                }
            }
            CatalogAction::Search => {
                let catalog = &mut self.catalog;
                catalog.search_due = None;
                let query = catalog.query.trim().to_owned();
                if query.is_empty() {
                    return operation::focus("album-search");
                }
                if !catalog.search_home() || catalog.confirm_replace {
                    return Task::none();
                }
                catalog.song = None;
                if catalog.searched != query {
                    catalog.results.clear();
                }
                let filter = catalog.filter;
                catalog.results.remove(&filter);
                catalog.searched.clone_from(&query);
                if let Some(desktop) = desktop {
                    self.start_catalog(
                        CatalogPending::Search,
                        desktop.search_catalog(SearchRequest { query, filter }),
                    );
                }
                return self.catalog_scroll(0.0);
            }
            CatalogAction::Back => {
                self.catalog.error = None;
                if self.catalog.confirm_replace {
                    self.catalog.confirm_replace = false;
                    self.catalog.song = None;
                    return operation::focus(if self.catalog.album.is_some() {
                        "review-album"
                    } else if self.catalog.artist.is_some() {
                        "artist-back"
                    } else {
                        "album-search"
                    });
                }
                if self.catalog.album.take().is_some() {
                    if let Some((id, offset)) = self.catalog.album_origin.take() {
                        return Task::batch([
                            self.catalog_scroll(offset),
                            operation::focus(result_id(SearchKind::Albums, &id)),
                        ]);
                    }
                    return self.catalog_scroll(0.0);
                }
                if let Some(artist) = self.catalog.artist.take() {
                    return Task::batch([
                        self.catalog_scroll(self.catalog.search_scroll_y),
                        operation::focus(result_id(SearchKind::Artists, &artist.artist.id)),
                    ]);
                }
                return self.update_catalog(CatalogAction::ClearSearch);
            }
            CatalogAction::Review | CatalogAction::ConfirmReview => {
                if self.catalog.song.is_none() && self.catalog.album.is_none() {
                    return Task::none();
                }
                if matches!(action, CatalogAction::ConfirmReview) {
                    if !self.catalog.confirm_replace {
                        return Task::none();
                    }
                } else if self.review.is_some() {
                    self.catalog.confirm_replace = true;
                    return operation::focus("keep-current-review");
                }
                let inspection = self
                    .catalog
                    .song
                    .take()
                    .or_else(|| self.catalog.album.clone());
                if let Some(inspection) = inspection {
                    self.catalog.confirm_replace = false;
                    self.url.clone_from(&inspection.source_url);
                    return self.finish_inspection(*inspection);
                }
            }
            CatalogAction::FocusResult
            | CatalogAction::ClearSearch
            | CatalogAction::Cancel
            | CatalogAction::Query(_)
            | CatalogAction::Scrolled(_)
            | CatalogAction::OpenInBrowser => {
                unreachable!("catalog controls handled before busy actions")
            }
        }
        Task::none()
    }

    pub(super) fn tick_catalog_search(&mut self, now: Instant) -> Task<Message> {
        if self.panel != Panel::Workspace
            || !self.catalog.visible
            || !self.catalog.search_home()
            || self.desktop.is_none()
            || self.catalog.search_due.is_none_or(|due| now < due)
        {
            return Task::none();
        }
        if self.busy() {
            if matches!(self.catalog.pending, Some(CatalogPending::Search))
                && !self.catalog.cancelling
            {
                return self.update(Message::Cancel);
            }
            return Task::none();
        }
        self.update_catalog(CatalogAction::Search)
    }

    fn catalog_scroll(&mut self, offset: f32) -> Task<Message> {
        self.catalog.scroll_y = offset;
        operation::scroll_to(
            "album-browser",
            iced::widget::scrollable::AbsoluteOffset { x: 0.0, y: offset },
        )
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
            return self
                .catalog
                .queued_action
                .take()
                .map_or_else(Task::none, |action| Task::done(Message::Catalog(action)));
        }
        match event {
            Ok(CatalogEvent::Results { filter, hits }) => {
                if self.catalog.query.trim() == self.catalog.searched
                    && !self.catalog.searched.is_empty()
                {
                    self.catalog.results.insert(filter, hits);
                    return self.catalog_scroll(0.0);
                }
            }
            Ok(CatalogEvent::Artist(artist)) => {
                self.catalog.all_artist_releases = false;
                self.catalog.artist = Some(artist);
                return Task::batch([self.catalog_scroll(0.0), operation::focus("artist-back")]);
            }
            Ok(CatalogEvent::Album(album)) => {
                self.catalog.album = Some(album);
                return Task::batch([self.catalog_scroll(0.0), operation::focus("review-album")]);
            }
            Ok(CatalogEvent::Song(song)) => {
                self.catalog.song = Some(song);
                return Task::done(Message::Catalog(CatalogAction::Review));
            }
            Err(error) => {
                if !matches!(pending, Some(CatalogPending::Search))
                    || self.catalog.query.trim() == self.catalog.searched
                {
                    self.catalog.error = Some(error);
                }
            }
        }
        Task::none()
    }
}

pub fn result_id(kind: SearchKind, id: &str) -> String {
    format!("catalog-{}-{id}", kind.label())
}

impl App {
    pub(super) fn catalog_snapshot(&mut self, scenario: &str) {
        use serde_json::json;

        const COVER: &str = "https://i.ytimg.com/example.jpg";
        self.review = None;
        self.catalog = Catalog::new(true);
        if scenario == "catalog" {
            return;
        }
        self.catalog.query = "The Paper Trails".into();
        self.catalog.searched = self.catalog.query.clone();
        if scenario == "catalog-unavailable" {
            self.catalog.error = Some("YouTube Music took too long to respond. Try again.".into());
            return;
        }

        let releases = [
            (
                "MPREb_example1",
                "Room to breathe (Expanded Edition)",
                2026,
                "Album",
            ),
            ("MPREb_example2", "The Quiet Between", 2023, "Album"),
            ("MPREb_example3", "Along the River", 2022, "EP"),
            ("MPREb_example4", "First Light", 2021, "Single"),
        ];
        let mut hits = releases
            .map(|(id, title, year, kind)| {
                json!({
                    "result_type": "album", "featured": false, "id": id, "title": title,
                    "artist": "The Paper Trails", "year": year, "kind": kind, "artwork_url": COVER,
                })
            })
            .to_vec();
        hits.push(json!({
            "result_type": "song", "featured": false, "id": "abcdefghijk",
            "title": "First Light", "artist": "The Paper Trails", "album": "Room to breathe",
            "duration": 192, "available": true, "artwork_url": COVER,
        }));
        if scenario == "catalog-artist" {
            self.catalog.artist = Some(
                serde_json::from_value(json!({
                    "artist": {"id": "UCtest000000000000000001", "name": "The Paper Trails"},
                    "hits": hits,
                }))
                .expect("fictional artist"),
            );
        } else {
            hits.insert(
                0,
                json!({
                    "result_type": "artist", "featured": true, "id": "UCtest000000000000000001",
                    "name": "The Paper Trails",
                }),
            );
            self.catalog.results.insert(
                None,
                serde_json::from_value(json!(hits)).expect("fictional search results"),
            );
        }
        if scenario == "catalog-album" {
            let mut album = crate::example::album();
            album.title = releases[0].1.into();
            album.artwork_url = Some(COVER.into());
            album.warnings.clear();
            for track in &mut album.tracks {
                track.metadata.album.clone_from(&album.title);
                track.source_metadata.album.clone_from(&album.title);
            }
            self.catalog.album = Some(Box::new(album));
        }
        self.catalog.media.complete(
            COVER.into(),
            Some(crate::artwork::example_cover().source().clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogAction, CatalogPending, SEARCH_DELAY};
    use crate::app::App;
    use jacket_desktop::catalog::SearchKind;
    use jacket_desktop::CatalogEvent;
    use std::time::{Duration, Instant};

    #[test]
    fn album_browsing_previews_every_occurrence_before_opening_review() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        app.catalog.pending = Some(CatalogPending::Album);

        let mut album = crate::example::album();
        album.title = "Room to breathe (Expanded Edition)".into();
        let track = album.tracks[0].clone();
        album.tracks = vec![track; 20];
        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(album))));
        assert!(app.review.is_none());
        assert!(app.catalog.visible);
        assert_eq!(app.catalog.album.as_ref().unwrap().tracks.len(), 20);

        let _ = app.update_catalog(CatalogAction::Review);
        let review = app.review.as_ref().unwrap();
        assert!(!app.catalog.visible);
        assert_eq!(
            review.inspection.title,
            "Room to breathe (Expanded Edition)"
        );
        assert_eq!(review.inspection.tracks.len(), 20);
        assert_eq!(review.selected.len(), 20);
    }

    #[test]
    fn album_back_restores_artist_and_search_context() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-artist");
        app.catalog.album_origin = Some(("MPREb_example1".into(), 128.0));
        app.catalog.search_scroll_y = 74.0;

        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(crate::example::album()))));
        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.album.is_none());
        assert!(app.catalog.artist.is_some());
        assert_eq!(app.catalog.scroll_y, 128.0);

        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.search_home());
        assert_eq!(app.catalog.query, "The Paper Trails");
        assert_eq!(app.catalog.scroll_y, 74.0);
    }

    #[test]
    fn a_song_result_opens_the_selected_recording_in_review() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        let _ = app.update_catalog(CatalogAction::OpenResult(
            SearchKind::Songs,
            "abcdefghijk".into(),
        ));
        let review = app.review.as_ref().unwrap();
        assert!(review.is_song());
        assert_eq!(review.inspection.title, "First Light");
        assert_eq!(review.inspection.tracks[0].id, "abcdefghijk");
        assert_eq!(
            review.inspection.tracks[0].metadata.album,
            "Room to breathe"
        );
    }

    #[test]
    fn debounce_submits_only_the_latest_query_and_clearing_discards_results() {
        let root = tempfile::tempdir().unwrap();
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        app.desktop = Some(
            jacket_desktop::Desktop::with_directories(
                root.path().join("data"),
                root.path().join("cache"),
                root.path().join("resources"),
                root.path().join("home"),
            )
            .unwrap(),
        );

        let now = Instant::now();
        app.catalog.edit_query("Daf".into(), now);
        let last_key = now + Duration::from_millis(100);
        app.catalog.edit_query("Daft Punk".into(), last_key);
        assert_eq!(app.tick_catalog_search(now + SEARCH_DELAY).units(), 0);
        assert_eq!(app.catalog.searched, "The Paper Trails");
        assert!(app.tick_catalog_search(last_key + SEARCH_DELAY).units() > 0);
        assert_eq!(app.catalog.searched, "Daft Punk");
        assert_eq!(app.tick_catalog_search(last_key + SEARCH_DELAY).units(), 0);

        app.catalog.edit_query("  ".into(), last_key);
        assert!(app.catalog.searched.is_empty());
        assert!(app.catalog.hits().is_empty());
        assert!(app.catalog.search_due.is_none());
    }
}
