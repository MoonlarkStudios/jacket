//! Native application state, controllers, and persistence orchestration.
mod artwork;
mod controller;
mod destination;
mod jobs;
mod lifecycle;
mod persistence;
mod startup;
mod state;
#[cfg(test)]
mod tests;

pub use state::{App, Message, Panel, ProfileChoice};
