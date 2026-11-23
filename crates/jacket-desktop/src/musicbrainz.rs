//! Public release search and edition lookup from MusicBrainz.
use audio_engine::Cancellation;
use jacket_core::{AlbumRelease, ReleaseTrack, SourceLink};
use reqwest::blocking::Client;
use serde::{de::DeserializeOwned, Deserialize};
use std::io::Read;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const PAGE: usize = 24;
const MAX_BYTES: u64 = 8 * 1024 * 1024;
static NEXT_REQUEST: Mutex<Option<Instant>> = Mutex::new(None);

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Album {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub first_release_date: Option<String>,
    pub disambiguation: Option<String>,
    #[serde(default)]
    pub primary_type: Option<String>,
    #[serde(default)]
    pub secondary_types: Vec<String>,
    #[serde(default)]
    artist_credit: Vec<Credit>,
}

impl Album {
    pub fn artist(&self) -> String {
        artist(&self.artist_credit)
    }
}

#[derive(Clone, Debug)]
pub struct SearchResults {
    pub albums: Vec<Album>,
    pub next_offset: Option<usize>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Edition {
    pub id: String,
    pub title: String,
    pub date: Option<String>,
    pub country: Option<String>,
    pub disambiguation: Option<String>,
    #[serde(default)]
    media: Vec<Medium>,
}

impl Edition {
    pub fn display_title(&self) -> String {
        if let Some(note) = self
            .disambiguation
            .as_deref()
            .map(str::trim)
            .filter(|note| !note.is_empty())
        {
            if !self.title.to_lowercase().contains(&note.to_lowercase()) {
                return format!("{} ({note})", self.title);
            }
        }
        self.title.clone()
    }

