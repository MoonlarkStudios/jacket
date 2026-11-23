//! Indexed search and artist-scoped catalog browsing.
use super::{request, validate_id, Album};
use crate::catalog::SearchKind;
use audio_engine::Cancellation;
use serde::Deserialize;
use std::collections::{HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

const PAGE_SIZE: usize = 24;
const MAX_RESULTS: usize = 240;
const ALBUM_TYPES: &str = "(primarytype:album^8 OR primarytype:ep^3 OR primarytype:single)";
const CACHE_CAPACITY: usize = 64;
const CACHE_LIFETIME: Duration = Duration::from_secs(15 * 60);

fn endpoint(kind: SearchKind) -> &'static str {
    match kind {
        SearchKind::Artists => "artist",
        SearchKind::Albums => "release-group",
        SearchKind::Songs => "recording",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SearchScope {
    Text(String),
    Artist(String),
}

#[derive(Clone, Debug)]
pub enum SearchHit {
    Artist(Artist),
    Album(Album),
    Song(Song),
}

impl SearchHit {
    pub fn id(&self) -> &str {
        match self {
            Self::Artist(artist) => &artist.id,
            Self::Album(album) => &album.id,
            Self::Song(song) => &song.id,
        }
    }

    pub fn cover_thumbnail(&self) -> Option<String> {
        match self {
            Self::Artist(_) => None,
            Self::Album(album) => album.cover_thumbnail(),
            Self::Song(song) => {
                let release = song.preferred_release()?;
                release
                    .release_group
                    .as_ref()
                    .and_then(Album::cover_thumbnail)
                    .or_else(|| super::cover_thumbnail("release", release.id.as_deref()?))
            }
        }
    }

    pub fn relevance(&self, query: &str) -> u8 {
        self.rank(&search_key(query))
    }

    fn rank(&self, query: &str) -> u8 {
        match self {
            Self::Artist(artist) => {
                if search_key(&artist.name) == query {
                    0
                } else if artist
                    .aliases
                    .iter()
                    .any(|alias| search_key(&alias.name) == query)
                {
                    1
                } else if is_prefix(query, &search_key(&artist.name)) {
                    2
                } else {
                    3
                }
            }
            Self::Album(album) => title_rank(
                query,
                &album.title,
                &album.artist(),
                album.disambiguation.as_deref(),
            ),
            Self::Song(song) => title_rank(
                query,
                &song.title,
                &song.artist(),
                song.disambiguation.as_deref(),
            ),
        }
    }
}

#[derive(Clone, Debug)]
pub struct SearchPage {
    pub hits: Vec<SearchHit>,
    pub next_offset: Option<usize>,
}

impl SearchPage {
    pub fn append(&mut self, next: Self) {
        let mut ids: HashSet<_> = self.hits.iter().map(|hit| hit.id().to_owned()).collect();
        self.hits.extend(
            next.hits
                .into_iter()
                .filter(|hit| ids.insert(hit.id().to_owned())),
        );
        self.next_offset = next.next_offset;
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Artist {
    pub id: String,
    pub name: String,
    pub disambiguation: Option<String>,
    #[serde(rename = "type")]
    pub kind: Option<String>,
    pub country: Option<String>,
    area: Option<Area>,
    #[serde(default)]
    aliases: Vec<Alias>,
}

impl Artist {
    pub fn location(&self) -> Option<&str> {
        self.area
            .as_ref()
            .map(|area| area.name.as_str())
            .or(self.country.as_deref())
    }
}

#[derive(Clone, Debug, Deserialize)]
struct Area {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Alias {
    name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Song {
    pub id: String,
    pub title: String,
    pub disambiguation: Option<String>,
    pub length: Option<u64>,
    pub first_release_date: Option<String>,
    #[serde(default)]
    artist_credit: Vec<super::Credit>,
    #[serde(default)]
    pub(crate) releases: Vec<SongRelease>,
}

impl Song {
    pub fn artist(&self) -> String {
        super::artist(&self.artist_credit)
    }

    pub fn album_hint(&self) -> Option<&str> {
        self.preferred_release().map(|release| {
            release
                .release_group
                .as_ref()
                .map_or(release.title.as_str(), |group| group.title.as_str())
        })
    }

    fn preferred_release(&self) -> Option<&SongRelease> {
        self.releases
            .iter()
            .filter(|release| {
                release
                    .status
                    .as_deref()
                    .is_none_or(|status| status == "Official")
            })
            .min_by_key(|release| {
                let group = release.release_group.as_ref();
                let album = group.and_then(|group| group.primary_type.as_deref()) == Some("Album");
                let compilation = group.is_some_and(|group| {
                    group
                        .secondary_types
                        .iter()
                        .any(|kind| kind == "Compilation")
                });
                (
                    compilation,
                    !album,
                    release.date.is_none(),
                    release.date.as_deref(),
                )
            })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) struct SongRelease {
    id: Option<String>,
    pub title: String,
    pub status: Option<String>,
    pub date: Option<String>,
    pub release_group: Option<Album>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SearchResponse {
    count: usize,
    #[serde(default)]
    artists: Vec<Artist>,
    #[serde(default)]
    release_groups: Vec<Album>,
    #[serde(default)]
    recordings: Vec<Song>,
}

#[derive(Default)]
pub(crate) struct SearchCache {
    entries: VecDeque<CachedPage>,
}

struct CachedPage {
    scope: SearchScope,
    kind: SearchKind,
    offset: usize,
    fetched: Instant,
    page: SearchPage,
}

impl SearchCache {
    fn get(
        &mut self,
        scope: &SearchScope,
        kind: SearchKind,
        offset: usize,
        now: Instant,
    ) -> Option<SearchPage> {
        let index = self.entries.iter().position(|entry| {
            &entry.scope == scope && entry.kind == kind && entry.offset == offset
        })?;
        let entry = self.entries.remove(index)?;
        if now.duration_since(entry.fetched) >= CACHE_LIFETIME {
            return None;
        }
        let page = entry.page.clone();
        self.entries.push_back(entry);
        Some(page)
    }

    fn insert(&mut self, entry: CachedPage) {
        if self.entries.len() == CACHE_CAPACITY {
            self.entries.pop_front();
        }
        self.entries.push_back(entry);
    }
}

pub(crate) fn search(
    cache: &Mutex<SearchCache>,
    scope: &SearchScope,
    kind: SearchKind,
    offset: usize,
    cancel: &Cancellation,
) -> Result<SearchPage, String> {
    cancel.check().map_err(|error| error.to_string())?;
    if let Ok(mut cache) = cache.lock() {
        if let Some(page) = cache.get(scope, kind, offset, Instant::now()) {
            return Ok(page);
        }
    }
    let page = fetch(scope, kind, offset, cancel)?;
    if let Ok(mut cache) = cache.lock() {
        cache.insert(CachedPage {
            scope: scope.clone(),
            kind,
            offset,
            fetched: Instant::now(),
            page: page.clone(),
        });
    }
    Ok(page)
}

fn fetch(
    scope: &SearchScope,
    kind: SearchKind,
    offset: usize,
    cancel: &Cancellation,
) -> Result<SearchPage, String> {
    let expression = expression(scope, kind)?;
    let limit = PAGE_SIZE.min(MAX_RESULTS.saturating_sub(offset));
    if limit == 0 {
        return Ok(SearchPage {
            hits: Vec::new(),
            next_offset: None,
        });
    }
    let response: SearchResponse = request(
        endpoint(kind),
        &[
            ("query", &expression),
            ("limit", &limit.to_string()),
            ("offset", &offset.to_string()),
        ],
        cancel,
    )?;
    let hits = match kind {
        SearchKind::Artists => response
            .artists
            .into_iter()
            .map(SearchHit::Artist)
            .collect::<Vec<_>>(),
        SearchKind::Albums => response
            .release_groups
            .into_iter()
            .map(SearchHit::Album)
            .collect(),
        SearchKind::Songs => response
            .recordings
            .into_iter()
            .map(SearchHit::Song)
            .collect(),
    };
    let next = offset.saturating_add(hits.len());
    let mut page = SearchPage {
        hits: Vec::new(),
        next_offset: None,
    };
    page.append(SearchPage {
        hits,
        next_offset: (next > offset && next < response.count.min(MAX_RESULTS)).then_some(next),
    });
    match scope {
        SearchScope::Text(query) => prioritize(query, &mut page.hits),
        SearchScope::Artist(_) if kind == SearchKind::Albums => {
            page.hits.sort_by_cached_key(|hit| match hit {
                SearchHit::Album(album) => (
                    album.primary_type.as_deref() != Some("Album"),
                    !album.secondary_types.is_empty(),
                    std::cmp::Reverse(album.first_release_date.clone()),
                ),
                _ => (true, true, std::cmp::Reverse(None)),
            });
        }
        SearchScope::Artist(_) => prioritize("", &mut page.hits),
    }
    Ok(page)
}

fn expression(scope: &SearchScope, kind: SearchKind) -> Result<String, String> {
    let query = match scope {
        SearchScope::Text(query) => query.trim(),
        SearchScope::Artist(id) => {
            validate_id(id)?;
            return match kind {
                SearchKind::Artists => Err("Choose Albums or Songs to browse an artist.".into()),
                SearchKind::Albums => {
                    Ok(format!("arid:{id} AND {ALBUM_TYPES} AND status:official"))
                }
                SearchKind::Songs => Ok(format!("arid:{id} AND -video:true")),
            };
        }
    };
    if query.chars().count() > 200 || !query.chars().any(char::is_alphanumeric) {
        return Err("Enter an artist, album, or song name up to 200 characters.".into());
    }
    let quoted = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let phrase = quoted(&query.to_lowercase());
    let fields: &[&str] = match kind {
        SearchKind::Artists => &["artist", "alias"],
        SearchKind::Albums => &["releasegroup", "artist", "alias", "comment"],
        SearchKind::Songs => &["recording", "artist", "alias", "comment"],
    };
    let alternatives = fields
        .iter()
        .zip([6, 4, 2, 1])
        .map(|(field, boost)| format!("{field}:\"{phrase}\"^{boost}"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let words: Vec<_> = query
        .split_whitespace()
        .filter(|word| word.chars().any(char::is_alphanumeric))
        .take(24)
        .collect();
    let terms = words
        .iter()
        .enumerate()
        .map(|(index, &word)| {
            let exact = quoted(word);
            let lower = word.to_lowercase();
            let count = word.chars().count();
            let plain = word.chars().all(char::is_alphanumeric);
            let last = index + 1 == words.len();
            let prefix = plain && last && (count >= 3 || (count == 2 && words.len() > 1));
            let fuzzy = plain && count >= 5 && words.len() <= 6;
            let token = if prefix {
                format!("{lower}*")
            } else if fuzzy {
                format!("{lower}~1")
            } else if plain {
                lower.clone()
            } else {
                format!("\"{exact}\"")
            };
            // Keep partial queries small. Phrase clauses above still use
            // aliases and version comments, and longer searches may combine them.
            let primary = if kind == SearchKind::Artists { 1 } else { 2 };
            let mut clauses: Vec<_> = fields[..primary]
                .iter()
                .map(|field| format!("{field}:{token}"))
                .collect();
            if prefix && fuzzy {
                clauses.push(format!("{}:{lower}~1", fields[0]));
            }
            if words.len() > 2 {
                clauses.extend(
                    fields[primary..]
                        .iter()
                        .map(|field| format!("{field}:\"{exact}\"")),
                );
            }
            if clauses.len() == 1 {
                clauses
                    .pop()
                    .expect("the primary search field supplies a term")
            } else {
                format!("({})", clauses.join(" OR "))
            }
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let filter = match kind {
        SearchKind::Artists => "",
        SearchKind::Albums => " AND (primarytype:album OR primarytype:ep OR primarytype:single)",
        SearchKind::Songs => " AND -video:true",
    };
    Ok(format!("({alternatives} OR ({terms})){filter}"))
}

fn prioritize(query: &str, hits: &mut [SearchHit]) {
    let query = search_key(query);
    hits.sort_by_cached_key(|hit| {
        let credits = match hit {
            SearchHit::Song(song) => song
                .releases
                .iter()
                .filter(|release| release.status.as_deref() == Some("Official"))
                .count(),
            _ => 0,
        };
        let rank = hit.rank(&query);
        let length = match hit {
            SearchHit::Artist(artist) if rank == 2 => artist.name.chars().count(),
            _ => 0,
        };
        (rank, std::cmp::Reverse(credits), length)
    });
}

fn title_rank(query: &str, title: &str, artist: &str, note: Option<&str>) -> u8 {
    let title = search_key(title);
    let artist = search_key(artist);
    let note = note.map(search_key).unwrap_or_default();
    if (!note.is_empty()
        && (query == format!("{artist}{title}{note}")
            || query == format!("{title}{note}{artist}")
            || query == format!("{title}{note}")))
        || query == format!("{artist}{title}")
        || query == format!("{title}{artist}")
    {
        0
    } else if query == title {
        1
    } else if is_prefix(query, &title)
        || (query.len() > artist.len() && is_prefix(query, &format!("{artist}{title}")))
        || (query.len() > title.len() && is_prefix(query, &format!("{title}{artist}")))
    {
        2
    } else {
        3
    }
}

fn is_prefix(query: &str, value: &str) -> bool {
    query.chars().take(5).count() == 5 && value.starts_with(query)
}

fn search_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        expression, prioritize, search, CachedPage, SearchCache, SearchHit, SearchKind, SearchPage,
        SearchScope, CACHE_CAPACITY, CACHE_LIFETIME,
    };
    use audio_engine::Cancellation;
    use serde_json::json;
    use std::{sync::Mutex, time::Instant};

    #[test]
    fn search_reuses_successful_pages_for_the_same_scope_kind_and_offset() {
        let mut cache = SearchCache::default();
        let scope = SearchScope::Artist("cached".into());
        let kind = SearchKind::Songs;
        let now = Instant::now();
        cache.insert(CachedPage {
            scope: scope.clone(),
            kind,
            offset: 24,
            fetched: now,
            page: SearchPage {
                hits: vec![],
                next_offset: Some(48),
            },
        });
        assert!(cache.get(&scope, kind, 0, now).is_none());
        assert!(cache.get(&scope, SearchKind::Albums, 24, now).is_none());
        assert!(cache
            .get(&SearchScope::Text("cached".into()), kind, 24, now)
            .is_none());
        let cache = Mutex::new(cache);
        let cancel = Cancellation::default();
        // This fixture has no valid artist ID, so it can only succeed from the cache.
        assert_eq!(
            search(&cache, &scope, kind, 24, &cancel)
                .unwrap()
                .next_offset,
            Some(48)
        );
        cancel.cancel();
        assert!(search(&cache, &scope, kind, 24, &cancel).is_err());
        assert!(cache
            .lock()
            .unwrap()
            .get(&scope, kind, 24, now + CACHE_LIFETIME)
            .is_none());
    }

    #[test]
    fn cache_evicts_the_least_recently_used_page_at_capacity() {
        let mut cache = SearchCache::default();
        let scope = SearchScope::Text("Artist".into());
        let kind = SearchKind::Albums;
        let now = Instant::now();
        let entry = |offset| CachedPage {
            scope: scope.clone(),
            kind,
            offset,
            fetched: now,
            page: SearchPage {
                hits: vec![],
                next_offset: None,
            },
        };
        for offset in 0..CACHE_CAPACITY {
            cache.insert(entry(offset));
        }
        assert!(cache.get(&scope, kind, 0, now).is_some());
        cache.insert(entry(CACHE_CAPACITY));
        assert!(cache.get(&scope, kind, 1, now).is_none());
        assert!(cache.get(&scope, kind, 0, now).is_some());
        assert_eq!(cache.entries.len(), CACHE_CAPACITY);
    }

    #[test]
    fn exact_credited_song_and_album_beat_title_overlap() {
        for kind in [SearchKind::Albums, SearchKind::Songs] {
            let hit = |value| match kind {
                SearchKind::Albums => SearchHit::Album(serde_json::from_value(value).unwrap()),
                SearchKind::Songs => SearchHit::Song(serde_json::from_value(value).unwrap()),
                _ => unreachable!(),
            };
            let mut hits = [
                hit(json!({"id":"cover","title":"The Band's An Album",
                           "artist-credit":[{"name":"String Quartet"}]})),
                hit(json!({"id":"original","title":"An Album",
                           "artist-credit":[{"name":"The Band"}]})),
            ];
            prioritize("The Band An Album", &mut hits);
            assert_eq!(hits[0].id(), "original");
        }
    }

    #[test]
    fn pages_remove_duplicate_ids_without_skipping_the_next_provider_offset() {
        let hit = |id| {
            SearchHit::Song(serde_json::from_value(json!({"id":id,"title":"A song"})).unwrap())
        };
        let mut page = SearchPage {
            hits: vec![hit("one")],
            next_offset: Some(24),
        };
        page.append(SearchPage {
            hits: vec![hit("one"), hit("two"), hit("two")],
            next_offset: Some(48),
        });
        assert_eq!(
            page.hits.iter().map(SearchHit::id).collect::<Vec<_>>(),
            ["one", "two"]
        );
        assert_eq!(page.next_offset, Some(48));
    }

    #[test]
    fn recording_thumbnail_uses_the_same_album_as_its_label() {
        let original = "50de827a-6631-4099-bd85-23dd6cb69d66";
        let song: super::Song = serde_json::from_value(json!({
            "id": "song",
            "title": "Recording",
            "releases": [
                {"title":"Hits","date":"1990","release-group":{
                    "id":"6c9ecc1f-ce5a-40ba-ac4c-afef3ab7a2d9","title":"Hits",
                    "primary-type":"Album","secondary-types":["Compilation"]
                }},
                {"title":"Original","date":"2000","release-group":{
                    "id":original,"title":"Original","primary-type":"Album"
                }}
            ]
        }))
        .unwrap();
        assert_eq!(song.album_hint(), Some("Original"));
        assert_eq!(
            SearchHit::Song(song).cover_thumbnail(),
            Some(format!(
                "https://coverartarchive.org/release-group/{original}/front-250"
            ))
        );
    }

    #[test]
    fn a_shorter_artist_name_wins_over_a_tribute_with_the_same_prefix() {
        let artist = |id, name| {
            SearchHit::Artist(serde_json::from_value(json!({"id":id,"name":name})).unwrap())
        };
        let mut hits = [
            artist("tribute", "Daft Punk Tribute"),
            artist("original", "Daft Punk"),
        ];
        prioritize("Daft Pun", &mut hits);
        assert_eq!(hits[0].id(), "original");
        assert_eq!(hits[0].relevance("Daft Pun"), 2);
    }

    #[test]
    fn a_short_artist_prefix_uses_only_name_terms_and_exact_aliases() {
        let query = expression(&SearchScope::Text("Daft Pun".into()), SearchKind::Artists).unwrap();
        assert_eq!(
            query,
            r#"(artist:"daft pun"^6 OR alias:"daft pun"^4 OR (artist:daft AND artist:pun*))"#
        );
    }

    #[test]
    fn search_expands_plain_prefixes_and_one_edit_misspellings() {
        let query = expression(
            &SearchScope::Text("RadIohed CRee".into()),
            SearchKind::Songs,
        )
        .unwrap();
        assert!(query.contains("artist:radiohed~1"));
        assert!(query.contains("recording:cree*"));
        assert!(!query.contains("artist:radiohed*"));
        let punctuation =
            expression(&SearchScope::Text("album:foo".into()), SearchKind::Albums).unwrap();
        assert!(!punctuation.contains("album:foo*"));
    }

    #[test]
    fn search_literals_remain_quoted_and_artist_browsing_uses_the_id() {
        let text = expression(
            &SearchScope::Text("Title \"part\"".into()),
            SearchKind::Songs,
        )
        .unwrap();
        assert!(text.contains(r#"recording:"title \"part\"""#));
        let id = "50de827a-6631-4099-bd85-23dd6cb69d66";
        let artist = SearchScope::Artist(id.into());
        assert_eq!(
            expression(&artist, SearchKind::Songs).unwrap(),
            format!("arid:{id} AND -video:true")
        );
        assert!(expression(&artist, SearchKind::Artists).is_err());
    }
}
