//! Search routing and categories shared by the catalog providers.

use crate::musicbrainz::SearchScope;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SearchProvider {
    YouTubeMusic,
    MusicBrainz,
}

impl SearchProvider {
    pub const fn label(self) -> &'static str {
        match self {
            Self::YouTubeMusic => "YouTube Music",
            Self::MusicBrainz => "MusicBrainz",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Hash)]
pub enum SearchKind {
    Artists,
    Albums,
    Songs,
}

impl SearchKind {
    pub const ALL: [Self; 3] = [Self::Artists, Self::Albums, Self::Songs];

    pub const fn label(self) -> &'static str {
        match self {
            Self::Artists => "Artists",
            Self::Albums => "Albums",
            Self::Songs => "Songs",
        }
    }
}

/// Each adapter owns the search options and identifiers its catalog supports.
pub enum SearchRequest {
    YouTubeMusic {
        query: String,
        filter: Option<SearchKind>,
    },
    MusicBrainz {
        scope: SearchScope,
        pages: Vec<(SearchKind, usize)>,
    },
}
