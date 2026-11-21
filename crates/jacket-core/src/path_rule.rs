use audio_engine::{export::sanitize_component, Metadata};
use serde::{Deserialize, Serialize};
use std::{fmt, path::PathBuf, str::FromStr};

const DEFAULT_RULE: &str = "{{artist}}/{{album}}/{{song}}";

/// A relative layout for one audio file. The exporter determines the codec extension.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "String", into = "String")]
pub struct PathRule {
    source: String,
    components: Vec<Vec<Part>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum Part {
    Literal(String),
    Artist,
    AlbumArtist,
    Album,
    Song,
}

impl Part {
    fn parse(token: &str) -> Result<Self, String> {
        match token {
            "artist" => Ok(Self::Artist),
            "album_artist" => Ok(Self::AlbumArtist),
            "album" => Ok(Self::Album),
            "song" => Ok(Self::Song),
            _ => Err(format!("Unknown field {{{{{token}}}}}.")),
        }
    }

    fn append(&self, metadata: &Metadata, output: &mut String) {
        fn value<'a>(field: &'a str, fallback: &'a str) -> &'a str {
            let field = field.trim();
            if field.is_empty() {
                fallback
            } else {
                field
            }
        }

        output.push_str(match self {
            Self::Literal(literal) => literal,
            Self::Artist => value(&metadata.artist, "Unknown artist"),
            Self::AlbumArtist => value(
                &metadata.album_artist,
                value(&metadata.artist, "Unknown artist"),
            ),
            Self::Album => value(&metadata.album, "Unknown album"),
            Self::Song => value(&metadata.title, "Untitled"),
        });
    }
}

impl PathRule {
    pub fn resolve(&self, metadata: &Metadata) -> PathBuf {
        let mut path = PathBuf::new();
        for parts in &self.components {
            if parts == &[Part::Album] && metadata.album.trim().is_empty() {
                continue;
            }
            let mut name = String::new();
            for part in parts {
                part.append(metadata, &mut name);
            }
            path.push(sanitize_component(&name));
        }
        path
    }
}

impl Default for PathRule {
    fn default() -> Self {
        DEFAULT_RULE
            .parse()
            .expect("valid default music folder rule")
    }
}

impl fmt::Display for PathRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.source)
    }
}

impl From<PathRule> for String {
    fn from(rule: PathRule) -> Self {
        rule.source
    }
}

impl TryFrom<String> for PathRule {
    type Error = String;

    fn try_from(source: String) -> Result<Self, Self::Error> {
        source.parse()
    }
}

impl FromStr for PathRule {
    type Err = String;

    fn from_str(source: &str) -> Result<Self, Self::Err> {
        const INVALID_PART: &str =
            "Use folder names and fields separated by /, relative to your music folder.";
        const INVALID_BRACES: &str = "Put field names inside double braces, such as {{song}}.";

        let source = source.trim();
        if source.len() > 512 || source.split('/').count() > 8 {
            return Err("Use at most 512 bytes and eight path components.".into());
        }
        let mut components = Vec::new();
        for component in source.split('/').map(str::trim) {
            if component.is_empty()
                || matches!(component, "." | "..")
                || component
                    .chars()
                    .any(|c| c.is_control() || "<>:\"\\|?*".contains(c))
            {
                return Err(INVALID_PART.into());
            }
            let mut parts = Vec::new();
            let mut rest = component;
            while let Some((literal, field)) = rest.split_once("{{") {
                push_literal(&mut parts, literal).ok_or(INVALID_BRACES)?;
                let (name, tail) = field.split_once("}}").ok_or(INVALID_BRACES)?;
                parts.push(Part::parse(name)?);
                rest = tail;
            }
            push_literal(&mut parts, rest).ok_or(INVALID_BRACES)?;
            components.push(parts);
        }
        if !components
            .last()
            .is_some_and(|parts| parts.contains(&Part::Song))
        {
            return Err("Include {{song}} in the filename at the end of the rule.".into());
        }
        Ok(Self {
            source: source.into(),
            components,
        })
    }
}

fn push_literal(parts: &mut Vec<Part>, literal: &str) -> Option<()> {
    if literal.contains(['{', '}']) {
        return None;
    }
    if !literal.is_empty() {
        parts.push(Part::Literal(literal.to_owned()));
    }
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_use_music_tags_and_skip_an_empty_album_directory() {
        let mut metadata = Metadata {
            title: "Dr. Song".into(),
            artist: "Mira Vale".into(),
            album_artist: "Various Artists".into(),
            album: "After hours".into(),
            ..Metadata::default()
        };
        let rule: PathRule = "{{album_artist}}/{{album}}/{{song}}".parse().unwrap();
        assert_eq!(
            rule.resolve(&metadata),
            PathBuf::from("Various Artists/After hours/Dr. Song")
        );
        metadata.album.clear();
        metadata.album_artist.clear();
        assert_eq!(rule.resolve(&metadata), PathBuf::from("Mira Vale/Dr. Song"));
        let rule: PathRule = "Singles/{{artist}} - {{song}}".parse().unwrap();
        assert_eq!(
            rule.resolve(&metadata),
            PathBuf::from("Singles/Mira Vale - Dr. Song")
        );
    }

    #[test]
    fn metadata_cannot_add_components_to_the_rule() {
        let metadata = Metadata {
            title: "C:\\Somewhere/First light".into(),
            artist: "..".into(),
            album: "/Another folder".into(),
            ..Metadata::default()
        };
        assert_eq!(
            PathRule::default().resolve(&metadata),
            PathBuf::from("Untitled/_Another folder/C__Somewhere_First light")
        );
    }

    #[test]
    fn reject_invalid_syntax_and_unrooted_paths_including_saved_rules() {
        for invalid in [
            "/tmp/{{song}}",
            "C:\\Music/{{song}}",
            "../{{song}}",
            "{{song}}/..",
            "album//{{song}}",
            "{{track}}/{{song}}",
            "{artist}/{{song}}",
            "{{album}}",
        ] {
            assert!(invalid.parse::<PathRule>().is_err(), "{invalid}");
            assert!(serde_json::from_value::<PathRule>(serde_json::json!(invalid)).is_err());
        }
        let rule = PathRule::default();
        assert_eq!(
            serde_json::from_value::<PathRule>(serde_json::to_value(&rule).unwrap()).unwrap(),
            rule
        );
    }
}
