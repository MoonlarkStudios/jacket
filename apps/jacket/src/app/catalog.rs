use super::{catalog_media::CatalogMedia, persistence, App, Message, Panel};
use audio_engine::Inspection;
use iced::{widget::operation, Task};
use jacket_core::{CompletionIdentity, SourceLink};
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
    ReturnToReview,
    Query(String),
    Submit(String),
    Search,
    Filter(Option<SearchKind>),
    ToggleArtistReleases,
    ArtistReleaseArtwork { id: String, url: Option<String> },
    OpenResult(SearchKind, String),
    ToggleSongMenu(String),
    DismissSongMenu,
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

struct AlbumReview {
    album_id: String,
    session_id: String,
    parked: Option<(CatalogPage, f32)>,
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
    album_review: Option<AlbumReview>,
    song_focus: Option<String>,
    song_menu: Option<String>,
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
            album_review: None,
            song_focus: None,
            song_menu: None,
        }
    }

    pub fn search_home(&self) -> bool {
        matches!(self.page, CatalogPage::Search)
    }

    pub fn link_input(&self) -> bool {
        let Some(first) = self.query.split_whitespace().next() else {
            return false;
        };
        let first = first.to_ascii_lowercase();
        first.contains("://")
            || first.starts_with("https:")
            || first.starts_with("http:")
            || first.starts_with("www.")
            || matches!(
                first.split(['/', '?', '#']).next(),
                Some("youtube.com" | "music.youtube.com" | "m.youtube.com" | "youtu.be")
            )
    }

    pub fn search_terms(&self) -> Option<&str> {
        let query = self.query.trim();
        (!query.is_empty() && !self.link_input()).then_some(query)
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

    pub(super) fn reviewed_album_on_page(&self, identity: &CompletionIdentity) -> bool {
        self.album_review.as_ref().is_some_and(|review| {
            review.session_id == identity.session_id
                && self
                    .album()
                    .is_some_and(|album| album.id == review.album_id)
        })
    }

    pub(super) fn clear_album_review(&mut self) {
        self.album_review = None;
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

    fn position(&self, focus: String, track_scroll: f32) -> BrowsePosition {
        BrowsePosition {
            filter: self.filter,
            scroll_y: if self.album().is_some() {
                track_scroll
            } else {
                self.scroll_y
            },
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
        let artist_match = featured.is_some_and(|hit| hit.item.kind() == SearchKind::Artists);
        kinds.sort_by_key(|kind| {
            let rank = self
                .hits()
                .iter()
                .position(|hit| {
                    hit.item.kind() == *kind
                        && featured.is_none_or(|first| first.item.id() != hit.item.id())
                })
                .unwrap_or(usize::MAX);
            (artist_match && *kind != SearchKind::Albums, rank)
        });
        kinds
    }

    pub fn song_menu_open(&self, id: &str) -> bool {
        self.song_menu.as_deref() == Some(id)
    }

    pub(super) fn dismiss_song_menu(&mut self) -> bool {
        self.song_menu.take().is_some()
    }

    fn edit_query(&mut self, query: String, now: Instant) {
        self.query = query;
        self.error = None;
        if self.query.trim().is_empty() {
            self.searched.clear();
            self.results.clear();
        }
        let Some(terms) = self.search_terms() else {
            self.queued_action = None;
            self.search_due = None;
            return;
        };
        self.search_due = (terms != self.searched
            && terms.chars().take(201).count() <= 200
            && terms
                .chars()
                .filter(|c| c.is_alphanumeric())
                .take(2)
                .count()
                == 2)
            .then_some(now + SEARCH_DELAY);
    }
}

impl App {
    pub fn reviewing_catalog_album(&self) -> bool {
        self.catalog.visible
            && self
                .review
                .as_ref()
                .is_some_and(|review| self.catalog.reviewed_album_on_page(&review.identity()))
    }

    pub(super) fn park_catalog_album(&mut self) {
        if self.reviewing_catalog_album() {
            let page = std::mem::replace(&mut self.catalog.page, CatalogPage::Search);
            if let Some(review) = &mut self.catalog.album_review {
                review.parked = Some((page, self.track_scroll));
            }
        }
    }

    pub(super) fn cancel_catalog(&mut self) {
        if let Some(job) = &self.catalog_job {
            self.catalog.cancelling = true;
            job.cancel();
        }
    }

    pub(super) fn update_catalog(&mut self, action: CatalogAction) -> Task<Message> {
        if self.downloads.is_open() && matches!(action, CatalogAction::FocusSearch) {
            self.downloads.dismiss();
            let focus = self.update_catalog(action);
            return Task::batch([focus, self.restore_discovery_scroll()]);
        }
        if self.downloads.is_open() && matches!(action, CatalogAction::FocusResult) {
            return Task::none();
        }
        if self.panel != Panel::Workspace
            || (!self.catalog.visible
                && !matches!(action, CatalogAction::Browse | CatalogAction::FocusSearch))
        {
            return Task::none();
        }
        match &action {
            CatalogAction::Submit(query) if *query != self.catalog.query => return Task::none(),
            CatalogAction::ToggleSongMenu(id) => {
                let was_open = self.catalog.song_menu_open(id);
                self.catalog.dismiss_song_menu();
                if !was_open && !self.busy() && self.catalog.result(SearchKind::Songs, id).is_some()
                {
                    self.catalog.song_menu = Some(id.clone());
                }
                return Task::none();
            }
            CatalogAction::DismissSongMenu => {
                self.catalog.dismiss_song_menu();
                return Task::none();
            }
            CatalogAction::FocusResult if self.catalog.song_menu.is_some() => return Task::none(),
            CatalogAction::ArtistReleaseArtwork { .. } => {}
            _ => {
                self.catalog.dismiss_song_menu();
            }
        }
        match action {
            CatalogAction::Cancel => {
                self.catalog.search_due = None;
                self.catalog.queued_action = None;
                self.cancel_catalog();
                return Task::none();
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
                if let (Some(query), Some(desktop)) =
                    (self.catalog.search_terms(), self.desktop.clone())
                {
                    if query.chars().count() <= 200 {
                        let query = query.to_owned();
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
                    if self.catalog.search_terms().is_none()
                        && matches!(self.catalog.pending, Some(CatalogPending::Search))
                    {
                        return self.update(Message::Cancel);
                    }
                }
                return Task::none();
            }
            CatalogAction::Submit(_) if !self.catalog.link_input() => {
                return self.update_catalog(CatalogAction::Search);
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
                            .chain(super::catalog_focus::reveal())
                    });
            }
            CatalogAction::FocusSearch if self.catalog.visible && self.catalog.search_home() => {
                return operation::focus("album-search");
            }
            _ => {}
        }
        if self.busy() {
            let replaces_search = match action {
                CatalogAction::Submit(_) => true,
                CatalogAction::Search => self
                    .catalog
                    .search_terms()
                    .is_some_and(|query| query != self.catalog.searched),
                _ => false,
            };
            if replaces_search
                && matches!(self.catalog.pending, Some(CatalogPending::Search))
                && self.catalog.search_home()
                && !self.catalog.confirm_replace
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
                self.park_catalog_album();
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
            CatalogAction::Submit(query) => {
                if self.catalog.search_home() && !self.catalog.confirm_replace {
                    match SourceLink::parse(&query) {
                        Ok(link) => {
                            self.catalog.search_due = None;
                            self.catalog.song = None;
                            self.catalog.song_focus = None;
                            return self.update(Message::Pasted(link.url));
                        }
                        Err(error) => self.catalog.error = Some(error),
                    }
                }
            }
            CatalogAction::ReturnToReview => {
                self.catalog.search_due = None;
                self.catalog.song = None;
                self.catalog.confirm_replace = false;
                if let (Some(review), Some(album)) = (&self.review, &mut self.catalog.album_review)
                {
                    if review.identity().session_id == album.session_id {
                        if let Some((page, scroll)) = album.parked.take() {
                            let origin = self
                                .catalog
                                .position("return-to-review".into(), self.track_scroll);
                            return self.finish_navigation(page, Some(origin), Some(scroll));
                        }
                    }
                }
                self.catalog.visible = false;
                if let Some(focus) = self.catalog.song_focus.take() {
                    self.catalog.review_origin =
                        Some(self.catalog.position(focus, self.track_scroll));
                }
            }
            CatalogAction::Filter(filter) => {
                if self.catalog.search_home()
                    && !self.catalog.confirm_replace
                    && self.catalog.filter != filter
                {
                    self.catalog.filter = filter;
                    self.catalog.error = None;
                    if self.catalog.search_terms().is_some_and(|query| {
                        query != self.catalog.searched
                            || !self.catalog.results.contains_key(&filter)
                    }) {
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
                if !catalog.search_home() || catalog.confirm_replace {
                    return Task::none();
                }
                let Some(query) = catalog.search_terms().map(str::to_owned) else {
                    return Task::none();
                };
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
                    self.park_catalog_album();
                    self.catalog.page = previous.page;
                    self.catalog.media.clear_release_artwork();
                    return self.restore_catalog_position(previous.position);
                }
                if !self.catalog.search_home() {
                    self.park_catalog_album();
                    self.catalog.page = CatalogPage::Search;
                    return Task::batch([
                        self.catalog_scroll(0.0),
                        operation::focus("album-search"),
                    ]);
                }
                return self.update_catalog(CatalogAction::ClearSearch);
            }
            CatalogAction::Review | CatalogAction::ConfirmReview => {
                if self.reviewing_catalog_album() {
                    return Task::none();
                }
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
                let album_id = if self.catalog.song.is_none() {
                    self.catalog.album().map(|album| album.id.clone())
                } else {
                    None
                };
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
                    self.catalog.review_origin = album_id
                        .is_none()
                        .then(|| self.catalog.position(focus, self.track_scroll));
                    self.catalog.confirm_replace = false;
                    self.url.clone_from(&inspection.source_url);
                    let scroll = if album_id.is_some() {
                        self.track_scroll
                    } else {
                        0.0
                    };
                    let admitted = self.finish_inspection_at(inspection, scroll);
                    if let (Some(album_id), Some(review)) = (album_id, &self.review) {
                        self.catalog.album_review = Some(AlbumReview {
                            album_id,
                            session_id: review.identity().session_id,
                            parked: None,
                        });
                        self.catalog.visible = true;
                        return admitted.chain(operation::focus("review-select-all"));
                    }
                    return admitted;
                }
            }
            CatalogAction::FocusResult
            | CatalogAction::ToggleSongMenu(_)
            | CatalogAction::DismissSongMenu
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
        self.restore_catalog_scroll()
    }

    fn restore_catalog_scroll(&self) -> Task<Message> {
        operation::scroll_to(
            super::CATALOG_SCROLL_ID,
            iced::widget::scrollable::AbsoluteOffset {
                x: 0.0,
                y: self.catalog.scroll_y,
            },
        )
    }

    pub(super) fn restore_discovery_scroll(&self) -> Task<Message> {
        if self.catalog.visible && self.catalog.album().is_none() {
            self.restore_catalog_scroll()
        } else {
            self.restore_scroll()
        }
    }

    fn restore_catalog_position(&mut self, position: BrowsePosition) -> Task<Message> {
        self.catalog.filter = position.filter;
        let scroll = if self.catalog.album().is_some() {
            self.track_scroll = position.scroll_y;
            self.restore_scroll()
        } else {
            self.catalog_scroll(position.scroll_y)
        };
        let focus = if position.focus == "return-to-review"
            && self.review.is_none()
            && self.result.is_none()
        {
            self.catalog.page.initial_focus().to_owned()
        } else {
            position.focus
        };
        Task::batch([scroll, operation::focus(focus)])
    }

    fn start_navigation(
        &mut self,
        pending: CatalogPending,
        focus: String,
        job: Result<Job, String>,
    ) {
        if job.is_ok() {
            self.catalog.song_focus = None;
            self.catalog.pending_origin = Some(self.catalog.position(focus, self.track_scroll));
        }
        self.start_catalog(pending, job);
    }

    fn finish_navigation(
        &mut self,
        page: CatalogPage,
        origin: Option<BrowsePosition>,
        track_scroll: Option<f32>,
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
        let (scroll, focus) = if self.reviewing_catalog_album() {
            let parked = self
                .catalog
                .album_review
                .as_mut()
                .and_then(|album| album.parked.take());
            (
                track_scroll
                    .or_else(|| parked.map(|(_, scroll)| scroll))
                    .unwrap_or(0.0),
                "review-select-all",
            )
        } else {
            (0.0, initial_focus)
        };
        self.track_scroll = scroll;
        Task::batch([
            self.catalog_scroll(0.0),
            self.restore_scroll(),
            operation::focus(focus),
        ])
    }

    fn start_catalog(&mut self, pending: CatalogPending, job: Result<Job, String>) {
        self.catalog.error = None;
        match job {
            Ok(job) => {
                self.catalog.pending = Some(pending);
                self.catalog.cancelling = false;
                self.catalog_job = Some(job);
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
                    None,
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
                return self.finish_navigation(CatalogPage::album(album), origin, None);
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
                Some(COVER),
            ),
            (
                "MPREb_example2",
                "The Quiet Between",
                2023,
                "Album",
                Some(COVER),
            ),
            ("MPREb_example3", "Along the River", 2022, "EP", Some(COVER)),
            ("MPREb_example4", "First Light", 2021, "Single", Some(COVER)),
            (
                "MPREb_example5",
                "Songs for an Unfinished Map",
                2020,
                "Album",
                None,
            ),
            (
                "MPREb_example6",
                "A Place Beyond the Pine Trees",
                2019,
                "EP",
                Some(COVER),
            ),
            ("MPREb_example7", "Northbound", 2018, "Album", Some(COVER)),
        ];
        let mut hits = releases
            .map(|(id, title, year, kind, artwork_url)| {
                json!({
                    "result_type": "album", "featured": false, "id": id, "title": title,
                    "artist": "The Paper Trails", "year": year, "kind": kind, "artwork_url": artwork_url,
                })
            })
            .to_vec();
        hits.extend(
            [
                (
                    "abcdefghijk",
                    "First Light",
                    "Room to breathe",
                    "MPREb_example1",
                    Some(192),
                ),
                (
                    "bcdefghijkl",
                    "Along the River",
                    "Along the River",
                    "MPREb_example3",
                    Some(725),
                ),
                (
                    "cdefghijklm",
                    "A Place Beyond the Pine Trees (Live)",
                    "A Place Beyond the Pine Trees",
                    "MPREb_example6",
                    Some(4029),
                ),
                (
                    "defghijklmn",
                    "Northbound",
                    "Northbound",
                    "MPREb_example7",
                    None,
                ),
            ]
            .map(|(id, title, album, album_id, duration)| {
                json!({
                    "result_type": "song", "featured": false, "id": id, "title": title,
                    "artist": "The Paper Trails", "album": album, "album_id": album_id,
                    "duration": duration, "available": true, "artwork_url": COVER,
                })
            }),
        );
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
        hits.extend(
            [
                ("UCtest000000000000000002", "Sofia Mercer", Some(COVER)),
                ("UCtest000000000000000003", "Northbound Quartet", None),
                (
                    "UCtest000000000000000004",
                    "Maya & the Quiet Orchestra",
                    None,
                ),
                ("UCtest000000000000000005", "Jonah Vale", Some(COVER)),
                ("UCtest000000000000000006", "River Hall Ensemble", None),
                ("UCtest000000000000000007", "Milo Grey", None),
                ("UCtest000000000000000008", "Westline Trio", None),
            ]
            .map(|(id, name, artwork_url)| {
                json!({
                    "result_type": "artist", "featured": false, "id": id, "name": name,
                    "artwork_url": artwork_url,
                })
            }),
        );
        let search_hits: Vec<SearchHit> =
            serde_json::from_value(json!(hits)).expect("fictional search results");
        for kind in [SearchKind::Artists, SearchKind::Songs] {
            self.catalog.results.insert(
                Some(kind),
                search_hits
                    .iter()
                    .filter(|hit| hit.item.kind() == kind)
                    .cloned()
                    .collect(),
            );
        }
        self.catalog.results.insert(None, search_hits);
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
                position: self.catalog.position(focus, self.track_scroll),
            });
        }
        if scenario != "catalog-results-loading" {
            self.catalog.media.complete(
                COVER.into(),
                crate::artwork::ThumbnailShape::Square,
                Some(crate::artwork::example_cover().source().clone()),
            );
            self.catalog.media.complete(
                COVER.into(),
                crate::artwork::ThumbnailShape::Circle,
                Some(crate::artwork::example_portrait()),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        artist_credit_id, song_album_id, CatalogAction, CatalogPage, CatalogPending, SEARCH_DELAY,
    };
    use crate::app::{App, Panel};
    use jacket_desktop::catalog::SearchKind;
    use jacket_desktop::youtube_music::AlbumPage;
    use jacket_desktop::CatalogEvent;
    use std::time::{Duration, Instant};

    #[test]
    fn a_featured_artist_puts_albums_ahead_of_higher_ranked_songs() {
        let (mut app, _) = App::new();
        app.catalog_snapshot("catalog-results");
        let search_hits = app.catalog.results.get_mut(&None).unwrap();
        let song = search_hits
            .iter()
            .position(|hit| hit.item.kind() == SearchKind::Songs)
            .unwrap();
        search_hits.swap(1, song);

        assert_eq!(
            app.catalog.result_order()[..2],
            [SearchKind::Albums, SearchKind::Songs]
        );

        let search_hits = app.catalog.results.get_mut(&None).unwrap();
        search_hits[0].featured = false;
        assert_eq!(
            app.catalog.result_order(),
            [SearchKind::Artists, SearchKind::Songs, SearchKind::Albums]
        );
    }

    #[test]
    fn escape_dismisses_the_song_menu_without_leaving_search_results() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        let id = "abcdefghijk";
        let _ = app.update_catalog(CatalogAction::ToggleSongMenu(id.into()));
        assert!(app.catalog.song_menu_open(id));

        let _ = app.update(crate::app::Message::Escape);
        assert!(!app.catalog.song_menu_open(id));
        assert_eq!(app.catalog.query, "The Paper Trails");
        assert!(!app.catalog.hits().is_empty());
    }

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

        app.track_scroll = 104.0;
        let _ = app.update_catalog(CatalogAction::Review);
        let review = app.review.as_ref().unwrap();
        assert!(app.reviewing_catalog_album());
        assert_eq!(app.track_scroll, 104.0);
        assert_eq!(
            review.inspection.title,
            "Room to breathe (Expanded Edition)"
        );
        assert_eq!(review.inspection.tracks.len(), 20);
        assert_eq!(review.selected.len(), 20);
        let identity = review.identity();

        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.search_home());
        assert!(!app.can_export());
        let _ = app.update_catalog(CatalogAction::ReturnToReview);
        assert!(app.reviewing_catalog_album());
        assert_eq!(app.review.as_ref().unwrap().identity(), identity);
        assert_eq!(app.track_scroll, 104.0);
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
        app.catalog.pending_origin =
            Some(app.catalog.position(song_link.clone(), app.track_scroll));
        let _ = app.catalog_finished(Ok(CatalogEvent::Album(Box::new(album))));
        assert_eq!(app.catalog.back_label(), "Back to results");
        assert_eq!(app.catalog.scroll_y, 0.0);
        assert_eq!(app.catalog.history[0].position.focus, song_link);

        app.track_scroll = 128.0;
        let credit_link = artist_credit_id(0);
        app.catalog.pending_origin =
            Some(app.catalog.position(credit_link.clone(), app.track_scroll));
        let _ = app.catalog_finished(Ok(CatalogEvent::Artist(artist)));
        assert_eq!(app.catalog.back_label(), "Back to album");
        assert_eq!(app.catalog.history[1].position.focus, credit_link);
        let _ = app.update_catalog(CatalogAction::Back);
        assert!(app.catalog.album().is_some());
        assert_eq!(app.track_scroll, 128.0);

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
            while app.catalog_job.is_some() {
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
        app.catalog.pending_origin =
            Some(app.catalog.position(artist_credit_id(0), app.track_scroll));
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

    #[test]
    fn submitting_search_terms_skips_the_debounce() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        let _ = app.update_catalog(CatalogAction::Query("AC/DC: Live".into()));
        assert!(app.catalog.search_due.is_some());
        assert_eq!(app.catalog.searched, "The Paper Trails");

        let _ = app.update_catalog(CatalogAction::Submit("AC/DC: Live".into()));
        assert_eq!(app.catalog.searched, "AC/DC: Live");
        assert!(app.catalog.search_due.is_none());
    }

    #[test]
    fn a_shared_link_waits_for_submission_then_opens_the_scope_picker() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");
        let link = "https://music.youtube.com/watch?v=fixture0001&list=PLSample123456";
        let _ = app.update_catalog(CatalogAction::Query("Paper Trails live".into()));
        let _ = app.update_catalog(CatalogAction::Query(link.into()));
        let _ = app.update_catalog(CatalogAction::Filter(Some(SearchKind::Albums)));
        let _ = app.update_catalog(CatalogAction::Search);
        assert!(app.catalog.search_due.is_none());
        assert_eq!(app.catalog.searched, "The Paper Trails");
        assert!(app.catalog.visible);
        assert_ne!(app.url, link);

        let _ = app.update_catalog(CatalogAction::Submit(link.into()));
        assert_eq!(app.url, link);
        assert_eq!(app.panel, Panel::Workspace);
        assert!(!app.catalog.visible);
        assert!(app.intake.is_none());
    }

    #[test]
    fn unsupported_links_stay_in_the_search_field_with_an_error() {
        let (mut app, _) = App::new();
        app.loading = false;
        app.catalog_snapshot("catalog-results");

        for link in ["https://example.com/album", "youtu.be/fixture0001"] {
            let _ = app.update_catalog(CatalogAction::Query(link.into()));
            assert!(app.catalog.search_due.is_none());
            assert!(app.catalog.error.is_none());

            let _ = app.update_catalog(CatalogAction::Submit(link.into()));
            assert_eq!(app.catalog.query, link);
            assert!(app.catalog.visible);
            assert!(app.catalog.error.is_some());
            assert_eq!(app.catalog.searched, "The Paper Trails");
        }

        let _ = app.update_catalog(CatalogAction::ClearSearch);
        assert!(app.catalog.query.is_empty());
        assert!(app.catalog.error.is_none());
    }

    #[test]
    fn a_link_submitted_during_search_does_not_apply_to_later_edits() {
        let (mut app, _) = App::new();
        app.catalog_snapshot("catalog-results");
        app.catalog.pending = Some(CatalogPending::Search);
        let link = "https://music.youtube.com/watch?v=fixture0001&list=PLSample123456";
        let _ = app.update_catalog(CatalogAction::Query(link.into()));
        let _ = app.update_catalog(CatalogAction::Submit(link.into()));
        let submitted = app.catalog.queued_action.take().expect("queued submission");

        app.catalog.pending = None;
        app.loading = false;
        let next_link = "https://music.youtube.com/watch?v=fixture0002&list=PLSample123456";
        let _ = app.update_catalog(CatalogAction::Query(next_link.into()));
        let _ = app.update_catalog(submitted);
        assert_eq!(app.catalog.query, next_link);
        assert_ne!(app.url, next_link);
        assert!(app.catalog.visible);
    }
}
