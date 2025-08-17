mod config;
mod neovim;
mod server;

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod config_test;

pub use config::{ConfigError, ServerConfig};
pub use server::NeovimMcpServer;

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
