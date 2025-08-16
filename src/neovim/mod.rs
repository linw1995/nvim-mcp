pub mod client;
mod connection;
mod error;

#[cfg(test)]
pub mod integration_tests;

pub use client::{
    CodeAction, DocumentIdentifier, NeovimClient, NeovimClientTrait, Position, PrepareRenameResult,
    Range, WorkspaceEdit, string_or_struct,
};

pub use error::NeovimError;
