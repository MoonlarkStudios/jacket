//! Optional artist images associated by exact MusicBrainz artist identifiers.
use super::validate_id;
use percent_encoding::percent_decode_str;
use reqwest::{blocking::Client, Url};
use serde_json::Value;
use std::collections::HashMap;
use std::io::Read;
use std::time::Duration;

const MAX_METADATA: u64 = 256 * 1024;
const MAX_ARTISTS: usize = 3;

#[derive(Clone, Debug)]
pub struct PortraitCredit {
    pub author: String,
    pub license: String,
    pub file: String,
}

pub struct ArtistPortrait {
    pub bytes: Vec<u8>,
    pub credit: PortraitCredit,
}

pub(crate) fn artist_portraits(ids: &[String]) -> Result<HashMap<String, ArtistPortrait>, String> {
    let ids: Vec<_> = ids
        .iter()
        .filter(|id| validate_id(id).is_ok())
        .take(MAX_ARTISTS)
        .collect();
    if ids.is_empty() {
        return Ok(HashMap::new());
    }
    let client = Client::builder()
        .user_agent(concat!(
            "Jacket/",
            env!("CARGO_PKG_VERSION"),
            " (https://github.com/MoonlarkStudios/jacket)"
        ))
        .connect_timeout(Duration::from_secs(3))
        .timeout(Duration::from_secs(5))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let values = ids
        .iter()
        .map(|id| format!("\"{id}\""))
        .collect::<Vec<_>>()
        .join(" ");
    let query = format!(
        "SELECT DISTINCT ?id ?image WHERE {{ VALUES ?id {{ {values} }} ?artist wdt:P434 ?id; wdt:P18 ?image. }} LIMIT 18"
    );
    let data = json(
        client
            .get("https://query.wikidata.org/sparql")
            .query(&[("format", "json"), ("query", &query)]),
    )?;
    let mut filenames = Vec::new();
    for entry in data["results"]["bindings"].as_array().into_iter().flatten() {
        if let (Some(id), Some(image)) = (
            entry["id"]["value"].as_str(),
            entry["image"]["value"].as_str(),
        ) {
            if ids.iter().any(|wanted| wanted.as_str() == id) {
                if let Some(title) = file_title(image) {
                    filenames.push((id, title));
                }
            }
        }
    }
    if filenames.is_empty() {
        return Ok(HashMap::new());
    }
    let titles = filenames
        .iter()
        .map(|(_, title)| title.as_str())
        .collect::<Vec<_>>()
        .join("|");
    let data = json(
        client
            .get("https://commons.wikimedia.org/w/api.php")
            .query(&[
                ("action", "query"),
                ("format", "json"),
                ("formatversion", "2"),
                ("titles", &titles),
                ("prop", "imageinfo"),
                ("iiprop", "url|extmetadata|mime"),
                ("iiurlwidth", "250"),
                (
                    "iiextmetadatafilter",
                    "Attribution|Artist|License|LicenseShortName|Restrictions",
                ),
            ]),
    )?;
    let files: HashMap<_, _> = data["query"]["pages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|page| Some((page["title"].as_str()?, &page["imageinfo"][0])))
        .collect();
    Ok(ids
        .into_iter()
        .filter_map(|id| {
            let (url, credit) = filenames
                .iter()
                .filter(|(artist_id, _)| *artist_id == id)
                .find_map(|(_, file)| licensed_thumbnail(files.get(file.as_str())?))?;
            let bytes = crate::artwork_preview::portrait(&url, valid_thumbnail_url).ok()?;
            Some((id.clone(), ArtistPortrait { bytes, credit }))
        })
        .collect())
}

fn json(request: reqwest::blocking::RequestBuilder) -> Result<Value, String> {
    let response = request
        .send()
        .and_then(reqwest::blocking::Response::error_for_status)
        .map_err(|error| error.to_string())?;
    let mut bytes = Vec::new();
    response
        .take(MAX_METADATA + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| error.to_string())?;
    if bytes.len() as u64 > MAX_METADATA {
        return Err("Wikimedia returned too much portrait metadata.".into());
    }
    serde_json::from_slice(&bytes).map_err(|error| error.to_string())
}

fn file_title(image: &str) -> Option<String> {
    let url = Url::parse(image).ok()?;
    if url.host_str() != Some("commons.wikimedia.org") {
        return None;
    }
    let encoded = url.path().strip_prefix("/wiki/Special:FilePath/")?;
    let title = percent_decode_str(encoded)
        .decode_utf8()
        .ok()?
        .replace('_', " ");
    (!title.is_empty()
        && title.len() <= 256
        && !title.contains(['|', '#', '[', ']', '{', '}'])
        && !title.chars().any(char::is_control))
    .then(|| format!("File:{title}"))
}

fn licensed_thumbnail(info: &Value) -> Option<(String, PortraitCredit)> {
    if !matches!(
        info["mime"].as_str()?,
        "image/jpeg" | "image/png" | "image/webp"
    ) {
        return None;
    }
    let meta = &info["extmetadata"];
    let field = |key: &str| {
        meta[key]["value"]
            .as_str()
            .map(str::trim)
            .filter(|text| !text.is_empty())
    };
    if field("Restrictions").is_some() {
        return None;
    }
    let license = field("License")?;
    let public_domain = matches!(license, "pd" | "cc-zero");
    if !public_domain
        && !matches!(
            license,
            "cc-by-4.0"
                | "cc-by-sa-4.0"
                | "cc-by-3.0"
                | "cc-by-sa-3.0"
                | "cc-by-2.0"
                | "cc-by-sa-2.0"
                | "cc-by-sa-2.5"
        )
    {
        return None;
    }
    // Use plain attribution where Commons supplies it. The file page retains
    // the original metadata, author links, and full license terms.
    let author = field("Attribution")
        .or_else(|| field("Artist"))
        .filter(|text| {
            text.len() <= 200 && !text.contains(['<', '>']) && !text.chars().any(char::is_control)
        });
    if !public_domain && author.is_none() {
        return None;
    }
    let file = info["descriptionurl"].as_str()?;
    let mut url = Url::parse(info["thumburl"].as_str()?).ok()?;
    url.set_query(None);
    (valid_credit_url(file) && valid_thumbnail_url(url.as_str())).then(|| {
        (
            url.into(),
            PortraitCredit {
                author: author.unwrap_or("").into(),
                license: field("LicenseShortName").unwrap_or(license).into(),
                file: file.into(),
            },
        )
    })
}

fn valid_url(input: &str, hosts: &[&str], path: &str) -> bool {
    Url::parse(input).is_ok_and(|url| {
        url.scheme() == "https"
            && url.username().is_empty()
            && url.password().is_none()
            && url.port().is_none()
            && url.host_str().is_some_and(|host| hosts.contains(&host))
            && url.path().starts_with(path)
    })
}

fn valid_thumbnail_url(input: &str) -> bool {
    valid_url(
        input,
        &["thumb.wikimedia.org", "upload.wikimedia.org"],
        "/wikipedia/commons/thumb/",
    )
}

pub(crate) fn valid_credit_url(input: &str) -> bool {
    valid_url(input, &["commons.wikimedia.org"], "/wiki/File:")
}

#[cfg(test)]
mod tests {
    use super::{file_title, licensed_thumbnail};
    use serde_json::json;

    #[test]
    fn commons_filename_preserves_unicode_and_rejects_title_separators() {
        assert_eq!(
            file_title(
                "http://commons.wikimedia.org/wiki/Special:FilePath/Fran%C3%A7oise_Hardy.jpg"
            )
            .as_deref(),
            Some("File:Françoise Hardy.jpg")
        );
        assert!(
            file_title("http://commons.wikimedia.org/wiki/Special:FilePath/One%7CTwo.jpg")
                .is_none()
        );
    }

    #[test]
    fn portrait_requires_a_supported_license_and_safe_image_and_credit_urls() {
        let mut image = json!({
            "mime": "image/jpeg",
            "thumburl": "https://thumb.wikimedia.org/wikipedia/commons/thumb/0/00/Artist.jpg/250px-Artist.jpg?utm_source=example",
            "descriptionurl": "https://commons.wikimedia.org/wiki/File:Artist.jpg",
            "extmetadata": {
                "Artist": { "value": "Photographer" },
                "License": { "value": "cc-by-4.0" },
                "LicenseShortName": { "value": "CC BY 4.0" }
            }
        });
        let (url, credit) = licensed_thumbnail(&image).expect("properly attributed photo");
        assert!(!url.contains('?'));
        assert_eq!(credit.author, "Photographer");
        assert_eq!(credit.license, "CC BY 4.0");
        image["extmetadata"]["Artist"]["value"] = json!("<a href='example'>Artist</a>");
        assert!(licensed_thumbnail(&image).is_none());
        image["extmetadata"]["License"]["value"] = json!("pd");
        assert!(licensed_thumbnail(&image).is_some());
        image["descriptionurl"] =
            json!("https://commons.wikimedia.org.example/wiki/File:Artist.jpg");
        assert!(licensed_thumbnail(&image).is_none());
    }
}
