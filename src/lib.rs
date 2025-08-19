mod neovim;
mod server;

#[cfg(test)]
pub mod test_utils;

pub use server::{
    NeovimMcpServer,
    core::{auto_connect_current_project_targets, auto_connect_single_target},
};

pub type Result<T> = std::result::Result<T, ServerError>;

#[derive(Debug, thiserror::Error)]
pub enum ServerError {
    #[error("MCP protocol error: {0}")]
    Mcp(#[from] rmcp::ErrorData),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Server error: {0}")]
    Server(String),
}
