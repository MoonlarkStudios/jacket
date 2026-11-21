//! Native desktop services independent of the windowing toolkit.

pub mod artwork_assets;
mod artwork_preview;
pub mod auth_session;
pub mod browser_sessions;
mod desktop;
pub mod downloader_updates;
mod export_completion;
pub mod recovery;
pub mod reviews;
mod worker;

pub use desktop::{Desktop, SessionAction};
pub use worker::{Event, Job};
