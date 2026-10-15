//! Optional public album metadata from YouTube Music. This supplements music
//! tags; video upload dates and copyright notices are never release years.
use crate::{Cancellation, CollectionMode, Inspection};
use serde_json::{json, Value};
use std::{io::Read, time::Duration};

pub(crate) struct AlbumYear {
    title: String,
    year: u32,
}

fn primary_contents(value: &Value) -> Option<&Value> {
    value
        .pointer("/contents/twoColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
        .or_else(|| value.pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents"))
}

fn header(value: &Value) -> Option<&Value> {
    value
        .pointer("/header/musicDetailHeaderRenderer")
        .or_else(|| {
            primary_contents(value)?
                .as_array()?
                .iter()
                .find_map(|item| {
                    item.get("musicResponsiveHeaderRenderer")
                        .or_else(|| item.get("musicDetailHeaderRenderer"))
                })
        })
}

fn text(value: &Value) -> String {
    value
        .get("runs")
        .and_then(Value::as_array)
        .map(|runs| {
            runs.iter()
                .filter_map(|run| run.get("text").and_then(Value::as_str))
                .collect::<String>()
        })
        .unwrap_or_default()
}

fn header_year(value: &Value) -> Option<AlbumYear> {
    let header = header(value)?;
    let title = text(header.get("title")?).trim().to_owned();
    let years: Vec<u32> = header
        .pointer("/subtitle/runs")?
        .as_array()?
        .iter()
        .filter_map(|run| {
            let value = run.get("text")?.as_str()?.trim();
            (value.len() == 4 && value.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.parse::<u32>().ok())
                .flatten()
                .filter(|year| (1000..=9999).contains(year))
        })
        .collect();
    if title.is_empty() || years.len() != 1 {
        return None;
    }
    Some(AlbumYear {
        title,
        year: years[0],
    })
}

// Resolve only album links on the actual playlist's track rows, never links
// from recommendations, menus, or unrelated shelves. All rows must agree.
fn album_browse_id(value: &Value) -> Option<String> {
    let contents = value.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents")
        .or_else(|| primary_contents(value))?.as_array()?;
    let rows = contents
        .iter()
        .find_map(|section| {
            section
                .pointer("/musicPlaylistShelfRenderer/contents")
                .or_else(|| section.pointer("/musicShelfRenderer/contents"))
        })?
        .as_array()?;
    let mut ids = Vec::new();
    for row in rows {
        let Some(columns) = row
            .pointer("/musicResponsiveListItemRenderer/flexColumns")
            .and_then(Value::as_array)
        else {
            continue;
        };
        for column in columns {
            let Some(runs) = column
                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(Value::as_array)
            else {
                continue;
            };
            for run in runs {
                let Some(endpoint) = run.pointer("/navigationEndpoint/browseEndpoint") else {
                    continue;
                };
                if endpoint.pointer("/browseEndpointContextSupportedConfigs/browseEndpointContextMusicConfig/pageType").and_then(Value::as_str) == Some("MUSIC_PAGE_TYPE_ALBUM") {
                    let id = endpoint.get("browseId")?.as_str()?;
                    if !id.starts_with("MP") { return None; }
                    ids.push(id);
                }
            }
        }
    }
    let first = *ids.first()?;
    ids.iter().all(|id| *id == first).then(|| first.to_owned())
}

pub(crate) fn lookup(input: &str, cancel: &Cancellation) -> Option<AlbumYear> {
    let url = url::Url::parse(input).ok()?;
    let id = if url.path().starts_with("/browse/MP") {
        url.path().strip_prefix("/browse/")?.to_owned()
    } else {
        format!("VL{}", url.query_pairs().find(|(key, _)| key == "list")?.1)
    };
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(4))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .ok()?;
    let browse = |id: &str| -> Option<Value> {
        cancel.check().ok()?;
        let body = json!({"context":{"client":{"clientName":"WEB_REMIX", "clientVersion":"1.20261012.01.00", "hl":"en"}}, "browseId": id});
        let response = client
            .post("https://music.youtube.com/youtubei/v1/browse?prettyPrint=false")
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).ok()?)
            .send()
            .ok()?
            .error_for_status()
            .ok()?;
        let mut bytes = Vec::new();
        response
            .take(4 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        cancel.check().ok()?;
        if bytes.len() > 4 * 1024 * 1024 {
            return None;
        }
        serde_json::from_slice(&bytes).ok()
    };
    let value = browse(&id)?;
    header_year(&value).or_else(|| header_year(&browse(&album_browse_id(&value)?)?))
}

