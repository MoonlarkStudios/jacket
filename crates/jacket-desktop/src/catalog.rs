//! Search requests and categories for the desktop catalog.

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

/// A text search through the configured music catalog.
pub struct SearchRequest {
    pub query: String,
    pub filter: Option<SearchKind>,
}
