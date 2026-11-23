use super::{catalog_media::CatalogMedia, persistence, App, Message, Panel};
use audio_engine::Inspection;
use iced::{widget::operation, Task};
use jacket_desktop::catalog::{SearchKind, SearchRequest};
use jacket_desktop::youtube_music::{AlbumEdition, AlbumPage, ArtistPage, SearchHit, SearchItem};
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
    ArtistReleaseArtwork { id: String, url: Option<String> },
    OpenResult(SearchKind, String),
    OpenSongAlbum(String),
    OpenAlbumArtist(usize),
    ToggleEditions,
    OpenEdition(String),
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
    ArtistReleases,
    Album,
    Editions,
    Song,
}

enum CatalogPage {
    Search,
    Artist {
        page: ArtistPage,
        all_releases: bool,
        expanded_hits: Option<Vec<SearchHit>>,
    },
    Album(Box<AlbumPage>, EditionShelf),
}

#[derive(Default)]
pub struct EditionShelf {
    pub open: bool,
    pub rows: Option<Vec<AlbumEdition>>,
}

impl CatalogPage {
    fn album(page: Box<AlbumPage>) -> Self {
        Self::Album(page, EditionShelf::default())
    }

    fn initial_focus(&self) -> &'static str {
        match self {
            Self::Search => "album-search",
            Self::Artist { .. } => "artist-back",
            Self::Album(..) => "review-album",
        }
    }
}

struct BrowsePosition {
    filter: Option<SearchKind>,
    scroll_y: f32,
    focus: String,
}

struct HistoryEntry {
    page: CatalogPage,
    position: BrowsePosition,
}

pub struct Catalog {
    pub visible: bool,
    pub query: String,
    pub searched: String,
    pub filter: Option<SearchKind>,
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
    page: CatalogPage,
    history: Vec<HistoryEntry>,
    pending_origin: Option<BrowsePosition>,
    review_origin: Option<BrowsePosition>,
    song_focus: Option<String>,
}

impl Catalog {
    pub(super) fn new(visible: bool) -> Self {
        Self {
            visible,
            query: String::new(),
            searched: String::new(),
            filter: None,
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
            page: CatalogPage::Search,
            history: Vec::new(),
            pending_origin: None,
            review_origin: None,
            song_focus: None,
        }
    }

    pub fn search_home(&self) -> bool {
        matches!(self.page, CatalogPage::Search)
    }

    pub fn artist(&self) -> Option<&ArtistPage> {
        match &self.page {
            CatalogPage::Artist { page, .. } => Some(page),
            _ => None,
        }
    }

    pub fn show_all_artist_releases(&self) -> bool {
        matches!(
            self.page,
            CatalogPage::Artist {
                all_releases: true,
                ..
            }
        )
    }

    pub fn album(&self) -> Option<&AlbumPage> {
        match &self.page {
            CatalogPage::Album(album, _) => Some(album),
            _ => None,
        }
    }

    pub fn edition_shelf(&self) -> Option<&EditionShelf> {
        match &self.page {
            CatalogPage::Album(_, shelf) => Some(shelf),
            _ => None,
        }
    }