    pub fn description(&self) -> String {
        let count: u64 = self
            .media
            .iter()
            .map(|medium| u64::from(medium.track_count))
            .sum();
        let format = self
            .media
            .first()
            .and_then(|medium| medium.format.as_deref());
        [
            self.date.clone().filter(|date| !date.is_empty()),
            format.map(str::to_owned),
            (self.media.len() > 1).then(|| format!("{} discs", self.media.len())),
            (count > 0).then(|| {
                if count == 1 {
                    "1 track".to_owned()
                } else {
                    format!("{count} tracks")
                }
            }),
            self.country.clone().filter(|country| country != "XW"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ")
    }

    fn is_digital(&self) -> bool {
        !self.media.is_empty()
            && self
                .media
                .iter()
                .all(|medium| medium.format.as_deref() == Some("Digital Media"))
    }
}

#[derive(Clone, Debug)]
pub struct Release {
    pub edition: Edition,
    pub album: AlbumRelease,
    pub cover: Option<String>,
    pub sources: Vec<String>,
    pub stores: Vec<StoreLink>,
}

#[derive(Clone, Debug)]
pub struct StoreLink {
    pub provider: &'static str,
    pub url: String,
}

#[derive(Clone, Debug, Deserialize)]
struct Credit {
    name: String,
    #[serde(default)]
    joinphrase: String,
}

fn artist(credits: &[Credit]) -> String {
    credits
        .iter()
        .flat_map(|credit| [&*credit.name, &*credit.joinphrase])
        .collect()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Medium {
    format: Option<String>,
    #[serde(default)]
    position: u32,
    #[serde(default)]
    track_count: u32,
    #[serde(default)]
    tracks: Vec<CatalogTrack>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CatalogTrack {
    title: String,
    position: u32,
    length: Option<u64>,
    #[serde(default)]
    artist_credit: Vec<Credit>,
    recording: Recording,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Recording {
    length: Option<u64>,
    #[serde(default)]
    artist_credit: Vec<Credit>,
    #[serde(default)]
    relations: Vec<Relation>,
}

#[derive(Clone, Debug, Deserialize)]
struct Relation {
    #[serde(rename = "type")]
    kind: String,
    url: Option<Resource>,
    #[serde(default)]
    ended: bool,
}

#[derive(Clone, Debug, Deserialize)]
struct Resource {
    resource: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct SearchResponse {
    count: usize,
    release_groups: Vec<Album>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct EditionsResponse {
    release_count: usize,
    releases: Vec<Edition>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct EditionResponse {
    #[serde(flatten)]
    edition: Edition,
    #[serde(default)]
    artist_credit: Vec<Credit>,
    #[serde(default)]
    relations: Vec<Relation>,
    cover_art_archive: Archive,
}

#[derive(Deserialize)]
struct Archive {
    front: bool,
}

pub(crate) fn search(
    query: &str,
    offset: usize,
    cancel: &Cancellation,
) -> Result<SearchResults, String> {
    let query = query.trim();
    if query.is_empty() || query.chars().count() > 200 {
        return Err("Enter an album or artist name up to 200 characters.".into());
    }
    let quote = |text: &str| text.replace('\\', "\\\\").replace('"', "\\\"");
    let phrase = quote(query);
    let terms = query
        .split_whitespace()
        .take(24)
        .map(|word| {
            let word = quote(word);
            format!("(releasegroup:\"{word}\" OR artist:\"{word}\")")
        })
        .collect::<Vec<_>>()
        .join(" AND ");
    let expression = format!("(releasegroup:\"{phrase}\"^6 OR artist:\"{phrase}\"^4 OR ({terms})) AND (primarytype:album OR primarytype:ep)");
    let mut response: SearchResponse = request(
        "release-group",
        &[
            ("query", &expression),
            ("limit", &PAGE.to_string()),
            ("offset", &offset.to_string()),
        ],
        cancel,
    )?;
    prioritize_exact_search_matches(query, &mut response.release_groups);
    let next = offset + response.release_groups.len();
    Ok(SearchResults {
        next_offset: (next > offset && next < response.count).then_some(next),
        albums: response.release_groups,
    })
}

fn prioritize_exact_search_matches(query: &str, albums: &mut [Album]) {
    let requested = search_key(query);
    albums.sort_by_cached_key(|album| {
        let title = search_key(&album.title);
        let artist = search_key(&album.artist());
        // A longer result can include every search term, such as a tribute.
        requested != title
            && requested != format!("{artist}{title}")
            && requested != format!("{title}{artist}")
    });
}

fn search_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

pub(crate) fn editions(
    id: &str,
    group: &Album,
    cancel: &Cancellation,
) -> Result<Vec<Edition>, String> {
    validate_id(id)?;
    let mut editions = Vec::new();
    // Paginate by the response count. MusicBrainz can cap a page at 500 tracks.
    while editions.len() < 300 {
        let response: EditionsResponse = request(
            "release",
            &[
                ("release-group", id),
                ("status", "official"),
                ("inc", "media"),
                ("limit", "25"),
                ("offset", &editions.len().to_string()),
            ],
            cancel,
        )?;
        if response.releases.is_empty() {
            break;
        }
        editions.extend(response.releases);
        if editions.len() >= response.release_count {
            break;
        }
    }
    editions.sort_by(|left, right| {
        let order = |edition: &Edition| {
            (
                u8::from(!edition.is_digital()),
                u8::from(
                    year(edition.date.as_deref()) != year(group.first_release_date.as_deref()),
                ),
                u8::from(edition.title != group.title),
                u8::from(
                    edition
                        .disambiguation
                        .as_deref()
                        .is_some_and(|note| !note.trim().is_empty()),
                ),
                u8::from(edition.country.as_deref() != Some("XW")),
            )
        };
        order(left)
            .cmp(&order(right))
            .then_with(|| left.date.is_none().cmp(&right.date.is_none()))
            .then_with(|| left.date.cmp(&right.date))
            .then_with(|| left.title.cmp(&right.title))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(editions)
}

pub(crate) fn release(id: &str, cancel: &Cancellation) -> Result<Release, String> {
    validate_id(id)?;
    let response: EditionResponse = request(
        &format!("release/{id}"),
        &[(
            "inc",
            "artist-credits+recordings+url-rels+recording-level-rels",
        )],
        cancel,
    )?;
    release_from_response(response)
}

fn release_from_response(response: EditionResponse) -> Result<Release, String> {
    let EditionResponse {
        edition,
        artist_credit,
        relations,
        cover_art_archive,
    } = response;
    validate_id(&edition.id)?;
    let album_artist = artist(&artist_credit);
    let mut tracks = Vec::new();
    for medium in &edition.media {
        for track in &medium.tracks {
            let track_artist = artist(&track.artist_credit);
            let credited = if track_artist.is_empty() {
                artist(&track.recording.artist_credit)
            } else {
                track_artist
            };
            tracks.push(ReleaseTrack {
                title: track.title.clone(),
                artist: if credited.is_empty() {
                    album_artist.clone()
                } else {
                    credited
                },
                disc: medium.position,
                number: track.position,
                duration_ms: track.length.or(track.recording.length),
                source_urls: track
                    .recording
                    .relations
                    .iter()
                    .filter_map(link)
                    .map(str::to_owned)
                    .collect(),
            });
        }
    }
    tracks.sort_by_key(|track| (track.disc, track.number));
    if tracks.is_empty() {
        return Err(
            "MusicBrainz has no track list for this edition. Choose another edition.".into(),
        );
    }
    if edition
        .media
        .iter()
        .any(|medium| medium.track_count as usize != medium.tracks.len())
        || tracks
            .iter()
            .any(|track| track.disc == 0 || track.number == 0 || track.title.trim().is_empty())
        || tracks
            .windows(2)
            .any(|pair| (pair[0].disc, pair[0].number) == (pair[1].disc, pair[1].number))
    {
        return Err(
            "MusicBrainz has an incomplete track list for this edition. Choose another edition."
                .into(),
        );
    }
    let mut sources = Vec::new();
    let mut stores: Vec<StoreLink> = Vec::new();
    for relation in &relations {
        let Some(url) = link(relation) else { continue };
        if let Some(url) = linked_album(url) {
            if !sources.contains(&url) {
                sources.push(url);
            }
        } else if relation.kind.starts_with("purchase for download") {
            if let Some(provider) = download_store(url) {
                if !stores.iter().any(|store| store.provider == provider) {
                    stores.push(StoreLink {
                        provider,
                        url: url.to_owned(),
                    });
                }
            }
        }
    }
    let cover = cover_art_archive.front.then(|| {
        format!(
            "https://coverartarchive.org/release/{}/front-500",
            edition.id
        )
    });
    Ok(Release {
        album: AlbumRelease {
            title: edition.display_title(),
            artist: album_artist,
            year: year(edition.date.as_deref()),
            tracks,
        },
        edition,
        cover,
        sources,
        stores,
    })
}

fn link(relation: &Relation) -> Option<&str> {
    relation
        .url
        .as_ref()
        .filter(|_| !relation.ended)
        .map(|url| url.resource.as_str())
}

fn linked_album(url: &str) -> Option<String> {
    let parsed = SourceLink::parse(url).ok()?;
    let link = parsed.playlist_url?;
    (link.starts_with("https://music.youtube.com/")
        && parsed.song_url.is_none()
        && audio_engine::source::collection_mode(&link) == audio_engine::CollectionMode::Album)
        .then_some(link)
}

pub(crate) fn download_store(url: &str) -> Option<&'static str> {
    let url = reqwest::Url::parse(url).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
    {
        return None;
    }
    let host = url.host_str()?;
    if host.ends_with(".bandcamp.com") && url.path().starts_with("/album/") {
        Some("Bandcamp")
    } else if matches!(host, "www.qobuz.com" | "qobuz.com") && url.path().contains("/album/") {
        Some("Qobuz")
    } else {
        None
    }
}

fn validate_id(id: &str) -> Result<(), String> {
    if id.len() != 36
        || !id.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
    {
        return Err("MusicBrainz returned an invalid release identifier.".into());
    }
    Ok(())
}

fn year(date: Option<&str>) -> Option<u32> {
    date?
        .get(..4)?
        .parse()
        .ok()
        .filter(|value| (1000..=9999).contains(value))
}

fn request<T: DeserializeOwned>(
    path: &str,
    params: &[(&str, &str)],
    cancel: &Cancellation,
) -> Result<T, String> {
    let client = Client::builder()
        .user_agent(concat!(
            "Jacket/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/MoonlarkStudios/jacket)"
        ))
        .connect_timeout(Duration::from_secs(4))
        .timeout(Duration::from_secs(8))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|_| "Could not start a MusicBrainz request.")?;
    let send = || {
        wait_for_request(cancel)?;
        let response = client
            .get(format!("https://musicbrainz.org/ws/2/{path}"))
            .query(&[("fmt", "json")])
            .query(params)
            .send()
            .map_err(|_| "Could not reach MusicBrainz. Check your connection and try again.")?;
        cancel.check().map_err(|error| error.to_string())?;
        Ok::<_, String>(response)
    };
    let initial = send()?;
    let response = if initial.status() == reqwest::StatusCode::SERVICE_UNAVAILABLE
        && !initial.headers().contains_key(reqwest::header::RETRY_AFTER)
    {
        drop(initial);
        send()?
    } else {
        initial
    };
    match response.status().as_u16() {
        200 => (),
        429 | 503 => return Err("MusicBrainz is busy. Wait a moment and try again.".into()),
        404 => return Err("MusicBrainz could not find that edition.".into()),
        _ => return Err("MusicBrainz could not complete this request. Try again.".into()),
    }
    let mut bytes = Vec::new();
    response
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "MusicBrainz returned an incomplete response. Try again.")?;
    cancel.check().map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("MusicBrainz returned more data than Jacket can display.".into());
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| "MusicBrainz returned unexpected release data.".into())
}

fn wait_for_request(cancel: &Cancellation) -> Result<(), String> {
    let mut last = NEXT_REQUEST
        .lock()
        .map_err(|_| "MusicBrainz request scheduling failed.")?;
    while last.is_some_and(|time| time.elapsed() < Duration::from_secs(1)) {
        cancel.check().map_err(|error| error.to_string())?;
        std::thread::sleep(Duration::from_millis(50));
    }
    cancel.check().map_err(|error| error.to_string())?;
    *last = Some(Instant::now());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        download_store, linked_album, prioritize_exact_search_matches, release_from_response, Album,
    };
    use serde_json::json;

    #[test]
    fn the_exact_artist_and_title_rank_ahead_of_a_tribute_album() {
        let albums: Vec<Album> = serde_json::from_value(json!([
            {"id":"tribute","title":"String Quartet Performs the Band's An Album",
             "artist-credit":[{"name":"String Quartet"}]},
            {"id":"original","title":"An Album","artist-credit":[{"name":"The Band"}]},
        ]))
        .unwrap();
        for query in ["The Band An Album", "An Album The Band"] {
            let mut albums = albums.clone();
            prioritize_exact_search_matches(query, &mut albums);
            assert_eq!(albums[0].id, "original");
        }
    }

    #[test]
    fn a_release_preserves_track_credit_and_only_exposes_eligible_source_links() {
        let response = serde_json::from_value(json!({
            "id":"e65fcfb0-02dc-407c-ab28-8168dafcb999",
            "title":"An album",
            "disambiguation":"expanded edition",
            "date":"2023-05-12",
            "country":"XW",
            "artist-credit":[{"name":"Artist","joinphrase":""}],
            "cover-art-archive":{"front":true},
            "relations":[
                {"type":"free streaming","url":{"resource":"https://music.youtube.com/playlist?list=OLAK5uy_fixture123"}},
                {"type":"free streaming","url":{"resource":"https://music.youtube.com/playlist?list=PLother123456"}},
                {"type":"purchase for download","url":{"resource":"https://artist.bandcamp.com/album/extended"}},
                {"type":"purchase for download","ended":true,"url":{"resource":"https://www.qobuz.com/us-en/album/extended/123"}}
            ],
            "media":[
                {"position":2,"format":"Digital Media","track-count":1,
                 "tracks":[{"title":"Extra","position":1,"length":193000,
                            "artist-credit":[{"name":"Artist","joinphrase":" feat. "},{"name":"Guest"}],
                            "recording":{"length":194000,
                                         "relations":[{"type":"free streaming","url":{"resource":"https://youtu.be/fixture0001"}}]}}]},
                {"position":1,"format":"Digital Media","track-count":1,
                 "tracks":[{"title":"Prelude","position":1,"recording":{"length":98000}}]}
            ]
        })).unwrap();
        let release = release_from_response(response).unwrap();
        assert_eq!(release.album.title, "An album (expanded edition)");
        assert_eq!(release.album.year, Some(2023));
        assert_eq!(release.album.tracks[0].title, "Prelude");
        assert_eq!(release.album.tracks[0].artist, "Artist");
        assert_eq!(release.album.tracks[0].duration_ms, Some(98_000));
        assert_eq!(release.album.tracks[1].disc, 2);
        assert_eq!(release.album.tracks[1].artist, "Artist feat. Guest");
        assert_eq!(release.album.tracks[1].duration_ms, Some(193_000));
        assert_eq!(
            release.album.tracks[1].source_urls,
            ["https://youtu.be/fixture0001"]
        );
        assert_eq!(
            release.sources,
            ["https://music.youtube.com/playlist?list=OLAK5uy_fixture123"]
        );
        assert_eq!(release.stores.len(), 1);
        assert_eq!(release.stores[0].provider, "Bandcamp");
        assert!(release.cover.unwrap().ends_with("/front-500"));
    }

    #[test]
    fn catalog_links_cannot_inject_a_command_or_substitute_an_unrelated_provider() {
        for url in [
            "file:///tmp/album",
            "http://artist.bandcamp.com/album/extended",
            "https://artist.bandcamp.com.invalid/album/extended",
            "https://user@artist.bandcamp.com/album/extended",
            "https://artist.bandcamp.com/track/extended",
        ] {
            assert_eq!(download_store(url), None, "{url}");
        }
        assert!(linked_album("https://www.youtube.com/playlist?list=OLAK5uy_fixture123").is_none());
        assert!(linked_album(
            "https://music.youtube.com/watch?v=fixture0001&list=OLAK5uy_fixture123"
        )
        .is_none());
        assert!(linked_album("https://music.youtube.com/browse/MPREb_fixture123").is_some());
    }
}
