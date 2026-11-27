//! Native desktop services independent of the windowing toolkit.

pub mod artwork_assets;
mod artwork_preview;
pub mod auth_session;
pub mod browser_sessions;
pub mod catalog;
mod desktop;
pub mod downloader_updates;
mod downloads;
mod export_completion;
pub mod recovery;
pub mod reviews;
mod worker;
pub mod youtube_music;

pub use desktop::{Desktop, SessionAction};
pub use downloads::{Download, DownloadDestination};
pub use worker::{CatalogEvent, Event, Job};
