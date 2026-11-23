//! Match a release's ordered tracks to the entries of a candidate audio source.
use crate::SourceLink;
use audio_engine::{CollectionMode, Inspection, Metadata, Track};

#[derive(Clone, Debug)]
pub struct ReleaseTrack {
    pub title: String,
    pub artist: String,
    pub disc: u32,
    pub number: u32,
    pub duration_ms: Option<u64>,
    pub source_urls: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct AlbumRelease {
    pub title: String,
    pub artist: String,
    pub year: Option<u32>,
    pub tracks: Vec<ReleaseTrack>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrackMatch {
    Matched,
    Missing,
    Ambiguous,
}

#[derive(Clone, Debug)]
pub struct AlbumMatch {
    pub tracks: Vec<TrackMatch>,
    pub inspection: Inspection,
    suggestions: Vec<Option<Track>>,
}

impl AlbumMatch {
    pub fn suggested_source_title(&self, index: usize) -> Option<&str> {
        Some(&self.suggestions.get(index)?.as_ref()?.source_metadata.title)
    }

    pub fn accept_suggestion(&mut self, index: usize) {
        let Some(track) = self.suggestions.get_mut(index).and_then(Option::take) else {
            return;
        };
        let position = self.tracks[..index]
            .iter()
            .filter(|state| **state == TrackMatch::Matched)
            .count();
        self.inspection.tracks.insert(position, track);
        self.tracks[index] = TrackMatch::Matched;
    }

    pub fn review_inspection(&self) -> Inspection {
        let mut inspection = self.inspection.clone();
        if inspection.tracks.len() < self.tracks.len() {
            inspection.warnings.push(format!(
                "Matched {} of {} tracks from the selected edition.",
                inspection.tracks.len(),
                self.tracks.len(),
            ));
        }
        inspection
    }
}

impl AlbumRelease {
    pub fn match_source(&self, mut source: Inspection) -> Result<AlbumMatch, String> {
        const MAX_TRACKS: usize = 500;
        let (rows, cols) = (self.tracks.len(), source.tracks.len());
        if rows == 0 || rows > MAX_TRACKS || cols > MAX_TRACKS {
            return Err("Album matching supports up to 500 tracks per edition and source.".into());
        }
        let keys: Vec<_> = source.tracks.iter().map(MatchKey::from_source).collect();
        let catalog_keys: Vec<_> = self.tracks.iter().map(MatchKey::from_release).collect();
        let evidence: Vec<Vec<_>> = catalog_keys
            .iter()
            .map(|key| keys.iter().map(|candidate| key.score(candidate)).collect())
            .collect();
        // Compare all optimal ordered alignments. Equally good paths must agree
        // on a track's audio before Jacket chooses it. Skips never shift a match.
        let mut prefix = vec![vec![Score::default(); cols + 1]; rows + 1];
        let mut suffix = prefix.clone();
        for row in 0..rows {
            for col in 0..cols {
                prefix[row + 1][col + 1] = prefix[row][col + 1].max(prefix[row + 1][col]);
                if let Some(score) = evidence[row][col] {
                    prefix[row + 1][col + 1] =
                        prefix[row + 1][col + 1].max(prefix[row][col].add(score));
                }
            }
        }
        for row in (0..rows).rev() {
            for col in (0..cols).rev() {
                suffix[row][col] = suffix[row + 1][col].max(suffix[row][col + 1]);
                if let Some(score) = evidence[row][col] {
                    suffix[row][col] = suffix[row][col].max(score.add(suffix[row + 1][col + 1]));
                }
            }
        }
        let best = prefix[rows][cols];
        let mut matches = Vec::with_capacity(rows);
        let mut tracks = Vec::with_capacity(rows);
        let mut matched_columns = vec![None; rows];
        for (row, release) in self.tracks.iter().enumerate() {
            let mut candidates = (0..cols).filter(|&col| {
                evidence[row][col].is_some_and(|score| {
                    prefix[row][col].add(score).add(suffix[row + 1][col + 1]) == best
                })
            });
            let Some(first) = candidates.next() else {
                matches.push(if evidence[row].iter().any(Option::is_some) {
                    TrackMatch::Ambiguous
                } else {
                    TrackMatch::Missing
                });
                continue;
            };
            let may_be_skipped =
                (0..=cols).any(|col| prefix[row][col].add(suffix[row + 1][col]) == best);
            let conflicting_audio =
                candidates.any(|col| source.tracks[col].id != source.tracks[first].id);
            if may_be_skipped || conflicting_audio {
                matches.push(TrackMatch::Ambiguous);
                continue;
            }
            matched_columns[row] = Some(first);
            matches.push(TrackMatch::Matched);
            tracks.push(self.tagged_track(release, source.tracks[first].clone()));
        }
        let mut suggestions = vec![None; rows];
        for (row, key) in catalog_keys.iter().enumerate() {
            if matches[row] != TrackMatch::Missing {
                continue;
            }
            let before = if row == 0 {
                Some(0)
            } else {
                matched_columns[row - 1].map(|col| col + 1)
            };
            let after = if row + 1 == rows {
                Some(cols)
            } else {
                matched_columns[row + 1]
            };
            // Only suggest a version when one unused entry sits between matched tracks.
            let Some((before, _)) = before
                .zip(after)
                .filter(|(before, after)| *before + 1 == *after)
            else {
                continue;
            };
            let candidate = &keys[before];
            if key.similar_variant(candidate) {
                suggestions[row] =
                    Some(self.tagged_track(&self.tracks[row], source.tracks[before].clone()));
                matches[row] = TrackMatch::Ambiguous;
            }
        }
        source.title.clone_from(&self.title);
        source.suggested_mode = CollectionMode::Album;
        source.album_release_year = self.year;
        source.tracks = tracks;
        Ok(AlbumMatch {
            tracks: matches,
            inspection: source,
            suggestions,
        })
    }

    fn tagged_track(&self, release: &ReleaseTrack, mut track: Track) -> Track {
        track.metadata = Metadata {
            title: release.title.clone(),
            artist: release.artist.clone(),
            album: self.title.clone(),
            album_artist: self.artist.clone(),
            disc_number: Some(release.disc),
            track_number: Some(release.number),
            release_year: self.year,
        };
        track
    }
}

/// A linked video outranks any number of title and duration similarities.
#[derive(Clone, Copy, Default, Eq, Ord, PartialEq, PartialOrd)]
struct Score {
    linked: u16,
    matched: u16,
    artist: u16,
}

impl Score {
    fn add(self, other: Self) -> Self {
        Self {
            linked: self.linked + other.linked,
            matched: self.matched + other.matched,
            artist: self.artist + other.artist,
        }
    }
}

struct MatchKey {
    title: String,
    base_title: String,
    artist: String,
    seconds: Option<f64>,
    videos: Vec<String>,
}

impl MatchKey {
    fn from_release(track: &ReleaseTrack) -> Self {
        Self {
            title: normalize(without_guest_credit(&track.title)),
            base_title: normalize(base_title(&track.title)),
            artist: normalized_credit(&track.artist),
            seconds: track.duration_ms.map(|ms| ms as f64 / 1000.0),
            videos: track
                .source_urls
                .iter()
                .filter_map(|url| SourceLink::parse(url).ok()?.song_url)
                .filter_map(|url| url.rsplit_once("?v=").map(|(_, id)| id.to_owned()))
                .collect(),
        }
    }

    fn from_source(track: &Track) -> Self {
        Self {
            title: normalize(without_guest_credit(&track.source_metadata.title)),
            base_title: normalize(base_title(&track.source_metadata.title)),
            artist: normalized_credit(&track.source_metadata.artist),
            seconds: track.duration,
            videos: vec![track.id.clone()],
        }
    }

    fn score(&self, candidate: &Self) -> Option<Score> {
        let duration_difference = self.duration_difference(candidate);
        let artist_equal = !self.artist.is_empty() && self.artist == candidate.artist;
        let linked = self.videos.iter().any(|id| candidate.videos.contains(id));
        if linked && duration_difference.is_none_or(|difference| difference <= 4.0) {
            return Some(Score {
                linked: 1,
                matched: 1,
                artist: u16::from(artist_equal),
            });
        }
        (duration_difference.is_some_and(|difference| difference <= 4.0)
            && !self.title.is_empty()
            && self.title == candidate.title
            && (artist_equal || self.artist.is_empty() || candidate.artist.is_empty()))
        .then_some(Score {
            linked: 0,
            matched: 1,
            artist: u16::from(artist_equal),
        })
    }

    fn duration_difference(&self, candidate: &Self) -> Option<f64> {
        self.seconds
            .zip(candidate.seconds)
            .map(|(left, right)| (left - right).abs())
    }

    fn similar_variant(&self, candidate: &Self) -> bool {
        self.title != candidate.title
            && !self.base_title.is_empty()
            && self.base_title == candidate.base_title
            && (self.artist.is_empty()
                || candidate.artist.is_empty()
                || self.artist == candidate.artist)
            && self
                .duration_difference(candidate)
                .is_some_and(|difference| difference <= 4.0)
    }
}

fn without_guest_credit(title: &str) -> &str {
    let title = title.trim_end();
    if let Some((base, suffix)) = final_qualifier(title) {
        let first = suffix
            .split(|c: char| !c.is_alphanumeric())
            .find(|part| !part.is_empty());
        if first.is_some_and(|part| {
            ["feat", "ft", "featuring"]
                .iter()
                .any(|word| part.eq_ignore_ascii_case(word))
        }) {
            return base;
        }
    }
    title
}

fn base_title(title: &str) -> &str {
    let title = without_guest_credit(title);
    final_qualifier(title).map_or(title, |(base, _)| base)
}

fn final_qualifier(title: &str) -> Option<(&str, &str)> {
    let (open, last) = if let Some(last) = title.strip_suffix(')') {
        ('(', last)
    } else {
        ('[', title.strip_suffix(']')?)
    };
    let (base, suffix) = last.rsplit_once(open)?;
    Some((base.trim_end(), suffix))
}

fn normalize(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn normalized_credit(value: &str) -> String {
    value
        .split(|character: char| !character.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|part| !matches!(part.as_str(), "feat" | "featuring" | "ft"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{AlbumRelease, ReleaseTrack, TrackMatch};
    use audio_engine::{CollectionMode, Inspection, Metadata, Track};

    fn release(titles: &[&str]) -> AlbumRelease {
        AlbumRelease {
            title: "The expanded edition".into(),
            artist: "The band".into(),
            year: Some(2004),
            tracks: titles
                .iter()
                .enumerate()
                .map(|(index, title)| ReleaseTrack {
                    title: (*title).into(),
                    artist: "The band".into(),
                    disc: 1,
                    number: index as u32 + 1,
                    duration_ms: Some(189_000),
                    source_urls: Vec::new(),
                })
                .collect(),
        }
    }

    fn source(entries: &[(&str, &str, f64)]) -> Inspection {
        Inspection {
            title: "Playlist".into(),
            source_url: "https://www.youtube.com/playlist?list=PLAlbumFixture123".into(),
            suggested_mode: CollectionMode::Collection,
            artwork_url: None,
            album_release_year: None,
            tracks: entries
                .iter()
                .map(|(id, title, seconds)| {
                    let metadata = Metadata {
                        title: (*title).into(),
                        artist: "The band".into(),
                        ..Metadata::default()
                    };
                    Track {
                        id: (*id).into(),
                        source_url: format!("https://www.youtube.com/watch?v={id}"),
                        uploader: String::new(),
                        duration: Some(*seconds),
                        artwork_url: None,
                        source_metadata: metadata.clone(),
                        metadata,
                    }
                })
                .collect(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn missing_track_and_extras_do_not_shift_disc_numbers_or_substitute_an_edit() {
        let mut edition = release(&["Start", "Interlude", "End"]);
        edition.tracks[2].disc = 2;
        edition.tracks[2].number = 1;
        let matched = edition
            .match_source(source(&[
                ("fixture0001", "Start", 190.0),
                ("fixture0002", "Interlude (live)", 189.0),
                ("fixture0003", "Bonus", 189.0),
                ("fixture0004", "End", 187.0),
            ]))
            .unwrap();
        assert_eq!(
            matched.tracks,
            [
                TrackMatch::Matched,
                TrackMatch::Missing,
                TrackMatch::Matched
            ]
        );
        assert_eq!(matched.inspection.title, "The expanded edition");
        assert_eq!(matched.inspection.album_release_year, Some(2004));
        assert_eq!(matched.inspection.suggested_mode, CollectionMode::Album);
        let end = &matched.inspection.tracks[1];
        assert_eq!(end.id, "fixture0004");
        assert_eq!(end.metadata.disc_number, Some(2));
        assert_eq!(end.metadata.track_number, Some(1));
        assert_eq!(end.source_metadata.album, "");
    }

    #[test]
    fn equal_evidence_for_different_audio_requires_review() {
        let matched = release(&["Start", "End"])
            .match_source(source(&[
                ("fixture0001", "Start", 188.0),
                ("fixture0002", "Start", 189.0),
                ("fixture0003", "End", 189.0),
            ]))
            .unwrap();
        assert_eq!(matched.tracks, [TrackMatch::Ambiguous, TrackMatch::Matched]);
        assert_eq!(matched.inspection.tracks[0].id, "fixture0003");
    }

    #[test]
    fn repeated_audio_can_match_repeated_positions_without_reusing_a_single_occurrence() {
        let edition = release(&["Start", "Start"]);
        let source_once = source(&[("fixture0001", "Start", 189.0)]);
        assert_eq!(
            edition.match_source(source_once).unwrap().tracks,
            [TrackMatch::Ambiguous, TrackMatch::Ambiguous],
        );
        let source_twice = source(&[
            ("fixture0001", "Start", 189.0),
            ("fixture0001", "Start", 189.0),
        ]);
        let matched = edition.match_source(source_twice).unwrap();
        assert_eq!(matched.tracks, [TrackMatch::Matched, TrackMatch::Matched]);
        assert_eq!(matched.inspection.tracks[1].metadata.track_number, Some(2));
    }

    #[test]
    fn an_explicit_recording_link_wins_over_a_matching_title_and_duration() {
        let mut edition = release(&["Start"]);
        edition.tracks[0]
            .source_urls
            .push("https://youtu.be/fixture0002".into());
        let matched = edition
            .match_source(source(&[
                ("fixture0001", "Start", 189.0),
                ("fixture0002", "Start / alternate upload title", 190.0),
            ]))
            .unwrap();
        assert_eq!(matched.tracks, [TrackMatch::Matched]);
        assert_eq!(matched.inspection.tracks[0].id, "fixture0002");
    }

    #[test]
    fn guest_credits_match_but_recording_variants_require_confirmation() {
        let edition = release(&["Start", "Horizon", "End (2013 demo)"]);
        let mut matched = edition
            .match_source(source(&[
                ("fixture0001", "Start (feat. A Guest)", 189.0),
                ("fixture0002", "Horizon (Japan CD)", 190.0),
                ("fixture0003", "End (2013 demo) [feat. A Guest]", 189.0),
            ]))
            .unwrap();
        assert_eq!(
            matched.tracks,
            [
                TrackMatch::Matched,
                TrackMatch::Ambiguous,
                TrackMatch::Matched
            ]
        );
        assert_eq!(
            matched.suggested_source_title(1),
            Some("Horizon (Japan CD)")
        );
        assert_eq!(matched.review_inspection().warnings.len(), 1);
        matched.accept_suggestion(1);
        assert_eq!(matched.tracks, [TrackMatch::Matched; 3]);
        let inspection = matched.review_inspection();
        assert!(inspection.warnings.is_empty());
        assert_eq!(inspection.tracks[1].id, "fixture0002");
        assert_eq!(inspection.tracks[1].metadata.title, "Horizon");
        assert_eq!(
            inspection.tracks[1].source_metadata.title,
            "Horizon (Japan CD)"
        );
        matched.accept_suggestion(1);
        assert_eq!(matched.inspection.tracks.len(), 3);
    }

    #[test]
    fn title_alone_does_not_select_another_performance() {
        let mut candidate = source(&[("fixture0001", "Start", 189.0)]);
        candidate.tracks[0].source_metadata.artist = "Another band".into();
        assert!(release(&["Start"])
            .match_source(candidate)
            .unwrap()
            .inspection
            .tracks
            .is_empty());
        let candidate = source(&[("fixture0001", "Start", 172.0)]);
        assert!(release(&["Start"])
            .match_source(candidate)
            .unwrap()
            .inspection
            .tracks
            .is_empty());
    }
}
