//! Native application state, controllers, and persistence orchestration.
mod artwork;
mod controller;
mod destination;
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

pub use intake::SourceKind;
pub use receipt::Receipt;
pub use state::{App, Message, Panel, ProfileChoice};
