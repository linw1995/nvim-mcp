pub mod client;
mod connection;
mod error;

#[cfg(test)]
pub mod integration_tests;

pub use client::{DocumentIdentifier, NeovimClient, NeovimClientTrait, Position, Range, CodeAction, WorkspaceEdit};
pub use error::NeovimError;
