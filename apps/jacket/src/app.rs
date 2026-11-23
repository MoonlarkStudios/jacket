//! Native application state, controllers, and persistence orchestration.
mod artwork;
mod catalog;
mod catalog_media;
mod controller;
mod destination;
mod destination_dialog;
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

pub use catalog::{artist_credit_id, result_id, song_album_id, CatalogAction, CatalogPending};
pub use intake::SourceKind;
pub use receipt::Receipt;
pub use state::{App, Message, Panel, ProfileChoice};
