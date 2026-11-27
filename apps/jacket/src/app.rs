//! Native application state, controllers, and persistence orchestration.
mod artwork;
mod catalog;
mod catalog_focus;
mod catalog_media;
mod controller;
mod destination;
mod destination_dialog;
mod downloads;
mod intake;
mod jobs;
mod lifecycle;
mod persistence;
mod receipt;
mod review_tools;
mod startup;
mod state;
#[cfg(test)]
mod tests;

pub use catalog::{
    artist_credit_id, edition_id, result_id, song_album_id, CatalogAction, CatalogPending,
};
pub(crate) use catalog_focus::CATALOG_SCROLL_ID;
pub use catalog_media::Thumbnail;
pub use downloads::DownloadAction;
pub(crate) use downloads::{DownloadSurface, DOWNLOADS_LIST_ID, DOWNLOADS_PANEL_ID};
pub use intake::SourceKind;
pub use receipt::Receipt;
pub use state::{App, Message, Panel, ProfileChoice};