    pub fn back_label(&self) -> &'static str {
        match self.history.last().map(|entry| &entry.page) {
            Some(CatalogPage::Artist { .. }) => "Back to artist",
            Some(CatalogPage::Album(..)) => "Back to album",
            _ => "Back to results",
        }
    }

    pub fn hits(&self) -> &[SearchHit] {
        match &self.page {
            CatalogPage::Search => self.results.get(&self.filter).map_or(&[], Vec::as_slice),
            CatalogPage::Artist {
                all_releases: true,
                expanded_hits: Some(hits),
                ..
            } => hits,
            CatalogPage::Artist { page, .. } => &page.hits,
            CatalogPage::Album(..) => &[],
        }
    }

    fn result(&self, kind: SearchKind, id: &str) -> Option<SearchItem> {
        if self.confirm_replace || (self.search_home() && self.query.trim() != self.searched) {
            return None;
        }
        self.hits()
            .iter()
            .find(|hit| hit.item.kind() == kind && hit.item.id() == id)
            .map(|hit| hit.item.clone())
    }

    fn position(&self, focus: String) -> BrowsePosition {
        BrowsePosition {
            filter: self.filter,
            scroll_y: self.scroll_y,
            focus,
        }
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
            CatalogAction::ArtistReleaseArtwork { id, url } => {
                if self.catalog.show_all_artist_releases() {
                    self.catalog.media.set_release_artwork(id, url);
                }
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
                if let Some(position) = self.catalog.review_origin.take() {
                    return self.restore_catalog_position(position);
                }
                return Task::batch([
                    self.catalog_scroll(self.catalog.scroll_y),
                    operation::focus(self.catalog.page.initial_focus()),
                ]);
            }
            CatalogAction::FocusSearch => {
                self.catalog.visible = true;
                self.catalog.page = CatalogPage::Search;
                self.catalog.media.clear_release_artwork();
                self.catalog.history.clear();
                self.catalog.song = None;
                self.catalog.song_focus = None;
                self.catalog.review_origin = None;
                self.catalog.confirm_replace = false;
                self.catalog.error = None;
                return Task::batch([self.catalog_scroll(0.0), operation::focus("album-search")]);
            }
            CatalogAction::PasteLink => {
                self.catalog.search_due = None;
                self.catalog.visible = false;
                self.catalog.song = None;
                self.catalog.song_focus = None;
                self.catalog.confirm_replace = false;
                return self.update(Message::PasteLink);
            }
            CatalogAction::ReturnToReview => {
                self.catalog.search_due = None;
                self.catalog.visible = false;
                self.catalog.song = None;
                if let Some(focus) = self.catalog.song_focus.take() {
                    self.catalog.review_origin = Some(self.catalog.position(focus));
                }
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
                if !self.catalog.confirm_replace {
                    if let CatalogPage::Artist {
                        page,
                        all_releases,
                        expanded_hits,
                    } = &mut self.catalog.page
                    {
                        self.catalog.error = None;
                        if !*all_releases && expanded_hits.is_none() && page.has_more_releases {
                            if let Some(desktop) = desktop {
                                let job =
                                    desktop.youtube_music_artist_releases(page.artist.id.clone());
                                self.start_catalog(CatalogPending::ArtistReleases, job);
                            }
                        } else {
                            *all_releases = !*all_releases;
                            if !*all_releases {
                                self.catalog.media.clear_release_artwork();
                            }
                        }
                    }
                }
            }
            CatalogAction::OpenResult(kind, id) => {
                let Some(item) = self.catalog.result(kind, &id) else {
                    return Task::none();
                };
                let focus = result_id(kind, &id);
                self.catalog.song = None;
                self.catalog.error = None;
                match item {
                    SearchItem::Artist(_) => {
                        if let Some(desktop) = desktop {
                            self.start_navigation(
                                CatalogPending::Artist,
                                focus,
                                desktop.open_youtube_music_artist(id),
                            );
                        }
                    }
                    SearchItem::Album(_) => {
                        if let Some(desktop) = desktop {
                            self.start_navigation(
                                CatalogPending::Album,
                                focus,
                                desktop.open_youtube_music_album(id),
                            );
                        }
                    }
                    SearchItem::Song(song) => {
                        self.catalog.song_focus = Some(focus);
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
            CatalogAction::OpenSongAlbum(id) => {
                if let (Some(desktop), Some(SearchItem::Song(song))) =
                    (desktop, self.catalog.result(SearchKind::Songs, &id))
                {
                    self.start_navigation(
                        CatalogPending::Album,
                        song_album_id(&id),
                        desktop.open_youtube_music_song_album(song),
                    );
                }
            }
            CatalogAction::OpenAlbumArtist(index) => {
                if !self.catalog.confirm_replace {
                    let id = self
                        .catalog
                        .album()
                        .and_then(|album| album.artists.get(index))
                        .and_then(|artist| artist.id.clone());
                    if let (Some(desktop), Some(id)) = (desktop, id) {
                        self.start_navigation(
                            CatalogPending::Artist,
                            artist_credit_id(index),
                            desktop.open_youtube_music_artist(id),
                        );
                    }
                }
            }
            CatalogAction::ToggleEditions => {
                if !self.catalog.confirm_replace {
                    if let CatalogPage::Album(album, shelf) = &mut self.catalog.page {
                        shelf.open = !shelf.open;
                        self.catalog.error = None;
                        if shelf.open {
                            if let Some(desktop) = desktop.filter(|_| shelf.rows.is_none()) {
                                let job = desktop.youtube_music_editions(album.id.clone());
                                self.start_catalog(CatalogPending::Editions, job);
                            }
                            return self.catalog_scroll(0.0);
                        }
                    }
                }
            }
            CatalogAction::OpenEdition(id) => {
                if !self.catalog.confirm_replace {
                    let known = self.catalog.edition_shelf().is_some_and(|shelf| {
                        shelf.open
                            && shelf
                                .rows
                                .as_ref()
                                .is_some_and(|rows| rows.iter().any(|row| row.id == id))
                    });
                    if let Some(desktop) = desktop.filter(|_| known) {
                        self.start_navigation(
                            CatalogPending::Album,
                            edition_id(&id),
                            desktop.open_youtube_music_album(id),
                        );
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
                catalog.song_focus = None;
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
                    let focus = self
                        .catalog
                        .song_focus
                        .take()
                        .unwrap_or_else(|| self.catalog.page.initial_focus().into());
                    return operation::focus(focus);
                }
                if let Some(previous) = self.catalog.history.pop() {
                    self.catalog.page = previous.page;
                    self.catalog.media.clear_release_artwork();
                    return self.restore_catalog_position(previous.position);
                }
                return self.update_catalog(CatalogAction::ClearSearch);
            }
            CatalogAction::Review | CatalogAction::ConfirmReview => {
                if self.catalog.song.is_none() && self.catalog.album().is_none() {
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
                    .map(|inspection| *inspection)
                    .or_else(|| self.catalog.album().map(|album| album.inspection.clone()));
                if let Some(inspection) = inspection {
                    let focus = self
                        .catalog
                        .song_focus
                        .take()
                        .unwrap_or_else(|| self.catalog.page.initial_focus().into());
                    self.catalog.review_origin = Some(self.catalog.position(focus));
                    self.catalog.confirm_replace = false;
                    self.url.clone_from(&inspection.source_url);
                    return self.finish_inspection(inspection);
                }
            }
            CatalogAction::FocusResult
            | CatalogAction::ClearSearch
            | CatalogAction::Cancel
            | CatalogAction::Query(_)
            | CatalogAction::Scrolled(_)
            | CatalogAction::ArtistReleaseArtwork { .. }
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

    fn restore_catalog_position(&mut self, position: BrowsePosition) -> Task<Message> {
        self.catalog.filter = position.filter;
        Task::batch([
            self.catalog_scroll(position.scroll_y),
            operation::focus(position.focus),
        ])
    }

    fn start_navigation(
        &mut self,
        pending: CatalogPending,
        focus: String,
        job: Result<Job, String>,
    ) {
        if job.is_ok() {
            self.catalog.song_focus = None;
            self.catalog.pending_origin = Some(self.catalog.position(focus));
        }
        self.start_catalog(pending, job);
    }

    fn finish_navigation(
        &mut self,
        page: CatalogPage,
        origin: Option<BrowsePosition>,
    ) -> Task<Message> {
        let initial_focus = page.initial_focus();
        let previous = std::mem::replace(&mut self.catalog.page, page);
        self.catalog.media.clear_release_artwork();
        if let Some(position) = origin {
            self.catalog.history.push(HistoryEntry {
                page: previous,
                position,
            });
        }
        Task::batch([self.catalog_scroll(0.0), operation::focus(initial_focus)])
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
        let origin = self.catalog.pending_origin.take();
        if matches!(pending, Some(CatalogPending::Song)) && event.is_err() {
            self.catalog.song_focus = None;
        }
        if std::mem::take(&mut self.catalog.cancelling) {
            if matches!(pending, Some(CatalogPending::ArtistReleases)) {
                return operation::focus("artist-releases");
            }
            if matches!(pending, Some(CatalogPending::Editions)) {
                if let CatalogPage::Album(_, shelf) = &mut self.catalog.page {
                    shelf.open = false;
                }
                return operation::focus("toggle-editions");
            }
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
                return self.finish_navigation(
                    CatalogPage::Artist {
                        page: artist,
                        all_releases: false,
                        expanded_hits: None,
                    },
                    origin,
                );
            }
            Ok(CatalogEvent::ArtistReleases(hits)) => {
                if let CatalogPage::Artist {
                    all_releases,
                    expanded_hits,
                    ..
                } = &mut self.catalog.page
                {
                    *expanded_hits = Some(hits);
                    *all_releases = true;
                    return operation::focus("artist-releases");
                }
            }
            Ok(CatalogEvent::Album(album)) => {
                return self.finish_navigation(CatalogPage::album(album), origin);
            }
            Ok(CatalogEvent::Editions(rows)) => {
                if let CatalogPage::Album(_, shelf) = &mut self.catalog.page {
                    let focus = rows.first().map_or_else(
                        || "toggle-editions".into(),
                        |edition| edition_id(&edition.id),
                    );
                    shelf.rows = Some(rows);
                    return operation::focus(focus);
                }
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
                if matches!(pending, Some(CatalogPending::ArtistReleases)) {
                    return operation::focus("artist-releases");
                }
            }
        }
        Task::none()
    }
}

pub fn result_id(kind: SearchKind, id: &str) -> String {
    format!("catalog-{}-{id}", kind.label())
}

pub fn edition_id(id: &str) -> String {
    format!("other-edition-{id}")
}

pub fn artist_credit_id(index: usize) -> String {
    format!("album-artist-{index}")
}

pub fn song_album_id(id: &str) -> String {
    format!("song-album-{id}")
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
            "album_id": "MPREb_example1", "duration": 192, "available": true, "artwork_url": COVER,
        }));
        let artist = json!({"id": "UCtest000000000000000001", "name": "The Paper Trails"});
        if scenario == "catalog-artist" {
            self.catalog.page = CatalogPage::Artist {
                page: serde_json::from_value(json!({ "artist": artist, "hits": hits }))
                    .expect("fictional artist"),
                all_releases: false,
                expanded_hits: None,
            };
        }
        hits.insert(0, json!({
            "result_type": "artist", "featured": true, "id": artist["id"], "name": artist["name"],
        }));
        self.catalog.results.insert(
            None,
            serde_json::from_value(json!(hits)).expect("fictional search results"),
        );
        if matches!(scenario, "catalog-album" | "catalog-editions") {
            let mut album = crate::example::album();
            album.title = releases[0].1.into();
            album.artwork_url = Some(COVER.into());
            album.warnings.clear();
            for track in &mut album.tracks {
                track.metadata.album.clone_from(&album.title);
                track.source_metadata.album.clone_from(&album.title);
            }
            self.catalog.page = CatalogPage::album(Box::new(AlbumPage {
                id: releases[0].0.into(),
                inspection: album,
                artists: vec![serde_json::from_value(artist).expect("fictional artist credit")],
            }));
            if scenario == "catalog-editions" {
                if let CatalogPage::Album(_, shelf) = &mut self.catalog.page {
                    shelf.open = true;
                    shelf.rows = Some(serde_json::from_value(json!([
                        {"id": "MPREb_ed1", "title": "Room to breathe", "year": 2018, "track_count": 10, "artwork_url": COVER},
                        {"id": "MPREb_ed2", "title": "Room to breathe (Deluxe Edition)", "year": 2024, "track_count": 20, "artwork_url": COVER},
                        {"id": "MPREb_ed3", "title": "Room to breathe (Remastered)", "year": 2025, "artwork_url": COVER}
                    ])).expect("fictional album editions"));
                }
            }
        }
        let focus = match &self.catalog.page {
            CatalogPage::Artist { page, .. } => {
                Some(result_id(SearchKind::Artists, &page.artist.id))
            }
            CatalogPage::Album(..) => Some(result_id(SearchKind::Albums, releases[0].0)),
            CatalogPage::Search => None,
        };
        if let Some(focus) = focus {
            self.catalog.history.push(HistoryEntry {
                page: CatalogPage::Search,
                position: self.catalog.position(focus),
            });
        }
        self.catalog.media.complete(
            COVER.into(),
            Some(crate::artwork::example_cover().source().clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        artist_credit_id, song_album_id, CatalogAction, CatalogPage, CatalogPending, SEARCH_DELAY,
    };
    use crate::app::App;
    use jacket_desktop::catalog::SearchKind;
    use jacket_desktop::youtube_music::AlbumPage;
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
        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(AlbumPage {
            id: "MPREb_fixture".into(),
            inspection: album,
            artists: Vec::new(),
        }))));
        assert!(app.review.is_none());
        assert!(app.catalog.visible);
        assert_eq!(app.catalog.album().unwrap().inspection.tracks.len(), 20);

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
    fn back_through_artist_and_album_restores_the_song_filter_scroll_and_link() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-artist");
        let artist = app.catalog.artist().unwrap().clone();
        app.catalog_snapshot("catalog-album");
        let album = app.catalog.album().unwrap().clone();
        app.catalog_snapshot("catalog-results");

        app.catalog.filter = Some(SearchKind::Songs);
        app.catalog.scroll_y = 74.0;
        let song_link = song_album_id("abcdefghijk");
        app.catalog.pending_origin = Some(app.catalog.position(song_link.clone()));
        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(album))));
        assert_eq!(app.catalog.back_label(), "Back to results");
        assert_eq!(app.catalog.scroll_y, 0.0);
        assert_eq!(app.catalog.history[0].position.focus, song_link);

        app.catalog.scroll_y = 128.0;
        let credit_link = artist_credit_id(0);
        app.catalog.pending_origin = Some(app.catalog.position(credit_link.clone()));
        let _ = app.catalog_finished(Ok(CatalogEvent::Artist(artist)));
        assert_eq!(app.catalog.back_label(), "Back to album");
        assert_eq!(app.catalog.history[1].position.focus, credit_link);
        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.album().is_some());
        assert_eq!(app.catalog.scroll_y, 128.0);

        let _ = app.update_catalog(CatalogAction::Back);
        assert!(matches!(app.catalog.page, CatalogPage::Search));
        assert_eq!(app.catalog.query, "The Paper Trails");
        assert_eq!(app.catalog.filter, Some(SearchKind::Songs));
        assert_eq!(app.catalog.scroll_y, 74.0);
    }

    #[cfg(unix)]
    #[test]
    fn show_all_loads_releases_on_demand_and_keeps_them_when_returning_from_an_album() {
        use serde_json::json;
        use std::{fs, os::unix::fs::PermissionsExt, thread};

        const ARTIST: &str = "UCtest000000000000000001";
        let root = tempfile::tempdir().unwrap();
        let resources = root.path().join("resources");
        let launcher = resources.join("catalog-tools/bin/youtube-music");
        fs::create_dir_all(launcher.parent().unwrap()).unwrap();
        let albums = (0..125)
            .map(|index| {
                json!({
                    "result_type": "album", "featured": false,
                    "id": format!("MPREb_release{index}"), "title": format!("Release {index}"),
                    "artist": "The Paper Trails", "kind": "Album",
                })
            })
            .collect::<Vec<_>>();
        let preview = json!({
            "artist": {"id": format!("MPLA{ARTIST}"), "name": "The Paper Trails"},
            "hits": &albums[..3], "has_more_releases": true,
        });
        let expanded = json!({
            "artist": {"id": ARTIST, "name": "The Paper Trails"},
            "hits": albums, "has_more_releases": true,
        });
        let final_album = json!({
            "title": "Release 124", "artist": "The Paper Trails",
            "tracks": [{"id": "abcdefghijk", "title": "First Light", "artist": "", "available": true}],
        });
        fs::write(
            &launcher,
            format!(
                r#"#!/bin/sh
case "$1:$2" in
    artist:{ARTIST}) printf '%s\n' '{preview}' ;;
    artist-releases:{ARTIST}) printf '%s\n' '{expanded}' ;;
    album:MPREb_release124) printf '%s\n' '{final_album}' ;;
    *) exit 1 ;;