pub(crate) fn apply(inspection: &mut Inspection, album: AlbumYear) {
    // The lookup follows the exact source album. Require its title to match
    // every inspected track before applying a shared year to missing tags.
    if inspection.suggested_mode != CollectionMode::Album
        || !inspection.tracks.iter().all(|track| {
            track
                .metadata
                .album
                .trim()
                .eq_ignore_ascii_case(album.title.trim())
        })
    {
        return;
    }
    inspection.album_release_year = Some(album.year);
    for track in &mut inspection.tracks {
        if track.metadata.release_year.is_none() {
            track.metadata.release_year = Some(album.year);
            track.source_metadata.release_year = Some(album.year);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::inspection_from_json;

    #[test]
    fn live_jaco_response_resolves_the_exact_album_and_rejects_conflicting_links() {
        let mut value: Value =
            serde_json::from_str(include_str!("../tests/fixtures/jaco-album-link.json")).unwrap();
        assert_eq!(
            album_browse_id(&value).as_deref(),
            Some("MPREb_MPwPsljX6n0")
        );
        let album: Value =
            serde_json::from_str(include_str!("../tests/fixtures/jaco-album-header.json")).unwrap();
        let year = header_year(&album).unwrap();
        assert_eq!(year.title, "Jaco Pastorius");
        assert_eq!(year.year, 1976);
        value["contents"]["twoColumnBrowseResultsRenderer"]["secondaryContents"]
            ["sectionListRenderer"]["contents"][0]["musicPlaylistShelfRenderer"]["contents"][1]
            ["musicResponsiveListItemRenderer"]["flexColumns"][0]
            ["musicResponsiveListItemFlexColumnRenderer"]["text"]["runs"][0]
            ["navigationEndpoint"]["browseEndpoint"]["browseId"] = json!("MPdifferent");
        assert!(album_browse_id(&value).is_none());
    }

    #[test]
    fn only_album_header_subtitle_supplies_a_year() {
        let value = json!({"contents":{"twoColumnBrowseResultsRenderer":{"tabs":[{"tabRenderer":{"content":{"sectionListRenderer":{"contents":[{"musicResponsiveHeaderRenderer":{"title":{"runs":[{"text":"Jaco Pastorius"}]},"subtitle":{"runs":[{"text":"Album"},{"text":" • "},{"text":"1976"}]}}}]}}}}]}}});
        assert_eq!(header_year(&value).unwrap().year, 1976);
        assert!(
            header_year(&json!({"description":"Released in 1976", "upload_date":"20250101"}))
                .is_none()
        );
        let mut wrong = value;
        wrong["contents"]["twoColumnBrowseResultsRenderer"]["tabs"][0]["tabRenderer"]["content"]
            ["sectionListRenderer"]["contents"][0]["musicResponsiveHeaderRenderer"]["subtitle"] =
            json!({"runs":[{"text":"1976"},{"text":"2000"}]});
        assert!(header_year(&wrong).is_none());
    }

    #[test]
    fn enrichment_preserves_explicit_years_and_rejects_mismatched_albums() {
        let mut inspection = inspection_from_json(&json!({"title":"Album", "entries":[{"id":"a","title":"Song","album":"Jaco Pastorius"},{"id":"b","title":"Bonus","album":"Jaco Pastorius","release_year":2000}]}), "https://music.youtube.com/playlist?list=OLAK5uy_test").unwrap();
        apply(
            &mut inspection,
            AlbumYear {
                title: "Other album".into(),
                year: 1976,
            },
        );
        assert_eq!(inspection.tracks[0].metadata.release_year, None);
        assert_eq!(inspection.album_release_year, None);
        apply(
            &mut inspection,
            AlbumYear {
                title: "Jaco Pastorius".into(),
                year: 1976,
            },
        );
        assert_eq!(inspection.tracks[0].metadata.release_year, Some(1976));
        assert_eq!(
            inspection.tracks[0].source_metadata.release_year,
            Some(1976)
        );
        assert_eq!(inspection.tracks[1].metadata.release_year, Some(2000));
        assert_eq!(inspection.album_release_year, Some(1976));
    }
}
