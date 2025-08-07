pub mod client;
mod connection;
mod error;

#[cfg(test)]
pub mod integration_tests;

pub use client::{NeovimClient, NeovimClientTrait, Position, Range};
pub use error::NeovimError;