esac
"#
            ),
        )
        .unwrap();
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o755)).unwrap();
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        app.desktop = Some(
            jacket_desktop::Desktop::with_directories(
                root.path().join("data"),
                root.path().join("cache"),
                resources,
                root.path().join("home"),
            )
            .unwrap(),
        );
        let finish_job = |app: &mut App| {
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.job.is_some() {
                assert!(
                    Instant::now() < deadline,
                    "catalog fixture did not complete"
                );
                let _ = app.tick();
                thread::sleep(Duration::from_millis(5));
            }
            assert!(app.catalog.error.is_none(), "{:?}", app.catalog.error);
        };

        let _ = app.update_catalog(CatalogAction::OpenResult(
            SearchKind::Artists,
            ARTIST.into(),
        ));
        finish_job(&mut app);
        assert_eq!(app.catalog.artist().unwrap().artist.id, ARTIST);
        assert_eq!(app.catalog.hits().len(), 3);
        assert!(!app.catalog.show_all_artist_releases());

        let _ = app.update_catalog(CatalogAction::ToggleArtistReleases);
        assert!(matches!(
            app.catalog.pending,
            Some(CatalogPending::ArtistReleases)
        ));
        assert_eq!(app.catalog.hits().len(), 3);
        finish_job(&mut app);
        assert!(app.catalog.show_all_artist_releases());
        assert_eq!(app.catalog.hits().len(), 125);

        let last_release = "MPREb_release124";
        app.catalog.scroll_y = 300.0;
        let _ = app.update_catalog(CatalogAction::OpenResult(
            SearchKind::Albums,
            last_release.into(),
        ));
        finish_job(&mut app);
        assert_eq!(app.catalog.album().unwrap().id, last_release);
        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.show_all_artist_releases());
        assert_eq!(app.catalog.hits().len(), 125);
        assert_eq!(app.catalog.scroll_y, 300.0);

        let _ = app.update_catalog(CatalogAction::ToggleArtistReleases);
        assert_eq!(app.catalog.hits().len(), 3);
        let _ = app.update_catalog(CatalogAction::ToggleArtistReleases);
        assert!(app.job.is_none());
        assert_eq!(app.catalog.hits().len(), 125);
    }

    #[test]
    fn failed_or_cancelled_artist_expansion_keeps_the_preview() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-artist");
        let preview_count = app.catalog.hits().len();
        app.catalog.pending = Some(CatalogPending::ArtistReleases);
        let _ = app.catalog_finished(Err("Could not load releases.".into()));
        assert_eq!(
            app.catalog.error.as_deref(),
            Some("Could not load releases.")
        );
        assert!(!app.catalog.show_all_artist_releases());
        assert_eq!(app.catalog.hits().len(), preview_count);

        app.catalog.error = None;
        app.catalog.pending = Some(CatalogPending::ArtistReleases);
        app.catalog.cancelling = true;
        let _ = app.catalog_finished(Ok(CatalogEvent::ArtistReleases(Vec::new())));
        assert!(app.catalog.error.is_none());
        assert!(!app.catalog.show_all_artist_releases());
        assert_eq!(app.catalog.hits().len(), preview_count);
    }

    #[test]
    fn a_cancelled_navigation_does_not_push_a_page() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-album");
        let album = app.catalog.album().unwrap().clone();
        let history = app.catalog.history.len();
        app.catalog.pending = Some(CatalogPending::Album);
        app.catalog.pending_origin = Some(app.catalog.position(artist_credit_id(0)));
        app.catalog.cancelling = true;

        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(album))));
        assert_eq!(app.catalog.history.len(), history);
        assert!(app.catalog.pending_origin.is_none());
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
